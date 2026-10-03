// CS-30003: Reliable Data Transfer over UDP
// Author: Kashish Gupta
// Unit Tests: Jacobson/Karels RTO Estimator, Karn's Algorithm & Retransmission Timers

use reliable_udp::timing::{MultiTimer, RetransmissionTimer, RtoConfig, RtoEstimator};
use std::time::{Duration, Instant};

#[test]
fn test_initial_rto_default() {
    let estimator = RtoEstimator::new();
    assert_eq!(estimator.current_rto(), Duration::from_millis(1000));
    assert!(estimator.srtt().is_none());
    assert!(estimator.rttvar().is_none());
    assert_eq!(estimator.stats.sample_count, 0);
}

#[test]
fn test_first_rtt_sample_rfc6298() {
    let mut estimator = RtoEstimator::new();
    let sample = Duration::from_millis(100);

    let accepted = estimator.update_rtt(sample, false);
    assert!(accepted);

    // RFC 6298 Rule 2.1:
    // SRTT = R = 100ms
    // RTTVAR = R / 2 = 50ms
    // RTO = SRTT + max(G, 4 * RTTVAR) = 100 + 4 * 50 = 300ms
    let srtt = estimator.srtt().unwrap();
    let rttvar = estimator.rttvar().unwrap();
    let rto = estimator.current_rto();

    assert_eq!(srtt, Duration::from_millis(100));
    assert_eq!(rttvar, Duration::from_millis(50));
    assert_eq!(rto, Duration::from_millis(300));
}

#[test]
fn test_subsequent_rtt_samples_smoothing() {
    let mut estimator = RtoEstimator::new();
    estimator.update_rtt(Duration::from_millis(100), false);

    // Second sample = 120ms
    estimator.update_rtt(Duration::from_millis(120), false);

    // RFC 6298 Rule 2.2:
    // prev_srtt = 100, prev_rttvar = 50
    // diff = |100 - 120| = 20
    // new_rttvar = 0.75 * 50 + 0.25 * 20 = 37.5 + 5.0 = 42.5ms
    // new_srtt = 0.875 * 100 + 0.125 * 120 = 87.5 + 15.0 = 102.5ms
    // new_rto = 102.5 + 4 * 42.5 = 102.5 + 170.0 = 272.5ms
    let srtt_ms = estimator.srtt().unwrap().as_secs_f64() * 1000.0;
    let rttvar_ms = estimator.rttvar().unwrap().as_secs_f64() * 1000.0;
    let rto_ms = estimator.current_rto().as_secs_f64() * 1000.0;

    assert!((srtt_ms - 102.5).abs() < 0.1);
    assert!((rttvar_ms - 42.5).abs() < 0.1);
    assert!((rto_ms - 272.5).abs() < 0.1);
}

#[test]
fn test_karns_algorithm_rejects_retransmissions() {
    let mut estimator = RtoEstimator::new();
    estimator.update_rtt(Duration::from_millis(100), false);

    let baseline_rto = estimator.current_rto();
    let baseline_samples = estimator.stats.sample_count;

    // Packet was retransmitted: Karn's algorithm must discard sample!
    let accepted = estimator.update_rtt(Duration::from_millis(500), true);
    assert!(!accepted);
    assert_eq!(estimator.stats.karn_rejected_count, 1);
    assert_eq!(estimator.stats.sample_count, baseline_samples);
    assert_eq!(estimator.current_rto(), baseline_rto);
}

#[test]
fn test_exponential_backoff_on_timeout() {
    let mut estimator = RtoEstimator::new();
    estimator.update_rtt(Duration::from_millis(100), false);
    assert_eq!(estimator.current_rto(), Duration::from_millis(300));

    // First timeout: doubles to 600ms
    let backoff1 = estimator.on_timeout();
    assert_eq!(backoff1, Duration::from_millis(600));
    assert_eq!(estimator.stats.timeout_count, 1);

    // Second timeout: doubles to 1200ms
    let backoff2 = estimator.on_timeout();
    assert_eq!(backoff2, Duration::from_millis(1200));
    assert_eq!(estimator.stats.timeout_count, 2);

    // Successful fresh ACK clears backoff
    estimator.update_rtt(Duration::from_millis(100), false);
    // After fresh sample, RTO recalculates from baseline without backoff
    assert!(estimator.current_rto() < Duration::from_millis(400));
}

#[test]
fn test_rto_clamped_to_min_and_max() {
    // Test min_rto clamp
    let config = RtoConfig {
        min_rto: Duration::from_millis(250),
        max_rto: Duration::from_millis(5000),
        ..RtoConfig::default()
    };
    let mut estimator = RtoEstimator::with_config(config);
    // Sample very small RTT (10ms): raw RTO would be ~30ms, but min_rto is 250ms
    estimator.update_rtt(Duration::from_millis(10), false);
    assert_eq!(estimator.current_rto(), Duration::from_millis(250));

    // Test max_rto clamp after backoffs
    for _ in 0..10 {
        estimator.on_timeout();
    }
    assert_eq!(estimator.current_rto(), Duration::from_millis(5000));
}

#[test]
fn test_rto_multiplier_scaling_e4() {
    let config_normal = RtoConfig {
        rto_multiplier: 1.0,
        ..RtoConfig::default()
    };
    let mut est_normal = RtoEstimator::with_config(config_normal);
    est_normal.update_rtt(Duration::from_millis(100), false);

    let config_scaled = RtoConfig {
        rto_multiplier: 2.0,
        ..RtoConfig::default()
    };
    let mut est_scaled = RtoEstimator::with_config(config_scaled);
    est_scaled.update_rtt(Duration::from_millis(100), false);

    // Normal = 300ms, Scaled (2.0x) = 600ms
    assert_eq!(est_normal.current_rto(), Duration::from_millis(300));
    assert_eq!(est_scaled.current_rto(), Duration::from_millis(600));
}

#[test]
fn test_retransmission_timer_generation_safety() {
    let mut timer = RetransmissionTimer::new();
    let t0 = Instant::now();

    // Arm timer for 50ms (gen 1)
    let gen1 = timer.start_at(t0, Duration::from_millis(50));
    assert_eq!(gen1, 1);
    assert!(timer.is_active());

    // Re-arm timer before expiry for 100ms (gen 2)
    let gen2 = timer.start_at(t0, Duration::from_millis(100));
    assert_eq!(gen2, 2);

    let at_75ms = t0 + Duration::from_millis(75);

    // A stale callback from gen 1 checking at 75ms must be rejected!
    assert!(!timer.is_expired_for_at(gen1, at_75ms));

    // Gen 2 is not yet expired at 75ms
    assert!(!timer.is_expired_for_at(gen2, at_75ms));

    // At 105ms, gen 2 IS expired
    let at_105ms = t0 + Duration::from_millis(105);
    assert!(timer.is_expired_for_at(gen2, at_105ms));
    // But gen 1 is STILL not valid because generation mismatch
    assert!(!timer.is_expired_for_at(gen1, at_105ms));
}

#[test]
fn test_multi_timer_selective_repeat() {
    let mut multi = MultiTimer::new();
    let t0 = Instant::now();

    multi.start_timer_at(101, t0, Duration::from_millis(30));
    multi.start_timer_at(102, t0, Duration::from_millis(10));
    multi.start_timer_at(103, t0, Duration::from_millis(50));

    assert_eq!(multi.len(), 3);
    assert_eq!(multi.earliest_deadline(), Some(t0 + Duration::from_millis(10)));

    // At 20ms: key 102 should be expired
    let expired = multi.pop_expired_at(t0 + Duration::from_millis(20));
    assert_eq!(expired, vec![102]);
    assert_eq!(multi.len(), 2);

    // Cancel key 101
    assert!(multi.cancel_timer(&101));
    assert_eq!(multi.len(), 1);

    // At 60ms: key 103 should be expired
    let expired_later = multi.pop_expired_at(t0 + Duration::from_millis(60));
    assert_eq!(expired_later, vec![103]);
    assert!(multi.is_empty());
}
