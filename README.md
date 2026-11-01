# Reliable Data Transfer over UDP

**CS-30003 · Coding Assignment 1 · Path A (catalogue) · P3 — Section CSE 33**

| | |
|---|---|
| **Team** | Ashwika Burman · Sahan Maiti · Soumyadeb Mukherjee · Kashish Gupta |
| **Stack** | Rust (transport) · Python 3 (experiments/analysis only) |
| **Status** | Feature-complete for submission (docs + frozen experiments) |

### Team contributions

Responsibility areas (not a claim that every listed task was completed solely by one person):

- **Soumyadeb Mukherjee (24052329)** — Protocol & ARQ: packet semantics, Stop-and-Wait, Go-Back-N, Selective Repeat, and protocol-state testing.
- **Kashish Gupta (24052495)** — Timing & RTO: retransmission timers, Jacobson/Karels estimation, SRTT, RTTVAR, exponential backoff, Karn's algorithm, and timing tests.
- **Ashwika Burman (24155095)** — Channel Emulator: deterministic fault injection, loss, duplication, reordering, corruption, delay/jitter, seeded replay, and emulator testing.
- **Sahan Maiti (24052247)** — Evaluation & Integration: file integrity, metrics, experiment automation, raw results, plots, integration, and end-to-end validation.

AI assistance is disclosed in [`AI-USE.md`](AI-USE.md).

---

## Overview

UDP provides datagram delivery without reliability. This project builds a **reliable file-transfer layer** with three **runtime-selectable** ARQ protocols:

- **Stop-and-Wait** — one outstanding packet
- **Go-Back-N** — cumulative ACKs; discard early packets; window retransmit on timeout
- **Selective Repeat** — individual ACKs; buffer in-window out-of-order data; selective retransmit

Shared pieces: packet framing (seq / type / flags / length / RFC 1071 checksum), chunking/reassembly, Jacobson/Karels adaptive RTO (SRTT, RTTVAR, Karn, backoff), SHA-256 integrity, and a **deterministic seeded channel emulator** for controlled experiments.

Two paths must stay conceptually separate:

1. **Deterministic experiment path** — virtual-time driver + channel emulator → JSON/CSV science data (E1, E4).
2. **Real UDP path** — localhost `UdpSocket` send/recv using the same ARQ machines (demo; no emulator).

Python never implements the transport; it only runs matrices and analyses Rust records.

---

## Architecture

```text
Application / File
       ↓
   Chunking
       ↓
  ARQ Protocol  ←—— timers / adaptive RTO
       ↓
 Packet Framing
       ↓
 ┌─────┴──────────────────────┐
 │                            │
 UDP sockets (demo)     Channel emulator (experiments)
 localhost only         seeded loss/dup/reorder/corrupt/delay
 │                            │
 └─────┬──────────────────────┘
       ↓
 Packet Parsing
       ↓
 ARQ Receiver
       ↓
 Reassembly
       ↓
 SHA-256 Verification
```

| Path | Entry | Emulator? | Clock |
|---|---|---|---|
| Experiment | `run-experiment` / `src/app/transfer.rs` | Yes | Virtual time |
| Demo | `send` / `recv` / `src/app/udp.rs` | No | Wall clock |

Do **not** compare socket durations with virtual-time `duration_secs`.

---

## Requirements

Verified during development/analysis:

- **Rust** 1.99.0 (Cargo/clippy as needed)
- **Python** 3.12.x (stdlib + scripts under `experiments/python/`; no extra pip deps required for the core runners)
- macOS/Linux-style environment with localhost UDP

---

## Build

```bash
cargo build --release
```

---

## Test

```bash
cargo test
```

Includes packet/ARQ/timing/channel unit tests, virtual-time transfer tests, and real UDP loopback for Stop-and-Wait, Go-Back-N, Selective Repeat, plus an empty-file case (`tests/test_udp.rs`).

---

## Real UDP Demo

Receiver:

```bash
cargo run --release -- recv \
  --bind 127.0.0.1:19001 \
  --output /tmp/rdt_out.bin \
  --protocol GoBackN \
  --window 8 \
  --chunk-size 1400
```

Sender:

```bash
cargo run --release -- send \
  --file fixtures/transfer_medium.bin \
  --to 127.0.0.1:19001 \
  --protocol GoBackN \
  --window 8 \
  --chunk-size 1400
```

Use matching `--protocol` on both sides (`StopAndWait`, `GoBackN`, or `SelectiveRepeat`).  
Optional sender RTO/deadline flags: `--rto-multiplier`, `--min-rto-ms`, `--max-rto-ms`, `--initial-rto-ms`, `--deadline-secs`.

Integrity check:

```bash
cargo run --release -- verify \
  --source fixtures/transfer_medium.bin \
  --received /tmp/rdt_out.bin
```

Step-by-step talk track: [`docs/DEMO.md`](docs/DEMO.md).

---

## Deterministic Experiments

`--study` is **required**. A bare runner invocation does nothing (protects frozen data).

### E1 — Primary Reordering Study (completed, frozen)

3 protocols × 6 reorder rates × 5 seeds = **90 trials**.  
Loss/dup/corrupt = 0; delay 20 ms; jitter 0; reorder hold 50 ms; chunk 1400; SW window 1; GBN/SR window 8; RTO multiplier 1.0; min RTO 200 ms.

```bash
python3 experiments/python/run_experiments.py --study primary
```

**Do not re-run for submission QA.** Dataset:

- `results/raw/experiments_raw.json`
- SHA-256 `d93dc2b18b49775c35587836e042a9daf6dfc844e85427a6be927180c6f0e8be`

### E4 — RTO Sensitivity Study (completed, separate)

3 protocols × 3 multipliers `{0.5,1.0,3.0}` × 5 seeds = **45 trials**.  
Loss = 0.05; reorder = 0; min RTO **10 ms (E4 only)**. Writes only under `results/rto_sensitivity/`.

```bash
python3 experiments/python/run_experiments.py --study rto
```

### Window-size goodput study (completed, separate)

65 trials under `results/window_goodput/`. Loss and reorder are 0. Go-Back-N and Selective Repeat use windows 1, 2, 4, 8, 16, and 32. Stop-and-Wait runs only at window 1. Same file, seeds, and 200 ms RTO floor as E1.

```bash
python3 experiments/python/run_experiments.py --study window
```

JSON SHA-256 `34a84ef13f7c059a437fead2e3f0cbd0906756fdd525d29b6250bdfcbff526b4`.

### Loss-rate goodput study (completed, separate)

90 trials under `results/loss_goodput/`. Loss is 0 through 0.25 in steps of 0.05. Reorder is 0. Window is 8 (Stop-and-Wait stays at 1). Same RTO floor as E1.

```bash
python3 experiments/python/run_experiments.py --study loss
```

JSON SHA-256 `b4e663644dbc4fab426322150de9755461ef0586cd4cbac9515e0ade915c526f`.

Analysis (reads existing JSON; does not invent metrics):

```bash
python3 experiments/python/plot_results.py \
  --input results/raw/experiments_raw.json \
  --output-dir results/analysis
python3 experiments/python/interpret_results.py
python3 experiments/python/analyze_rto_sensitivity.py
python3 experiments/python/analyze_catalogue_sweeps.py --study window
python3 experiments/python/analyze_catalogue_sweeps.py --study loss
```

Details: [`docs/REPRODUCIBILITY.md`](docs/REPRODUCIBILITY.md).

---

## Results (summary)

Source of truth: generated analysis files, not this paragraph alone.

**E1 primary claim (under tested zero-loss reordering):** Go-Back-N retransmissions and virtual duration rise with configured reorder; Selective Repeat recorded **zero** data retransmissions across the grid and stayed under ~1 s virtual duration; Stop-and-Wait also avoided retransmissions but with ~4–5 s duration and lower goodput. All 90 trials SUCCESS with matching SHA-256. See `results/analysis/research_findings.md` and `primary_comparison.txt`.

**E4:** Premature retransmissions appear at **0.5×** only; **3.0×** tends to lengthen recovery versus **1.0×** under 5% loss. See `results/rto_sensitivity/analysis/findings.md`.

Academic write-up: [`docs/FINAL_REPORT.md`](docs/FINAL_REPORT.md).  
Proposal mapping: [`docs/PROPOSAL_COMPLIANCE.md`](docs/PROPOSAL_COMPLIANCE.md).

---

## Reproducibility

| Item | Location / value |
|---|---|
| Fixture | `fixtures/transfer_medium.bin` (140,000 bytes; SHA-256 `8e6c7049a65aeff8211b2bbd78db47683a1b86f75565ad3362fb6e53a0a4cbbf`) |
| E1 seeds | 101, 202, 303, 404, 505 |
| E1 reorder grid | 0.00 … 0.25 step 0.05 |
| E4 multipliers | 0.5, 1.0, 3.0 |
| E1 hash | `d93dc2b18b49775c35587836e042a9daf6dfc844e85427a6be927180c6f0e8be` |
| Analysis method | Student-t 95% CI, n=5, df=4 |

Provenance snapshot: `results/analysis/provenance.md`.

---

## Project Structure

```text
src/
  packet.rs          framing + checksum
  arq/               SW, GBN, SR
  timing/            RTO + timers
  channel/           deterministic emulator
  app/               file I/O, integrity, transfer (virtual), udp (sockets)
  metrics/           ExperimentRecord JSON/CSV
  main.rs            CLI
experiments/python/  runners + analysis
tests/               Rust tests including test_udp.rs
fixtures/            transfer_medium.bin
results/raw/         frozen E1
results/rto_sensitivity/  E4
results/window_goodput/   window-size study
results/loss_goodput/     loss-rate study
results/analysis/    E1 analysis
docs/                report, compliance, demo, viva, reproducibility
AI-USE.md
```

---

## Limitations

- E1/E4 use **virtual time**; not a WAN deployment.
- Real UDP path is **localhost-focused**.
- Socket wall-clock ≠ virtual-time experiment durations.
- E1 uses **loss = 0** to isolate reordering; E4 uses **loss = 0.05**, reorder = 0, and a lower min RTO.
- Five seeds per cell; one primary fixture/chunk size. E1 and E4 keep windows at 1/8/8. Window size and loss rate were swept in separate studies, not crossed.
- E1 does **not** vary the RTO multiplier (no causal RTO claim from E1 alone).
- Old synthetic pilot under `results/raw/archive/synthetic_pilot/` is **not** evidence.

---

## Documentation Index

| Doc | Purpose |
|---|---|
| [`docs/FINAL_REPORT.md`](docs/FINAL_REPORT.md) | Academic report |
| [`docs/PROPOSAL_COMPLIANCE.md`](docs/PROPOSAL_COMPLIANCE.md) | Requirement → evidence table |
| [`docs/REPRODUCIBILITY.md`](docs/REPRODUCIBILITY.md) | Exact commands |
| [`docs/DEMO.md`](docs/DEMO.md) | 3–5 minute demo |
| [`docs/VIVA_NOTES.md`](docs/VIVA_NOTES.md) | Oral defence notes |
| [`docs/SUBMISSION_CHECKLIST.md`](docs/SUBMISSION_CHECKLIST.md) | Hand-in checklist |
| [`AI-USE.md`](AI-USE.md) | AI assistance statement |
| [`docs/PACKET_FORMAT.md`](docs/PACKET_FORMAT.md) | Wire format |

---

## License

Project coursework materials; see repository policy / course rules for distribution.
