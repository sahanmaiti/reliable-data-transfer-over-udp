// CS-30003: Reliable Data Transfer over UDP
// Author: Soumyadeb Mukherjee
// Component: Protocol & ARQ - Stop-and-Wait State Machines

use crate::packet::{Packet, PacketType};

/// Stop-and-Wait Sender State Machine.
///
/// Permits exactly one DATA packet in flight (Window size W = 1).
/// Retransmits on timeout until ACK is received or retry limit is exceeded.
#[derive(Debug, Clone)]
pub struct StopAndWaitSender {
    current_seq: u32,
    in_flight: Option<Packet>,
    max_retries: u32,
    current_retries: u32,
    pub total_sent: u64,
    pub retransmissions: u64,
}

impl StopAndWaitSender {
    pub fn new(max_retries: u32) -> Self {
        Self {
            current_seq: 0,
            in_flight: None,
            max_retries,
            current_retries: 0,
            total_sent: 0,
            retransmissions: 0,
        }
    }

    /// Can send next chunk if no packet is currently in flight.
    pub fn can_send(&self) -> bool {
        self.in_flight.is_none()
    }

    /// Prepares a new DATA packet for transmission.
    pub fn send_chunk(&mut self, payload: Vec<u8>) -> Option<Packet> {
        if !self.can_send() {
            return None;
        }

        let mut pkt = Packet::new_data(self.current_seq, payload);
        pkt.compute_and_set_checksum();
        self.in_flight = Some(pkt.clone());
        self.total_sent += 1;
        self.current_retries = 0;
        Some(pkt)
    }

    /// Handles an incoming ACK packet.
    /// Returns `true` if the ACK acknowledges the current in-flight packet.
    pub fn handle_ack(&mut self, ack_pkt: &Packet) -> bool {
        if !ack_pkt.is_valid() || ack_pkt.pkt_type != PacketType::Ack {
            return false;
        }

        if let Some(ref inflight) = self.in_flight {
            if ack_pkt.seq_num == inflight.seq_num {
                // In-flight packet acknowledged! Advance sequence and clear in-flight state.
                self.in_flight = None;
                self.current_seq = self.current_seq.wrapping_add(1);
                self.current_retries = 0;
                return true;
            }
        }

        false
    }

    /// Handles a retransmission timeout event.
    /// Returns the packet to retransmit, or None if retry limit is exceeded or no packet in flight.
    pub fn handle_timeout(&mut self) -> Option<Packet> {
        if let Some(ref mut pkt) = self.in_flight {
            if self.current_retries >= self.max_retries {
                return None; // Retry limit exceeded (failure condition)
            }
            self.current_retries += 1;
            self.retransmissions += 1;
            self.total_sent += 1;

            pkt.set_retransmitted();
            pkt.compute_and_set_checksum();
            Some(pkt.clone())
        } else {
            None
        }
    }

    pub fn current_seq(&self) -> u32 {
        self.current_seq
    }

    pub fn is_waiting_for_ack(&self) -> bool {
        self.in_flight.is_some()
    }

    pub fn current_retries(&self) -> u32 {
        self.current_retries
    }
}

/// Stop-and-Wait Receiver State Machine.
///
/// Expects sequential sequence numbers 0, 1, 2...
/// ACKs every valid received DATA packet. Discards duplicates and corrupt packets.
#[derive(Debug, Clone)]
pub struct StopAndWaitReceiver {
    expected_seq: u32,
    delivered_data: Vec<Vec<u8>>,
    pub total_received: u64,
    pub duplicates_count: u64,
    pub corrupted_count: u64,
}

impl StopAndWaitReceiver {
    pub fn new() -> Self {
        Self {
            expected_seq: 0,
            delivered_data: Vec::new(),
            total_received: 0,
            duplicates_count: 0,
            corrupted_count: 0,
        }
    }

    /// Processes an incoming packet. Returns an ACK packet if valid or duplicate; None if corrupted.
    pub fn handle_packet(&mut self, pkt: &Packet) -> Option<Packet> {
        self.total_received += 1;

        // Check packet integrity via 16-bit Internet Checksum
        if !pkt.is_valid() {
            self.corrupted_count += 1;
            return None; // Corrupted packet: silently drop
        }

        match pkt.pkt_type {
            PacketType::Data => {
                if pkt.seq_num == self.expected_seq {
                    // In-order packet arrived!
                    self.delivered_data.push(pkt.payload.clone());
                    let ack_seq = self.expected_seq;
                    self.expected_seq = self.expected_seq.wrapping_add(1);

                    let mut ack = Packet::new_ack(ack_seq);
                    ack.compute_and_set_checksum();
                    Some(ack)
                } else if pkt.seq_num < self.expected_seq {
                    // Duplicate packet (past packet arrived again due to lost/delayed ACK).
                    // Re-send ACK for this packet so sender can advance.
                    self.duplicates_count += 1;
                    let mut ack = Packet::new_ack(pkt.seq_num);
                    ack.compute_and_set_checksum();
                    Some(ack)
                } else {
                    // Out-of-order future packet (should not happen in S&W under normal conditions,
                    // but if it arrives, re-ACK the last acknowledged sequence number).
                    let last_ack = if self.expected_seq > 0 { self.expected_seq - 1 } else { 0 };
                    let mut ack = Packet::new_ack(last_ack);
                    ack.compute_and_set_checksum();
                    Some(ack)
                }
            }
            PacketType::Fin => {
                // FIN packet: acknowledge FIN
                let mut ack = Packet::new_ack(pkt.seq_num);
                ack.compute_and_set_checksum();
                Some(ack)
            }
            PacketType::Ack => None, // Receivers don't ingest standalone ACKs
        }
    }

    pub fn expected_seq(&self) -> u32 {
        self.expected_seq
    }

    pub fn delivered_chunks(&self) -> &[Vec<u8>] {
        &self.delivered_data
    }

    pub fn drain_delivered(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.delivered_data)
    }
}

impl Default for StopAndWaitReceiver {
    fn default() -> Self {
        Self::new()
    }
}
