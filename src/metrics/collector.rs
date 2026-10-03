// CS-30003: Reliable Data Transfer over UDP
// Author: Sahan Maiti (Team Lead)
// Component: Metrics & Evaluation - Raw Result Logging & Goodput Calculation

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Metadata identifying a specific experimental run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentMeta {
    pub experiment_id: String,
    pub protocol: String,
    pub window_size: usize,
    pub seed: u64,
    pub trial_id: usize,
    pub configured_loss_rate: f64,
    pub configured_reorder_rate: f64,
    pub configured_delay_ms: u64,
    pub configured_jitter_ms: u64,
}

/// End-to-end application transfer results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationMetrics {
    pub source_file_bytes: u64,
    pub delivered_unique_bytes: u64,
    pub transfer_status: String, // e.g. "SUCCESS", "INTEGRITY_FAIL", "TIMEOUT"
    pub sha256_match: bool,
    pub source_sha256: String,
    pub delivered_sha256: String,
}

/// Detailed round-trip time and timeout tracking metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingMetrics {
    pub duration_secs: f64,
    pub total_rtt_samples: usize,
    pub min_rtt_ms: f64,
    pub max_rtt_ms: f64,
    pub mean_rtt_ms: f64,
    pub final_srtt_ms: f64,
    pub final_rttvar_ms: f64,
    pub final_rto_ms: f64,
    pub timeout_count: usize,
    pub backoff_count: usize,
}

/// Protocol-level message and retransmission counters.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProtocolMetrics {
    pub data_sent: usize,
    pub ack_sent: usize,
    pub data_received: usize,
    pub ack_received: usize,
    pub data_retransmissions: usize,
    pub timeout_retransmissions: usize,
    pub duplicate_data_received: usize,
    pub duplicate_acks_received: usize,
}

/// Fault and impairment counters observed by the channel emulator.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelMetrics {
    pub offered_datagrams: usize,
    pub dropped_datagrams: usize,
    pub duplicated_datagrams: usize,
    pub corrupted_datagrams: usize,
    pub reordered_datagrams: usize,
    pub scheduled_deliveries: usize,
}

/// Master experimental trial record combining all measurement subsystems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRecord {
    pub meta: ExperimentMeta,
    pub app: ApplicationMetrics,
    pub timing: TimingMetrics,
    pub protocol: ProtocolMetrics,
    pub channel: ChannelMetrics,
    pub goodput_bytes_sec: f64,
    pub retransmission_ratio: f64,
}

impl ExperimentRecord {
    /// Computes and instantiates a complete `ExperimentRecord`.
    pub fn new(
        meta: ExperimentMeta,
        app: ApplicationMetrics,
        timing: TimingMetrics,
        protocol: ProtocolMetrics,
        channel: ChannelMetrics,
    ) -> Self {
        // Goodput = unique application bytes delivered / completion time (README line 733)
        let goodput_bytes_sec = if timing.duration_secs > 0.0 && app.sha256_match {
            app.delivered_unique_bytes as f64 / timing.duration_secs
        } else {
            0.0
        };

        // Retransmission Ratio = retransmissions / initial data sends
        let retransmission_ratio = if protocol.data_sent > 0 {
            protocol.data_retransmissions as f64 / protocol.data_sent as f64
        } else {
            0.0
        };

        ExperimentRecord {
            meta,
            app,
            timing,
            protocol,
            channel,
            goodput_bytes_sec,
            retransmission_ratio,
        }
    }

    /// Serializes this trial record to a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Saves the trial record as a JSON file.
    pub fn save_json<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let json_str = self.to_json().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut file = File::create(path)?;
        file.write_all(json_str.as_bytes())?;
        Ok(())
    }

    /// Standard CSV column header matching our experimental analysis schema.
    pub fn csv_header() -> &'static str {
        "experiment_id,protocol,window_size,seed,trial_id,configured_loss_rate,configured_reorder_rate,duration_secs,source_bytes,delivered_bytes,sha256_match,data_sent,data_retransmissions,retransmission_ratio,goodput_bytes_sec,timeout_count,mean_rtt_ms,final_rto_ms,dropped_datagrams,corrupted_datagrams,reordered_datagrams"
    }

    /// Exports the trial record as a single CSV row.
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{:.4},{:.4},{:.4},{},{},{},{},{},{:.4},{:.2},{},{:.2},{:.2},{},{},{}",
            self.meta.experiment_id,
            self.meta.protocol,
            self.meta.window_size,
            self.meta.seed,
            self.meta.trial_id,
            self.meta.configured_loss_rate,
            self.meta.configured_reorder_rate,
            self.timing.duration_secs,
            self.app.source_file_bytes,
            self.app.delivered_unique_bytes,
            self.app.sha256_match,
            self.protocol.data_sent,
            self.protocol.data_retransmissions,
            self.retransmission_ratio,
            self.goodput_bytes_sec,
            self.timing.timeout_count,
            self.timing.mean_rtt_ms,
            self.timing.final_rto_ms,
            self.channel.dropped_datagrams,
            self.channel.corrupted_datagrams,
            self.channel.reordered_datagrams,
        )
    }

    /// Appends this trial record to a shared CSV file, automatically creating and writing the header if newly created.
    pub fn append_to_csv<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file_exists = path.as_ref().exists();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        if !file_exists {
            writeln!(file, "{}", Self::csv_header())?;
        }
        writeln!(file, "{}", self.to_csv_row())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goodput_and_csv_formatting() {
        let meta = ExperimentMeta {
            experiment_id: "EXP_REORDER_01".to_string(),
            protocol: "SelectiveRepeat".to_string(),
            window_size: 8,
            seed: 42,
            trial_id: 1,
            configured_loss_rate: 0.05,
            configured_reorder_rate: 0.10,
            configured_delay_ms: 20,
            configured_jitter_ms: 5,
        };

        let app = ApplicationMetrics {
            source_file_bytes: 14000,
            delivered_unique_bytes: 14000,
            transfer_status: "SUCCESS".to_string(),
            sha256_match: true,
            source_sha256: "abc123hash".to_string(),
            delivered_sha256: "abc123hash".to_string(),
        };

        let timing = TimingMetrics {
            duration_secs: 2.0,
            total_rtt_samples: 10,
            min_rtt_ms: 20.0,
            max_rtt_ms: 35.0,
            mean_rtt_ms: 25.0,
            final_srtt_ms: 24.5,
            final_rttvar_ms: 3.2,
            final_rto_ms: 40.0,
            timeout_count: 1,
            backoff_count: 0,
        };

        let protocol = ProtocolMetrics {
            data_sent: 12,
            ack_sent: 11,
            data_received: 12,
            ack_received: 11,
            data_retransmissions: 2,
            timeout_retransmissions: 1,
            duplicate_data_received: 1,
            duplicate_acks_received: 0,
        };

        let channel = ChannelMetrics {
            offered_datagrams: 23,
            dropped_datagrams: 1,
            duplicated_datagrams: 1,
            corrupted_datagrams: 0,
            reordered_datagrams: 2,
            scheduled_deliveries: 23,
        };

        let record = ExperimentRecord::new(meta, app, timing, protocol, channel);

        // Goodput = 14000 / 2.0 = 7000.0 B/s
        assert_eq!(record.goodput_bytes_sec, 7000.0);
        // Retransmission ratio = 2 / 12 = 0.1666...
        assert!((record.retransmission_ratio - (2.0 / 12.0)).abs() < 1e-4);

        let csv_row = record.to_csv_row();
        assert!(csv_row.contains("EXP_REORDER_01,SelectiveRepeat,8,42,1"));
        assert!(csv_row.contains("7000.00"));
    }
}
