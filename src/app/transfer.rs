// CS-30003: Reliable Data Transfer over UDP
// Component: Deterministic virtual-time transfer driver
//
// The clock is an Instant used only as an origin. Every delivery and timer
// deadline is origin + a duration from the channel or the RTO estimator.
// The loop jumps to the next deadline. It does not sleep and it does not read
// the wall clock again.

use super::file_io::{Chunker, Reassembler};
use super::integrity::compute_sha256;
use crate::arq::{
    GoBackNReceiver, GoBackNSender, ProtocolType, SelectiveRepeatReceiver, SelectiveRepeatSender,
    StopAndWaitReceiver, StopAndWaitSender,
};
use crate::channel::scheduler::ChannelScheduler;
use crate::channel::{Channel, ChannelConfig};
use crate::metrics::{
    ApplicationMetrics, ChannelMetrics, ExperimentMeta, ExperimentRecord, ForwardChannelConfig,
    ProtocolMetrics, ReverseChannelConfig, RtoParameters, TimingMetrics,
};
use crate::packet::{Packet, PacketType, MAX_PAYLOAD_SIZE};
use crate::timing::{MultiTimer, RetransmissionTimer, RtoConfig, RtoEstimator};
use std::collections::{BTreeSet, VecDeque};
use std::time::{Duration, Instant};

const MAX_EVENTS: u32 = 1_000_000;
const MAX_SAME_TIME_SPINS: u32 = 100_000;

/// One forward submission, in channel order. Delays come from `Channel::process`.
/// An empty `delays_ms` means the datagram was dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardEvent {
    pub seq: u32,
    pub retransmission: bool,
    pub delays_ms: Vec<u64>,
    pub copy_folds: Vec<u16>,
}

/// Input of one headless transfer. The payload is the source file bytes.
#[derive(Debug, Clone)]
pub struct TransferConfig {
    pub payload: Vec<u8>,
    pub protocol: ProtocolType,
    pub window_size: usize,
    pub chunk_size: usize,
    pub seed: u64,
    pub trial_id: usize,
    pub experiment_id: String,
    pub loss_rate: f64,
    pub reorder_rate: f64,
    pub duplicate_rate: f64,
    pub corrupt_rate: f64,
    pub base_delay_ms: u64,
    pub jitter_ms: u64,
    pub reorder_extra_ms: u64,
    pub rto_multiplier: f64,
    pub min_rto: Duration,
    pub max_rto: Duration,
    pub initial_rto: Duration,
    pub alpha: f64,
    pub beta: f64,
    pub max_virtual_time: Duration,
    pub max_retransmissions: u64,
}

/// Measurements from one virtual-time run.
///
/// `record` uses the existing experiment schema and is filled from this run.
/// Counters that the schema does not have yet are returned beside it.
#[derive(Debug, Clone)]
pub struct TransferOutput {
    pub record: ExperimentRecord,
    pub premature_retransmissions: u64,
    pub karn_rejected: u64,
    pub reverse_seed: u64,
    pub segment_count: u32,
    pub forward_events: Vec<ForwardEvent>,
    pub reverse: ChannelMetrics,
}

#[derive(Debug, Clone)]
struct SegMeta {
    original_send_at: Instant,
    last_send_at: Instant,
    rto_at_send: Duration,
    ever_retransmitted: bool,
    original_dropped: bool,
    ack_scheduled: bool,
    rtt_sampled: bool,
}

enum Session {
    Sw {
        sender: StopAndWaitSender,
        receiver: StopAndWaitReceiver,
    },
    Gbn {
        sender: GoBackNSender,
        receiver: GoBackNReceiver,
    },
    Sr {
        sender: SelectiveRepeatSender,
        receiver: SelectiveRepeatReceiver,
    },
}

enum Due {
    None,
    Sw,
    Gbn,
    Sr(Vec<u32>),
}

struct Driver {
    session: Session,
    pending: VecDeque<Vec<u8>>,
    total_chunks: u32,
    reassembler: Reassembler,
    source_sha: String,
    source_len: u64,
    forward: Channel,
    reverse: Channel,
    forward_sched: ChannelScheduler,
    reverse_sched: ChannelScheduler,
    estimator: RtoEstimator,
    /// Single timer for Stop-and-Wait and Go-Back-N (oldest outstanding segment).
    timer: RetransmissionTimer,
    sr_timers: MultiTimer<u32>,
    /// Armed Selective Repeat sequence numbers, in order, so expiry is deterministic.
    sr_armed: BTreeSet<u32>,
    meta: Vec<SegMeta>,
    epoch: Instant,
    completed: bool,
    exhausted: bool,
    retx_submitted: u64,
    premature: u64,
    max_retransmissions: u64,
    data_received: u64,
    ack_sent: u64,
    ack_received: u64,
    duplicate_data: u64,
    duplicate_acks: u64,
    forward_events: Vec<ForwardEvent>,
    protocol_name: String,
    window_recorded: usize,
    chunk_size: usize,
    experiment_id: String,
    trial_id: usize,
}

/// Runs one transfer on a virtual clock.
///
/// Forward impairments come from `config`. The reverse channel uses
/// `seed.wrapping_add(1)` and does not drop, reorder, duplicate, or corrupt;
/// it still applies the configured base delay and jitter.
pub fn run_transfer(config: &TransferConfig) -> TransferOutput {
    if config.chunk_size == 0 || config.chunk_size > MAX_PAYLOAD_SIZE || config.window_size == 0 {
        return aborted(config, "INVALID_CONFIG");
    }

    let mut driver = Driver::new(config);
    let epoch = driver.epoch;
    let mut now = epoch;
    let mut events = 0u32;
    // Application completion time. The sender may still be retiring ACKs after this.
    let mut completed_at: Option<Instant> = None;

    loop {
        events += 1;
        if events > MAX_EVENTS || driver.exhausted {
            return driver.finish(completed_at.unwrap_or(now));
        }
        if now.saturating_duration_since(epoch) > config.max_virtual_time {
            return driver.finish(completed_at.unwrap_or(now));
        }

        let mut spins = 0u32;
        loop {
            spins += 1;
            if spins > MAX_SAME_TIME_SPINS {
                driver.exhausted = true;
                return driver.finish(completed_at.unwrap_or(now));
            }

            let mut progress = false;

            while let Some(bytes) = driver.forward_sched.pop_ready_at(now) {
                progress = true;
                driver.on_forward(now, bytes);
                if driver.completed && completed_at.is_none() {
                    completed_at = Some(now);
                }
            }

            while let Some(bytes) = driver.reverse_sched.pop_ready_at(now) {
                progress = true;
                driver.on_ack(now, bytes);
            }

            if driver.fire_timers(now) {
                progress = true;
            }
            if driver.exhausted {
                return driver.finish(completed_at.unwrap_or(now));
            }

            if driver.pump_sends(now) {
                progress = true;
            }
            if !progress {
                break;
            }
        }

        if driver.completed && driver.sender_idle() {
            return driver.finish(completed_at.unwrap_or(now));
        }

        match driver.next_deadline() {
            None => return driver.finish(completed_at.unwrap_or(now)),
            Some(deadline) if deadline <= now => {
                driver.exhausted = true;
                return driver.finish(completed_at.unwrap_or(now));
            }
            Some(deadline) => {
                let cap = epoch + config.max_virtual_time;
                if deadline > cap {
                    return driver.finish(completed_at.unwrap_or(cap));
                }
                now = deadline;
            }
        }
    }
}

fn aborted(config: &TransferConfig, status: &str) -> TransferOutput {
    let source_sha = compute_sha256(&config.payload);
    let delivered_sha = compute_sha256(&[]);
    let forward = forward_channel_config(config);
    let reverse = reverse_channel_config(&forward);
    let record = ExperimentRecord::new(
        meta_from(config, effective_window(config), 0, reverse.seed),
        forward_record(&forward),
        reverse_record(&reverse),
        rto_parameters(&estimator_config(config)),
        ApplicationMetrics {
            source_bytes: config.payload.len() as u64,
            delivered_unique_bytes: 0,
            transfer_status: status.to_string(),
            sha256_match: false,
            source_sha256: source_sha,
            delivered_sha256: delivered_sha,
        },
        zero_timing(),
        ProtocolMetrics::default(),
        ChannelMetrics::default(),
        ChannelMetrics::default(),
    );
    TransferOutput {
        record,
        premature_retransmissions: 0,
        karn_rejected: 0,
        reverse_seed: reverse.seed,
        segment_count: 0,
        forward_events: Vec::new(),
        reverse: ChannelMetrics::default(),
    }
}

fn effective_window(config: &TransferConfig) -> usize {
    match config.protocol {
        ProtocolType::StopAndWait => 1,
        ProtocolType::GoBackN | ProtocolType::SelectiveRepeat => config.window_size,
    }
}

fn meta_from(
    config: &TransferConfig,
    window: usize,
    segment_count: usize,
    reverse_seed: u64,
) -> ExperimentMeta {
    ExperimentMeta {
        experiment_id: config.experiment_id.clone(),
        protocol: config.protocol.to_string(),
        window_size: window,
        chunk_size: config.chunk_size,
        segment_count,
        seed: config.seed,
        reverse_seed,
        trial_id: config.trial_id,
    }
}

fn forward_channel_config(config: &TransferConfig) -> ChannelConfig {
    ChannelConfig {
        seed: config.seed,
        loss: config.loss_rate,
        duplicate: config.duplicate_rate,
        reorder: config.reorder_rate,
        reorder_extra_ms: config.reorder_extra_ms,
        corrupt: config.corrupt_rate,
        base_delay_ms: config.base_delay_ms,
        jitter_ms: config.jitter_ms,
    }
}

fn reverse_channel_config(forward: &ChannelConfig) -> ChannelConfig {
    ChannelConfig {
        seed: forward.seed.wrapping_add(1),
        loss: 0.0,
        duplicate: 0.0,
        reorder: 0.0,
        reorder_extra_ms: forward.reorder_extra_ms,
        corrupt: 0.0,
        base_delay_ms: forward.base_delay_ms,
        jitter_ms: forward.jitter_ms,
    }
}

fn forward_record(config: &ChannelConfig) -> ForwardChannelConfig {
    ForwardChannelConfig {
        configured_loss_rate: config.loss,
        configured_reorder_rate: config.reorder,
        configured_duplicate_rate: config.duplicate,
        configured_corrupt_rate: config.corrupt,
        base_delay_ms: config.base_delay_ms,
        jitter_ms: config.jitter_ms,
        reorder_extra_ms: config.reorder_extra_ms,
    }
}

fn reverse_record(config: &ChannelConfig) -> ReverseChannelConfig {
    ReverseChannelConfig {
        configured_loss_rate: config.loss,
        configured_reorder_rate: config.reorder,
        configured_duplicate_rate: config.duplicate,
        configured_corrupt_rate: config.corrupt,
        base_delay_ms: config.base_delay_ms,
        jitter_ms: config.jitter_ms,
        reorder_extra_ms: config.reorder_extra_ms,
    }
}

fn estimator_config(config: &TransferConfig) -> RtoConfig {
    RtoConfig {
        initial_rto: config.initial_rto,
        min_rto: config.min_rto,
        max_rto: config.max_rto,
        granularity: Duration::from_millis(1),
        alpha: config.alpha,
        beta: config.beta,
        k: 4.0,
        rto_multiplier: config.rto_multiplier,
    }
}

fn rto_parameters(config: &RtoConfig) -> RtoParameters {
    RtoParameters {
        rto_multiplier: config.rto_multiplier,
        initial_rto_ms: millis(config.initial_rto),
        min_rto_ms: millis(config.min_rto),
        max_rto_ms: millis(config.max_rto),
        alpha: config.alpha,
        beta: config.beta,
        k: config.k,
    }
}

fn zero_timing() -> TimingMetrics {
    TimingMetrics {
        duration_secs: 0.0,
        total_rtt_samples: 0,
        karn_rejected_count: 0,
        min_rtt_ms: 0.0,
        max_rtt_ms: 0.0,
        mean_rtt_ms: 0.0,
        final_srtt_ms: 0.0,
        final_rttvar_ms: 0.0,
        final_rto_ms: 0.0,
        timeout_count: 0,
        backoff_count: 0,
    }
}

fn channel_metrics(stats: &crate::channel::ChannelStats) -> ChannelMetrics {
    ChannelMetrics {
        offered_datagrams: stats.packets_seen as usize,
        dropped_datagrams: stats.lost as usize,
        duplicated_datagrams: stats.duplicated as usize,
        corrupted_datagrams: stats.corrupted as usize,
        reordered_datagrams: stats.reordered as usize,
        scheduled_deliveries: stats.copies_delivered as usize,
    }
}

fn fold_bytes(data: &[u8]) -> u16 {
    data.iter()
        .fold(0u16, |acc, byte| acc.wrapping_add(*byte as u16))
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

impl Driver {
    fn new(config: &TransferConfig) -> Self {
        let packets = Chunker::new(config.chunk_size).chunk_bytes(&config.payload);
        let pending: VecDeque<Vec<u8>> = packets.into_iter().map(|pkt| pkt.payload).collect();
        let total_chunks = pending.len() as u32;
        let window = effective_window(config);

        let session = match config.protocol {
            ProtocolType::StopAndWait => Session::Sw {
                // The driver's retransmission ceiling stops the run.
                // u32::MAX keeps the Stop-and-Wait machine from stopping first.
                sender: StopAndWaitSender::new(u32::MAX),
                receiver: StopAndWaitReceiver::new(),
            },
            ProtocolType::GoBackN => Session::Gbn {
                sender: GoBackNSender::new(window),
                receiver: GoBackNReceiver::new(),
            },
            ProtocolType::SelectiveRepeat => Session::Sr {
                sender: SelectiveRepeatSender::new(window),
                receiver: SelectiveRepeatReceiver::new(window),
            },
        };

        let estimator = RtoEstimator::with_config(estimator_config(config));

        let forward = Channel::new(forward_channel_config(config));
        let reverse = Channel::new(reverse_channel_config(&forward.config));

        Self {
            session,
            pending,
            total_chunks,
            reassembler: Reassembler::new(),
            source_sha: compute_sha256(&config.payload),
            source_len: config.payload.len() as u64,
            forward,
            reverse,
            forward_sched: ChannelScheduler::new(),
            reverse_sched: ChannelScheduler::new(),
            estimator,
            timer: RetransmissionTimer::new(),
            sr_timers: MultiTimer::new(),
            sr_armed: BTreeSet::new(),
            meta: Vec::new(),
            epoch: Instant::now(),
            completed: false,
            exhausted: false,
            retx_submitted: 0,
            premature: 0,
            max_retransmissions: config.max_retransmissions,
            data_received: 0,
            ack_sent: 0,
            ack_received: 0,
            duplicate_data: 0,
            duplicate_acks: 0,
            forward_events: Vec::new(),
            protocol_name: config.protocol.to_string(),
            window_recorded: window,
            chunk_size: config.chunk_size,
            experiment_id: config.experiment_id.clone(),
            trial_id: config.trial_id,
        }
    }

    fn pump_sends(&mut self, now: Instant) -> bool {
        let mut sent = false;
        while self.can_send() {
            let Some(payload) = self.pending.pop_front() else {
                break;
            };
            match self.send_chunk(payload) {
                Some(pkt) => {
                    self.submit(now, pkt, false);
                    sent = true;
                }
                None => break,
            }
        }
        sent
    }

    fn can_send(&self) -> bool {
        match &self.session {
            Session::Sw { sender, .. } => sender.can_send(),
            Session::Gbn { sender, .. } => sender.can_send(),
            Session::Sr { sender, .. } => sender.can_send(),
        }
    }

    fn send_chunk(&mut self, payload: Vec<u8>) -> Option<Packet> {
        match &mut self.session {
            Session::Sw { sender, .. } => sender.send_chunk(payload),
            Session::Gbn { sender, .. } => sender.send_chunk(payload),
            Session::Sr { sender, .. } => sender.send_chunk(payload),
        }
    }

    fn submit(&mut self, now: Instant, pkt: Packet, retransmission: bool) {
        let seq = pkt.seq_num;
        let rto = self.estimator.current_rto();
        if retransmission {
            if let Some(meta) = self.meta.get_mut(seq as usize) {
                meta.ever_retransmitted = true;
                meta.last_send_at = now;
                meta.rto_at_send = rto;
            }
            self.retx_submitted += 1;
        } else if seq as usize == self.meta.len() {
            self.meta.push(SegMeta {
                original_send_at: now,
                last_send_at: now,
                rto_at_send: rto,
                ever_retransmitted: pkt.is_retransmitted(),
                original_dropped: false,
                ack_scheduled: false,
                rtt_sampled: false,
            });
        }

        let wire = pkt.serialize();
        let deliveries = self.forward.process(&wire);
        if !retransmission {
            if let Some(meta) = self.meta.get_mut(seq as usize) {
                meta.original_dropped = deliveries.is_empty();
            }
        }

        self.forward_events.push(ForwardEvent {
            seq,
            retransmission,
            delays_ms: deliveries
                .iter()
                .map(|delivery| delivery.delay_ms)
                .collect(),
            copy_folds: deliveries
                .iter()
                .map(|delivery| fold_bytes(&delivery.data))
                .collect(),
        });
        self.forward_sched.schedule_at(now, deliveries);
        self.arm_after_submit(now, seq, retransmission);
    }

    fn arm_after_submit(&mut self, now: Instant, seq: u32, retransmission: bool) {
        if matches!(self.session, Session::Sr { .. }) {
            let rto = self
                .meta
                .get(seq as usize)
                .map(|meta| meta.rto_at_send)
                .unwrap_or_else(|| self.estimator.current_rto());
            self.sr_timers.start_timer_at(seq, now, rto);
            self.sr_armed.insert(seq);
            return;
        }
        if retransmission || self.oldest_outstanding() == Some(seq) {
            self.arm_oldest(now);
        }
    }

    fn arm_oldest(&mut self, now: Instant) {
        if matches!(self.session, Session::Sr { .. }) {
            return;
        }
        let Some(seq) = self.oldest_outstanding() else {
            self.timer.cancel();
            return;
        };
        let Some(meta) = self.meta.get(seq as usize) else {
            self.timer.cancel();
            return;
        };
        let deadline = meta.last_send_at + meta.rto_at_send;
        let duration = if deadline > now {
            deadline - now
        } else {
            Duration::ZERO
        };
        self.timer.start_at(now, duration);
    }

    fn sender_idle(&self) -> bool {
        if !self.pending.is_empty()
            || !self.forward_sched.is_empty()
            || !self.reverse_sched.is_empty()
        {
            return false;
        }
        match &self.session {
            Session::Sw { sender, .. } => !sender.is_waiting_for_ack(),
            Session::Gbn { sender, .. } => !sender.has_unacked_packets(),
            Session::Sr { sender, .. } => sender.in_flight_count() == 0 && self.sr_armed.is_empty(),
        }
    }

    fn oldest_outstanding(&self) -> Option<u32> {
        match &self.session {
            Session::Sw { sender, .. } => sender.is_waiting_for_ack().then(|| sender.current_seq()),
            Session::Gbn { sender, .. } => sender.has_unacked_packets().then(|| sender.send_base()),
            Session::Sr { .. } => None,
        }
    }

    fn on_forward(&mut self, now: Instant, bytes: Vec<u8>) {
        let pkt = match Packet::deserialize(&bytes) {
            Ok(pkt) => pkt,
            Err(_) => return,
        };
        if pkt.pkt_type != PacketType::Data {
            return;
        }
        self.data_received += 1;

        let gbn_duplicate = pkt.is_valid() && self.gbn_already_delivered(pkt.seq_num);
        let dups_before = self.sw_sr_duplicate_count();
        let ack = self.deliver_data(&pkt);
        let dups_after = self.sw_sr_duplicate_count();
        self.duplicate_data += dups_after.saturating_sub(dups_before);
        if gbn_duplicate {
            self.duplicate_data += 1;
        }

        self.absorb();
        if let Some(ack) = ack {
            self.submit_ack(now, ack);
        }
    }

    fn gbn_already_delivered(&self, seq: u32) -> bool {
        match &self.session {
            Session::Gbn { receiver, .. } => seq < receiver.expected_seq(),
            _ => false,
        }
    }

    fn sw_sr_duplicate_count(&self) -> u64 {
        match &self.session {
            Session::Sw { receiver, .. } => receiver.duplicates_count,
            Session::Sr { receiver, .. } => receiver.duplicates_count,
            Session::Gbn { .. } => 0,
        }
    }

    fn deliver_data(&mut self, pkt: &Packet) -> Option<Packet> {
        match &mut self.session {
            Session::Sw { receiver, .. } => receiver.handle_packet(pkt),
            Session::Gbn { receiver, .. } => receiver.handle_packet(pkt),
            Session::Sr { receiver, .. } => receiver.handle_packet(pkt),
        }
    }

    fn absorb(&mut self) {
        let chunks = match &mut self.session {
            Session::Sw { receiver, .. } => receiver.drain_delivered(),
            Session::Gbn { receiver, .. } => receiver.drain_delivered(),
            Session::Sr { receiver, .. } => receiver.drain_delivered(),
        };
        for chunk in chunks {
            let seq = self.reassembler.next_expected_seq;
            self.reassembler.push_chunk(seq, &chunk);
        }
        if self.reassembler.is_complete(self.total_chunks) {
            self.completed = true;
        }
    }

    fn submit_ack(&mut self, now: Instant, ack: Packet) {
        self.ack_sent += 1;
        let wire = ack.serialize();
        let deliveries = self.reverse.process(&wire);
        // A dropped ACK was never scheduled, so a later timeout is not premature.
        if !deliveries.is_empty() {
            self.mark_ack_scheduled(ack.seq_num);
        }
        self.reverse_sched.schedule_at(now, deliveries);
    }

    fn mark_ack_scheduled(&mut self, ack_seq: u32) {
        let cumulative = matches!(self.session, Session::Gbn { .. });
        if cumulative {
            let last = (ack_seq as usize).min(self.meta.len().saturating_sub(1));
            for meta in self.meta.iter_mut().take(last + 1) {
                meta.ack_scheduled = true;
            }
        } else if let Some(meta) = self.meta.get_mut(ack_seq as usize) {
            meta.ack_scheduled = true;
        }
    }

    fn on_ack(&mut self, now: Instant, bytes: Vec<u8>) {
        let pkt = match Packet::deserialize(&bytes) {
            Ok(pkt) => pkt,
            Err(_) => return,
        };
        if pkt.pkt_type != PacketType::Ack || !pkt.is_valid() {
            return;
        }
        self.ack_received += 1;

        let sample_seq = match &self.session {
            Session::Sw { sender, .. } => sender.current_seq(),
            Session::Gbn { sender, .. } => sender.send_base(),
            Session::Sr { .. } => pkt.seq_num,
        };
        let accepted = self.accept_ack(&pkt);
        if !accepted {
            self.duplicate_acks += 1;
            return;
        }

        // Go-Back-N samples only the oldest segment this cumulative ACK newly covers.
        // Stop-and-Wait has one outstanding segment. Selective Repeat samples that ACK.
        self.sample_rtt(sample_seq, now);
        self.rearm_after_ack(now, pkt.seq_num);
    }

    fn accept_ack(&mut self, pkt: &Packet) -> bool {
        match &mut self.session {
            Session::Sw { sender, .. } => sender.handle_ack(pkt),
            Session::Gbn { sender, .. } => sender.handle_ack(pkt),
            Session::Sr { sender, .. } => sender.handle_ack(pkt),
        }
    }

    fn sample_rtt(&mut self, seq: u32, now: Instant) {
        let Some(meta) = self.meta.get_mut(seq as usize) else {
            return;
        };
        if meta.rtt_sampled {
            return;
        }
        meta.rtt_sampled = true;
        let sample = now.saturating_duration_since(meta.original_send_at);
        let retransmitted = meta.ever_retransmitted;
        self.estimator.update_rtt(sample, retransmitted);
    }

    fn rearm_after_ack(&mut self, now: Instant, acked_seq: u32) {
        if matches!(self.session, Session::Sr { .. }) {
            self.sr_timers.cancel_timer(&acked_seq);
            self.sr_armed.remove(&acked_seq);
            return;
        }
        self.arm_oldest(now);
    }

    fn due(&self, now: Instant) -> Due {
        match &self.session {
            Session::Sw { sender, .. } => {
                if self.timer.is_expired_at(now) && sender.is_waiting_for_ack() {
                    Due::Sw
                } else {
                    Due::None
                }
            }
            Session::Gbn { sender, .. } => {
                if self.timer.is_expired_at(now) && sender.has_unacked_packets() {
                    Due::Gbn
                } else {
                    Due::None
                }
            }
            Session::Sr { .. } => {
                let seqs: Vec<u32> = self
                    .sr_armed
                    .iter()
                    .copied()
                    .filter(|seq| self.sr_timers.is_expired_at(seq, now))
                    .collect();
                if seqs.is_empty() {
                    Due::None
                } else {
                    Due::Sr(seqs)
                }
            }
        }
    }

    fn fire_timers(&mut self, now: Instant) -> bool {
        if self.retx_submitted >= self.max_retransmissions {
            self.exhausted = true;
            return false;
        }
        match self.due(now) {
            Due::None => false,
            Due::Sw => {
                self.timer.cancel();
                let pkt = match &mut self.session {
                    Session::Sw { sender, .. } => sender.handle_timeout(),
                    _ => None,
                };
                if let Some(pkt) = pkt {
                    self.estimator.on_timeout();
                    self.retransmit(now, pkt);
                }
                true
            }
            Due::Gbn => {
                self.timer.cancel();
                let packets = match &mut self.session {
                    Session::Gbn { sender, .. } => sender.handle_timeout(),
                    _ => Vec::new(),
                };
                if !packets.is_empty() {
                    // One Go-Back-N timer backs the RTO off once, then the
                    // existing sender retransmits the outstanding window.
                    self.estimator.on_timeout();
                    for pkt in packets {
                        if self.retx_submitted >= self.max_retransmissions {
                            self.exhausted = true;
                            break;
                        }
                        self.retransmit(now, pkt);
                    }
                }
                true
            }
            Due::Sr(seqs) => {
                for seq in seqs {
                    if self.retx_submitted >= self.max_retransmissions {
                        self.exhausted = true;
                        break;
                    }
                    self.sr_armed.remove(&seq);
                    self.sr_timers.cancel_timer(&seq);
                    let pkt = match &mut self.session {
                        Session::Sr { sender, .. } => sender.handle_timeout(seq),
                        _ => None,
                    };
                    if let Some(pkt) = pkt {
                        self.estimator.on_timeout();
                        self.retransmit(now, pkt);
                    }
                }
                true
            }
        }
    }

    fn retransmit(&mut self, now: Instant, pkt: Packet) {
        if self.is_premature(pkt.seq_num) {
            self.premature += 1;
        }
        self.submit(now, pkt, true);
    }

    /// Timer fired, the original forward datagram was not dropped, and an ACK
    /// covering this segment was already handed to the reverse scheduler.
    fn is_premature(&self, seq: u32) -> bool {
        self.meta
            .get(seq as usize)
            .map(|meta| !meta.original_dropped && meta.ack_scheduled)
            .unwrap_or(false)
    }

    fn next_deadline(&self) -> Option<Instant> {
        let sr_deadline = if matches!(self.session, Session::Sr { .. }) {
            self.sr_timers.earliest_deadline()
        } else {
            self.timer.deadline()
        };
        [
            self.forward_sched.next_deadline(),
            self.reverse_sched.next_deadline(),
            sr_deadline,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn protocol_totals(&self) -> (u64, u64) {
        match &self.session {
            Session::Sw { sender, .. } => (sender.total_sent, sender.retransmissions),
            Session::Gbn { sender, .. } => (sender.total_sent, sender.retransmissions),
            Session::Sr { sender, .. } => (sender.total_sent, sender.retransmissions),
        }
    }

    fn finish(self, now: Instant) -> TransferOutput {
        let delivered_sha = self.reassembler.sha256_digest();
        let hash_ok =
            self.reassembler.is_complete(self.total_chunks) && delivered_sha == self.source_sha;
        let status = if hash_ok {
            "SUCCESS"
        } else if self.reassembler.is_complete(self.total_chunks) {
            "INTEGRITY_FAIL"
        } else {
            "TIMEOUT"
        };

        let (data_sent, retransmissions) = self.protocol_totals();
        let duration = now.saturating_duration_since(self.epoch);
        let rto_config = rto_parameters(self.estimator.config());
        let sample_count = self.estimator.stats.sample_count as usize;
        let karn_rejected = self.estimator.stats.karn_rejected_count;
        let timeout_count = self.estimator.stats.timeout_count as usize;
        let backoff_count = self.estimator.stats.backoff_count as usize;
        let min_rtt_ms = self.estimator.stats.min_rtt.map(millis).unwrap_or(0.0);
        let max_rtt_ms = self.estimator.stats.max_rtt.map(millis).unwrap_or(0.0);
        let mean_rtt_ms = self.estimator.mean_rtt_ms();
        let final_srtt_ms = self.estimator.srtt().map(millis).unwrap_or(0.0);
        let final_rttvar_ms = self.estimator.rttvar().map(millis).unwrap_or(0.0);
        let final_rto_ms = millis(self.estimator.current_rto());
        let forward_config = forward_record(&self.forward.config);
        let reverse_config = reverse_record(&self.reverse.config);
        let reverse_seed = self.reverse.config.seed;
        let forward_channel = channel_metrics(&self.forward.stats);
        let reverse_channel = channel_metrics(&self.reverse.stats);

        let record = ExperimentRecord::new(
            ExperimentMeta {
                experiment_id: self.experiment_id,
                protocol: self.protocol_name,
                window_size: self.window_recorded,
                chunk_size: self.chunk_size,
                segment_count: self.total_chunks as usize,
                seed: self.forward.config.seed,
                reverse_seed,
                trial_id: self.trial_id,
            },
            forward_config,
            reverse_config,
            rto_config,
            ApplicationMetrics {
                source_bytes: self.source_len,
                delivered_unique_bytes: self.reassembler.total_bytes_accepted as u64,
                transfer_status: status.to_string(),
                sha256_match: hash_ok,
                source_sha256: self.source_sha,
                delivered_sha256: delivered_sha,
            },
            TimingMetrics {
                duration_secs: duration.as_secs_f64(),
                total_rtt_samples: sample_count,
                karn_rejected_count: karn_rejected as usize,
                min_rtt_ms,
                max_rtt_ms,
                mean_rtt_ms,
                final_srtt_ms,
                final_rttvar_ms,
                final_rto_ms,
                timeout_count,
                backoff_count,
            },
            ProtocolMetrics {
                data_sent: data_sent as usize,
                ack_sent: self.ack_sent as usize,
                data_received: self.data_received as usize,
                ack_received: self.ack_received as usize,
                data_retransmissions: retransmissions as usize,
                // Packets resubmitted from fire_timers. This driver has no other retransmission path.
                timeout_retransmissions: self.retx_submitted as usize,
                premature_retransmissions: self.premature as usize,
                duplicate_data_received: self.duplicate_data as usize,
                duplicate_acks_received: self.duplicate_acks as usize,
            },
            forward_channel,
            reverse_channel.clone(),
        );

        TransferOutput {
            record,
            premature_retransmissions: self.premature,
            karn_rejected,
            reverse_seed,
            segment_count: self.total_chunks,
            forward_events: self.forward_events,
            reverse: reverse_channel,
        }
    }
}
