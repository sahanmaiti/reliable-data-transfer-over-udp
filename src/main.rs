// CS-30003: Reliable Data Transfer over UDP
// Author: Sahan Maiti (Evaluation & Integration)
// Entry Point: Binary CLI Driver

use clap::{Parser, Subcommand};
use reliable_udp::app::{
    compute_file_sha256, recv_file_on_socket, run_transfer, send_file, verify_file_integrity,
    Chunker, Reassembler, TransferConfig, UdpTransferConfig,
};
use reliable_udp::arq::{
    GoBackNReceiver, GoBackNSender, ProtocolType, SelectiveRepeatReceiver, SelectiveRepeatSender,
    StopAndWaitReceiver, StopAndWaitSender,
};
use reliable_udp::channel::{Channel, ChannelConfig};
use reliable_udp::metrics::{
    ApplicationMetrics, ChannelMetrics, ExperimentMeta, ExperimentRecord, ForwardChannelConfig,
    ProtocolMetrics, ReverseChannelConfig, RtoParameters, TimingMetrics,
};
use reliable_udp::packet::{Packet, MAX_PAYLOAD_SIZE};
use reliable_udp::timing::{RtoConfig, RtoEstimator};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, Instant};

/// Driver loop ceilings. These stop a stuck virtual-time run; they are not experiment factors.
const DRIVER_MAX_VIRTUAL_TIME: Duration = Duration::from_secs(3600);
const DRIVER_MAX_RETRANSMISSIONS: u64 = 5_000;

#[derive(Parser, Debug)]
#[command(name = "reliable_udp")]
#[command(author = "Sahan Maiti, Soumyadeb Mukherjee, Kashish Gupta, Ashwika Burman")]
#[command(version = "0.1.0")]
#[command(about = "Reliable Data Transfer over UDP (Stop-and-Wait, GBN, Selective Repeat)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Chunks a file and prints chunk statistics and SHA-256 digest
    ChunkInspect {
        /// Path to the file to inspect
        #[arg(short, long)]
        file: PathBuf,
        /// Chunk size in bytes (default: 1400)
        #[arg(short, long, default_value_t = MAX_PAYLOAD_SIZE)]
        chunk_size: usize,
    },

    /// Verifies SHA-256 hash match between source and received files
    Verify {
        /// Original source file path
        #[arg(short, long)]
        source: PathBuf,
        /// Received/reconstructed file path
        #[arg(short, long)]
        received: PathBuf,
    },

    /// Runs an in-memory vertical slice test (Chunk -> Reassemble -> Verify -> Log Metrics)
    BenchPipeline {
        /// File to run pipeline verification on
        #[arg(short, long)]
        file: PathBuf,
        /// Protocol identifier (StopAndWait, GoBackN, SelectiveRepeat)
        #[arg(short, long, default_value = "StopAndWait")]
        protocol: String,
        /// Window size (default: 1 for SW, 8 for GBN/SR)
        #[arg(short, long, default_value_t = 1)]
        window: usize,
        /// Path to append raw CSV metrics to
        #[arg(long)]
        csv_out: Option<PathBuf>,
        /// Path to save JSON experiment output to
        #[arg(long)]
        json_out: Option<PathBuf>,
    },

    /// Simulates a reliable transfer through the deterministic channel emulator & ARQ protocol
    Simulate {
        /// File to transfer
        #[arg(short, long)]
        file: PathBuf,
        /// Protocol: StopAndWait, GoBackN, SelectiveRepeat
        #[arg(short, long, default_value = "StopAndWait")]
        protocol: String,
        /// Window size (1 for SW, >= 1 for GBN/SR)
        #[arg(short, long, default_value_t = 4)]
        window: usize,
        /// Packet loss probability [0.0 - 1.0]
        #[arg(long, default_value_t = 0.0)]
        loss: f64,
        /// Packet reordering probability [0.0 - 1.0]
        #[arg(long, default_value_t = 0.0)]
        reorder: f64,
        /// Random seed for deterministic reproducibility
        #[arg(long, default_value_t = 42)]
        seed: u64,
    },

    /// Runs one virtual-time transfer through the real ARQ driver and writes its record.
    RunExperiment {
        /// Source file to transfer.
        #[arg(short, long)]
        file: PathBuf,
        /// StopAndWait, GoBackN, or SelectiveRepeat. Case and hyphen variants are accepted.
        #[arg(short, long, value_parser = parse_protocol)]
        protocol: ProtocolType,
        /// Sender window. Stop-and-Wait always runs with one outstanding segment.
        #[arg(short, long, default_value_t = 4)]
        window: usize,
        /// Payload bytes per segment. Default is the packet module's maximum payload.
        #[arg(long, default_value_t = MAX_PAYLOAD_SIZE)]
        chunk_size: usize,
        /// Forward-channel seed. The reverse channel uses seed + 1.
        #[arg(long, default_value_t = 42)]
        seed: u64,
        /// Trial number stored on the result. Not random.
        #[arg(long, default_value_t = 1)]
        trial_id: usize,
        /// Stable run name. When omitted, derived from the other options.
        #[arg(long)]
        experiment_id: Option<String>,
        /// Forward loss probability, 0.0 to 1.0.
        #[arg(long, default_value_t = 0.0)]
        loss: f64,
        /// Forward reorder probability, 0.0 to 1.0.
        #[arg(long, default_value_t = 0.0)]
        reorder: f64,
        /// Forward duplication probability, 0.0 to 1.0.
        #[arg(long, default_value_t = 0.0)]
        duplicate: f64,
        /// Forward corruption probability, 0.0 to 1.0.
        #[arg(long, default_value_t = 0.0)]
        corrupt: f64,
        /// One-way base delay in milliseconds. Same default as `simulate`.
        #[arg(long, default_value_t = 10)]
        base_delay_ms: u64,
        /// Extra random delay bound in milliseconds.
        #[arg(long, default_value_t = 0)]
        jitter_ms: u64,
        /// Extra hold, in milliseconds, when the forward channel reorders a datagram.
        /// Same default as `simulate`.
        #[arg(long, default_value_t = 20)]
        reorder_extra_ms: u64,
        /// Scales the computed RTO. Default matches `RtoConfig`.
        #[arg(long, default_value_t = 1.0)]
        rto_multiplier: f64,
        /// Minimum RTO in milliseconds. Default matches `RtoConfig`.
        #[arg(long, default_value_t = 200)]
        min_rto_ms: u64,
        /// Maximum RTO in milliseconds. Default matches `RtoConfig`.
        #[arg(long, default_value_t = 60000)]
        max_rto_ms: u64,
        /// RTO used before the first RTT sample, in milliseconds. Default matches `RtoConfig`.
        #[arg(long, default_value_t = 1000)]
        initial_rto_ms: u64,
        /// Write the ExperimentRecord JSON produced by this run.
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Append the ExperimentRecord CSV row produced by this run.
        #[arg(long)]
        csv_out: Option<PathBuf>,
    },

    /// Real UDP file send demonstration (localhost). Not part of the deterministic experiment matrix.
    Send {
        /// Source file to transfer.
        #[arg(short, long)]
        file: PathBuf,
        /// Receiver socket address, e.g. 127.0.0.1:9000.
        #[arg(long)]
        to: String,
        /// StopAndWait, GoBackN, or SelectiveRepeat.
        #[arg(short, long, value_parser = parse_protocol, default_value = "StopAndWait")]
        protocol: ProtocolType,
        /// Sender window. Stop-and-Wait always uses 1.
        #[arg(short, long, default_value_t = 8)]
        window: usize,
        /// Payload bytes per DATA segment.
        #[arg(long, default_value_t = MAX_PAYLOAD_SIZE)]
        chunk_size: usize,
        /// Scales the computed RTO. Default matches `RtoConfig`.
        #[arg(long, default_value_t = 1.0)]
        rto_multiplier: f64,
        /// Minimum RTO in milliseconds.
        #[arg(long, default_value_t = 200)]
        min_rto_ms: u64,
        /// Maximum RTO in milliseconds.
        #[arg(long, default_value_t = 60000)]
        max_rto_ms: u64,
        /// Initial RTO in milliseconds before the first RTT sample.
        #[arg(long, default_value_t = 1000)]
        initial_rto_ms: u64,
        /// Hard wall-clock deadline for the whole transfer, in seconds.
        #[arg(long, default_value_t = 30)]
        deadline_secs: u64,
    },

    /// Real UDP file receive demonstration (localhost). Not part of the deterministic experiment matrix.
    Recv {
        /// Local bind address, e.g. 127.0.0.1:9000.
        #[arg(long, default_value = "127.0.0.1:0")]
        bind: String,
        /// Output path for the reconstructed file.
        #[arg(short, long)]
        output: PathBuf,
        /// StopAndWait, GoBackN, or SelectiveRepeat. Must match the sender.
        #[arg(short, long, value_parser = parse_protocol, default_value = "StopAndWait")]
        protocol: ProtocolType,
        /// Receiver window for Selective Repeat (ignored for Stop-and-Wait / Go-Back-N receive window).
        #[arg(short, long, default_value_t = 8)]
        window: usize,
        /// Expected payload chunk size (must match the sender).
        #[arg(long, default_value_t = MAX_PAYLOAD_SIZE)]
        chunk_size: usize,
        /// Hard wall-clock deadline for the whole transfer, in seconds.
        #[arg(long, default_value_t = 30)]
        deadline_secs: u64,
        /// How long to linger after FIN to re-ACK lost FIN ACKs, in milliseconds.
        #[arg(long, default_value_t = 500)]
        linger_ms: u64,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ChunkInspect { file, chunk_size } => {
            println!("== File Chunker Inspection ==");
            println!("Target file: {:?}", file);
            let sha = compute_file_sha256(&file)?;
            println!("SHA-256: {}", sha);

            let chunker = Chunker::new(chunk_size);
            let packets = chunker.chunk_file(&file)?;
            println!("Total Chunks: {}", packets.len());
            println!("Chunk payload size: {} bytes", chunk_size);
            if let Some(first) = packets.first() {
                println!(
                    "First packet seq: {}, payload len: {}",
                    first.seq_num,
                    first.payload.len()
                );
            }
            if let Some(last) = packets.last() {
                println!(
                    "Last packet seq: {}, payload len: {}",
                    last.seq_num,
                    last.payload.len()
                );
            }
        }

        Commands::Verify { source, received } => {
            println!("== SHA-256 End-to-End Verification ==");
            let report = verify_file_integrity(&source, &received)?;
            println!(
                "Source:   {} (SHA-256: {})",
                report.source_path, report.source_sha256
            );
            println!(
                "Received: {} (SHA-256: {})",
                report.received_path, report.received_sha256
            );
            println!("Total Bytes: {}", report.total_bytes);
            if report.is_match {
                println!("Result:   SUCCESS [Checksums Match! File is byte-identical]");
            } else {
                eprintln!("Result:   FAILED [Checksum mismatch or size difference]");
                std::process::exit(1);
            }
        }

        Commands::BenchPipeline {
            file,
            protocol,
            window,
            csv_out,
            json_out,
        } => {
            println!("== Running In-Memory Pipeline Benchmark ==");
            let start = Instant::now();
            let source_sha = compute_file_sha256(&file)?;
            let file_bytes = std::fs::metadata(&file)?.len();

            let chunker = Chunker::default();
            let packets = chunker.chunk_file(&file)?;
            let total_chunks = packets.len() as u32;

            let mut reassembler = Reassembler::new();
            for pkt in &packets {
                reassembler.push_chunk(pkt.seq_num, &pkt.payload);
            }

            let elapsed = start.elapsed().as_secs_f64();
            let reconstructed_sha = reassembler.sha256_digest();
            let is_match = source_sha == reconstructed_sha;

            println!("Reconstruction Complete: {}", is_match);
            println!(
                "Elapsed: {:.4}s | Goodput: {:.2} KB/s",
                elapsed,
                (file_bytes as f64 / elapsed) / 1024.0
            );

            // bench-pipeline only chunks and reassembles in memory. It does not run
            // the channel or the RTO estimator, so those counters stay zero.
            let record = ExperimentRecord::new(
                ExperimentMeta {
                    experiment_id: format!("LOCAL_BENCH_{}", protocol),
                    protocol: protocol.clone(),
                    window_size: window,
                    chunk_size: MAX_PAYLOAD_SIZE,
                    segment_count: total_chunks as usize,
                    seed: 12345,
                    reverse_seed: 0,
                    trial_id: 1,
                },
                ForwardChannelConfig {
                    configured_loss_rate: 0.0,
                    configured_reorder_rate: 0.0,
                    configured_duplicate_rate: 0.0,
                    configured_corrupt_rate: 0.0,
                    base_delay_ms: 0,
                    jitter_ms: 0,
                    reorder_extra_ms: 0,
                },
                ReverseChannelConfig {
                    configured_loss_rate: 0.0,
                    configured_reorder_rate: 0.0,
                    configured_duplicate_rate: 0.0,
                    configured_corrupt_rate: 0.0,
                    base_delay_ms: 0,
                    jitter_ms: 0,
                    reorder_extra_ms: 0,
                },
                RtoParameters {
                    rto_multiplier: 0.0,
                    initial_rto_ms: 0.0,
                    min_rto_ms: 0.0,
                    max_rto_ms: 0.0,
                    alpha: 0.0,
                    beta: 0.0,
                    k: 0.0,
                },
                ApplicationMetrics {
                    source_bytes: file_bytes,
                    delivered_unique_bytes: reassembler.total_bytes_accepted as u64,
                    transfer_status: if is_match {
                        "SUCCESS".to_string()
                    } else {
                        "INTEGRITY_FAIL".to_string()
                    },
                    sha256_match: is_match,
                    source_sha256: source_sha,
                    delivered_sha256: reconstructed_sha,
                },
                TimingMetrics {
                    duration_secs: elapsed,
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
                },
                ProtocolMetrics::default(),
                ChannelMetrics::default(),
                ChannelMetrics::default(),
            );

            if let Some(csv_path) = csv_out {
                record.append_to_csv(&csv_path)?;
                println!("Appended metrics to CSV: {:?}", csv_path);
            }

            if let Some(json_path) = json_out {
                record.save_json(&json_path)?;
                println!("Saved experiment record to JSON: {:?}", json_path);
            }
        }

        Commands::Simulate {
            file,
            protocol,
            window,
            loss,
            reorder,
            seed,
        } => {
            println!("== Running Simulated ARQ Transfer over Deterministic Channel ==");
            let proto = ProtocolType::from_str(&protocol)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            let source_sha = compute_file_sha256(&file)?;
            let file_bytes = std::fs::metadata(&file)?.len();

            let chunker = Chunker::default();
            let packets = chunker.chunk_file(&file)?;
            let total_chunks = packets.len();

            let channel_config = ChannelConfig {
                seed,
                loss,
                duplicate: 0.0,
                reorder,
                reorder_extra_ms: 20,
                corrupt: 0.0,
                base_delay_ms: 10,
                jitter_ms: 0,
            };
            let mut channel = Channel::new(channel_config);
            let mut rto_estimator = RtoEstimator::new();
            let mut reassembler = Reassembler::new();

            let start = Instant::now();
            let mut retransmissions: u64 = 0;

            match proto {
                ProtocolType::StopAndWait => {
                    let mut sender = StopAndWaitSender::new(10);
                    let mut receiver = StopAndWaitReceiver::new();

                    for pkt in packets {
                        let mut current_pkt = sender.send_chunk(pkt.payload).unwrap();
                        loop {
                            let wire = current_pkt.serialize();
                            let deliveries = channel.process(&wire);

                            if deliveries.is_empty() {
                                // Packet was lost! Timeout and retransmit.
                                rto_estimator.on_timeout();
                                if let Some(retry) = sender.handle_timeout() {
                                    current_pkt = retry;
                                    continue;
                                } else {
                                    eprintln!("Transfer failed: max retries exceeded");
                                    std::process::exit(1);
                                }
                            }

                            // Receiver gets delivery
                            let mut acked = false;
                            for d in deliveries {
                                if let Ok(recv_pkt) = Packet::deserialize(&d.data) {
                                    if let Some(ack) = receiver.handle_packet(&recv_pkt) {
                                        // Forward ACK through channel back to sender
                                        let ack_wire = ack.serialize();
                                        let ack_deliveries = channel.process(&ack_wire);
                                        for ad in ack_deliveries {
                                            if let Ok(ack_p) = Packet::deserialize(&ad.data) {
                                                if sender.handle_ack(&ack_p) {
                                                    acked = true;
                                                    rto_estimator.update_rtt(
                                                        std::time::Duration::from_millis(20),
                                                        false,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if acked {
                                break;
                            } else {
                                // ACK lost or delayed
                                rto_estimator.on_timeout();
                                if let Some(retry) = sender.handle_timeout() {
                                    current_pkt = retry;
                                }
                            }
                        }
                    }

                    for chunk in receiver.drain_delivered() {
                        reassembler.push_chunk(reassembler.next_expected_seq, &chunk);
                    }
                    retransmissions = sender.retransmissions;
                }

                ProtocolType::GoBackN => {
                    let mut sender = GoBackNSender::new(window);
                    let mut receiver = GoBackNReceiver::new();

                    for pkt in packets {
                        if let Some(data_pkt) = sender.send_chunk(pkt.payload) {
                            let wire = data_pkt.serialize();
                            let deliveries = channel.process(&wire);
                            for d in deliveries {
                                if let Ok(recv_pkt) = Packet::deserialize(&d.data) {
                                    if let Some(ack) = receiver.handle_packet(&recv_pkt) {
                                        let ack_wire = ack.serialize();
                                        let ack_deliveries = channel.process(&ack_wire);
                                        for ad in ack_deliveries {
                                            if let Ok(ack_p) = Packet::deserialize(&ad.data) {
                                                sender.handle_ack(&ack_p);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Flush any remaining unacked packets
                    let retries = sender.handle_timeout();
                    for r in retries {
                        let wire = r.serialize();
                        for d in channel.process(&wire) {
                            if let Ok(recv_pkt) = Packet::deserialize(&d.data) {
                                if let Some(ack) = receiver.handle_packet(&recv_pkt) {
                                    sender.handle_ack(&ack);
                                }
                            }
                        }
                    }

                    for chunk in receiver.drain_delivered() {
                        reassembler.push_chunk(reassembler.next_expected_seq, &chunk);
                    }
                    retransmissions = sender.retransmissions;
                }

                ProtocolType::SelectiveRepeat => {
                    let mut sender = SelectiveRepeatSender::new(window);
                    let mut receiver = SelectiveRepeatReceiver::new(window);

                    for pkt in packets {
                        if let Some(data_pkt) = sender.send_chunk(pkt.payload) {
                            let wire = data_pkt.serialize();
                            let deliveries = channel.process(&wire);
                            for d in deliveries {
                                if let Ok(recv_pkt) = Packet::deserialize(&d.data) {
                                    if let Some(ack) = receiver.handle_packet(&recv_pkt) {
                                        let ack_wire = ack.serialize();
                                        let ack_deliveries = channel.process(&ack_wire);
                                        for ad in ack_deliveries {
                                            if let Ok(ack_p) = Packet::deserialize(&ad.data) {
                                                sender.handle_ack(&ack_p);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    for chunk in receiver.drain_delivered() {
                        reassembler.push_chunk(reassembler.next_expected_seq, &chunk);
                    }
                    retransmissions = sender.retransmissions;
                }
            }

            let elapsed = start.elapsed().as_secs_f64();
            let reconstructed_sha = reassembler.sha256_digest();
            let is_match = source_sha == reconstructed_sha;

            println!("Protocol:           {}", proto);
            println!("Total Chunks:       {}", total_chunks);
            println!("Transferred Bytes:  {}", file_bytes);
            println!("Elapsed Time:       {:.4}s", elapsed);
            println!("Data Retransmits:   {}", retransmissions);
            println!("SHA-256 Match:      {}", is_match);
            println!("Channel Stats:      {:?}", channel.stats);
        }
        Commands::RunExperiment {
            file,
            protocol,
            window,
            chunk_size,
            seed,
            trial_id,
            experiment_id,
            loss,
            reorder,
            duplicate,
            corrupt,
            base_delay_ms,
            jitter_ms,
            reorder_extra_ms,
            rto_multiplier,
            min_rto_ms,
            max_rto_ms,
            initial_rto_ms,
            json_out,
            csv_out,
        } => {
            run_experiment_command(RunExperimentArgs {
                file,
                protocol,
                window,
                chunk_size,
                seed,
                trial_id,
                experiment_id,
                loss,
                reorder,
                duplicate,
                corrupt,
                base_delay_ms,
                jitter_ms,
                reorder_extra_ms,
                rto_multiplier,
                min_rto_ms,
                max_rto_ms,
                initial_rto_ms,
                json_out,
                csv_out,
            })?;
        }

        Commands::Send {
            file,
            to,
            protocol,
            window,
            chunk_size,
            rto_multiplier,
            min_rto_ms,
            max_rto_ms,
            initial_rto_ms,
            deadline_secs,
        } => {
            let peer: std::net::SocketAddr = to
                .parse()
                .map_err(|err| format!("invalid --to address {to:?}: {err}"))?;
            let config = UdpTransferConfig {
                protocol,
                window,
                chunk_size,
                rto: RtoConfig {
                    rto_multiplier,
                    min_rto: Duration::from_millis(min_rto_ms),
                    max_rto: Duration::from_millis(max_rto_ms),
                    initial_rto: Duration::from_millis(initial_rto_ms),
                    ..RtoConfig::default()
                },
                transfer_deadline: Duration::from_secs(deadline_secs),
                ..UdpTransferConfig::default()
            };
            println!("== Real UDP Send ==");
            println!("File: {}", file.display());
            println!("To: {peer}");
            println!("Protocol: {protocol}");
            println!("Window: {}", config.effective_window());
            match send_file(&file, peer, &config) {
                Ok(result) => {
                    println!("Status: SUCCESS");
                    println!("Bytes sent: {}", result.bytes_sent);
                    println!("Chunks: {}", result.chunks);
                    println!("Duration: {:.4}s", result.duration.as_secs_f64());
                    println!(
                        "Final RTO: {:.2} ms",
                        result.final_rto.as_secs_f64() * 1000.0
                    );
                    println!("Source SHA-256: {}", result.source_sha256);
                }
                Err(err) => {
                    eprintln!("Status: FAILURE");
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            }
        }

        Commands::Recv {
            bind,
            output,
            protocol,
            window,
            chunk_size,
            deadline_secs,
            linger_ms,
        } => {
            let bind_addr: std::net::SocketAddr = bind
                .parse()
                .map_err(|err| format!("invalid --bind address {bind:?}: {err}"))?;
            let config = UdpTransferConfig {
                protocol,
                window,
                chunk_size,
                transfer_deadline: Duration::from_secs(deadline_secs),
                recv_linger: Duration::from_millis(linger_ms),
                ..UdpTransferConfig::default()
            };
            println!("== Real UDP Recv ==");
            println!("Bind: {bind_addr}");
            println!("Output: {}", output.display());
            println!("Protocol: {protocol}");
            println!("Window: {}", config.effective_window());
            let listener = std::net::UdpSocket::bind(bind_addr)?;
            let local = listener.local_addr()?;
            println!("Listening on: {local}");
            match recv_file_on_socket(listener, &output, &config) {
                Ok(result) => {
                    println!("Status: SUCCESS");
                    println!("Bytes written: {}", result.bytes_written);
                    println!("Duration: {:.4}s", result.duration.as_secs_f64());
                    if let Some(peer) = result.peer {
                        println!("Peer: {peer}");
                    }
                    println!("Delivered SHA-256: {}", result.delivered_sha256);
                    println!("Output file: {}", result.output_path.display());
                }
                Err(err) => {
                    eprintln!("Status: FAILURE");
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

struct RunExperimentArgs {
    file: PathBuf,
    protocol: ProtocolType,
    window: usize,
    chunk_size: usize,
    seed: u64,
    trial_id: usize,
    experiment_id: Option<String>,
    loss: f64,
    reorder: f64,
    duplicate: f64,
    corrupt: f64,
    base_delay_ms: u64,
    jitter_ms: u64,
    reorder_extra_ms: u64,
    rto_multiplier: f64,
    min_rto_ms: u64,
    max_rto_ms: u64,
    initial_rto_ms: u64,
    json_out: Option<PathBuf>,
    csv_out: Option<PathBuf>,
}

fn parse_protocol(raw: &str) -> Result<ProtocolType, String> {
    ProtocolType::from_str(raw)
}

fn run_experiment_command(args: RunExperimentArgs) -> Result<(), Box<dyn std::error::Error>> {
    validate_experiment_args(&args)?;

    let payload = std::fs::read(&args.file).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("input file {}: {err}", args.file.display()),
        )
    })?;

    let rto_defaults = RtoConfig::default();
    let experiment_id = args
        .experiment_id
        .clone()
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| deterministic_experiment_id(&args));

    let output = run_transfer(&TransferConfig {
        payload,
        protocol: args.protocol,
        window_size: args.window,
        chunk_size: args.chunk_size,
        seed: args.seed,
        trial_id: args.trial_id,
        experiment_id,
        loss_rate: args.loss,
        reorder_rate: args.reorder,
        duplicate_rate: args.duplicate,
        corrupt_rate: args.corrupt,
        base_delay_ms: args.base_delay_ms,
        jitter_ms: args.jitter_ms,
        reorder_extra_ms: args.reorder_extra_ms,
        rto_multiplier: args.rto_multiplier,
        min_rto: Duration::from_millis(args.min_rto_ms),
        max_rto: Duration::from_millis(args.max_rto_ms),
        initial_rto: Duration::from_millis(args.initial_rto_ms),
        alpha: rto_defaults.alpha,
        beta: rto_defaults.beta,
        max_virtual_time: DRIVER_MAX_VIRTUAL_TIME,
        max_retransmissions: DRIVER_MAX_RETRANSMISSIONS,
    });

    let record = &output.record;
    print_experiment_summary(&args.file, args.chunk_size, args.rto_multiplier, record);

    if let Some(path) = &args.json_out {
        prepare_output(path)?;
        record.save_json(path)?;
        println!("JSON: {}", path.display());
    }
    if let Some(path) = &args.csv_out {
        prepare_output(path)?;
        record.append_to_csv(path)?;
        println!("CSV: {}", path.display());
    }

    if record.app.transfer_status != "SUCCESS" {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "transfer finished with status {}",
                record.app.transfer_status
            ),
        )
        .into());
    }
    Ok(())
}

fn validate_experiment_args(args: &RunExperimentArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut problems = Vec::new();
    if args.window == 0 {
        problems.push("window must be greater than 0".to_string());
    }
    if args.chunk_size == 0 || args.chunk_size > MAX_PAYLOAD_SIZE {
        problems.push(format!(
            "chunk-size must be from 1 to {MAX_PAYLOAD_SIZE} bytes"
        ));
    }
    for (name, value) in [
        ("loss", args.loss),
        ("reorder", args.reorder),
        ("duplicate", args.duplicate),
        ("corrupt", args.corrupt),
    ] {
        if !(0.0..=1.0).contains(&value) {
            problems.push(format!("{name} must be between 0.0 and 1.0"));
        }
    }
    if args.min_rto_ms == 0 {
        problems.push("min-rto-ms must be greater than 0".to_string());
    }
    if args.initial_rto_ms == 0 {
        problems.push("initial-rto-ms must be greater than 0".to_string());
    }
    if args.max_rto_ms < args.min_rto_ms {
        problems.push("max-rto-ms must be greater than or equal to min-rto-ms".to_string());
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, problems.join("; ")).into())
    }
}

fn deterministic_experiment_id(args: &RunExperimentArgs) -> String {
    let file_name = args
        .file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    format!(
        "{protocol}_{file_name}_w{window}_c{chunk}_s{seed}_t{trial}_loss{loss}_reorder{reorder}_dup{dup}_corrupt{corrupt}_delay{delay}_jitter{jitter}_hold{hold}_m{mult}_min{min_rto}_max{max_rto}_init{initial}",
        protocol = args.protocol,
        file_name = file_name,
        window = args.window,
        chunk = args.chunk_size,
        seed = args.seed,
        trial = args.trial_id,
        loss = args.loss,
        reorder = args.reorder,
        dup = args.duplicate,
        corrupt = args.corrupt,
        delay = args.base_delay_ms,
        jitter = args.jitter_ms,
        hold = args.reorder_extra_ms,
        mult = args.rto_multiplier,
        min_rto = args.min_rto_ms,
        max_rto = args.max_rto_ms,
        initial = args.initial_rto_ms,
    )
}

fn print_experiment_summary(
    file: &Path,
    chunk_size: usize,
    rto_multiplier: f64,
    record: &ExperimentRecord,
) {
    let sha = if record.app.sha256_match {
        "MATCH"
    } else {
        "MISMATCH"
    };
    println!("Protocol: {}", record.meta.protocol);
    println!("File: {}", file.display());
    println!("Experiment: {}", record.meta.experiment_id);
    println!("Seed: {}", record.meta.seed);
    println!("Trial: {}", record.meta.trial_id);
    println!("Window: {}", record.meta.window_size);
    println!("Chunk size: {chunk_size}");
    println!("RTO multiplier: {rto_multiplier}");
    println!("Source bytes: {}", record.app.source_bytes);
    println!("Delivered bytes: {}", record.app.delivered_unique_bytes);
    println!("SHA-256: {sha}");
    println!("Duration: {:.3} ms", record.timing.duration_secs * 1000.0);
    println!("Data sent: {}", record.protocol.data_sent);
    println!("Retransmissions: {}", record.protocol.data_retransmissions);
    println!("Timeouts: {}", record.timing.timeout_count);
    println!("Mean RTT: {:.2} ms", record.timing.mean_rtt_ms);
    println!("Final RTO: {:.2} ms", record.timing.final_rto_ms);
    println!("Status: {}", record.app.transfer_status);
}

fn prepare_output(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}
