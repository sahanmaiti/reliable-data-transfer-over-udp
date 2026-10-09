# 2-Week Accelerated Project Roadmap & Task Distribution

> **Historical planning document.** Kept for process provenance. For current behaviour, experiments, and commands, use `README.md`, `docs/FINAL_REPORT.md`, and `docs/REPRODUCIBILITY.md`.

**Project:** Reliable Data Transfer over UDP (CS-30003)  
**Timeline:** 14 Days (Two 7-Day Sprints)  

**Team responsibility areas** (not a claim that every listed task was completed solely by one person):

- Soumyadeb Mukherjee (24052329) — Protocol & ARQ
- Kashish Gupta (24052495) — Timing & RTO
- Ashwika Burman (24155095) — Channel Emulator
- Sahan Maiti (24052247) — Evaluation & Integration

---

## 1. Executive Timeline

```
========================================================================================
WEEK 1: FOUNDATIONS & VERTICAL SLICE (STOP-AND-WAIT)
========================================================================================
Day 1 - 2: Module interfaces finalized, Packet codec completed, Seeded RNG channel stubbed
Day 3 - 4: Jacobson/Karels RTO implemented, Channel fault injection (Loss + Corrupt) ready
Day 5 - 6: Stop-and-Wait ARQ end-to-end working over UDP, File transfer loop verified
Day 7    : Sprint 1 Integration Review: Working file transfer with Stop-and-Wait + SHA-256
========================================================================================
WEEK 2: PIPELINED PROTOCOLS, EXPERIMENTAL SUITE & VIVA PREPARATION
========================================================================================
Day 8 - 9  : Go-Back-N (GBN) sender & receiver implemented + cumulative ACK handling
Day 10 - 11: Selective Repeat (SR) sender & receiver + individual buffering + reordering
Day 12     : Python experiment runner, parameter sweeps (reordering vs loss), plots
Day 13     : Statistical analysis, confidence intervals, CSV/JSON verification
Day 14     : Code freeze, viva defense mock, documentation finalization, demo prep
========================================================================================
```

---

## 2. Team Responsibility Matrix

| Member | Primary Ownership | Secondary / Integration |
|---|---|---|
| **Soumyadeb Mukherjee** (24052329) — Protocol & ARQ | Protocol & ARQ state machines (`packet.rs`, `sw.rs`, `gbn.rs`, `sr.rs`) | Conformance testing, RFC 1071 checksum |
| **Kashish Gupta** (24052495) — Timing & RTO | Retransmission timing & RTO (`rto.rs`, `timer.rs`, Jacobson/Karels, Karn) | Timer unit tests, timeout backoff |
| **Ashwika Burman** (24155095) — Channel Emulator | Deterministic channel emulator (loss/dup/reorder/corrupt/delay) | Seed repeatability, channel stress tests |
| **Sahan Maiti** (24052247) — Evaluation & Integration | System integration, file I/O, SHA-256, metrics, Python testbed, plots | Architecture, PR reviews, end-to-end validation |

---

## 3. Sprint 1 Breakdown (Week 1: Days 1 – 7)

### Track A: Soumyadeb Mukherjee (Protocol & ARQ)
- [x] Complete `src/packet.rs`: Implement RFC 1071 16-bit Internet Checksum calculation & validation.
- [x] Implement `FIN` packet semantics and serialization in `packet.rs`.
- [x] Build `src/arq/sw.rs`: Stop-and-Wait sender and receiver state machines.
- [x] Define protocol architecture in `src/arq/mod.rs`.
- [x] Deliver unit tests demonstrating:
  - Valid packet roundtrip with checksum verification (`tests/test_packet.rs`).
  - Drop handling on checksum mismatch.
  - S&W state machine handling of delayed and duplicate ACKs (`tests/test_arq.rs`).

### Track B: Kashish Gupta (Timing & RTO)
- [x] Build `src/timing/rto.rs`: Implement Jacobson/Karels RTO estimator (RFC 6298):
  - Initial RTO = 1.0s (or configurable).
  - First RTT sample: $SRTT = R, RTTVAR = R / 2, RTO = SRTT + \max(G, 4 \times RTTVAR)$.
  - Subsequent samples: update $RTTVAR$ and $SRTT$.
  - Karn's Algorithm: do not sample RTT for retransmitted packets.
  - Exponential timer backoff on retransmission ($RTO = \min(RTO \times 2, RTO_{max})$).
- [x] Build `src/timing/timer.rs`: Non-blocking retransmission timer abstraction with generation safety.
- [x] Deliver unit tests verifying RTO bounds, smoothing behavior, and backoff limits (`tests/test_timing.rs`).

### Track C: Ashwika Burman (Channel Emulator)
- [x] Build `src/channel/mod.rs` and `src/channel/scheduler.rs`.
- [x] Implement deterministic PRNG using standard seeded generator (e.g., PCG32 or seeded standard RNG).
- [x] Implement fault injection mechanisms:
  - **Packet Loss**: drop datagram with probability $P_{loss}$.
  - **Bit Corruption**: flip random bits in payload/header with probability $P_{corrupt}$.
  - **Packet Duplication**: duplicate datagram with probability $P_{dup}$.
  - **Delay & Jitter**: add fixed delay + uniform/normal jitter.
  - **Packet Reordering**: buffer packets with small random delays before release.
- [x] Deliver unit tests verifying: same seed produces the exact identical sequence of drops/corruptions.

### Track D: Sahan Maiti (Evaluation & Integration)
- [x] Set up project structure, modules in `src/lib.rs`, and dependencies in `Cargo.toml`.
- [x] Build file chunker (reading files in 1400-byte chunks) and file reassembler in `src/app/`.
- [x] Build SHA-256 end-to-end checksum verification tool.
- [x] Integrate Stop-and-Wait with UDP socket loop + Channel emulator for Day 7 milestone.
- [x] Set up `results/raw/` schema (JSON/CSV) for logging metrics (throughput, retransmissions, RTT, elapsed time).

---

## 4. Sprint 2 Breakdown (Week 2: Days 8 – 14)

### Soumyadeb Mukherjee
- [x] Build Go-Back-N (`src/arq/gbn.rs`): window management, cumulative ACKs, timer restart on oldest unACKed packet.
- [x] Build Selective Repeat (`src/arq/sr.rs`): sender & receiver sliding windows, individual ACKs, out-of-order receive buffer.
- [x] Validate invariant: window size $W \le 2^{31} - 1$ (or $W \le \text{SeqSpace} / 2$).

### Kashish Gupta
- [x] Adapt RTO estimator for pipelined protocols (tracking timestamps per in-flight packet).
- [x] Implement multi-timer management for Selective Repeat (individual packet deadlines).
- [x] Benchmark RTO accuracy vs fixed static timeouts under simulated jitter.

### Ashwika Burman
- [x] Stress-test channel emulator under combined fault profiles (e.g., 5% loss + 10% reordering + 20ms jitter).
- [x] Create pre-configured experiment profiles in `configs/experiments/`.
- [x] Verify zero memory leaks or unbounded buffer growth in reordering queues.

### Sahan Maiti
- [x] Wire CLI commands in `src/main.rs` (`reliable_udp send/recv/experiment`).
- [x] Write Python automation script `experiments/python/run_experiments.py`.
- [x] Run parameter sweep: Reordering Rate (0% to 25%) across Stop-and-Wait, GBN, and SR.
- [x] Generate comparative plots: Retransmissions vs Reordering, Goodput vs Loss.
- [ ] Write final report, prepare viva answers, and conduct dry run.

---

## 5. Git & Collaboration Rules

1. **Main is Protected**: Never push directly to `main`. All work enters via Pull Request.
2. **Branch Naming**:
   - `feature/arq-<protocol>` (Soumyadeb)
   - `feature/timing-<module>` (Kashish)
   - `feature/channel-<module>` (Ashwika)
   - `feature/integration-<module>` (Sahan)
3. **Commit Format**:
   - `feat(scope): concise description`
   - `fix(scope): fix bug description`
   - `test(scope): add tests for feature`
   - `docs(scope): update documentation`
4. **Acceptance Criteria for PRs**:
   - All code compiles cleanly (`cargo check`).
   - All tests pass (`cargo test`).
   - No compiler warnings allowed (`#[deny(warnings)]`).
   - At least 1 peer review approval.
