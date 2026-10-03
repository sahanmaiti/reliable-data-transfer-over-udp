// CS-30003: Reliable Data Transfer over UDP
// Author: Soumyadeb Mukherjee
// Component: Protocol & ARQ - Selective Repeat State Machines

use crate::packet::{Packet, PacketType};
use std::collections::BTreeMap;

/// Entry tracking an in-flight packet within the sender window.
#[derive(Debug, Clone)]
struct SrSenderEntry {
    packet: Packet,
    acked: bool,
}

/// Selective Repeat Sender State Machine.
///
/// Maintains a sliding window of size `W`.
/// Handles individual ACKs and retransmits ONLY specifically timed-out packets.
#[derive(Debug, Clone)]
pub struct SelectiveRepeatSender {
    window_size: usize,
    send_base: u32,
    next_seq_num: u32,
    buffer: Vec<SrSenderEntry>,
    pub total_sent: u64,
    pub retransmissions: u64,
}

impl SelectiveRepeatSender {
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

    /// Checks if sender window has room for another packet.
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
        self.buffer.push(SrSenderEntry {
            packet: pkt.clone(),
            acked: false,
        });
        self.next_seq_num = self.next_seq_num.wrapping_add(1);
        self.total_sent += 1;
        Some(pkt)
    }

    /// Handles an individual ACK packet.
    ///
    /// Marks the specific acknowledged packet. If the ACK is for `send_base`,
    /// slides the window forward past all consecutive acknowledged packets.
    pub fn handle_ack(&mut self, ack_pkt: &Packet) -> bool {
        if !ack_pkt.is_valid() || ack_pkt.pkt_type != PacketType::Ack {
            return false;
        }

        let ack_seq = ack_pkt.seq_num;
        if ack_seq >= self.send_base && ack_seq < self.next_seq_num {
            let offset = (ack_seq - self.send_base) as usize;
            if offset < self.buffer.len() {
                self.buffer[offset].acked = true;

                // Consecutive sliding: advance send_base past all contiguous acked packets
                let mut advance_count = 0;
                for entry in &self.buffer {
                    if entry.acked {
                        advance_count += 1;
                    } else {
                        break;
                    }
                }

                if advance_count > 0 {
                    self.buffer.drain(0..advance_count);
                    self.send_base = self.send_base.wrapping_add(advance_count as u32);
                }

                return true;
            }
        }

        false
    }

    /// Handles timeout for an individual packet.
    ///
    /// Selective Repeat Rule: Retransmit ONLY this single packet!
    pub fn handle_timeout(&mut self, seq_num: u32) -> Option<Packet> {
        if seq_num >= self.send_base && seq_num < self.next_seq_num {
            let offset = (seq_num - self.send_base) as usize;
            if offset < self.buffer.len() && !self.buffer[offset].acked {
                let entry = &mut self.buffer[offset];
                entry.packet.set_retransmitted();
                entry.packet.compute_and_set_checksum();
                self.retransmissions += 1;
                self.total_sent += 1;
                return Some(entry.packet.clone());
            }
        }
        None
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
        self.buffer.iter().filter(|e| !e.acked).count()
    }
}

/// Selective Repeat Receiver State Machine.
///
/// Maintains a receiver sliding window of size `W`.
/// Buffers valid out-of-order packets in a BTreeMap and sends individual ACKs.
/// Slides window forward and delivers chunks in strictly consecutive order.
#[derive(Debug, Clone)]
pub struct SelectiveRepeatReceiver {
    window_size: usize,
    recv_base: u32,
    buffer: BTreeMap<u32, Vec<u8>>,
    delivered_data: Vec<Vec<u8>>,
    pub total_received: u64,
    pub buffered_out_of_order: u64,
    pub duplicates_count: u64,
}

impl SelectiveRepeatReceiver {
    pub fn new(window_size: usize) -> Self {
        assert!(window_size > 0, "Window size must be greater than 0");
        Self {
            window_size,
            recv_base: 0,
            buffer: BTreeMap::new(),
            delivered_data: Vec::new(),
            total_received: 0,
            buffered_out_of_order: 0,
            duplicates_count: 0,
        }
    }

    /// Processes an incoming packet.
    /// Returns an individual ACK packet for any valid packet within [recv_base - W, recv_base + W - 1].
    pub fn handle_packet(&mut self, pkt: &Packet) -> Option<Packet> {
        self.total_received += 1;

        if !pkt.is_valid() {
            return None; // Drop corrupted packets
        }

        match pkt.pkt_type {
            PacketType::Data => {
                let seq = pkt.seq_num;
                let w = self.window_size as u32;

                // Case 1: Packet falls inside the active receiver window [recv_base, recv_base + W - 1]
                if seq >= self.recv_base && seq < self.recv_base.wrapping_add(w) {
                    // Send individual ACK for this packet
                    let mut ack = Packet::new_ack(seq);
                    ack.compute_and_set_checksum();

                    if seq == self.recv_base {
                        // Deliver this packet immediately
                        self.delivered_data.push(pkt.payload.clone());
                        self.recv_base = self.recv_base.wrapping_add(1);

                        // Consecutive Draining: deliver all buffered consecutive packets
                        while let Some(buffered_payload) = self.buffer.remove(&self.recv_base) {
                            self.delivered_data.push(buffered_payload);
                            self.recv_base = self.recv_base.wrapping_add(1);
                        }
                    } else if !self.buffer.contains_key(&seq) {
                        // Out-of-order packet within window: BUFFER IT!
                        self.buffer.insert(seq, pkt.payload.clone());
                        self.buffered_out_of_order += 1;
                    } else {
                        // Already buffered duplicate
                        self.duplicates_count += 1;
                    }

                    Some(ack)
                }
                // Case 2: Packet falls in previous window [recv_base - W, recv_base - 1]
                // (Already delivered in the past, but sender didn't receive our earlier ACK!)
                else if seq < self.recv_base && seq >= self.recv_base.saturating_sub(w) {
                    self.duplicates_count += 1;
                    let mut ack = Packet::new_ack(seq);
                    ack.compute_and_set_checksum();
                    Some(ack)
                }
                // Case 3: Outside window: ignore
                else {
                    None
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

    pub fn recv_base(&self) -> u32 {
        self.recv_base
    }

    pub fn buffered_count(&self) -> usize {
        self.buffer.len()
    }

    pub fn delivered_chunks(&self) -> &[Vec<u8>] {
        &self.delivered_data
    }

    pub fn drain_delivered(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.delivered_data)
    }
}
