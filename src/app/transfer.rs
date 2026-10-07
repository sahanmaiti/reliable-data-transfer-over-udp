// CS-30003: Reliable Data Transfer over UDP
// Author: Sahan Maiti (Team Lead)
// Component: UDP integration loop (wires ARQ + channel + timing when merged)

use crate::metrics::ExperimentRecord;
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Configuration for one reliable file transfer trial.
#[derive(Debug, Clone)]
pub struct TransferConfig {
    pub source_file: PathBuf,
    pub output_file: PathBuf,
    pub protocol: String,
    pub window_size: usize,
    pub seed: u64,
    pub trial_id: usize,
    pub loss_rate: f64,
    pub reorder_rate: f64,
    pub corrupt_rate: f64,
    pub delay_ms: u64,
    pub jitter_ms: u64,
    pub rto_multiplier: f64,
    pub sender_addr: SocketAddr,
    pub receiver_addr: SocketAddr,
    pub experiment_id: String,
}

/// Subsystems required before the UDP loop can run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingIntegration {
    ArqModule,
    TimingModule,
    ChannelModule,
}

impl fmt::Display for MissingIntegration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MissingIntegration::ArqModule => write!(f, "src/arq (Stop-and-Wait / traits from Soumyadeb)"),
            MissingIntegration::TimingModule => write!(f, "src/timing (RTO + timer from Kashish)"),
            MissingIntegration::ChannelModule => write!(f, "src/channel (emulator from Ashwika)"),
        }
    }
}

#[derive(Debug)]
pub enum TransferError {
    NotReady {
        missing: Vec<MissingIntegration>,
    },
}

impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransferError::NotReady { missing } => {
                write!(
                    f,
                    "UDP transfer integration not ready; merge feature branches: {}",
                    missing
                        .iter()
                        .map(|m| m.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
    }
}

impl std::error::Error for TransferError {}

/// Returns which integration pieces are still absent in this crate graph.
pub fn missing_integration_pieces() -> Vec<MissingIntegration> {
    let mut missing = Vec::new();
    // Detect merged modules via path_exists at compile time we use cfg flags later;
    // until submodules exist in lib.rs, all three are required.
    missing.push(MissingIntegration::ArqModule);
    missing.push(MissingIntegration::TimingModule);
    missing.push(MissingIntegration::ChannelModule);
    missing
}

/// Runs one end-to-end transfer and produces metrics.
///
/// Implementation: open UDP sockets, drive `ArqSender`/`ArqReceiver`, pass datagrams
/// through the deterministic channel emulator, reassemble, verify SHA-256, emit record.
pub fn run_udp_transfer(_config: &TransferConfig) -> Result<ExperimentRecord, TransferError> {
    let missing = missing_integration_pieces();
    if !missing.is_empty() {
        return Err(TransferError::NotReady { missing });
    }
    unreachable!("missing_integration_pieces() should be empty when modules are linked")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr};

    #[test]
    fn integration_reports_missing_modules() {
        let missing = missing_integration_pieces();
        assert_eq!(missing.len(), 3);
    }

    #[test]
    fn run_udp_transfer_fails_until_modules_merged() {
        let cfg = TransferConfig {
            source_file: PathBuf::from("fixtures/dummy.bin"),
            output_file: PathBuf::from("/tmp/out.bin"),
            protocol: "StopAndWait".to_string(),
            window_size: 1,
            seed: 1,
            trial_id: 1,
            loss_rate: 0.0,
            reorder_rate: 0.0,
            corrupt_rate: 0.0,
            delay_ms: 0,
            jitter_ms: 0,
            rto_multiplier: 1.0,
            sender_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, 9000)),
            receiver_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, 9001)),
            experiment_id: "TEST".to_string(),
        };
        assert!(matches!(
            run_udp_transfer(&cfg),
            Err(TransferError::NotReady { .. })
        ));
    }
}
