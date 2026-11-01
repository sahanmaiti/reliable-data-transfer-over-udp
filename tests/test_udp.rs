// CS-30003: Reliable Data Transfer over UDP
// Real localhost UDP loopback integration tests (Phase 10B).
//
// These tests use actual UdpSocket I/O. They do not use the deterministic
// channel emulator and do not touch results/raw/ or results/rto_sensitivity/.

use reliable_udp::app::{recv_file_on_socket, send_file, UdpTransferConfig};
use reliable_udp::arq::ProtocolType;
use reliable_udp::timing::RtoConfig;
use std::io::Write;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn scratch_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "reliable_udp_loopback_{label}_{}_{}",
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn write_source(path: &PathBuf, protocol: ProtocolType) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(b"reliable-udp-loopback:");
    body.extend_from_slice(protocol.to_string().as_bytes());
    body.push(b'\n');
    for i in 0u16..400 {
        body.extend_from_slice(&i.to_le_bytes());
    }
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(&body).unwrap();
    body
}

fn config_for(protocol: ProtocolType, window: usize) -> UdpTransferConfig {
    UdpTransferConfig {
        protocol,
        window,
        chunk_size: 128,
        rto: RtoConfig {
            // Keep localhost demo responsive without changing the estimator equations.
            initial_rto: Duration::from_millis(200),
            min_rto: Duration::from_millis(50),
            max_rto: Duration::from_secs(5),
            ..RtoConfig::default()
        },
        transfer_deadline: Duration::from_secs(15),
        recv_linger: Duration::from_millis(250),
        ..UdpTransferConfig::default()
    }
}

fn run_loopback(protocol: ProtocolType, window: usize) {
    let dir = scratch_dir(&protocol.to_string());
    let input = dir.join("source.bin");
    let output = dir.join("received.bin");
    let original = write_source(&input, protocol);

    let (ready_tx, ready_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();

    let recv_config = config_for(protocol, window);
    let output_path = output.clone();
    let receiver = thread::spawn(move || {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("bind receiver");
        let addr = socket.local_addr().expect("local addr");
        ready_tx.send(addr).expect("send ready");
        let result = recv_file_on_socket(socket, output_path, &recv_config);
        done_tx.send(result).expect("send done");
    });

    let peer = ready_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("receiver did not bind in time");

    let send_config = config_for(protocol, window);
    let send_started = Instant::now();
    let send_result = send_file(&input, peer, &send_config).expect("sender failed");
    assert!(
        send_started.elapsed() < Duration::from_secs(15),
        "sender exceeded hard deadline"
    );

    let recv_result = done_rx
        .recv_timeout(Duration::from_secs(15))
        .expect("receiver did not finish in time")
        .expect("receiver failed");

    receiver.join().expect("receiver thread panicked");

    let reconstructed = std::fs::read(&output).expect("read output");
    assert_eq!(
        reconstructed, original,
        "{protocol}: output bytes must equal input bytes"
    );
    assert_eq!(
        send_result.source_sha256, recv_result.delivered_sha256,
        "{protocol}: SHA-256 must match"
    );
    assert_eq!(recv_result.bytes_written, original.len());
    assert_eq!(send_result.bytes_sent, original.len());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn udp_loopback_stop_and_wait() {
    run_loopback(ProtocolType::StopAndWait, 1);
}

#[test]
fn udp_loopback_go_back_n() {
    run_loopback(ProtocolType::GoBackN, 8);
}

#[test]
fn udp_loopback_selective_repeat() {
    run_loopback(ProtocolType::SelectiveRepeat, 8);
}

#[test]
fn udp_loopback_empty_file_stop_and_wait() {
    let dir = scratch_dir("empty");
    let input = dir.join("empty.bin");
    let output = dir.join("out.bin");
    std::fs::File::create(&input).unwrap();

    let (ready_tx, ready_rx) = mpsc::channel();
    let config = config_for(ProtocolType::StopAndWait, 1);
    let recv_config = config.clone();
    let output_path = output.clone();
    let receiver = thread::spawn(move || {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = socket.local_addr().unwrap();
        ready_tx.send(addr).unwrap();
        recv_file_on_socket(socket, output_path, &recv_config)
    });

    let peer = ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let send_result = send_file(&input, peer, &config).expect("empty send");
    let recv_result = receiver.join().unwrap().expect("empty recv");

    assert_eq!(send_result.bytes_sent, 0);
    assert_eq!(recv_result.bytes_written, 0);
    assert_eq!(send_result.source_sha256, recv_result.delivered_sha256);
    assert_eq!(std::fs::read(&output).unwrap(), b"");
    let _ = std::fs::remove_dir_all(&dir);
}
