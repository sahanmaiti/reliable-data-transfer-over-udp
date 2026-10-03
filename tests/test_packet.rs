// CS-30003: Reliable Data Transfer over UDP
// Author: Soumyadeb Mukherjee
// Unit Tests: Packet Framing, Codec, Flags & RFC 1071 Checksum

use reliable_udp::packet::{Packet, PacketError, PacketType, HEADER_SIZE, MAX_PAYLOAD_SIZE};

#[test]
fn test_packet_header_size_is_10_bytes() {
    assert_eq!(HEADER_SIZE, 10);
}

#[test]
fn test_packet_serialize_deserialize_roundtrip() {
    let payload = b"Hello, reliable UDP world!".to_vec();
    let mut pkt = Packet::new_data(42, payload.clone());
    pkt.compute_and_set_checksum();

    let bytes = pkt.serialize();
    assert_eq!(bytes.len(), HEADER_SIZE + payload.len());

    let deserialized = Packet::deserialize(&bytes).expect("Failed to deserialize valid packet");
    assert_eq!(deserialized.seq_num, 42);
    assert_eq!(deserialized.pkt_type, PacketType::Data);
    assert_eq!(deserialized.flags, 0);
    assert_eq!(deserialized.payload, payload);
    assert!(deserialized.is_valid());
}

#[test]
fn test_ack_packet_has_empty_payload() {
    let mut ack = Packet::new_ack(105);
    ack.compute_and_set_checksum();

    let bytes = ack.serialize();
    assert_eq!(bytes.len(), HEADER_SIZE);

    let deserialized = Packet::deserialize(&bytes).expect("Failed to deserialize ACK");
    assert_eq!(deserialized.seq_num, 105);
    assert_eq!(deserialized.pkt_type, PacketType::Ack);
    assert!(deserialized.payload.is_empty());
    assert!(deserialized.is_valid());
}

#[test]
fn test_fin_packet_handling() {
    let mut fin = Packet::new_fin(999);
    fin.compute_and_set_checksum();

    let bytes = fin.serialize();
    let deserialized = Packet::deserialize(&bytes).expect("Failed to deserialize FIN");
    assert_eq!(deserialized.seq_num, 999);
    assert_eq!(deserialized.pkt_type, PacketType::Fin);
    assert!(deserialized.is_valid());
}

#[test]
fn test_checksum_detects_bit_corruption() {
    let mut pkt = Packet::new_data(1, b"Uncorrupted original payload".to_vec());
    pkt.compute_and_set_checksum();
    assert!(pkt.is_valid());

    let mut wire_bytes = pkt.serialize();
    // Corrupt a byte in the payload
    let last_idx = wire_bytes.len() - 1;
    wire_bytes[last_idx] ^= 0x01; // flip single bit

    let corrupted_pkt = Packet::deserialize(&wire_bytes).expect("Should deserialize structure");
    assert!(!corrupted_pkt.is_valid(), "Checksum must detect flipped payload bit!");
}

#[test]
fn test_truncated_buffer_returns_error() {
    // Buffer smaller than 10-byte header
    let short_bytes = vec![0x00, 0x01, 0x02];
    let result = Packet::deserialize(&short_bytes);
    assert_eq!(result, Err(PacketError::BufferTooShort { length: 3 }));
}

#[test]
fn test_retransmitted_flag_toggle() {
    let mut pkt = Packet::new_data(7, b"retry test".to_vec());
    assert!(!pkt.is_retransmitted());

    pkt.set_retransmitted();
    assert!(pkt.is_retransmitted());
    pkt.compute_and_set_checksum();

    let wire = pkt.serialize();
    let decoded = Packet::deserialize(&wire).unwrap();
    assert!(decoded.is_retransmitted());
}

#[test]
fn test_max_mtu_payload_boundary() {
    let big_payload = vec![0xAA; MAX_PAYLOAD_SIZE];
    let mut pkt = Packet::new_data(0, big_payload.clone());
    pkt.compute_and_set_checksum();

    let wire = pkt.serialize();
    assert_eq!(wire.len(), HEADER_SIZE + MAX_PAYLOAD_SIZE);

    let decoded = Packet::deserialize(&wire).unwrap();
    assert_eq!(decoded.payload.len(), MAX_PAYLOAD_SIZE);
    assert!(decoded.is_valid());
}
