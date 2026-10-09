// CS-30003: Reliable Data Transfer over UDP
// Author: Sahan Maiti (Evaluation & Integration)
// Component: Metrics & Evaluation - Raw Result Logging & Goodput Calculation

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Identity of one experimental run. These values are the run's configuration,
/// not measurements derived from a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentMeta {
    pub experiment_id: String,
    pub protocol: String,
    pub window_size: usize,
    pub chunk_size: usize,
    pub segment_count: usize,
    pub seed: u64,
    pub reverse_seed: u64,
    pub trial_id: usize,
}

/// Forward-channel settings actually installed on the data path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForwardChannelConfig {
    pub configured_loss_rate: f64,
    pub configured_reorder_rate: f64,
    pub configured_duplicate_rate: f64,
    pub configured_corrupt_rate: f64,
    pub base_delay_ms: u64,
    pub jitter_ms: u64,
    pub reorder_extra_ms: u64,
}

/// Reverse-channel settings actually installed on the ACK path.
///
/// This is recorded separately because the ACK path is not a copy of the
/// forward impairments. The primary reordering study leaves it clean.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReverseChannelConfig {
    pub configured_loss_rate: f64,
    pub configured_reorder_rate: f64,
    pub configured_duplicate_rate: f64,
    pub configured_corrupt_rate: f64,
    pub base_delay_ms: u64,
    pub jitter_ms: u64,
    pub reorder_extra_ms: u64,
}

/// Jacobson/Karels parameters copied from the `RtoConfig` the estimator used.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RtoParameters {
    pub rto_multiplier: f64,
    pub initial_rto_ms: f64,
    pub min_rto_ms: f64,
    pub max_rto_ms: f64,
    pub alpha: f64,
    pub beta: f64,
    pub k: f64,
}

/// End-to-end application transfer results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationMetrics {
    pub source_bytes: u64,
    pub delivered_unique_bytes: u64,
    /// `SUCCESS`, `INTEGRITY_FAIL`, or `TIMEOUT`. `INVALID_CONFIG` means the
    /// driver rejected the configuration before sending.
    pub transfer_status: String,
    /// True when `source_sha256` and `delivered_sha256` are identical.
    /// `ExperimentRecord::new` overwrites this from the two hashes.
    pub sha256_match: bool,
    pub source_sha256: String,
    pub delivered_sha256: String,
}

/// Timing figures taken from the virtual clock and the RTO estimator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingMetrics {
    /// Virtual time from the origin until the receiver had the whole file,
    /// or until the driver gave up.
    pub duration_secs: f64,
    /// RTT samples `RtoEstimator` accepted. Karn rejections are not included.
    pub total_rtt_samples: usize,
    /// RTT samples rejected because the segment had been retransmitted.
    pub karn_rejected_count: usize,
    pub min_rtt_ms: f64,
    pub max_rtt_ms: f64,
    pub mean_rtt_ms: f64,
    pub final_srtt_ms: f64,
    pub final_rttvar_ms: f64,
    pub final_rto_ms: f64,
    /// `RtoEstimator` timeout events. One Go-Back-N expiry counts once, even
    /// when it retransmits several packets. `on_timeout` also increments
    /// `backoff_count`, so the two match for this estimator.
    pub timeout_count: usize,
    /// Exponential-backoff applications recorded by `RtoEstimator::on_timeout`.
    pub backoff_count: usize,
}

/// Protocol counters from the transfer that just ran.
///
/// `data_sent` is the ARQ sender's `total_sent`: original DATA transmissions
/// plus retransmissions. `data_retransmissions` is that sender's retransmission
/// counter. `timeout_retransmissions` counts DATA packets this driver actually
/// resubmitted after a timer expiry. There is no fast-retransmit path, so the
/// two retransmission counts match unless the driver ceiling stops a Go-Back-N
/// burst after the sender has already counted it.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProtocolMetrics {
    pub data_sent: usize,
    pub ack_sent: usize,
    pub data_received: usize,
    pub ack_received: usize,
    pub data_retransmissions: usize,
    pub timeout_retransmissions: usize,
    /// Timer firings whose original forward datagram was not dropped and whose
    /// ACK was already on the reverse scheduler. Not inferred from `data_retransmissions`.
    pub premature_retransmissions: usize,
    pub duplicate_data_received: usize,
    pub duplicate_acks_received: usize,
}

/// Fault counters observed by one channel instance.
///
/// `offered_datagrams` is `ChannelStats::packets_seen`.
/// `dropped_datagrams` is `lost`.
/// `scheduled_deliveries` is `copies_delivered`.
/// None of these are computed from the configured probabilities.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ChannelMetrics {
    pub offered_datagrams: usize,
    pub dropped_datagrams: usize,
    pub duplicated_datagrams: usize,
    pub corrupted_datagrams: usize,
    pub reordered_datagrams: usize,
    pub scheduled_deliveries: usize,
}

/// One trial. Nested JSON objects keep configuration separate from counters.
/// The CSV row is the same information in the column order of `csv_header`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRecord {
    pub meta: ExperimentMeta,
    pub forward_config: ForwardChannelConfig,
    pub reverse_config: ReverseChannelConfig,
    pub rto_config: RtoParameters,
    pub app: ApplicationMetrics,
    pub timing: TimingMetrics,
    pub protocol: ProtocolMetrics,
    /// Observed forward-channel counters.
    pub channel: ChannelMetrics,
    /// Observed reverse-channel counters.
    pub reverse_channel: ChannelMetrics,
    pub goodput_bytes_sec: f64,
    pub retransmission_ratio: f64,
}

impl ExperimentRecord {
    /// Fills derived fields from the values the caller measured.
    ///
    /// `sha256_match` is set from the two digests. Goodput is positive only
    /// when the status is `SUCCESS`, the digests match, and virtual duration
    /// is positive. The retransmission ratio is `data_retransmissions / data_sent`.
    pub fn new(
        meta: ExperimentMeta,
        forward_config: ForwardChannelConfig,
        reverse_config: ReverseChannelConfig,
        rto_config: RtoParameters,
        mut app: ApplicationMetrics,
        timing: TimingMetrics,
        protocol: ProtocolMetrics,
        channel: ChannelMetrics,
        reverse_channel: ChannelMetrics,
    ) -> Self {
        app.sha256_match = app.source_sha256 == app.delivered_sha256;

        let goodput_bytes_sec =
            if timing.duration_secs > 0.0 && app.transfer_status == "SUCCESS" && app.sha256_match {
                app.delivered_unique_bytes as f64 / timing.duration_secs
            } else {
                0.0
            };

        let retransmission_ratio = if protocol.data_sent > 0 {
            protocol.data_retransmissions as f64 / protocol.data_sent as f64
        } else {
            0.0
        };

        ExperimentRecord {
            meta,
            forward_config,
            reverse_config,
            rto_config,
            app,
            timing,
            protocol,
            channel,
            reverse_channel,
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
        let json_str = self
            .to_json()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut file = File::create(path)?;
        file.write_all(json_str.as_bytes())?;
        Ok(())
    }

    /// Flat column order consumed by the analysis scripts.
    pub fn csv_header() -> &'static str {
        "\
experiment_id,\
protocol,\
window_size,\
chunk_size,\
segment_count,\
seed,\
reverse_seed,\
trial_id,\
configured_loss_rate,\
configured_reorder_rate,\
configured_duplicate_rate,\
configured_corrupt_rate,\
base_delay_ms,\
jitter_ms,\
reorder_extra_ms,\
reverse_loss_rate,\
reverse_reorder_rate,\
reverse_duplicate_rate,\
reverse_corrupt_rate,\
reverse_base_delay_ms,\
reverse_jitter_ms,\
reverse_reorder_extra_ms,\
rto_multiplier,\
initial_rto_ms,\
min_rto_ms,\
max_rto_ms,\
alpha,\
beta,\
k,\
source_bytes,\
delivered_unique_bytes,\
transfer_status,\
sha256_match,\
source_sha256,\
delivered_sha256,\
duration_secs,\
total_rtt_samples,\
karn_rejected_count,\
min_rtt_ms,\
max_rtt_ms,\
mean_rtt_ms,\
final_srtt_ms,\
final_rttvar_ms,\
final_rto_ms,\
timeout_count,\
backoff_count,\
data_sent,\
ack_sent,\
data_received,\
ack_received,\
data_retransmissions,\
timeout_retransmissions,\
premature_retransmissions,\
duplicate_data_received,\
duplicate_acks_received,\
offered_datagrams,\
dropped_datagrams,\
duplicated_datagrams,\
corrupted_datagrams,\
reordered_datagrams,\
scheduled_deliveries,\
reverse_offered_datagrams,\
reverse_dropped_datagrams,\
reverse_duplicated_datagrams,\
reverse_corrupted_datagrams,\
reverse_reordered_datagrams,\
reverse_scheduled_deliveries,\
goodput_bytes_sec,\
retransmission_ratio"
    }

    /// One CSV row in the same order as `csv_header`.
    pub fn to_csv_row(&self) -> String {
        format!(
            "\
{experiment_id},\
{protocol},\
{window_size},\
{chunk_size},\
{segment_count},\
{seed},\
{reverse_seed},\
{trial_id},\
{loss:.6},\
{reorder:.6},\
{duplicate:.6},\
{corrupt:.6},\
{base_delay},\
{jitter},\
{hold},\
{rev_loss:.6},\
{rev_reorder:.6},\
{rev_dup:.6},\
{rev_corrupt:.6},\
{rev_delay},\
{rev_jitter},\
{rev_hold},\
{multiplier:.6},\
{initial_rto:.4},\
{min_rto:.4},\
{max_rto:.4},\
{alpha:.6},\
{beta:.6},\
{k:.6},\
{source_bytes},\
{delivered_bytes},\
{status},\
{sha},\
{source_sha},\
{delivered_sha},\
{duration:.6},\
{rtt_samples},\
{karn},\
{min_rtt:.4},\
{max_rtt:.4},\
{mean_rtt:.4},\
{srtt:.4},\
{rttvar:.4},\
{final_rto:.4},\
{timeouts},\
{backoffs},\
{data_sent},\
{ack_sent},\
{data_received},\
{ack_received},\
{data_retx},\
{timeout_retx},\
{premature},\
{dup_data},\
{dup_acks},\
{offered},\
{dropped},\
{duplicated},\
{corrupted},\
{reordered},\
{scheduled},\
{rev_offered},\
{rev_dropped},\
{rev_duplicated},\
{rev_corrupted},\
{rev_reordered},\
{rev_scheduled},\
{goodput:.2},\
{ratio:.6}",
            experiment_id = self.meta.experiment_id,
            protocol = self.meta.protocol,
            window_size = self.meta.window_size,
            chunk_size = self.meta.chunk_size,
            segment_count = self.meta.segment_count,
            seed = self.meta.seed,
            reverse_seed = self.meta.reverse_seed,
            trial_id = self.meta.trial_id,
            loss = self.forward_config.configured_loss_rate,
            reorder = self.forward_config.configured_reorder_rate,
            duplicate = self.forward_config.configured_duplicate_rate,
            corrupt = self.forward_config.configured_corrupt_rate,
            base_delay = self.forward_config.base_delay_ms,
            jitter = self.forward_config.jitter_ms,
            hold = self.forward_config.reorder_extra_ms,
            rev_loss = self.reverse_config.configured_loss_rate,
            rev_reorder = self.reverse_config.configured_reorder_rate,
            rev_dup = self.reverse_config.configured_duplicate_rate,
            rev_corrupt = self.reverse_config.configured_corrupt_rate,
            rev_delay = self.reverse_config.base_delay_ms,
            rev_jitter = self.reverse_config.jitter_ms,
            rev_hold = self.reverse_config.reorder_extra_ms,
            multiplier = self.rto_config.rto_multiplier,
            initial_rto = self.rto_config.initial_rto_ms,
            min_rto = self.rto_config.min_rto_ms,
            max_rto = self.rto_config.max_rto_ms,
            alpha = self.rto_config.alpha,
            beta = self.rto_config.beta,
            k = self.rto_config.k,
            source_bytes = self.app.source_bytes,
            delivered_bytes = self.app.delivered_unique_bytes,
            status = self.app.transfer_status,
            sha = self.app.sha256_match,
            source_sha = self.app.source_sha256,
            delivered_sha = self.app.delivered_sha256,
            duration = self.timing.duration_secs,
            rtt_samples = self.timing.total_rtt_samples,
            karn = self.timing.karn_rejected_count,
            min_rtt = self.timing.min_rtt_ms,
            max_rtt = self.timing.max_rtt_ms,
            mean_rtt = self.timing.mean_rtt_ms,
            srtt = self.timing.final_srtt_ms,
            rttvar = self.timing.final_rttvar_ms,
            final_rto = self.timing.final_rto_ms,
            timeouts = self.timing.timeout_count,
            backoffs = self.timing.backoff_count,
            data_sent = self.protocol.data_sent,
            ack_sent = self.protocol.ack_sent,
            data_received = self.protocol.data_received,
            ack_received = self.protocol.ack_received,
            data_retx = self.protocol.data_retransmissions,
            timeout_retx = self.protocol.timeout_retransmissions,
            premature = self.protocol.premature_retransmissions,
            dup_data = self.protocol.duplicate_data_received,
            dup_acks = self.protocol.duplicate_acks_received,
            offered = self.channel.offered_datagrams,
            dropped = self.channel.dropped_datagrams,
            duplicated = self.channel.duplicated_datagrams,
            corrupted = self.channel.corrupted_datagrams,
            reordered = self.channel.reordered_datagrams,
            scheduled = self.channel.scheduled_deliveries,
            rev_offered = self.reverse_channel.offered_datagrams,
            rev_dropped = self.reverse_channel.dropped_datagrams,
            rev_duplicated = self.reverse_channel.duplicated_datagrams,
            rev_corrupted = self.reverse_channel.corrupted_datagrams,
            rev_reordered = self.reverse_channel.reordered_datagrams,
            rev_scheduled = self.reverse_channel.scheduled_deliveries,
            goodput = self.goodput_bytes_sec,
            ratio = self.retransmission_ratio,
        )
    }

    /// Appends this trial record to a shared CSV file, writing the header when the file is new.
    pub fn append_to_csv<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file_exists = path.as_ref().exists();
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;

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

    fn sample_record(status: &str, hashes_match: bool) -> ExperimentRecord {
        let digest = "abc123hash".to_string();
        ExperimentRecord::new(
            ExperimentMeta {
                experiment_id: "EXP_REORDER_01".to_string(),
                protocol: "SelectiveRepeat".to_string(),
                window_size: 8,
                chunk_size: 1400,
                segment_count: 10,
                seed: 42,
                reverse_seed: 43,
                trial_id: 1,
            },
            ForwardChannelConfig {
                configured_loss_rate: 0.05,
                configured_reorder_rate: 0.10,
                configured_duplicate_rate: 0.0,
                configured_corrupt_rate: 0.0,
                base_delay_ms: 20,
                jitter_ms: 5,
                reorder_extra_ms: 50,
            },
            ReverseChannelConfig {
                configured_loss_rate: 0.0,
                configured_reorder_rate: 0.0,
                configured_duplicate_rate: 0.0,
                configured_corrupt_rate: 0.0,
                base_delay_ms: 20,
                jitter_ms: 5,
                reorder_extra_ms: 50,
            },
            RtoParameters {
                rto_multiplier: 1.0,
                initial_rto_ms: 1000.0,
                min_rto_ms: 200.0,
                max_rto_ms: 60000.0,
                alpha: 0.125,
                beta: 0.25,
                k: 4.0,
            },
            ApplicationMetrics {
                source_bytes: 14000,
                delivered_unique_bytes: if status == "SUCCESS" { 14000 } else { 0 },
                transfer_status: status.to_string(),
                sha256_match: !hashes_match,
                source_sha256: digest.clone(),
                delivered_sha256: if hashes_match {
                    digest
                } else {
                    "other".to_string()
                },
            },
            TimingMetrics {
                duration_secs: 2.0,
                total_rtt_samples: 10,
                karn_rejected_count: 1,
                min_rtt_ms: 20.0,
                max_rtt_ms: 35.0,
                mean_rtt_ms: 25.0,
                final_srtt_ms: 24.5,
                final_rttvar_ms: 3.2,
                final_rto_ms: 40.0,
                timeout_count: 1,
                backoff_count: 1,
            },
            ProtocolMetrics {
                data_sent: 12,
                ack_sent: 11,
                data_received: 12,
                ack_received: 11,
                data_retransmissions: 2,
                timeout_retransmissions: 2,
                premature_retransmissions: 1,
                duplicate_data_received: 1,
                duplicate_acks_received: 0,
            },
            ChannelMetrics {
                offered_datagrams: 23,
                dropped_datagrams: 1,
                duplicated_datagrams: 1,
                corrupted_datagrams: 0,
                reordered_datagrams: 2,
                scheduled_deliveries: 23,
            },
            ChannelMetrics::default(),
        )
    }

    #[test]
    fn test_goodput_ratio_and_csv_columns() {
        let record = sample_record("SUCCESS", true);
        assert!(record.app.sha256_match);
        assert_eq!(record.goodput_bytes_sec, 7000.0);
        assert!((record.retransmission_ratio - (2.0 / 12.0)).abs() < 1e-9);

        let header_cols = ExperimentRecord::csv_header().split(',').count();
        let row_cols = record.to_csv_row().split(',').count();
        assert_eq!(header_cols, row_cols);
        assert!(record
            .to_csv_row()
            .contains("EXP_REORDER_01,SelectiveRepeat,8,1400,10,42,43,1"));
        assert!(record.to_csv_row().contains("7000.00"));
        assert!(record.to_csv_row().contains("0.125000"));
    }

    #[test]
    fn failed_transfer_does_not_receive_goodput() {
        let record = sample_record("TIMEOUT", false);
        assert!(!record.app.sha256_match);
        assert_eq!(record.goodput_bytes_sec, 0.0);
    }
}
