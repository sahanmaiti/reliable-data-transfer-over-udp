// Virtual-time transfer driver. Assertions are about outcomes of the Rust
// implementation: completion, determinism, and the direction of protocol
// differences. They do not encode a synthetic retransmission formula.

use reliable_udp::app::{compute_sha256, run_transfer, TransferConfig, TransferOutput};
use reliable_udp::arq::ProtocolType;
use reliable_udp::metrics::ExperimentRecord;
use std::time::Duration;

fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn config(protocol: ProtocolType, seed: u64, bytes: Vec<u8>, window: usize) -> TransferConfig {
    TransferConfig {
        payload: bytes,
        protocol,
        window_size: window,
        chunk_size: 16,
        seed,
        trial_id: 1,
        experiment_id: "phase2".to_string(),
        loss_rate: 0.0,
        reorder_rate: 0.0,
        duplicate_rate: 0.0,
        corrupt_rate: 0.0,
        base_delay_ms: 20,
        jitter_ms: 0,
        reorder_extra_ms: 50,
        rto_multiplier: 1.0,
        min_rto: Duration::from_millis(200),
        max_rto: Duration::from_millis(60_000),
        initial_rto: Duration::from_millis(1000),
        alpha: 0.125,
        beta: 0.25,
        max_virtual_time: Duration::from_secs(3600),
        max_retransmissions: 5_000,
    }
}

fn assert_matches_source(output: &TransferOutput, source: &[u8]) {
    assert_eq!(output.record.app.transfer_status, "SUCCESS", "{output:?}");
    assert!(output.record.app.sha256_match);
    assert_eq!(output.record.app.source_sha256, compute_sha256(source));
    assert_eq!(output.record.app.delivered_sha256, compute_sha256(source));
    assert_eq!(
        output.record.app.delivered_unique_bytes,
        source.len() as u64
    );
    assert_eq!(output.record.app.source_bytes, source.len() as u64);
    assert_eq!(
        output.record.protocol.premature_retransmissions as u64,
        output.premature_retransmissions
    );
    assert_eq!(
        output.record.timing.karn_rejected_count as u64,
        output.karn_rejected
    );
    assert_eq!(output.record.meta.reverse_seed, output.reverse_seed);
    assert_eq!(
        output.record.channel.offered_datagrams,
        output.forward_events.len()
    );
    let traced_drops = output
        .forward_events
        .iter()
        .filter(|event| event.delays_ms.is_empty())
        .count();
    assert_eq!(output.record.channel.dropped_datagrams, traced_drops);
    assert!(output.record.timing.duration_secs > 0.0);
    assert!(output.record.goodput_bytes_sec > 0.0);
    assert!(output.record.timing.total_rtt_samples > 0);
    assert!(output.record.timing.final_rto_ms > 0.0);
    assert_eq!(output.reverse.dropped_datagrams, 0);
    assert_eq!(output.reverse.reordered_datagrams, 0);
    assert_eq!(output.reverse.duplicated_datagrams, 0);
    assert_eq!(output.reverse.corrupted_datagrams, 0);
    assert!(output.reverse.offered_datagrams > 0);
}

#[test]
fn clean_transfer_succeeds_and_matches_sha256_for_every_protocol() {
    let source = payload(64);
    for protocol in [
        ProtocolType::StopAndWait,
        ProtocolType::GoBackN,
        ProtocolType::SelectiveRepeat,
    ] {
        let window = if protocol == ProtocolType::StopAndWait {
            1
        } else {
            4
        };
        let output = run_transfer(&config(protocol, 7, source.clone(), window));
        assert_matches_source(&output, &source);
        assert_eq!(output.record.protocol.data_retransmissions, 0);
        assert_eq!(output.record.timing.timeout_count, 0);
        assert_eq!(output.record.channel.dropped_datagrams, 0);
        assert_eq!(output.record.channel.reordered_datagrams, 0);
        assert_eq!(output.premature_retransmissions, 0);
        assert_eq!(output.reverse_seed, 8);
    }
}

#[test]
fn file_longer_than_the_sender_window_completes() {
    let source = payload(100);
    for protocol in [
        ProtocolType::StopAndWait,
        ProtocolType::GoBackN,
        ProtocolType::SelectiveRepeat,
    ] {
        let mut cfg = config(protocol, 3, source.clone(), 3);
        cfg.chunk_size = 10;
        let output = run_transfer(&cfg);
        assert_matches_source(&output, &source);
        assert!(
            output.segment_count > 3,
            "segment_count {}",
            output.segment_count
        );
        assert_eq!(output.record.protocol.data_retransmissions, 0);
    }
}

#[test]
fn same_seed_and_config_produce_identical_results() {
    let mut cfg = config(ProtocolType::GoBackN, 21, payload(160), 4);
    cfg.loss_rate = 0.15;
    cfg.reorder_rate = 0.2;
    cfg.duplicate_rate = 0.05;
    cfg.corrupt_rate = 0.02;
    cfg.jitter_ms = 4;

    let first = run_transfer(&cfg);
    let second = run_transfer(&cfg);

    assert_eq!(
        first.record.app.transfer_status,
        second.record.app.transfer_status
    );
    assert_eq!(
        first.record.app.sha256_match,
        second.record.app.sha256_match
    );
    assert_eq!(
        first.record.protocol.data_retransmissions,
        second.record.protocol.data_retransmissions
    );
    assert_eq!(
        first.record.timing.duration_secs,
        second.record.timing.duration_secs
    );
    assert_eq!(
        first.record.timing.final_rto_ms,
        second.record.timing.final_rto_ms
    );
    assert_eq!(
        first.record.channel.dropped_datagrams,
        second.record.channel.dropped_datagrams
    );
    assert_eq!(
        first.record.channel.reordered_datagrams,
        second.record.channel.reordered_datagrams
    );
    assert_eq!(
        first.premature_retransmissions,
        second.premature_retransmissions
    );
    assert_eq!(first.forward_events, second.forward_events);
    assert_eq!(
        first.record.app.delivered_sha256,
        second.record.app.delivered_sha256
    );
}

#[test]
fn different_seeds_change_channel_outcomes_when_impaired() {
    let mut left = config(ProtocolType::SelectiveRepeat, 1, payload(240), 4);
    let mut right = left.clone();
    right.seed = 2;
    for cfg in [&mut left, &mut right] {
        cfg.loss_rate = 0.2;
        cfg.reorder_rate = 0.3;
        cfg.duplicate_rate = 0.1;
        cfg.corrupt_rate = 0.05;
        cfg.jitter_ms = 8;
    }

    let a = run_transfer(&left);
    let b = run_transfer(&right);
    assert_ne!(
        a.forward_events, b.forward_events,
        "distinct seeds produced the same forward channel trace"
    );
}

#[test]
fn packet_loss_causes_retransmission_and_recovery() {
    let source = payload(128);
    let mut cfg = config(ProtocolType::StopAndWait, 42, source.clone(), 1);
    cfg.loss_rate = 0.5;
    let output = run_transfer(&cfg);
    assert_matches_source(&output, &source);
    assert!(output.record.channel.dropped_datagrams > 0);
    assert!(output.record.protocol.data_retransmissions > 0);
    assert!(output.record.timing.timeout_count > 0);
    assert_eq!(
        output.record.protocol.timeout_retransmissions,
        output.record.protocol.data_retransmissions
    );
    assert_eq!(
        output.record.timing.timeout_count,
        output.record.timing.backoff_count
    );
}

#[test]
fn record_preserves_the_configuration_that_was_run() {
    let source = payload(96);
    let mut cfg = config(ProtocolType::SelectiveRepeat, 77, source.clone(), 4);
    cfg.chunk_size = 12;
    cfg.trial_id = 4;
    cfg.experiment_id = "config-check".to_string();
    cfg.loss_rate = 0.0;
    cfg.reorder_rate = 0.25;
    cfg.duplicate_rate = 0.0;
    cfg.corrupt_rate = 0.0;
    cfg.base_delay_ms = 15;
    cfg.jitter_ms = 0;
    cfg.reorder_extra_ms = 40;
    cfg.rto_multiplier = 1.5;
    cfg.min_rto = Duration::from_millis(10);
    cfg.initial_rto = Duration::from_millis(800);
    cfg.max_rto = Duration::from_millis(30_000);
    cfg.alpha = 0.125;
    cfg.beta = 0.25;

    let output = run_transfer(&cfg);
    assert_matches_source(&output, &source);
    let record = &output.record;
    assert_eq!(record.meta.experiment_id, "config-check");
    assert_eq!(record.meta.protocol, "SelectiveRepeat");
    assert_eq!(record.meta.window_size, 4);
    assert_eq!(record.meta.chunk_size, 12);
    assert_eq!(record.meta.segment_count, output.segment_count as usize);
    assert_eq!(record.meta.seed, 77);
    assert_eq!(record.meta.reverse_seed, 78);
    assert_eq!(record.meta.trial_id, 4);
    assert_eq!(record.forward_config.configured_loss_rate, 0.0);
    assert_eq!(record.forward_config.configured_reorder_rate, 0.25);
    assert_eq!(record.forward_config.configured_duplicate_rate, 0.0);
    assert_eq!(record.forward_config.configured_corrupt_rate, 0.0);
    assert_eq!(record.forward_config.base_delay_ms, 15);
    assert_eq!(record.forward_config.jitter_ms, 0);
    assert_eq!(record.forward_config.reorder_extra_ms, 40);
    assert_eq!(record.reverse_config.configured_loss_rate, 0.0);
    assert_eq!(record.reverse_config.configured_reorder_rate, 0.0);
    assert_eq!(record.reverse_config.configured_duplicate_rate, 0.0);
    assert_eq!(record.reverse_config.configured_corrupt_rate, 0.0);
    assert_eq!(record.reverse_config.base_delay_ms, 15);
    assert_eq!(record.rto_config.rto_multiplier, 1.5);
    assert_eq!(record.rto_config.initial_rto_ms, 800.0);
    assert_eq!(record.rto_config.min_rto_ms, 10.0);
    assert_eq!(record.rto_config.max_rto_ms, 30_000.0);
    assert_eq!(record.rto_config.alpha, 0.125);
    assert_eq!(record.rto_config.beta, 0.25);
    assert_eq!(record.rto_config.k, 4.0);
    assert_eq!(record.reverse_channel.reordered_datagrams, 0);
    assert_eq!(record.reverse_channel.dropped_datagrams, 0);
    assert_eq!(
        ExperimentRecord::csv_header().split(',').count(),
        record.to_csv_row().split(',').count()
    );
}

#[test]
fn timed_out_transfer_has_zero_goodput() {
    let mut cfg = config(ProtocolType::StopAndWait, 5, payload(16), 1);
    cfg.loss_rate = 1.0;
    cfg.max_virtual_time = Duration::from_secs(5);
    let output = run_transfer(&cfg);
    assert_eq!(output.record.app.transfer_status, "TIMEOUT");
    assert!(!output.record.app.sha256_match);
    assert_eq!(output.record.goodput_bytes_sec, 0.0);
    assert!(output.record.channel.dropped_datagrams > 0);
    assert_eq!(
        output.record.protocol.premature_retransmissions as u64,
        output.premature_retransmissions
    );
}

#[test]
fn reordering_makes_selective_repeat_retransmit_less_than_go_back_n() {
    let source = payload(48 * 16);
    let mut gbn = config(ProtocolType::GoBackN, 101, source.clone(), 8);
    let mut sr = gbn.clone();
    sr.protocol = ProtocolType::SelectiveRepeat;
    let mut sw = gbn.clone();
    sw.protocol = ProtocolType::StopAndWait;
    sw.window_size = 1;
    for cfg in [&mut gbn, &mut sr, &mut sw] {
        cfg.loss_rate = 0.0;
        cfg.reorder_rate = 0.25;
        cfg.reorder_extra_ms = 50;
        cfg.min_rto = Duration::from_millis(200);
        cfg.base_delay_ms = 20;
        cfg.jitter_ms = 0;
    }

    let gbn_out = run_transfer(&gbn);
    let sr_out = run_transfer(&sr);
    let sw_out = run_transfer(&sw);

    assert_matches_source(&gbn_out, &source);
    assert_matches_source(&sr_out, &source);
    assert_matches_source(&sw_out, &source);
    assert!(gbn_out.record.channel.reordered_datagrams > 0);
    assert_eq!(gbn_out.reverse.reordered_datagrams, 0);

    assert!(
        sr_out.record.protocol.data_retransmissions < gbn_out.record.protocol.data_retransmissions,
        "sr {} gbn {}",
        sr_out.record.protocol.data_retransmissions,
        gbn_out.record.protocol.data_retransmissions
    );
    assert_eq!(
        sw_out.record.protocol.data_retransmissions, 0,
        "stop-and-wait retransmitted under a reorder hold below the RTO floor"
    );
}

#[test]
fn rto_multiplier_changes_final_rto() {
    let source = payload(64);
    let mut low = config(ProtocolType::GoBackN, 9, source.clone(), 4);
    low.min_rto = Duration::from_millis(10);
    low.rto_multiplier = 1.0;
    let mut high = low.clone();
    high.rto_multiplier = 3.0;

    let low_out = run_transfer(&low);
    let high_out = run_transfer(&high);
    assert_matches_source(&low_out, &source);
    assert_matches_source(&high_out, &source);
    assert!(
        high_out.record.timing.final_rto_ms > low_out.record.timing.final_rto_ms,
        "3x final RTO {} vs 1x {}",
        high_out.record.timing.final_rto_ms,
        low_out.record.timing.final_rto_ms
    );
}

#[test]
fn smaller_rto_multiplier_causes_more_premature_retransmissions() {
    let source = payload(64 * 8);
    for protocol in [
        ProtocolType::StopAndWait,
        ProtocolType::GoBackN,
        ProtocolType::SelectiveRepeat,
    ] {
        let window = if protocol == ProtocolType::StopAndWait {
            1
        } else {
            8
        };
        let mut low = config(protocol, 15, source.clone(), window);
        low.chunk_size = 8;
        low.loss_rate = 0.05;
        low.reorder_rate = 0.0;
        low.min_rto = Duration::from_millis(10);
        low.base_delay_ms = 20;
        low.jitter_ms = 0;
        low.rto_multiplier = 0.5;
        let mut high = low.clone();
        high.rto_multiplier = 3.0;

        let low_out = run_transfer(&low);
        let high_out = run_transfer(&high);
        assert_eq!(
            low_out.record.app.transfer_status, "SUCCESS",
            "{protocol} 0.5x {low_out:?}"
        );
        assert_eq!(
            high_out.record.app.transfer_status, "SUCCESS",
            "{protocol} 3x {high_out:?}"
        );
        assert!(low_out.record.app.sha256_match && high_out.record.app.sha256_match);
        assert!(
            low_out.premature_retransmissions > high_out.premature_retransmissions,
            "{protocol}: 0.5x premature {} vs 3x premature {}",
            low_out.premature_retransmissions,
            high_out.premature_retransmissions
        );
    }
}

#[test]
fn experiment_record_json_and_csv_round_trip() {
    let source = payload(48);
    let output = run_transfer(&config(ProtocolType::SelectiveRepeat, 4, source.clone(), 4));
    assert_matches_source(&output, &source);

    let json = output.record.to_json().expect("json");
    let parsed: ExperimentRecord = serde_json::from_str(&json).expect("parse");
    assert_eq!(
        parsed.app.transfer_status,
        output.record.app.transfer_status
    );
    assert_eq!(parsed.app.sha256_match, output.record.app.sha256_match);
    assert_eq!(
        parsed.protocol.data_retransmissions,
        output.record.protocol.data_retransmissions
    );
    assert_eq!(parsed.protocol.data_sent, output.record.protocol.data_sent);
    assert!((parsed.timing.final_rto_ms - output.record.timing.final_rto_ms).abs() < 1e-9);
    assert!((parsed.timing.duration_secs - output.record.timing.duration_secs).abs() < 1e-12);
    assert_eq!(
        parsed.channel.reordered_datagrams,
        output.record.channel.reordered_datagrams
    );

    let path = std::env::temp_dir().join(format!("rdt-phase2-{}.csv", std::process::id()));
    output.record.append_to_csv(&path).expect("csv");
    let text = std::fs::read_to_string(&path).expect("read csv");
    let _ = std::fs::remove_file(&path);
    assert!(text.starts_with(ExperimentRecord::csv_header()));
    assert!(text.contains(&output.record.to_csv_row()));
}
