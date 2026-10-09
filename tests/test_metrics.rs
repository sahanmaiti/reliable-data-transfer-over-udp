// CS-30003: Reliable Data Transfer over UDP
// Integration Tests: Metrics and CSV / JSON Reporting

use reliable_udp::metrics::{
    ApplicationMetrics, ChannelMetrics, ExperimentMeta, ExperimentRecord, ForwardChannelConfig,
    ProtocolMetrics, ReverseChannelConfig, RtoParameters, TimingMetrics,
};

fn record() -> ExperimentRecord {
    ExperimentRecord::new(
        ExperimentMeta {
            experiment_id: "TEST_001".to_string(),
            protocol: "GoBackN".to_string(),
            window_size: 4,
            chunk_size: 1400,
            segment_count: 20,
            seed: 999,
            reverse_seed: 1000,
            trial_id: 1,
        },
        ForwardChannelConfig {
            configured_loss_rate: 0.02,
            configured_reorder_rate: 0.05,
            configured_duplicate_rate: 0.0,
            configured_corrupt_rate: 0.0,
            base_delay_ms: 10,
            jitter_ms: 2,
            reorder_extra_ms: 50,
        },
        ReverseChannelConfig {
            configured_loss_rate: 0.0,
            configured_reorder_rate: 0.0,
            configured_duplicate_rate: 0.0,
            configured_corrupt_rate: 0.0,
            base_delay_ms: 10,
            jitter_ms: 2,
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
            source_bytes: 28000,
            delivered_unique_bytes: 28000,
            transfer_status: "SUCCESS".to_string(),
            sha256_match: false,
            source_sha256: "hash_original".to_string(),
            delivered_sha256: "hash_original".to_string(),
        },
        TimingMetrics {
            duration_secs: 4.0,
            total_rtt_samples: 20,
            karn_rejected_count: 3,
            min_rtt_ms: 10.0,
            max_rtt_ms: 25.0,
            mean_rtt_ms: 15.0,
            final_srtt_ms: 15.2,
            final_rttvar_ms: 2.1,
            final_rto_ms: 35.0,
            timeout_count: 2,
            backoff_count: 2,
        },
        ProtocolMetrics {
            data_sent: 25,
            ack_sent: 23,
            data_received: 25,
            ack_received: 23,
            data_retransmissions: 5,
            timeout_retransmissions: 5,
            premature_retransmissions: 1,
            duplicate_data_received: 2,
            duplicate_acks_received: 1,
        },
        ChannelMetrics {
            offered_datagrams: 30,
            dropped_datagrams: 2,
            duplicated_datagrams: 0,
            corrupted_datagrams: 0,
            reordered_datagrams: 1,
            scheduled_deliveries: 28,
        },
        ChannelMetrics::default(),
    )
}

#[test]
fn test_experiment_record_goodput_calculation() {
    let record = record();

    assert!(record.app.sha256_match);
    // 28,000 bytes / 4.0 secs = 7,000 B/s
    assert_eq!(record.goodput_bytes_sec, 7000.0);
    // 5 retransmissions / 25 sent = 0.20
    assert_eq!(record.retransmission_ratio, 0.20);

    let json_output = record.to_json().expect("Serialization failed");
    let parsed: ExperimentRecord = serde_json::from_str(&json_output).expect("round trip");
    assert_eq!(parsed.meta.protocol, "GoBackN");
    assert_eq!(parsed.meta.seed, 999);
    assert_eq!(parsed.meta.reverse_seed, 1000);
    assert_eq!(parsed.meta.chunk_size, 1400);
    assert_eq!(parsed.protocol.premature_retransmissions, 1);
    assert_eq!(parsed.timing.karn_rejected_count, 3);
    assert_eq!(parsed.channel.dropped_datagrams, 2);
    assert_eq!(parsed.rto_config.alpha, 0.125);
    assert!((parsed.goodput_bytes_sec - 7000.0).abs() < 1e-9);
    assert_eq!(
        ExperimentRecord::csv_header().split(',').count(),
        record.to_csv_row().split(',').count()
    );
}
