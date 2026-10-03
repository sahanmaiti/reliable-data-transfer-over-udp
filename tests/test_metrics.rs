// CS-30003: Reliable Data Transfer over UDP
// Author: Sahan Maiti (Team Lead)
// Integration Tests: Metrics and CSV / JSON Reporting

use reliable_udp::metrics::{
    ApplicationMetrics, ChannelMetrics, ExperimentMeta, ExperimentRecord, ProtocolMetrics,
    TimingMetrics,
};

#[test]
fn test_experiment_record_goodput_calculation() {
    let meta = ExperimentMeta {
        experiment_id: "TEST_001".to_string(),
        protocol: "GoBackN".to_string(),
        window_size: 4,
        seed: 999,
        trial_id: 1,
        configured_loss_rate: 0.02,
        configured_reorder_rate: 0.05,
        configured_delay_ms: 10,
        configured_jitter_ms: 2,
    };

    let app = ApplicationMetrics {
        source_file_bytes: 28000,
        delivered_unique_bytes: 28000,
        transfer_status: "SUCCESS".to_string(),
        sha256_match: true,
        source_sha256: "hash_original".to_string(),
        delivered_sha256: "hash_original".to_string(),
    };

    let timing = TimingMetrics {
        duration_secs: 4.0,
        total_rtt_samples: 20,
        min_rtt_ms: 10.0,
        max_rtt_ms: 25.0,
        mean_rtt_ms: 15.0,
        final_srtt_ms: 15.2,
        final_rttvar_ms: 2.1,
        final_rto_ms: 35.0,
        timeout_count: 2,
        backoff_count: 1,
    };

    let protocol = ProtocolMetrics {
        data_sent: 25,
        ack_sent: 23,
        data_received: 25,
        ack_received: 23,
        data_retransmissions: 5,
        timeout_retransmissions: 2,
        duplicate_data_received: 2,
        duplicate_acks_received: 1,
    };

    let record = ExperimentRecord::new(meta, app, timing, protocol, ChannelMetrics::default());

    // 28,000 bytes / 4.0 secs = 7,000 B/s
    assert_eq!(record.goodput_bytes_sec, 7000.0);
    // 5 retransmissions / 25 sent = 0.20
    assert_eq!(record.retransmission_ratio, 0.20);

    let json_output = record.to_json().expect("Serialization failed");
    assert!(json_output.contains("\"protocol\": \"GoBackN\""));
    assert!(json_output.contains("\"goodput_bytes_sec\": 7000.0"));
}
