// CLI coverage for `run-experiment`. Each run executes the real virtual-time driver.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_reliable_udp")
}

fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rdt-cli-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write input");
    path
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("spawn reliable_udp")
}

fn sample(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn clean_args(file: &Path, protocol: &str, window: &str, json: &Path, csv: &Path) -> Vec<String> {
    vec![
        "run-experiment".to_string(),
        "--file".to_string(),
        file.display().to_string(),
        "--protocol".to_string(),
        protocol.to_string(),
        "--window".to_string(),
        window.to_string(),
        "--chunk-size".to_string(),
        "16".to_string(),
        "--seed".to_string(),
        "11".to_string(),
        "--trial-id".to_string(),
        "3".to_string(),
        "--experiment-id".to_string(),
        format!("cli-{protocol}"),
        "--loss".to_string(),
        "0".to_string(),
        "--reorder".to_string(),
        "0".to_string(),
        "--duplicate".to_string(),
        "0".to_string(),
        "--corrupt".to_string(),
        "0".to_string(),
        "--json-out".to_string(),
        json.display().to_string(),
        "--csv-out".to_string(),
        csv.display().to_string(),
    ]
}

fn assert_clean_success(protocol: &str, window: &str) {
    let dir = workspace(protocol);
    let input = write_file(&dir, "input.bin", &sample(64));
    let json = dir.join("out.json");
    let csv = dir.join("out.csv");
    let args = clean_args(&input, protocol, window, &json, &csv);
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = run(&arg_refs);
    assert!(
        output.status.success(),
        "{protocol} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("Protocol: {protocol}")));
    assert!(stdout.contains("SHA-256: MATCH"));
    assert!(stdout.contains("Status: SUCCESS"));
    assert!(stdout.contains("Retransmissions: 0"));

    let json_text = fs::read_to_string(&json).expect("json");
    assert!(json_text.contains(&format!("\"protocol\": \"{protocol}\"")));
    assert!(json_text.contains("\"transfer_status\": \"SUCCESS\""));
    assert!(json_text.contains("\"sha256_match\": true"));
    assert!(json_text.contains("\"source_bytes\": 64"));
    assert!(json_text.contains("\"chunk_size\": 16"));
    assert!(json_text.contains("\"reverse_seed\": 12"));
    assert!(json_text.contains("\"rto_multiplier\": 1.0"));
    assert!(json_text.contains("\"premature_retransmissions\": 0"));
    assert!(json_text.contains("\"alpha\": 0.125"));
    assert!(json_text.contains("\"configured_duplicate_rate\": 0.0"));
    assert!(json_text.contains("\"karn_rejected_count\""));
    assert!(json_text.contains("\"delivered_unique_bytes\": 64"));
    assert!(json_text.contains(&format!("\"experiment_id\": \"cli-{protocol}\"")));
    assert!(json_text.contains("\"seed\": 11"));
    assert!(json_text.contains("\"trial_id\": 3"));
    assert!(json_text.contains("\"data_retransmissions\": 0"));
    assert!(json_text.contains("\"timeout_count\": 0"));
    assert!(json_text.contains("\"final_rto_ms\""));
    assert!(json_text.contains("\"mean_rtt_ms\""));
    assert!(json_text.contains("\"dropped_datagrams\""));

    let csv_text = fs::read_to_string(&csv).expect("csv");
    assert!(csv_text.starts_with(
        "experiment_id,protocol,window_size,chunk_size,segment_count,seed,reverse_seed,trial_id,"
    ));
    assert!(csv_text.contains(&format!("cli-{protocol},{protocol}")));
    assert!(csv_text.contains(",true,"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_experiment_stop_and_wait_writes_json_and_csv() {
    assert_clean_success("StopAndWait", "1");
}

#[test]
fn run_experiment_go_back_n_writes_json_and_csv() {
    assert_clean_success("GoBackN", "4");
}

#[test]
fn run_experiment_selective_repeat_writes_json_and_csv() {
    assert_clean_success("SelectiveRepeat", "4");
}

#[test]
fn invalid_protocol_fails() {
    let dir = workspace("bad-protocol");
    let input = write_file(&dir, "input.bin", b"abc");
    let output = run(&[
        "run-experiment",
        "--file",
        input.to_str().unwrap(),
        "--protocol",
        "NotAProtocol",
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown ARQ protocol") || stderr.contains("invalid value"),
        "{stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_fails() {
    let missing =
        std::env::temp_dir().join(format!("rdt-missing-{}-no-such.bin", std::process::id()));
    let _ = fs::remove_file(&missing);
    let output = run(&[
        "run-experiment",
        "--file",
        missing.to_str().unwrap(),
        "--protocol",
        "StopAndWait",
        "--window",
        "1",
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("input file"), "{stderr}");
}

#[test]
fn timeout_exits_nonzero() {
    let dir = workspace("timeout");
    let input = write_file(&dir, "input.bin", b"lost");
    let json = dir.join("timeout.json");
    let output = run(&[
        "run-experiment",
        "--file",
        input.to_str().unwrap(),
        "--protocol",
        "StopAndWait",
        "--window",
        "1",
        "--chunk-size",
        "16",
        "--seed",
        "5",
        "--loss",
        "1",
        "--json-out",
        json.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Status: TIMEOUT"), "{stdout}");
    let json_text = fs::read_to_string(&json).expect("timeout json");
    assert!(json_text.contains("\"transfer_status\": \"TIMEOUT\""));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn same_command_produces_the_same_record() {
    let dir = workspace("deterministic");
    let input = write_file(&dir, "input.bin", &sample(80));
    let first = dir.join("first.json");
    let second = dir.join("second.json");
    let base = [
        "run-experiment",
        "--file",
        input.to_str().unwrap(),
        "--protocol",
        "go-back-n",
        "--window",
        "4",
        "--chunk-size",
        "10",
        "--seed",
        "99",
        "--trial-id",
        "2",
        "--loss",
        "0",
        "--reorder",
        "0.1",
        "--reorder-extra-ms",
        "50",
        "--min-rto-ms",
        "200",
    ];
    let mut first_args = base.to_vec();
    first_args.extend(["--json-out", first.to_str().unwrap()]);
    let mut second_args = base.to_vec();
    second_args.extend(["--json-out", second.to_str().unwrap()]);

    let first_run = run(&first_args);
    let second_run = run(&second_args);
    assert!(
        first_run.status.success(),
        "{}",
        String::from_utf8_lossy(&first_run.stderr)
    );
    assert!(
        second_run.status.success(),
        "{}",
        String::from_utf8_lossy(&second_run.stderr)
    );

    let first_json = fs::read_to_string(&first).unwrap();
    let second_json = fs::read_to_string(&second).unwrap();
    assert_eq!(first_json, second_json);
    assert!(first_json.contains("\"protocol\": \"GoBackN\""));
    assert!(first_json.contains("\"sha256_match\": true"));
    assert!(first_json.contains("GoBackN_input.bin_"));
    let _ = fs::remove_dir_all(&dir);
}
