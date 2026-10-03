// CS-30003: Reliable Data Transfer over UDP
// Author: Ashwika Burman
// Component: Deterministic Channel Emulator & Delivery Scheduler

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

pub mod scheduler; // lives in src/channel/scheduler.rs

/// All emulator settings live in one place.
/// Probabilities are between 0.0 (never) and 1.0 (always).
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub seed: u64,             // same seed => same sequence of faults
    pub loss: f64,             // chance a packet is dropped
    pub duplicate: f64,        // chance a packet is delivered twice
    pub reorder: f64,          // chance a packet is held back so later ones overtake it
    pub reorder_extra_ms: u64, // how long a reordered packet is held back
    pub corrupt: f64,          // chance a bit in the packet is flipped
    pub base_delay_ms: u64,    // fixed one-way delay
    pub jitter_ms: u64,        // extra random delay between 0 and jitter_ms
}

/// Counts of what the channel actually did. Used for result logging.
#[derive(Debug, Clone, Default)]
pub struct ChannelStats {
    pub packets_seen: u64,     // packets handed to the channel
    pub lost: u64,             // packets dropped
    pub corrupted: u64,        // packets with one bit flipped
    pub duplicated: u64,       // packets delivered twice
    pub reordered: u64,        // packets held back (reorder decision)
    pub copies_delivered: u64, // total copies that left the channel
}

impl ChannelStats {
    /// The counters as CSV values (Channel::to_csv_row adds the seed and
    /// configured rates in front of these).
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{}",
            self.packets_seen,
            self.lost,
            self.corrupted,
            self.duplicated,
            self.reordered,
            self.copies_delivered
        )
    }
}

/// One copy of a packet that the channel will deliver.
pub struct Delivery {
    pub delay_ms: u64, // how long to wait before delivering it
    pub data: Vec<u8>, // the packet bytes
}

pub struct Channel {
    pub config: ChannelConfig,
    pub stats: ChannelStats,
    rng: StdRng,
}

impl Channel {
    pub fn new(config: ChannelConfig) -> Self {
        let rng = StdRng::seed_from_u64(config.seed);
        Channel {
            config,
            stats: ChannelStats::default(),
            rng,
        }
    }

    /// Full report header: the seed and configured settings first, so every
    /// result row is traceable, then the observed counters.
    pub fn csv_header() -> &'static str {
        "seed,configured_loss_rate,configured_duplicate_rate,configured_reorder_rate,configured_corruption_rate,reorder_extra_ms,base_delay_ms,jitter_ms,packets_seen,lost,corrupted,duplicated,reordered,copies_delivered"
    }

    /// One CSV row (same column order as `csv_header`).
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{}",
            self.config.seed,
            self.config.loss,
            self.config.duplicate,
            self.config.reorder,
            self.config.corrupt,
            self.config.reorder_extra_ms,
            self.config.base_delay_ms,
            self.config.jitter_ms,
            self.stats.to_csv_row()
        )
    }

    /// The same report as a JSON object.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"seed\":{},\"configured_loss_rate\":{},\"configured_duplicate_rate\":{},\"configured_reorder_rate\":{},\"configured_corruption_rate\":{},\"reorder_extra_ms\":{},\"base_delay_ms\":{},\"jitter_ms\":{},\"packets_seen\":{},\"lost\":{},\"corrupted\":{},\"duplicated\":{},\"reordered\":{},\"copies_delivered\":{}}}",
            self.config.seed,
            self.config.loss,
            self.config.duplicate,
            self.config.reorder,
            self.config.corrupt,
            self.config.reorder_extra_ms,
            self.config.base_delay_ms,
            self.config.jitter_ms,
            self.stats.packets_seen,
            self.stats.lost,
            self.stats.corrupted,
            self.stats.duplicated,
            self.stats.reordered,
            self.stats.copies_delivered
        )
    }

    /// Random extra delay in the range 0..=jitter_ms.
    /// `roll` is a random number in [0,1) that was already drawn.
    fn jitter(&self, roll: f64) -> u64 {
        (roll * (self.config.jitter_ms + 1) as f64) as u64
    }

    /// Takes one packet (raw wire bytes) and returns what the network does
    /// with it: empty list = lost, one item = normal, two items = duplicated.
    pub fn process(&mut self, packet: &[u8]) -> Vec<Delivery> {
        // Fixed-order draws: every packet uses the SAME number of random
        // values, even if it is dropped.
        let loss_roll: f64 = self.rng.random(); // loss
        let dup_roll: f64 = self.rng.random(); // duplication
        let reorder_roll: f64 = self.rng.random(); // reordering
        let corrupt_roll: f64 = self.rng.random(); // corruption
        let byte_roll: f64 = self.rng.random(); // which byte to damage
        let bit_roll: f64 = self.rng.random(); // which bit to flip
        let jitter_roll: f64 = self.rng.random(); // jitter of the first copy
        let dup_jitter_roll: f64 = self.rng.random(); // jitter of the duplicate

        self.stats.packets_seen += 1;

        // LOSS: a lost packet never reaches the network.
        if loss_roll < self.config.loss {
            self.stats.lost += 1;
            return Vec::new();
        }

        // Start from a copy of the original bytes.
        let mut data = packet.to_vec();

        // CORRUPTION: flip exactly one bit (length unchanged).
        if corrupt_roll < self.config.corrupt && !data.is_empty() {
            let byte_index = (byte_roll * data.len() as f64) as usize;
            let bit_index = (bit_roll * 8.0) as u32;
            data[byte_index] ^= 1u8 << bit_index;
            self.stats.corrupted += 1;
        }

        // REORDERING: with probability `reorder`, hold the packet back.
        let hold = if reorder_roll < self.config.reorder {
            self.stats.reordered += 1;
            self.config.reorder_extra_ms
        } else {
            0
        };

        // DELAY + JITTER + hold-back.
        let delay = self.config.base_delay_ms + self.jitter(jitter_roll) + hold;

        let mut out = vec![Delivery {
            delay_ms: delay,
            data: data.clone(),
        }];

        // DUPLICATION: identical bytes, own jitter, same hold-back.
        if dup_roll < self.config.duplicate {
            let dup_delay = self.config.base_delay_ms + self.jitter(dup_jitter_roll) + hold;
            out.push(Delivery {
                delay_ms: dup_delay,
                data,
            });
            self.stats.duplicated += 1;
        }

        self.stats.copies_delivered += out.len() as u64;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A config with every fault switched off. Each test turns on only
    /// the fault it wants to check.
    fn clean_config(seed: u64) -> ChannelConfig {
        ChannelConfig {
            seed,
            loss: 0.0,
            duplicate: 0.0,
            reorder: 0.0,
            reorder_extra_ms: 50,
            corrupt: 0.0,
            base_delay_ms: 50,
            jitter_ms: 0,
        }
    }

    /// Sends n packets and records exactly what came out of the channel.
    fn run(config: ChannelConfig, n: usize) -> Vec<Vec<(u64, Vec<u8>)>> {
        let mut ch = Channel::new(config);
        (0..n)
            .map(|i| {
                let packet = vec![i as u8; 10];
                ch.process(&packet)
                    .into_iter()
                    .map(|d| (d.delay_ms, d.data))
                    .collect()
            })
            .collect()
    }

    fn all_faults(seed: u64) -> ChannelConfig {
        ChannelConfig {
            loss: 0.1,
            duplicate: 0.1,
            reorder: 0.15,
            corrupt: 0.05,
            jitter_ms: 10,
            ..clean_config(seed)
        }
    }

    #[test]
    fn same_seed_gives_identical_outcomes() {
        // Seed replay: the core of reproducibility.
        assert_eq!(run(all_faults(42), 2000), run(all_faults(42), 2000));
    }

    #[test]
    fn different_seed_gives_different_outcomes() {
        assert_ne!(run(all_faults(42), 2000), run(all_faults(7), 2000));
    }

    #[test]
    fn zero_faults_passes_every_packet_unchanged() {
        let results = run(clean_config(1), 1000);
        for (i, out) in results.iter().enumerate() {
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].0, 50); // exactly the base delay
            assert_eq!(out[0].1, vec![i as u8; 10]); // identical bytes
        }
    }

    #[test]
    fn loss_one_drops_everything() {
        let config = ChannelConfig { loss: 1.0, ..clean_config(1) };
        assert!(run(config, 1000).iter().all(|out| out.is_empty()));
    }

    #[test]
    fn loss_rate_close_to_configured() {
        let config = ChannelConfig { loss: 0.3, ..clean_config(42) };
        let mut ch = Channel::new(config);
        for i in 0..100_000u32 {
            ch.process(&[i as u8; 10]);
        }
        let rate = ch.stats.lost as f64 / ch.stats.packets_seen as f64;
        assert!((rate - 0.3).abs() < 0.01, "observed loss rate {}", rate);
    }

    #[test]
    fn duplicate_rate_close_to_configured() {
        let config = ChannelConfig { duplicate: 0.2, ..clean_config(42) };
        let mut ch = Channel::new(config);
        for i in 0..100_000u32 {
            ch.process(&[i as u8; 10]);
        }
        let rate = ch.stats.duplicated as f64 / ch.stats.packets_seen as f64;
        assert!((rate - 0.2).abs() < 0.01, "observed duplicate rate {}", rate);
    }

    #[test]
    fn corruption_changes_bytes_but_not_length() {
        let config = ChannelConfig { corrupt: 1.0, ..clean_config(5) };
        let results = run(config, 1000);
        for (i, out) in results.iter().enumerate() {
            let original = vec![i as u8; 10];
            assert_eq!(out[0].1.len(), original.len()); // same length
            assert_ne!(out[0].1, original); // different bytes
            // exactly one bit differs
            let flipped: u32 = out[0]
                .1
                .iter()
                .zip(&original)
                .map(|(a, b)| (a ^ b).count_ones())
                .sum();
            assert_eq!(flipped, 1);
        }
    }

    #[test]
    fn reorder_zero_never_holds_a_packet_back() {
        let mut ch = Channel::new(clean_config(3));
        for i in 0..10_000u32 {
            ch.process(&[i as u8; 10]);
        }
        assert_eq!(ch.stats.reordered, 0);
    }

    #[test]
    fn reorder_rate_close_to_configured() {
        // 25% is the top of the proposal's sweep.
        let config = ChannelConfig { reorder: 0.25, ..clean_config(42) };
        let mut ch = Channel::new(config);
        for i in 0..100_000u32 {
            ch.process(&[i as u8; 10]);
        }
        let rate = ch.stats.reordered as f64 / ch.stats.packets_seen as f64;
        assert!((rate - 0.25).abs() < 0.01, "observed reorder rate {}", rate);
    }

    #[test]
    fn delay_stays_within_base_plus_jitter() {
        let config = ChannelConfig { jitter_ms: 10, ..clean_config(9) };
        for out in run(config, 5000) {
            let delay = out[0].0;
            assert!(delay >= 50 && delay <= 60, "delay {}", delay);
        }
    }

    #[test]
    fn stats_are_consistent() {
        let mut ch = Channel::new(all_faults(42));
        for i in 0..10_000u32 {
            ch.process(&[i as u8; 10]);
        }
        let s = &ch.stats;
        assert_eq!(s.packets_seen, 10_000);
        assert_eq!(s.copies_delivered, s.packets_seen - s.lost + s.duplicated);
    }

    #[test]
    fn report_header_and_row_have_same_column_count() {
        let mut ch = Channel::new(all_faults(42));
        ch.process(&[1u8; 10]);
        let header_cols = Channel::csv_header().split(',').count();
        let row_cols = ch.to_csv_row().split(',').count();
        assert_eq!(header_cols, row_cols);
    }

    #[test]
    fn report_contains_seed_and_configured_rates() {
        let ch = Channel::new(all_faults(42));
        let json = ch.to_json();
        assert!(json.contains("\"seed\":42"));
        assert!(json.contains("\"configured_loss_rate\":0.1"));
        assert!(json.contains("\"configured_reorder_rate\":0.15"));
        assert!(ch.to_csv_row().starts_with("42,0.1,0.1,0.15,0.05,"));
    }
}