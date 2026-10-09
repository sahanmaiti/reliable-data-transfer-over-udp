// CS-30003: Reliable Data Transfer over UDP
// Author: Kashish Gupta
// Component: Retransmission Timing & RTO - Jacobson/Karels RTO Estimator (RFC 6298)

use std::time::Duration;

/// Configuration parameters for the Jacobson/Karels RTO estimator (RFC 6298).
#[derive(Debug, Clone)]
pub struct RtoConfig {
    /// Initial RTO before any RTT measurement is taken (RFC 6298 recommends 1.0s).
    pub initial_rto: Duration,
    /// Minimum allowed RTO bound to avoid spurious retransmissions on low RTT links.
    pub min_rto: Duration,
    /// Maximum allowed RTO bound to prevent infinite hang on persistent loss.
    pub max_rto: Duration,
    /// Clock granularity G (typically 1ms).
    pub granularity: Duration,
    /// Smoothing factor alpha for SRTT (default: 0.125 = 1/8).
    pub alpha: f64,
    /// Variance weighting factor beta for RTTVAR (default: 0.25 = 1/4).
    pub beta: f64,
    /// Multiplier K for RTTVAR (default: 4.0).
    pub k: f64,
    /// Scaling multiplier for RTO sensitivity experiments (E4 in proposal; default 1.0).
    pub rto_multiplier: f64,
}

impl Default for RtoConfig {
    fn default() -> Self {
        Self {
            initial_rto: Duration::from_millis(1000), // 1.0 second
            min_rto: Duration::from_millis(200),      // 200 ms floor
            max_rto: Duration::from_millis(60000),    // 60 seconds ceiling
            granularity: Duration::from_millis(1),    // 1 ms
            alpha: 0.125,                             // 1/8
            beta: 0.25,                               // 1/4
            k: 4.0,                                   // 4
            rto_multiplier: 1.0,                      // 1.0x baseline
        }
    }
}

/// Statistics and counters tracked by the RTO estimator.
#[derive(Debug, Clone, Default)]
pub struct RtoStats {
    /// Total number of valid RTT samples processed.
    pub sample_count: u64,
    /// Total number of RTT samples rejected by Karn's algorithm (due to retransmissions).
    pub karn_rejected_count: u64,
    /// Total number of retransmission timeout events triggered.
    pub timeout_count: u64,
    /// Total number of consecutive exponential backoffs applied.
    pub backoff_count: u64,
    /// Minimum observed RTT sample.
    pub min_rtt: Option<Duration>,
    /// Maximum observed RTT sample.
    pub max_rtt: Option<Duration>,
    /// Sum of all valid RTT samples (for computing arithmetic mean).
    pub sum_rtt_millis: f64,
}

/// Dynamic RTO estimator implementing the Jacobson/Karels algorithm (RFC 6298)
/// with Karn's Algorithm and Exponential Backoff.
#[derive(Debug, Clone)]
pub struct RtoEstimator {
    config: RtoConfig,
    /// Smoothed Round-Trip Time (SRTT)
    srtt: Option<Duration>,
    /// Round-Trip Time Variation (RTTVAR)
    rttvar: Option<Duration>,
    /// Current calculated Retransmission Timeout (RTO)
    current_rto: Duration,
    /// Base RTO before timeout backoff was applied
    base_rto: Duration,
    /// Tracks if we are currently in exponential backoff mode
    in_backoff: bool,
    /// Telemetry and statistics
    pub stats: RtoStats,
}

impl RtoEstimator {
    /// Creates a new RTO estimator with default RFC 6298 configuration.
    pub fn new() -> Self {
        Self::with_config(RtoConfig::default())
    }

    /// Creates a new RTO estimator with custom parameters.
    pub fn with_config(config: RtoConfig) -> Self {
        let initial = config.initial_rto;
        Self {
            config,
            srtt: None,
            rttvar: None,
            current_rto: initial,
            base_rto: initial,
            in_backoff: false,
            stats: RtoStats::default(),
        }
    }

    /// Returns the current effective Retransmission Timeout (RTO).
    pub fn current_rto(&self) -> Duration {
        self.current_rto
    }

    /// Configuration this estimator was built with.
    ///
    /// Result records read alpha, beta, k, the multiplier, and the RTO bounds
    /// from here so the metrics layer does not keep a second copy of the constants.
    pub fn config(&self) -> &RtoConfig {
        &self.config
    }

    /// Returns the current Smoothed RTT (SRTT), if at least one sample was measured.
    pub fn srtt(&self) -> Option<Duration> {
        self.srtt
    }

    /// Returns the current RTT Variation (RTTVAR), if at least one sample was measured.
    pub fn rttvar(&self) -> Option<Duration> {
        self.rttvar
    }

    /// Updates the RTO estimate using a new RTT measurement sample.
    ///
    /// Implements Karn's Algorithm:
    /// - If `is_retransmission` is true, the sample is discarded because of ACK ambiguity.
    /// - Returns `true` if the sample was accepted and used, `false` if rejected by Karn's algorithm.
    pub fn update_rtt(&mut self, sample: Duration, is_retransmission: bool) -> bool {
        // Karn's Algorithm: Do not update RTT estimates for packets that were retransmitted!
        if is_retransmission {
            self.stats.karn_rejected_count += 1;
            return false;
        }

        let sample_ms = sample.as_secs_f64() * 1000.0;
        self.stats.sample_count += 1;
        self.stats.sum_rtt_millis += sample_ms;

        // Track min/max observed RTT
        self.stats.min_rtt = Some(match self.stats.min_rtt {
            Some(prev) => prev.min(sample),
            None => sample,
        });
        self.stats.max_rtt = Some(match self.stats.max_rtt {
            Some(prev) => prev.max(sample),
            None => sample,
        });

        // Any successful non-retransmitted ACK clears backoff state
        self.in_backoff = false;

        let g_ms = self.config.granularity.as_secs_f64() * 1000.0;
        let alpha = self.config.alpha;
        let beta = self.config.beta;
        let k = self.config.k;

        match (self.srtt, self.rttvar) {
            (None, _) => {
                // RFC 6298 Rule 2.1: First RTT measurement
                // SRTT <- R
                // RTTVAR <- R / 2
                // RTO <- SRTT + max(G, K * RTTVAR)
                let srtt_ms = sample_ms;
                let rttvar_ms = sample_ms / 2.0;
                let rto_ms = srtt_ms + (k * rttvar_ms).max(g_ms);

                self.srtt = Some(Duration::from_secs_f64(srtt_ms / 1000.0));
                self.rttvar = Some(Duration::from_secs_f64(rttvar_ms / 1000.0));
                self.set_computed_rto(rto_ms);
            }
            (Some(prev_srtt), Some(prev_rttvar)) => {
                // RFC 6298 Rule 2.2: Subsequent measurements
                // RTTVAR <- (1 - beta) * RTTVAR + beta * |SRTT - R|
                // SRTT <- (1 - alpha) * SRTT + alpha * R
                // RTO <- SRTT + max(G, K * RTTVAR)
                let prev_srtt_ms = prev_srtt.as_secs_f64() * 1000.0;
                let prev_rttvar_ms = prev_rttvar.as_secs_f64() * 1000.0;

                let rtt_diff_ms = (prev_srtt_ms - sample_ms).abs();
                let new_rttvar_ms = (1.0 - beta) * prev_rttvar_ms + beta * rtt_diff_ms;
                let new_srtt_ms = (1.0 - alpha) * prev_srtt_ms + alpha * sample_ms;
                let rto_ms = new_srtt_ms + (k * new_rttvar_ms).max(g_ms);

                self.srtt = Some(Duration::from_secs_f64(new_srtt_ms / 1000.0));
                self.rttvar = Some(Duration::from_secs_f64(new_rttvar_ms / 1000.0));
                self.set_computed_rto(rto_ms);
            }
            (Some(_), None) => unreachable!(),
        }

        true
    }

    /// Handles a Retransmission Timeout (RTO) event by applying exponential timer backoff.
    ///
    /// RFC 6298 Rule 5.5: RTO <- RTO * 2 (backed off exponentially up to max_rto).
    pub fn on_timeout(&mut self) -> Duration {
        self.stats.timeout_count += 1;
        self.stats.backoff_count += 1;
        self.in_backoff = true;

        // Exponential backoff: double the current RTO
        let backed_off = self.current_rto.saturating_mul(2);
        self.current_rto = backed_off.clamp(self.config.min_rto, self.config.max_rto);
        self.current_rto
    }

    /// Sets the RTO applying bounds clamping and the experimental sensitivity multiplier.
    fn set_computed_rto(&mut self, raw_rto_ms: f64) {
        // Apply experimental multiplier (E4 RTO study)
        let scaled_ms = raw_rto_ms * self.config.rto_multiplier;
        let duration = Duration::from_secs_f64(scaled_ms / 1000.0);

        // RFC 6298 Rule 2.4: Clamp within [min_rto, max_rto]
        let clamped = duration.clamp(self.config.min_rto, self.config.max_rto);
        self.base_rto = clamped;
        self.current_rto = clamped;
    }

    /// Computes the arithmetic mean of all accepted RTT samples in milliseconds.
    pub fn mean_rtt_ms(&self) -> f64 {
        if self.stats.sample_count == 0 {
            0.0
        } else {
            self.stats.sum_rtt_millis / self.stats.sample_count as f64
        }
    }

    /// Returns a JSON-formatted summary of timing statistics.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"srtt_ms\":{:.2},\"rttvar_ms\":{:.2},\"current_rto_ms\":{:.2},\"mean_rtt_ms\":{:.2},\"sample_count\":{},\"karn_rejected\":{},\"timeout_count\":{},\"backoff_count\":{}}}",
            self.srtt.map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0),
            self.rttvar.map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0),
            self.current_rto.as_secs_f64() * 1000.0,
            self.mean_rtt_ms(),
            self.stats.sample_count,
            self.stats.karn_rejected_count,
            self.stats.timeout_count,
            self.stats.backoff_count,
        )
    }

    /// Returns a CSV-formatted data row of timing statistics.
    pub fn to_csv_row(&self) -> String {
        format!(
            "{:.2},{:.2},{:.2},{:.2},{},{},{},{}",
            self.srtt.map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0),
            self.rttvar.map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0),
            self.current_rto.as_secs_f64() * 1000.0,
            self.mean_rtt_ms(),
            self.stats.sample_count,
            self.stats.karn_rejected_count,
            self.stats.timeout_count,
            self.stats.backoff_count,
        )
    }

    /// CSV column headers matching `to_csv_row()`.
    pub fn csv_header() -> &'static str {
        "srtt_ms,rttvar_ms,current_rto_ms,mean_rtt_ms,sample_count,karn_rejected_count,timeout_count,backoff_count"
    }
}
