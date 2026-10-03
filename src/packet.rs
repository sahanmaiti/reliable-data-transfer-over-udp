// CS-30003: Reliable Data Transfer over UDP
// Author: Soumyadeb Mukherjee
// Component: Protocol & ARQ - Packet Framing & Codec

use std::convert::TryFrom;
use std::fmt;

/// The fixed size of our packet header in bytes on the wire.
/// Layout:
///   seq_num (4B) + pkt_type (1B) + flags (1B) + payload_len (2B) + checksum (2B) = 10 Bytes
pub const HEADER_SIZE: usize = 10;

/// Maximum payload size per UDP packet to ensure datagram fits inside standard 1500-byte MTU.
pub const MAX_PAYLOAD_SIZE: usize = 1400;

/// Represents the type and purpose of a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    Data = 0x01,
    Ack = 0x02,
    Fin = 0x03,
}

/// Errors that can occur during packet deserialization.
#[derive(Debug, PartialEq, Eq)]
pub enum PacketError {
    /// Received buffer is too small to even contain the 10-byte header.
    BufferTooShort { length: usize },
    /// Found an invalid packet type byte on the wire.
    UnknownPacketType(u8),
    /// The buffer ends before the declared payload length is satisfied.
    TruncatedPayload { declared: usize, available: usize },
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PacketError::BufferTooShort { length } => {
                write!(f, "buffer too short: {} bytes, expected at least {}", length, HEADER_SIZE)
            }
            PacketError::UnknownPacketType(val) => {
                write!(f, "unknown packet type: 0x{:02X}", val)
            }
            PacketError::TruncatedPayload { declared, available } => {
                write!(f, "truncated payload: declared {} bytes, but only {} available", declared, available)
            }
        }
    }
}

impl std::error::Error for PacketError {}

impl TryFrom<u8> for PacketType {
    type Error = PacketError;

    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0x01 => Ok(PacketType::Data),
            0x02 => Ok(PacketType::Ack),
            0x03 => Ok(PacketType::Fin),
            unknown => Err(PacketError::UnknownPacketType(unknown)),
        }
    }
}

/// The universal packet structure used across Stop-and-Wait, Go-Back-N, and Selective Repeat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    /// Monotonically increasing sequence number or acknowledged number.
    pub seq_num: u32,
    /// Type of the packet (DATA, ACK, or FIN).
    pub pkt_type: PacketType,
    /// Control flags for extensions or retransmissions (default: 0).
    pub flags: u8,
    /// 16-bit Internet Checksum (computed over header + payload).
    pub checksum: u16,
    /// Raw payload bytes.
    pub payload: Vec<u8>,
}

impl Packet {
    /// Creates a new DATA packet.
    /// Checksum is initialized to 0 and will be computed before wire transmission.
    pub fn new_data(seq_num: u32, payload: Vec<u8>) -> Self {
        Packet {
            seq_num,
            pkt_type: PacketType::Data,
            flags: 0,
            checksum: 0,
            payload,
        }
    }

    /// Creates a new ACK packet with an empty payload.
    pub fn new_ack(seq_num: u32) -> Self {
        Packet {
            seq_num,
            pkt_type: PacketType::Ack,
            flags: 0,
            checksum: 0,
            payload: Vec::new(),
        }
    }

    /// Serializes the packet into a byte vector using big-endian (network byte order).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HEADER_SIZE + self.payload.len());

        // 0..4: Sequence number (4 bytes, big-endian)
        buf.extend_from_slice(&self.seq_num.to_be_bytes());

        // 4: Packet type (1 byte)
        buf.push(self.pkt_type as u8);

        // 5: Flags (1 byte)
        buf.push(self.flags);

        // 6..8: Payload length (2 bytes, big-endian)
        let payload_len = self.payload.len() as u16;
        buf.extend_from_slice(&payload_len.to_be_bytes());

        // 8..10: Checksum (2 bytes, big-endian)
        buf.extend_from_slice(&self.checksum.to_be_bytes());

        // 10..end: Payload data
        buf.extend_from_slice(&self.payload);

        buf
    }

    /// Deserializes a raw byte buffer into a Packet.
    pub fn from_bytes(raw: &[u8]) -> Result<Self, PacketError> {
        if raw.len() < HEADER_SIZE {
            return Err(PacketError::BufferTooShort { length: raw.len() });
        }

        // Parse header fields safely from slice indices
        let seq_num = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
        let pkt_type = PacketType::try_from(raw[4])?;
        let flags = raw[5];
        let payload_len = u16::from_be_bytes([raw[6], raw[7]]) as usize;
        let checksum = u16::from_be_bytes([raw[8], raw[9]]);

        // Validate that the buffer contains the full declared payload
        let available = raw.len() - HEADER_SIZE;
        if available < payload_len {
            return Err(PacketError::TruncatedPayload {
                declared: payload_len,
                available,
            });
        }

        // Slices only the declared payload length (ignoring any trailing padding)
        let payload = raw[HEADER_SIZE..HEADER_SIZE + payload_len].to_vec();

        Ok(Packet {
            seq_num,
            pkt_type,
            flags,
            checksum,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_packet_roundtrip() {
        let original = Packet::new_data(42, b"Hello, Reliability!".to_vec());
        let wire_bytes = original.to_bytes();

        assert_eq!(wire_bytes.len(), HEADER_SIZE + b"Hello, Reliability!".len());
        let decoded = Packet::from_bytes(&wire_bytes).expect("Failed to decode valid packet");

        assert_eq!(original, decoded);
    }

    #[test]
    fn test_ack_packet_roundtrip() {
        let original = Packet::new_ack(105);
        let wire_bytes = original.to_bytes();

        assert_eq!(wire_bytes.len(), HEADER_SIZE); // ACKs have 0 payload length
        let decoded = Packet::from_bytes(&wire_bytes).expect("Failed to decode valid ACK");

        assert_eq!(original, decoded);
        assert_eq!(decoded.pkt_type, PacketType::Ack);
        assert_eq!(decoded.seq_num, 105);
        assert!(decoded.payload.is_empty());
    }

    #[test]
    fn test_buffer_too_short() {
        let short_bytes = vec![0x00, 0x01, 0x02]; // Only 3 bytes
        let result = Packet::from_bytes(&short_bytes);

        assert_eq!(result, Err(PacketError::BufferTooShort { length: 3 }));
    }

    #[test]
    fn test_unknown_packet_type() {
        let mut bytes = Packet::new_ack(1).to_bytes();
        bytes[4] = 0x99; // Corrupt packet type byte

        let result = Packet::from_bytes(&bytes);
        assert_eq!(result, Err(PacketError::UnknownPacketType(0x99)));
    }

    #[test]
    fn test_truncated_payload() {
        let mut bytes = Packet::new_data(1, vec![1, 2, 3, 4, 5]).to_bytes();
        bytes.pop(); // Remove last payload byte so length header says 5 but only 4 bytes exist

        let result = Packet::from_bytes(&bytes);
        assert_eq!(
            result,
            Err(PacketError::TruncatedPayload {
                declared: 5,
                available: 4
            })
        );
    }
}
