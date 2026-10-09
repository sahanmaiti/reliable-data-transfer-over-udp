# Reliable Data Transfer over UDP — Final Report

**CS-30003 · Coding Assignment 1 · Path A (catalogue) · P3**

### Team contributions

Responsibility areas (not a claim that every listed task was completed solely by one person):

- **Soumyadeb Mukherjee (24052329)** — Protocol & ARQ: packet semantics, Stop-and-Wait, Go-Back-N, Selective Repeat, and protocol-state testing.
- **Kashish Gupta (24052495)** — Timing & RTO: retransmission timers, Jacobson/Karels estimation, SRTT, RTTVAR, exponential backoff, Karn's algorithm, and timing tests.
- **Ashwika Burman (24155095)** — Channel Emulator: deterministic fault injection, loss, duplication, reordering, corruption, delay/jitter, seeded replay, and emulator testing.
- **Sahan Maiti (24052247)** — Evaluation & Integration: file integrity, metrics, experiment automation, raw results, plots, integration, and end-to-end validation.

AI assistance is disclosed in `AI-USE.md`.

This report describes the **implemented** system and the experiments that were actually run. Numbers are taken from generated analysis files, not invented.

Primary evidence:

- `results/raw/experiments_raw.json` (90 trials; SHA-256 `d93dc2b18b49775c35587836e042a9daf6dfc844e85427a6be927180c6f0e8be`)
- `results/analysis/research_findings.md`, `primary_comparison.txt`, `primary_results.csv`, `protocol_comparisons.csv`
- `results/rto_sensitivity/` (45 trials) and `results/rto_sensitivity/analysis/findings.md`
- `results/window_goodput/` (65 trials; JSON SHA-256 `34a84ef13f7c059a437fead2e3f0cbd0906756fdd525d29b6250bdfcbff526b4`) and `results/window_goodput/analysis/findings.md`
- `results/loss_goodput/` (90 trials; JSON SHA-256 `b4e663644dbc4fab426322150de9755461ef0586cd4cbac9515e0ade915c526f`) and `results/loss_goodput/analysis/findings.md`

The archived synthetic pilot under `results/raw/archive/synthetic_pilot/` is **not** evidence.

---

## 1. Abstract

UDP does not recover lost, reordered, duplicated, or corrupted datagrams. This project implements three runtime-selectable ARQ protocols—Stop-and-Wait, Go-Back-N, and Selective Repeat—on a shared packet format, with Jacobson/Karels RTO estimation and a seeded channel emulator. Scientific comparisons use a deterministic **virtual-time** driver. A separate **real UDP** localhost path demonstrates socket-level file transfer with the same ARQ state machines.

Under the frozen 90-trial zero-loss reordering study (E1), Stop-and-Wait and Selective Repeat recorded zero data retransmissions at every configured reorder rate. Selective Repeat kept virtual completion under 1 s; Stop-and-Wait stayed near 4–5 s. Go-Back-N matched Selective Repeat at 0% reorder, then showed increasing timeout-driven retransmissions and much longer durations as reorder rose. A separate 45-trial RTO sensitivity study (E4) under 5% loss showed premature retransmissions concentrated at a 0.5× RTO multiplier, with longer recovery tending to appear at 3.0× relative to 1.0×. A 65-trial window study at zero loss showed goodput rising with window size. A 90-trial loss study at zero reorder showed Go-Back-N retransmitting more than Selective Repeat once loss was above zero. Claims are scoped to these configurations.

---

## 2. Problem Statement

A file transfer is useful only if the receiver reconstructs the sender’s bytes. UDP provides datagram delivery without that guarantee. The assignment asks how textbook ARQ strategies and adaptive timeouts behave under controlled impairments, and whether a reliable layer can also operate over real UDP sockets.

---

## 3. Objectives

Mapped to the proposal and to what was delivered:

1. Implement SW / GBN / SR with shared framing, ACKs, duplicates handling, and in-order delivery.
2. Implement adaptive Jacobson/Karels RTO with Karn’s rule and exponential backoff.
3. Provide a deterministic seeded channel emulator for controlled experiments.
4. Verify end-to-end integrity with SHA-256.
5. Automate experiments in Python over Rust-produced JSON/CSV.
6. Run the primary reordering study, the secondary RTO sensitivity study, the window-size goodput study, and the loss-rate goodput study.
7. Demonstrate a real UDP file-transfer path (localhost).

---

## 4. System Architecture

Two paths share ARQ, packet, timing, chunking, and integrity code. They must not be conflated.

### Deterministic experiment path (scientific measurements)

```text
file → Chunker → ARQ sender → forward Channel + scheduler
     → ARQ receiver → reverse Channel → ACK → RTO update
     → Reassembler → SHA-256 → ExperimentRecord (JSON/CSV)
```

Implemented in `src/app/transfer.rs`, invoked by `reliable_udp run-experiment`.

### Real UDP path (demonstration)

```text
file → Chunker → ARQ sender → Packet serialize → UdpSocket → localhost
localhost → UdpSocket → deserialize → ARQ receiver → Reassembler → SHA-256
```

Implemented in `src/app/udp.rs`, invoked by `reliable_udp send` / `recv`.  
**The channel emulator is not attached to this path.**

---

## 5. ARQ Protocols

### Stop-and-Wait

Window of one. Sender waits for the matching ACK before sending the next DATA. Timeout retransmits the single outstanding packet (subject to a retry ceiling on the socket path).

### Go-Back-N

Fixed sender window; cumulative ACKs. Receiver accepts only the next expected sequence and discards later packets. On timeout, the sender retransmits the entire outstanding window.

### Selective Repeat

Sender and receiver windows; individual ACKs. The receiver may buffer early packets inside the window. On timeout, only the timed-out sequence is retransmitted.

---

## 6. Packet Format

From `src/packet.rs` (10-byte header + payload):

| Field | Size | Notes |
|---|---|---|
| `seq_num` | 4 B | DATA sequence or ACK number |
| `pkt_type` | 1 B | DATA `0x01`, ACK `0x02`, FIN `0x03` |
| `flags` | 1 B | bit 0 = retransmitted |
| `payload_len` | 2 B | big-endian |
| `checksum` | 2 B | RFC 1071 Internet checksum over header+payload |
| payload | ≤ 1400 B | `MAX_PAYLOAD_SIZE` |

Further detail: `docs/PACKET_FORMAT.md`.

---

## 7. Adaptive RTO

`src/timing/rto.rs` implements Jacobson/Karels / RFC 6298-style estimation:

- First accepted RTT sample initializes SRTT and RTTVAR.
- Later samples smooth SRTT (`alpha`) and RTTVAR (`beta`).
- RTO ≈ SRTT + K·RTTVAR, scaled by `rto_multiplier`, clamped to `[min_rto, max_rto]`.
- Karn’s algorithm rejects RTT samples for retransmitted segments.
- Timeout applies exponential backoff (doubling) up to `max_rto`.

Timers: `RetransmissionTimer` (SW/GBN) and `MultiTimer` (SR) in `src/timing/timer.rs`.

---

## 8. Deterministic Channel Emulator

`src/channel/mod.rs` applies seeded Bernoulli impairments and delay:

- loss, duplication, reordering (with extra hold), corruption (single-bit flip), base delay, jitter.

Forward and reverse paths use related seeds in the experiment driver. Determinism enables matched seeds across protocols so differences are attributable to protocol behaviour under the same impairment draws—not to run-to-run network noise.

---

## 9. Real UDP Implementation

`src/app/udp.rs`:

- Sender fills the ARQ window, sends serialized datagrams, samples RTT into `RtoEstimator`, retransmits on wall-clock timeouts, then sends FIN and waits for FIN ACK.
- Receiver deserializes datagrams, passes them to the ARQ receiver, writes ACKs, absorbs delivered chunks, and on FIN writes the file and verifies SHA-256.
- Hard transfer deadline and FIN retry ceiling prevent indefinite hangs.
- Tested for SW, GBN, and SR on localhost (`tests/test_udp.rs`).

Socket wall-clock timings must **not** be compared with virtual-time `duration_secs` from E1/E4.

---

## 10. Experimental Methodology

### E1 — Primary Reordering Study

**Hypothesis:** Under identical controlled conditions with increasing packet reordering and **zero loss**, Go-Back-N will retransmit more than Selective Repeat because GBN discards early packets and recovers by window retransmission after timeout, while SR can accept and ACK out-of-order packets inside its window.

**Actual configuration (executed and frozen):**

| Factor | Value |
|---|---|
| Protocols | StopAndWait, GoBackN, SelectiveRepeat |
| Reorder | 0.00, 0.05, 0.10, 0.15, 0.20, 0.25 |
| Seeds | 101, 202, 303, 404, 505 |
| Loss / dup / corrupt | 0 |
| Base delay / jitter / reorder hold | 20 ms / 0 / 50 ms |
| File | `fixtures/transfer_medium.bin` (140,000 bytes) |
| Chunk | 1,400 bytes |
| Windows | SW=1; GBN/SR=8 |
| RTO | multiplier 1.0; min 200 ms; initial 1000 ms; max 60000 ms |
| Trials | 3 × 6 × 5 = **90** |

Independent variable: configured forward reorder rate (and protocol). Constants include zero loss and fixed RTO bounds.

### E4 — RTO Sensitivity Study

**Purpose:** Scale the adaptive RTO (0.5× / 1.0× / 3.0×) under fixed loss so premature retransmission versus delayed recovery can be observed. α/β/k remain Rust defaults.

**Actual configuration (executed, separate directory):**

| Factor | Value |
|---|---|
| Protocols | StopAndWait, GoBackN, SelectiveRepeat |
| RTO multipliers | 0.5, 1.0, 3.0 |
| Seeds | 101, 202, 303, 404, 505 |
| Loss | 0.05 |
| Reorder / dup / corrupt | 0 |
| Delay / jitter / hold | 20 ms / 0 / 50 ms |
| File / chunk / windows | same as E1 |
| RTO | min **10 ms (E4 only)**; initial 1000 ms; max 60000 ms |
| Trials | 3 × 3 × 5 = **45** |

E4 never writes under `results/raw/`.

### Window-size goodput study

Loss and reorder are 0. Go-Back-N and Selective Repeat use windows 1, 2, 4, 8, 16, and 32. Stop-and-Wait runs only at window 1. Five seeds. **65** trials, written under `results/window_goodput/`. RTO bounds match E1, including the 200 ms floor.

### Loss-rate goodput study

Reorder is 0. Loss is 0, 0.05, 0.10, 0.15, 0.20, and 0.25. Window is 8, except Stop-and-Wait at window 1. Five seeds. **90** trials, written under `results/loss_goodput/`. RTO bounds match E1. This is not a loss × reorder grid.

---

## 11. Results

### E1 (from `results/analysis/`)

All 90 trials: `SUCCESS`, SHA-256 match, 140,000 bytes delivered. Cell means with 95% Student-t CIs are in `primary_comparison.txt` / `primary_results.csv`.

Highlights under the tested zero-loss schedule:

- **Stop-and-Wait:** 0 data retransmissions at every reorder rate; duration ≈ 3.98 s → 5.17 s from reorder 0 → 0.25; goodput falls accordingly.
- **Selective Repeat:** 0 data retransmissions at every rate; duration ≈ 0.50 s → 0.918 s; goodput remains far above Stop-and-Wait.
- **Go-Back-N:** matches SR at reorder 0; at nonzero reorder, mean data retransmissions rise (≈ 30.8 → 164.8 from 0.05 → 0.25), with `timeout_retransmissions == data_retransmissions` and premature count 0; duration and final RTO spread sharply at higher reorder, with wide intervals (n = 5).

Paired GBN − SR retransmission intervals lie above 0 at every nonzero reorder rate in this matrix. Exploratory paired t-tests in `protocol_comparisons.csv` are uncorrected.

### Window size (from `results/window_goodput/analysis/`)

All 65 trials succeeded and the digests matched. At this zero-loss setting, data retransmissions were 0 in every cell. Mean goodput rose with window: about 3.52×10^4 bytes/s at window 1, 2.80×10^5 at window 8, and 1.00×10^6 at window 32. Paired Go-Back-N minus Selective Repeat differences for goodput and retransmissions were 0 at every window in this matrix. That agreement is an observation under zero loss, not a requirement. The window-8 cells matched the frozen E1 reorder-0 records on the fields in `regression_vs_primary.csv`.

### Loss rate (from `results/loss_goodput/analysis/`)

All 90 trials succeeded and the digests matched. At loss 0 the three protocols matched the frozen E1 reorder-0 cells. At every nonzero loss rate, the paired Go-Back-N minus Selective Repeat data-retransmission interval lies above 0 (mean differences about 22.4, 73.2, 111.2, 140.2, and 183.4 from 0.05 to 0.25). The paired goodput difference lies entirely below 0 at 0.15 and 0.20. At 0.05, 0.10, and 0.25 that goodput interval includes 0. Several duration intervals are wide enough to include 0 even though every recorded duration is positive. n = 5.

### E4 (from `results/rto_sensitivity/analysis/`)

All 45 trials succeeded. Premature retransmissions were nonzero only at 0.5× (means ≈ 45 / 27.2 / 40.4 for SW / GBN / SR) and exactly 0 at 1.0× and 3.0× for all seeds. Relative to 1.0×, 3.0× tended to lengthen duration (especially GBN). Selective Repeat’s mean final RTO at 0.5× exceeded 1.0×, consistent with backoff after aggressive timeouts—not a pure scaled SRTT snapshot.

---

## 12. Primary Finding

**Under the tested E1 conditions** (virtual time, zero loss, fixed 20 ms delay, 50 ms reorder hold, 200 ms RTO floor, one 140 KB fixture, five seeds), the results **support** the primary claim that Go-Back-N retransmits more under reordering than Selective Repeat, while Selective Repeat avoids timeout-driven cascades by buffering in-window out-of-order DATA. Stop-and-Wait also avoided retransmissions here but at substantially lower goodput than Selective Repeat.

This is not a claim that Selective Repeat is universally better on every network or under loss.

---

## 13. RTO Findings

**Under the tested E4 conditions** (5% loss, no reorder, min RTO 10 ms), a short multiplier (0.5×) produced clear premature retransmissions and more retransmission work; a long multiplier (3.0×) removed premature counts in this matrix and tended to delay recovery relative to 1.0×. Endpoint `final_rto` can be inflated by backoff and is not always a simple scaled SRTT.

E4’s min RTO differs from E1’s 200 ms floor, so absolute RTO values are not directly comparable across studies.

---

## 14. Threats to Validity / Limitations

- Virtual-time clock for E1/E4; not a physical WAN stack.
- E1 isolates reorder with loss = 0; E4 isolates RTO with reorder = 0 and loss = 0.05—neither crosses both factors fully.
- n = 5 seeds (df = 4); some GBN intervals are wide.
- One fixture size and chunk size. E1 and E4 keep windows at 1 / 8 / 8. The window study varies window at loss 0. The loss study varies loss at window 8. Those two factors are not crossed.
- Real UDP demo is localhost-focused.
- Dirty git tree at E1 acquisition time is recorded in `results/analysis/provenance.md`.

---

## 15. Reproducibility

See `docs/REPRODUCIBILITY.md` for exact commands. Dataset hashes and analysis methodology are recorded in `results/analysis/provenance.md` and `results/rto_sensitivity/README.md`.

---

## 16. Conclusion

The project delivers a shared Rust reliable-transport stack, a deterministic experiment path, a localhost UDP demonstration, and four experimental matrices. Under the frozen E1 design, reordering exposes Go-Back-N’s cascade cost relative to Selective Repeat. Under E4, RTO aggressiveness trades premature retransmissions against recovery delay. Under zero loss, goodput rose with window size and the two pipelined protocols recorded the same goodput. Under loss with reorder held at 0, Go-Back-N retransmitted more than Selective Repeat at every nonzero loss rate in the grid. Those conclusions are configuration-scoped.

---

## 17. Future Work

- A crossed loss × reorder grid (the two factors were swept in separate studies)
- More seeds / larger files
- Multi-host or WAN UDP testing
- Burst-loss channel models

These were **not** completed in this submission.
