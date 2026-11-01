# Proposal Compliance Audit

**Status date:** Window and loss studies added after the frozen E1 and E4 datasets. Those two datasets were not regenerated.  
**Rule:** Every row points at repository evidence. Status is not granted merely because a file exists.

| Requirement | Status | Evidence |
|---|---|---|
| Runtime-selectable Stop-and-Wait | COMPLETE | `src/arq/sw.rs`; CLI `--protocol StopAndWait`; tests in `tests/test_arq.rs`, `tests/test_transfer.rs`, `tests/test_udp.rs` |
| Runtime-selectable Go-Back-N | COMPLETE | `src/arq/gbn.rs`; CLI `--protocol GoBackN`; same test suites |
| Runtime-selectable Selective Repeat | COMPLETE | `src/arq/sr.rs`; CLI `--protocol SelectiveRepeat`; same test suites |
| Common packet framing (seq, type/flag, length, checksum) | COMPLETE | `src/packet.rs` (`HEADER_SIZE = 10`); `docs/PACKET_FORMAT.md`; `tests/test_packet.rs` |
| File chunking and reassembly | COMPLETE | `src/app/file_io.rs` (`Chunker`, `Reassembler`); unit tests in that module |
| Acknowledgements | COMPLETE | ARQ `handle_ack` / receiver ACK emission in `src/arq/{sw,gbn,sr}.rs` |
| Duplicate handling | COMPLETE | Duplicate DATA/ACK paths in ARQ receivers/senders; covered by ARQ tests |
| In-order delivery to application | COMPLETE | Receivers deliver in order; `Reassembler` enforces consecutive seq; transfer + UDP paths absorb drained chunks sequentially |
| Adaptive Jacobson/Karels RTO (SRTT, RTTVAR) | COMPLETE | `src/timing/rto.rs`; `tests/test_timing.rs` |
| Exponential backoff | COMPLETE | `RtoEstimator::on_timeout` in `src/timing/rto.rs`; timing tests |
| Karn's algorithm | COMPLETE | `RtoEstimator::update_rtt(..., is_retransmission)`; timing tests |
| Deterministic seeded channel emulator (loss, dup, reorder, corrupt, delay, jitter) | COMPLETE | `src/channel/mod.rs`, `src/channel/scheduler.rs`; channel unit tests; wired only into virtual-time `run_transfer` |
| SHA-256 end-to-end integrity | COMPLETE | `src/app/integrity.rs`; recorded in every `ExperimentRecord`; UDP recv verifies written digest |
| Rust-generated JSON/CSV metrics | COMPLETE | `src/metrics/collector.rs`; `run-experiment --json-out` / `--csv-out` |
| Python experiment automation | COMPLETE | `experiments/python/run_experiments.py` (`--study primary\|rto\|window\|loss`) |
| Statistical analysis + 95% Student-t CIs | COMPLETE | `experiments/python/plot_results.py`, `interpret_results.py`, `analyze_rto_sensitivity.py`, `analyze_catalogue_sweeps.py`; outputs under `results/analysis/`, `results/rto_sensitivity/analysis/`, `results/window_goodput/analysis/`, and `results/loss_goodput/analysis/` |
| Primary reordering study (E1) | COMPLETE — EXPERIMENTAL | Frozen 90-trial dataset `results/raw/experiments_raw.json` (SHA-256 `d93dc2b18b49775c35587836e042a9daf6dfc844e85427a6be927180c6f0e8be`); findings in `results/analysis/research_findings.md` |
| RTO sensitivity study (E4) | COMPLETE — EXPERIMENTAL | Separate 45-trial dataset under `results/rto_sensitivity/`; findings in `results/rto_sensitivity/analysis/findings.md` |
| Real UDP socket file-transfer demo | COMPLETE | `src/app/udp.rs`; CLI `send` / `recv` in `src/main.rs`; `tests/test_udp.rs` (SW, GBN, SR, empty file) |
| Window-size goodput study | COMPLETE — EXPERIMENTAL | 65 trials under `results/window_goodput/` (JSON SHA-256 `34a84ef13f7c059a437fead2e3f0cbd0906756fdd525d29b6250bdfcbff526b4`). Loss 0, reorder 0, windows 1/2/4/8/16/32 for GBN and SR, Stop-and-Wait only at window 1. Findings in `results/window_goodput/analysis/findings.md` |
| Loss-rate goodput study | COMPLETE — EXPERIMENTAL | 90 trials under `results/loss_goodput/` (JSON SHA-256 `b4e663644dbc4fab426322150de9755461ef0586cd4cbac9515e0ade915c526f`). Loss 0–0.25, reorder 0, window 8 (Stop-and-Wait window 1). Findings in `results/loss_goodput/analysis/findings.md` |
| Channel emulator on the real UDP path | DOCUMENTED LIMITATION | Emulator is experiment-only (`run_transfer`). Real UDP uses clean localhost sockets without the emulator |
| WAN / multi-host deployment | DOCUMENTED LIMITATION | Real UDP path is implemented and tested for localhost demonstration |
| Compare socket wall-clock to virtual-time durations | DOCUMENTED LIMITATION | Explicitly forbidden; clocks measure different things |

## Summary

The proposal’s core transport, emulator, primary reordering experiment, secondary RTO study, window-size study, loss-rate study, and localhost UDP demonstration are present with evidence. The window and loss datasets are separate from `results/raw/` and `results/rto_sensitivity/`. Identical-configuration cells (loss 0, reorder 0, primary window) matched the frozen reorder-0 records on the fields checked in `regression_vs_primary.csv`.
