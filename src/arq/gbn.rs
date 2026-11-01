// CS-30003: Reliable Data Transfer over UDP
// Author: Soumyadeb Mukherjee
// Component: Protocol & ARQ - Go-Back-N State Machines

use crate::packet::{Packet, PacketType};

/// Go-Back-N Sender State Machine.
///
/// Implements fixed sender sliding window of size `W` with cumulative ACKs.
/// On timeout, retransmits ALL unacknowledged packets currently outstanding in the window.
#[derive(Debug, Clone)]
pub struct GoBackNSender {
    window_size: usize,
    send_base: u32,
    next_seq_num: u32,
    buffer: Vec<Packet>,
    pub total_sent: u64,
    pub retransmissions: u64,
}

impl GoBackNSender {
    pub fn new(window_size: usize) -> Self {
        assert!(window_size > 0, "Window size must be greater than 0");
        Self {
            window_size,
            send_base: 0,
            next_seq_num: 0,
            buffer: Vec::new(),
            total_sent: 0,
            retransmissions: 0,
        }
    }

    /// Can send if window is not full: next_seq_num < send_base + window_size.
    pub fn can_send(&self) -> bool {
        (self.next_seq_num - self.send_base) < self.window_size as u32
    }

    /// Prepares a new DATA packet if window space is available.
    pub fn send_chunk(&mut self, payload: Vec<u8>) -> Option<Packet> {
        if !self.can_send() {
            return None;
        }

        let mut pkt = Packet::new_data(self.next_seq_num, payload);
        pkt.compute_and_set_checksum();
        self.buffer.push(pkt.clone());
        self.next_seq_num = self.next_seq_num.wrapping_add(1);
        self.total_sent += 1;
        Some(pkt)
    }

    /// Handles a cumulative ACK packet.
    ///
    /// An ACK with sequence `K` cumulatively confirms all packets with seq <= K.
    /// Returns `true` if this ACK advanced the window base.
    pub fn handle_ack(&mut self, ack_pkt: &Packet) -> bool {
        if !ack_pkt.is_valid() || ack_pkt.pkt_type != PacketType::Ack {
            return false;
        }

        let ack_seq = ack_pkt.seq_num;
        // Check if ack_seq falls within currently outstanding window: [send_base, next_seq_num - 1]
        if ack_seq >= self.send_base && ack_seq < self.next_seq_num {
            let packets_to_drain = (ack_seq - self.send_base + 1) as usize;
            if packets_to_drain <= self.buffer.len() {
                self.buffer.drain(0..packets_to_drain);
                self.send_base = ack_seq.wrapping_add(1);
                return true;
            }
        }

        false
    }

    /// Handles timeout on the oldest unacknowledged packet (`send_base`).
    ///
    /// Go-Back-N Rule: Retransmit ALL packets currently in the pipeline!
    pub fn handle_timeout(&mut self) -> Vec<Packet> {
        let mut retransmits = Vec::new();
        for pkt in &mut self.buffer {
            pkt.set_retransmitted();
            pkt.compute_and_set_checksum();
            retransmits.push(pkt.clone());
            self.retransmissions += 1;
            self.total_sent += 1;
        }
        retransmits
    }

    pub fn window_size(&self) -> usize {
        self.window_size
    }

    pub fn send_base(&self) -> u32 {
        self.send_base
    }

    pub fn next_seq_num(&self) -> u32 {
        self.next_seq_num
    }

    pub fn in_flight_count(&self) -> usize {
        self.buffer.len()
    }

    pub fn has_unacked_packets(&self) -> bool {
        !self.buffer.is_empty()
    }
}

/// Go-Back-N Receiver State Machine.
///
/// Has a receiver window of 1: only accepts the exact `expected_seq`.
/// Any out-of-order packet is DISCARDED and a cumulative duplicate ACK is returned.
#[derive(Debug, Clone)]
pub struct GoBackNReceiver {
    expected_seq: u32,
    delivered_data: Vec<Vec<u8>>,
    pub total_received: u64,
    pub discarded_out_of_order: u64,
    pub duplicate_acks_sent: u64,
}

impl GoBackNReceiver {
    pub fn new() -> Self {
        Self {
            expected_seq: 0,
            delivered_data: Vec::new(),
            total_received: 0,
            discarded_out_of_order: 0,
            duplicate_acks_sent: 0,
        }
    }

    /// Processes an incoming packet. Returns a cumulative ACK packet.
    pub fn handle_packet(&mut self, pkt: &Packet) -> Option<Packet> {
        self.total_received += 1;

        if !pkt.is_valid() {
            return None; // Drop corrupted packets
        }

        match pkt.pkt_type {
            PacketType::Data => {
                if pkt.seq_num == self.expected_seq {
                    // Packet arrived in strict sequence! Deliver and advance.
                    self.delivered_data.push(pkt.payload.clone());
                    let ack_seq = self.expected_seq;
                    self.expected_seq = self.expected_seq.wrapping_add(1);

                    let mut ack = Packet::new_ack(ack_seq);
                    ack.compute_and_set_checksum();
                    Some(ack)
                } else {
                    // Out-of-order or duplicate packet.
                    // Discard it and, only after at least one in-order delivery,
                    // resend the cumulative ACK of the last in-order sequence.
                    //
                    // Before sequence 0 has been received, expected_seq is 0.
                    // ACK(0) would tell the sender that packet 0 is delivered.
                    // Emit no ACK in that case.
                    self.discarded_out_of_order += 1;

                    if self.expected_seq > 0 {
                        self.duplicate_acks_sent += 1;
                        let mut ack = Packet::new_ack(self.expected_seq - 1);
                        ack.compute_and_set_checksum();
                        Some(ack)
                    } else {
                        None
                    }
                }
            }
            PacketType::Fin => {
                let mut ack = Packet::new_ack(pkt.seq_num);
                ack.compute_and_set_checksum();
                Some(ack)
            }
            PacketType::Ack => None,
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

impl Default for GoBackNReceiver {
    fn default() -> Self {
        Self::new()
    }
}
