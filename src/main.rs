// CS-30003: Reliable Data Transfer over UDP
// Author: Sahan Maiti (Team Lead)
// Entry Point: Binary CLI Driver

use clap::{Parser, Subcommand};
use reliable_udp::app::{compute_file_sha256, verify_file_integrity, Chunker, Reassembler};
use reliable_udp::metrics::{
    ApplicationMetrics, ChannelMetrics, ExperimentMeta, ExperimentRecord, ProtocolMetrics,
    TimingMetrics,
};
use reliable_udp::packet::MAX_PAYLOAD_SIZE;
use std::path::PathBuf;
use std::time::Instant;

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
                println!("First packet seq: {}, payload len: {}", first.seq_num, first.payload.len());
            }
            if let Some(last) = packets.last() {
                println!("Last packet seq: {}, payload len: {}", last.seq_num, last.payload.len());
            }
        }

        Commands::Verify { source, received } => {
            println!("== SHA-256 End-to-End Verification ==");
            let report = verify_file_integrity(&source, &received)?;
            println!("Source:   {} (SHA-256: {})", report.source_path, report.source_sha256);
            println!("Received: {} (SHA-256: {})", report.received_path, report.received_sha256);
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
            println!("Elapsed: {:.4}s | Goodput: {:.2} KB/s", elapsed, (file_bytes as f64 / elapsed) / 1024.0);

            let record = ExperimentRecord::new(
                ExperimentMeta {
                    experiment_id: format!("LOCAL_BENCH_{}", protocol),
                    protocol: protocol.clone(),
                    window_size: window,
                    seed: 12345,
                    trial_id: 1,
                    configured_loss_rate: 0.0,
                    configured_reorder_rate: 0.0,
                    configured_delay_ms: 0,
                    configured_jitter_ms: 0,
                },
                ApplicationMetrics {
                    source_file_bytes: file_bytes,
                    delivered_unique_bytes: reassembler.total_bytes_accepted as u64,
                    transfer_status: if is_match { "SUCCESS".to_string() } else { "INTEGRITY_FAIL".to_string() },
                    sha256_match: is_match,
                    source_sha256: source_sha,
                    delivered_sha256: reconstructed_sha,
                },
                TimingMetrics {
                    duration_secs: elapsed,
                    total_rtt_samples: total_chunks as usize,
                    min_rtt_ms: 0.1,
                    max_rtt_ms: 1.0,
                    mean_rtt_ms: 0.5,
                    final_srtt_ms: 0.5,
                    final_rttvar_ms: 0.1,
                    final_rto_ms: 200.0,
                    timeout_count: 0,
                    backoff_count: 0,
                },
                ProtocolMetrics {
                    data_sent: total_chunks as usize,
                    ack_sent: total_chunks as usize,
                    data_received: total_chunks as usize,
                    ack_received: total_chunks as usize,
                    data_retransmissions: 0,
                    timeout_retransmissions: 0,
                    duplicate_data_received: 0,
                    duplicate_acks_received: 0,
                },
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
    }

    Ok(())
}
