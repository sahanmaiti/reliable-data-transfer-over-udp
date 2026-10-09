// CS-30003: Reliable Data Transfer over UDP
// Author: Sahan Maiti (Evaluation & Integration)
// Component: Application Layer - Real localhost UDP transport demo
//
// Thin socket path around the existing ARQ state machines, Packet codec,
// RtoEstimator, and timers. Does not use the deterministic Channel emulator
// or the virtual-time transfer driver.

use crate::app::file_io::{Chunker, Reassembler};
use crate::arq::{
    GoBackNReceiver, GoBackNSender, ProtocolType, SelectiveRepeatReceiver, SelectiveRepeatSender,
    StopAndWaitReceiver, StopAndWaitSender,
};
use crate::packet::{Packet, PacketType, HEADER_SIZE, MAX_PAYLOAD_SIZE};
use crate::timing::{MultiTimer, RetransmissionTimer, RtoConfig, RtoEstimator};
use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DEFAULT_TRANSFER_DEADLINE: Duration = Duration::from_secs(30);
const DEFAULT_MAX_DATA_RETRIES: u32 = 64;
const DEFAULT_MAX_FIN_RETRIES: u32 = 32;
const DEFAULT_RECV_LINGER: Duration = Duration::from_millis(500);
const MAX_DATAGRAM: usize = HEADER_SIZE + MAX_PAYLOAD_SIZE;

/// Configuration for a real-UDP send or receive demonstration.
#[derive(Debug, Clone)]
pub struct UdpTransferConfig {
    pub protocol: ProtocolType,
    pub window: usize,
    pub chunk_size: usize,
    pub rto: RtoConfig,
    pub max_data_retries: u32,
    pub max_fin_retries: u32,
    pub transfer_deadline: Duration,
    pub recv_linger: Duration,
}

impl Default for UdpTransferConfig {
    fn default() -> Self {
        Self {
            protocol: ProtocolType::StopAndWait,
            window: 1,
            chunk_size: MAX_PAYLOAD_SIZE,
            rto: RtoConfig::default(),
            max_data_retries: DEFAULT_MAX_DATA_RETRIES,
            max_fin_retries: DEFAULT_MAX_FIN_RETRIES,
            transfer_deadline: DEFAULT_TRANSFER_DEADLINE,
            recv_linger: DEFAULT_RECV_LINGER,
        }
    }
}

impl UdpTransferConfig {
    pub fn effective_window(&self) -> usize {
        match self.protocol {
            ProtocolType::StopAndWait => 1,
            ProtocolType::GoBackN | ProtocolType::SelectiveRepeat => {
                assert!(self.window > 0, "window must be > 0");
                self.window
            }
        }
    }
}

/// Outcome of a successful real-UDP send.
#[derive(Debug, Clone)]
pub struct UdpSendResult {
    pub bytes_sent: usize,
    pub source_sha256: String,
    pub chunks: usize,
    pub duration: Duration,
    pub final_rto: Duration,
}

/// Outcome of a successful real-UDP receive.
#[derive(Debug, Clone)]
pub struct UdpRecvResult {
    pub bytes_written: usize,
    pub delivered_sha256: String,
    pub output_path: PathBuf,
    pub duration: Duration,
    pub peer: Option<SocketAddr>,
}

/// Errors from the real-UDP demonstration path.
#[derive(Debug)]
pub enum UdpError {
    Io(io::Error),
    Timeout(String),
    RetryExhausted(String),
    Protocol(String),
    Integrity(String),
}

impl std::fmt::Display for UdpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UdpError::Io(err) => write!(f, "I/O error: {err}"),
            UdpError::Timeout(msg) => write!(f, "transfer timeout: {msg}"),
            UdpError::RetryExhausted(msg) => write!(f, "retry exhausted: {msg}"),
            UdpError::Protocol(msg) => write!(f, "protocol error: {msg}"),
            UdpError::Integrity(msg) => write!(f, "integrity error: {msg}"),
        }
    }
}

impl std::error::Error for UdpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            UdpError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for UdpError {
    fn from(value: io::Error) -> Self {
        UdpError::Io(value)
    }
}

#[derive(Debug, Clone)]
struct SendMeta {
    first_sent_at: Instant,
    ever_retransmitted: bool,
    rtt_sampled: bool,
}

enum SenderEngine {
    Sw {
        sender: StopAndWaitSender,
        timer: RetransmissionTimer,
    },
    Gbn {
        sender: GoBackNSender,
        timer: RetransmissionTimer,
    },
    Sr {
        sender: SelectiveRepeatSender,
        timers: MultiTimer<u32>,
    },
}

enum ReceiverEngine {
    Sw(StopAndWaitReceiver),
    Gbn(GoBackNReceiver),
    Sr(SelectiveRepeatReceiver),
}

/// Send `input` to `peer` over localhost UDP using the selected ARQ protocol.
pub fn send_file<P: AsRef<Path>>(
    input: P,
    peer: SocketAddr,
    config: &UdpTransferConfig,
) -> Result<UdpSendResult, UdpError> {
    let input = input.as_ref();
    let started = Instant::now();
    let deadline = started + config.transfer_deadline;

    let chunker = Chunker::new(config.chunk_size);
    let packets = chunker.chunk_file(input)?;
    let payloads: Vec<Vec<u8>> = packets.into_iter().map(|p| p.payload).collect();
    let bytes_sent: usize = payloads.iter().map(|p| p.len()).sum();
    let source_sha256 = crate::app::integrity::compute_file_sha256(input)?;

    let socket = UdpSocket::bind("127.0.0.1:0")?;
    socket.connect(peer)?;
    socket.set_nonblocking(false)?;

    let mut engine = match config.protocol {
        ProtocolType::StopAndWait => SenderEngine::Sw {
            sender: StopAndWaitSender::new(config.max_data_retries),
            timer: RetransmissionTimer::new(),
        },
        ProtocolType::GoBackN => SenderEngine::Gbn {
            sender: GoBackNSender::new(config.effective_window()),
            timer: RetransmissionTimer::new(),
        },
        ProtocolType::SelectiveRepeat => SenderEngine::Sr {
            sender: SelectiveRepeatSender::new(config.effective_window()),
            timers: MultiTimer::new(),
        },
    };

    let mut estimator = RtoEstimator::with_config(config.rto.clone());
    let mut next_chunk: usize = 0;
    let mut meta: HashMap<u32, SendMeta> = HashMap::new();
    let mut fin_seq: Option<u32> = None;
    let mut fin_packet: Option<Packet> = None;
    let mut fin_retries: u32 = 0;
    let mut fin_timer = RetransmissionTimer::new();
    let mut buf = [0u8; MAX_DATAGRAM];

    loop {
        if Instant::now() >= deadline {
            return Err(UdpError::Timeout(
                "sender exceeded transfer deadline".into(),
            ));
        }

        // Fill the sending window with remaining DATA chunks.
        if fin_seq.is_none() {
            while next_chunk < payloads.len() && can_send(&engine) {
                let payload = payloads[next_chunk].clone();
                let Some(pkt) = send_chunk(&mut engine, payload) else {
                    break;
                };
                let seq = pkt.seq_num;
                send_datagram(&socket, &pkt)?;
                meta.insert(
                    seq,
                    SendMeta {
                        first_sent_at: Instant::now(),
                        ever_retransmitted: false,
                        rtt_sampled: false,
                    },
                );
                arm_after_send(&mut engine, seq, estimator.current_rto());
                next_chunk += 1;
            }

            if next_chunk >= payloads.len() && data_fully_acked(&engine) {
                let seq = next_data_seq(&engine);
                let mut fin = Packet::new_fin(seq);
                fin.compute_and_set_checksum();
                send_datagram(&socket, &fin)?;
                fin_seq = Some(seq);
                fin_packet = Some(fin);
                fin_retries = 0;
                fin_timer.start(estimator.current_rto());
            }
        }

        let wait = next_wait(
            &engine,
            &fin_timer,
            fin_seq.is_some(),
            deadline,
            estimator.current_rto(),
        );
        socket.set_read_timeout(Some(wait.max(Duration::from_millis(1))))?;

        match socket.recv(&mut buf) {
            Ok(n) => match Packet::deserialize(&buf[..n]) {
                Ok(pkt) if pkt.pkt_type == PacketType::Ack && pkt.is_valid() => {
                    if let Some(seq) = fin_seq {
                        if pkt.seq_num == seq {
                            return Ok(UdpSendResult {
                                bytes_sent,
                                source_sha256,
                                chunks: payloads.len(),
                                duration: started.elapsed(),
                                final_rto: estimator.current_rto(),
                            });
                        }
                    }
                    handle_data_ack(&mut engine, &mut estimator, &mut meta, &pkt);
                }
                Ok(_) | Err(_) => {
                    // Malformed, checksum failure, or unexpected type: ignore.
                }
            },
            Err(err) if is_timeout_error(&err) => {
                // Fall through to timer handling below.
            }
            Err(err) => return Err(UdpError::Io(err)),
        }

        let now = Instant::now();
        if let Some(seq) = fin_seq {
            if fin_timer.is_expired_at(now) {
                if fin_retries >= config.max_fin_retries {
                    return Err(UdpError::RetryExhausted(format!(
                        "FIN seq {seq} exceeded max retries"
                    )));
                }
                fin_retries += 1;
                estimator.on_timeout();
                if let Some(ref mut fin) = fin_packet {
                    fin.set_retransmitted();
                    fin.compute_and_set_checksum();
                    send_datagram(&socket, fin)?;
                }
                fin_timer.start(estimator.current_rto());
            }
            continue;
        }

        handle_data_timeouts(&mut engine, &mut estimator, &mut meta, &socket, now)?;
    }
}

/// Bind `bind_addr`, receive a transfer, write `output`, and verify SHA-256 of written bytes.
pub fn recv_file<P: AsRef<Path>>(
    bind_addr: SocketAddr,
    output: P,
    config: &UdpTransferConfig,
) -> Result<UdpRecvResult, UdpError> {
    let socket = UdpSocket::bind(bind_addr)?;
    recv_file_on_socket(socket, output, config)
}

/// Receive on an already-bound socket (used by CLI when advertising an ephemeral port).
pub fn recv_file_on_socket<P: AsRef<Path>>(
    socket: UdpSocket,
    output: P,
    config: &UdpTransferConfig,
) -> Result<UdpRecvResult, UdpError> {
    let output = output.as_ref();
    let started = Instant::now();
    let deadline = started + config.transfer_deadline;

    socket.set_nonblocking(false)?;

    let mut engine = match config.protocol {
        ProtocolType::StopAndWait => ReceiverEngine::Sw(StopAndWaitReceiver::new()),
        ProtocolType::GoBackN => ReceiverEngine::Gbn(GoBackNReceiver::new()),
        ProtocolType::SelectiveRepeat => {
            ReceiverEngine::Sr(SelectiveRepeatReceiver::new(config.effective_window()))
        }
    };

    let mut reassembler = Reassembler::new();
    let mut peer: Option<SocketAddr> = None;
    let mut fin_seen = false;
    let mut linger_until: Option<Instant> = None;
    let mut buf = [0u8; MAX_DATAGRAM];

    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err(UdpError::Timeout(
                "receiver exceeded transfer deadline".into(),
            ));
        }
        if let Some(until) = linger_until {
            if now >= until {
                break;
            }
        }

        let wait = if let Some(until) = linger_until {
            until.saturating_duration_since(now)
        } else {
            deadline
                .saturating_duration_since(now)
                .min(Duration::from_millis(250))
        };
        socket.set_read_timeout(Some(wait.max(Duration::from_millis(1))))?;

        match socket.recv_from(&mut buf) {
            Ok((n, addr)) => {
                peer = Some(addr);
                let pkt = match Packet::deserialize(&buf[..n]) {
                    Ok(pkt) => pkt,
                    Err(_) => continue,
                };
                if !pkt.is_valid() {
                    continue;
                }

                match pkt.pkt_type {
                    PacketType::Data | PacketType::Fin => {
                        if let Some(ack) = handle_receiver_packet(&mut engine, &pkt) {
                            let _ = socket.send_to(&ack.serialize(), addr);
                        }
                        absorb(&mut engine, &mut reassembler);

                        if pkt.pkt_type == PacketType::Fin {
                            if !fin_seen {
                                fin_seen = true;
                                reassembler.write_to_disk(output)?;
                                linger_until = Some(Instant::now() + config.recv_linger);
                            }
                        }
                    }
                    PacketType::Ack => {
                        // Receiver ignores ACKs.
                    }
                }
            }
            Err(err) if is_timeout_error(&err) => {
                if linger_until.is_some() {
                    break;
                }
            }
            Err(err) => return Err(UdpError::Io(err)),
        }
    }

    if !fin_seen {
        return Err(UdpError::Timeout("receiver never received FIN".into()));
    }

    let delivered_sha256 = reassembler.sha256_digest();
    let file_sha = crate::app::integrity::compute_file_sha256(output)?;
    if file_sha != delivered_sha256 {
        return Err(UdpError::Integrity(format!(
            "written file digest {file_sha} != reconstructed {delivered_sha256}"
        )));
    }

    Ok(UdpRecvResult {
        bytes_written: reassembler.total_bytes_accepted,
        delivered_sha256,
        output_path: output.to_path_buf(),
        duration: started.elapsed(),
        peer,
    })
}

fn is_timeout_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

fn send_datagram(socket: &UdpSocket, pkt: &Packet) -> Result<(), UdpError> {
    socket.send(&pkt.serialize())?;
    Ok(())
}

fn can_send(engine: &SenderEngine) -> bool {
    match engine {
        SenderEngine::Sw { sender, .. } => sender.can_send(),
        SenderEngine::Gbn { sender, .. } => sender.can_send(),
        SenderEngine::Sr { sender, .. } => sender.can_send(),
    }
}

fn send_chunk(engine: &mut SenderEngine, payload: Vec<u8>) -> Option<Packet> {
    match engine {
        SenderEngine::Sw { sender, .. } => sender.send_chunk(payload),
        SenderEngine::Gbn { sender, .. } => sender.send_chunk(payload),
        SenderEngine::Sr { sender, .. } => sender.send_chunk(payload),
    }
}

fn data_fully_acked(engine: &SenderEngine) -> bool {
    match engine {
        SenderEngine::Sw { sender, .. } => !sender.is_waiting_for_ack(),
        SenderEngine::Gbn { sender, .. } => !sender.has_unacked_packets(),
        SenderEngine::Sr { sender, .. } => sender.in_flight_count() == 0,
    }
}

fn next_data_seq(engine: &SenderEngine) -> u32 {
    match engine {
        SenderEngine::Sw { sender, .. } => sender.current_seq(),
        SenderEngine::Gbn { sender, .. } => sender.next_seq_num(),
        SenderEngine::Sr { sender, .. } => sender.next_seq_num(),
    }
}

fn arm_after_send(engine: &mut SenderEngine, seq: u32, rto: Duration) {
    match engine {
        SenderEngine::Sw { timer, .. } | SenderEngine::Gbn { timer, .. } => {
            timer.start(rto);
        }
        SenderEngine::Sr { timers, .. } => {
            timers.start_timer(seq, rto);
        }
    }
}

fn next_wait(
    engine: &SenderEngine,
    fin_timer: &RetransmissionTimer,
    fin_active: bool,
    deadline: Instant,
    fallback: Duration,
) -> Duration {
    let now = Instant::now();
    let until_deadline = deadline.saturating_duration_since(now);
    if fin_active {
        let fin_rem = fin_timer.remaining_at(now).unwrap_or(fallback);
        return until_deadline.min(fin_rem).max(Duration::from_millis(1));
    }
    let timer_rem = match engine {
        SenderEngine::Sw { timer, .. } | SenderEngine::Gbn { timer, .. } => {
            timer.remaining_at(now).unwrap_or(fallback)
        }
        SenderEngine::Sr { timers, .. } => timers
            .earliest_deadline()
            .map(|dl| if now >= dl { Duration::ZERO } else { dl - now })
            .unwrap_or(fallback),
    };
    until_deadline.min(timer_rem).max(Duration::from_millis(1))
}

fn handle_data_ack(
    engine: &mut SenderEngine,
    estimator: &mut RtoEstimator,
    meta: &mut HashMap<u32, SendMeta>,
    pkt: &Packet,
) {
    let sample_seq = match engine {
        SenderEngine::Sw { sender, .. } => sender.current_seq(),
        SenderEngine::Gbn { sender, .. } => sender.send_base(),
        SenderEngine::Sr { .. } => pkt.seq_num,
    };

    let accepted = match engine {
        SenderEngine::Sw { sender, timer } => {
            let ok = sender.handle_ack(pkt);
            if ok {
                timer.cancel();
                if sender.is_waiting_for_ack() {
                    timer.start(estimator.current_rto());
                }
            }
            ok
        }
        SenderEngine::Gbn { sender, timer } => {
            let ok = sender.handle_ack(pkt);
            if ok {
                timer.cancel();
                if sender.has_unacked_packets() {
                    timer.start(estimator.current_rto());
                }
            }
            ok
        }
        SenderEngine::Sr { sender, timers } => {
            let ok = sender.handle_ack(pkt);
            if ok {
                timers.cancel_timer(&pkt.seq_num);
            }
            ok
        }
    };

    if !accepted {
        return;
    }

    if let Some(entry) = meta.get_mut(&sample_seq) {
        if !entry.rtt_sampled {
            entry.rtt_sampled = true;
            let sample = Instant::now().saturating_duration_since(entry.first_sent_at);
            estimator.update_rtt(sample, entry.ever_retransmitted);
        }
    }
}

fn handle_data_timeouts(
    engine: &mut SenderEngine,
    estimator: &mut RtoEstimator,
    meta: &mut HashMap<u32, SendMeta>,
    socket: &UdpSocket,
    now: Instant,
) -> Result<(), UdpError> {
    match engine {
        SenderEngine::Sw { sender, timer } => {
            if timer.is_expired_at(now) && sender.is_waiting_for_ack() {
                estimator.on_timeout();
                match sender.handle_timeout() {
                    Some(pkt) => {
                        mark_retransmitted(meta, pkt.seq_num);
                        send_datagram(socket, &pkt)?;
                        timer.start(estimator.current_rto());
                    }
                    None => {
                        return Err(UdpError::RetryExhausted(
                            "StopAndWait data retries exhausted".into(),
                        ));
                    }
                }
            }
        }
        SenderEngine::Gbn { sender, timer } => {
            if timer.is_expired_at(now) && sender.has_unacked_packets() {
                estimator.on_timeout();
                let retries = sender.handle_timeout();
                if retries.is_empty() {
                    return Err(UdpError::RetryExhausted(
                        "GoBackN timeout with empty window".into(),
                    ));
                }
                for pkt in retries {
                    mark_retransmitted(meta, pkt.seq_num);
                    send_datagram(socket, &pkt)?;
                }
                timer.start(estimator.current_rto());
            }
        }
        SenderEngine::Sr { sender, timers } => {
            let expired = timers.pop_expired_at(now);
            for seq in expired {
                estimator.on_timeout();
                if let Some(pkt) = sender.handle_timeout(seq) {
                    mark_retransmitted(meta, pkt.seq_num);
                    send_datagram(socket, &pkt)?;
                    timers.start_timer(seq, estimator.current_rto());
                }
            }
        }
    }
    Ok(())
}

fn mark_retransmitted(meta: &mut HashMap<u32, SendMeta>, seq: u32) {
    if let Some(entry) = meta.get_mut(&seq) {
        entry.ever_retransmitted = true;
    }
}

fn handle_receiver_packet(engine: &mut ReceiverEngine, pkt: &Packet) -> Option<Packet> {
    match engine {
        ReceiverEngine::Sw(receiver) => receiver.handle_packet(pkt),
        ReceiverEngine::Gbn(receiver) => receiver.handle_packet(pkt),
        ReceiverEngine::Sr(receiver) => receiver.handle_packet(pkt),
    }
}

fn absorb(engine: &mut ReceiverEngine, reassembler: &mut Reassembler) {
    let chunks = match engine {
        ReceiverEngine::Sw(receiver) => receiver.drain_delivered(),
        ReceiverEngine::Gbn(receiver) => receiver.drain_delivered(),
        ReceiverEngine::Sr(receiver) => receiver.drain_delivered(),
    };
    for chunk in chunks {
        let seq = reassembler.next_expected_seq;
        reassembler.push_chunk(seq, &chunk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fin_packet_is_acknowledged_by_all_receivers() {
        let fin = {
            let mut pkt = Packet::new_fin(7);
            pkt.compute_and_set_checksum();
            pkt
        };

        let mut sw = StopAndWaitReceiver::new();
        let ack = sw.handle_packet(&fin).expect("SW FIN ACK");
        assert_eq!(ack.pkt_type, PacketType::Ack);
        assert_eq!(ack.seq_num, 7);

        let mut gbn = GoBackNReceiver::new();
        let ack = gbn.handle_packet(&fin).expect("GBN FIN ACK");
        assert_eq!(ack.seq_num, 7);

        let mut sr = SelectiveRepeatReceiver::new(4);
        let ack = sr.handle_packet(&fin).expect("SR FIN ACK");
        assert_eq!(ack.seq_num, 7);
    }

    #[test]
    fn effective_window_forces_stop_and_wait_to_one() {
        let cfg = UdpTransferConfig {
            protocol: ProtocolType::StopAndWait,
            window: 99,
            ..UdpTransferConfig::default()
        };
        assert_eq!(cfg.effective_window(), 1);
    }
}
