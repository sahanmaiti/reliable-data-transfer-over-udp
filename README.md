<div align="center">

# Reliable Data Transfer over UDP

**Making UDP reliable — one packet at a time.**

A modular and reproducible reliable file-transfer system built on raw UDP datagram sockets, implementing **Stop-and-Wait, Go-Back-N, and Selective Repeat ARQ**, with adaptive retransmission timeout estimation and a deterministic network channel emulator.

<br>

![Rust](https://img.shields.io/badge/Rust-1.XX%2B-orange?style=flat-square&logo=rust&logoColor=white)
![Python](https://img.shields.io/badge/Python-3.11%2B-3776AB?style=flat-square&logo=python&logoColor=white)
![UDP](https://img.shields.io/badge/transport-UDP-111111?style=flat-square)
![ARQ](https://img.shields.io/badge/protocol-ARQ-6f42c1?style=flat-square)
![Status](https://img.shields.io/badge/status-active%20development-orange?style=flat-square)
![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)

<br>

</div>

---

**CS-30003 · Coding Assignment 1 · Path A (catalogue)**  
**P3 — Reliable Data Transfer over UDP · Section CSE 33**

| | |
|---|---|
| **Repository** | [github.com/sahanmaiti/reliable-data-transfer-over-udp](https://github.com/sahanmaiti/reliable-data-transfer-over-udp) |
| **Team** | Ashwika Burman (24155095) · Sahan Maiti (24052247) · Soumyadeb Mukherjee (24052329) · Kashish Gupta (24052495) |

The three ARQ protocols are **selected at runtime** on the same application and shared packet format.

---

## What is Reliable Data Transfer over UDP?

UDP provides a lightweight datagram transport, but it does not guarantee that packets will arrive, arrive only once, arrive in order, or remain unmodified.

This project builds a **reliable file-transfer layer on top of raw UDP sockets**.

The system implements three **runtime-selectable** interchangeable Automatic Repeat reQuest (ARQ) protocols:

- **Stop-and-Wait** — one packet in flight at a time
- **Go-Back-N** — fixed sender window with cumulative acknowledgements
- **Selective Repeat** — sender and receiver buffering with individual acknowledgements

All three protocols operate over a shared packet format, retransmission-timing subsystem, deterministic channel emulator, and metrics/result interface.

The reliable-transport path is implemented entirely in **Rust**.

**Python is used only for experiment orchestration, structured-result processing, statistical analysis, and visualization.**

The project is designed not only to make UDP reliable, but to provide a **controlled and reproducible environment for implementing, testing, measuring, and comparing ARQ protocols under identical network conditions**.

---

## Why this project?

Comparing reliability protocols on a real network makes controlled experimentation difficult. Packet loss, delay, reordering, and other network events are not guaranteed to be identical between runs.

This project addresses that problem with a **deterministic, seeded channel emulator**.

The same experiment can be executed against all three ARQ protocols using the same:

- source file
- packet size
- sender/receiver configuration
- window size
- delay model
- impairment configuration
- random seed

This allows protocol behavior to be compared under **matched and reproducible conditions**.

The project therefore combines:

```text
Reliable Transport
        +
Controlled Fault Injection
        +
Adaptive Retransmission Timing
        +
Integrity Verification
        +
Reproducible Experiments
```

---

## Core Deliverables

The final system provides (per approved proposal):

- **Stop-and-Wait:** one packet in flight with acknowledgement and timeout-based retransmission.
- **Go-Back-N:** fixed sender window, cumulative ACKs, and retransmission of the outstanding packets after a timeout.
- **Selective Repeat:** sender and receiver buffering, individual ACKs, a receiver window, and retransmission of only the required packets.
- **Common packet framing** (sequence number, checksum, length, flag/type) with source-file chunking and reassembly.
- Duplicate ACK and duplicate packet handling with **guaranteed in-order delivery** to the application layer.
- Adaptive retransmission timers using **Jacobson/Karels** estimation with SRTT, RTTVAR, exponential backoff, and **Karn's algorithm**.
- **Deterministic channel emulator** with seeded loss, duplication, reordering, corruption, delay, and jitter.
- **SHA-256** comparison of source and received files for end-to-end integrity.
- **Rust-generated JSON/CSV** raw results consumed by a **Python** experiment and analysis pipeline (automation, statistical analysis, plots for goodput, retransmissions, window size, reordering, and RTO sensitivity).

---

## Architecture

```text
                         Experiment Configuration
                                  |
                                  v
                  +-------------------------------+
                  |       Python Experiment       |
                  |     Orchestration Layer       |
                  +---------------+---------------+
                                  |
                         JSON / CLI Interface
                                  |
                                  v
              +-------------------------------------------+
              |             Rust Reliable Transport        |
              |                                           |
              |  UDP Sockets                              |
              |  Packet Framing / Encoding                |
              |  ARQ Protocols                            |
              |  Retransmission Timers                    |
              |  Adaptive RTO                             |
              |  Channel Emulator                         |
              +----------------------+--------------------+
                                     |
                                     v
                          Deterministic Channel
                                     |
                 +-------------------+-------------------+
                 |                   |                   |
               Loss               Duplication         Corruption
                 |                   |                   |
                 +-------------------+-------------------+
                                     |
                              Delay / Jitter
                                     |
                               Reordering
                                     |
                                     v
                                UDP Receiver
                                     |
                                     v
                            Packet Validation
                                     |
                                     v
                               ARQ Receiver
                                     |
                                     v
                           File Reconstruction
                                     |
                                     v
                              SHA-256 Check
                                     |
                                     v
                            JSON / CSV Result
                                     |
                                     v
                         Python Analysis Pipeline
                                     |
                         +-----------+-----------+
                         |                       |
                         v                       v
                      Tables                  Plots
```

### Implementation Boundary

The project deliberately separates **transport implementation** from **experimental analysis**.

### Rust

Rust implements the complete reliable-transport path:

- UDP sockets
- Packet framing
- Byte-level encoding and decoding
- Stop-and-Wait
- Go-Back-N
- Selective Repeat
- Retransmission timers
- RTT measurement
- Jacobson/Karels RTO estimation
- Deterministic channel emulator
- File transfer
- Integrity verification
- Raw result generation

### Python

Python is used only for:

- Experiment orchestration
- Parameter sweeps
- Running repeated trials
- JSON/CSV processing
- Statistical analysis
- Confidence intervals
- Aggregation
- Visualization

The Rust programs produce reproducible raw results that form the interface to the Python analysis pipeline.

---

# ARQ Protocols

## Stop-and-Wait

Stop-and-Wait permits exactly one outstanding DATA packet.

```text
Sender                         Receiver
  |                              |
  | -------- DATA(seq=0) ------> |
  |                              |
  | <--------- ACK(1) ---------- |
  |                              |
  | -------- DATA(seq=1) ------> |
  |                              |
  | <--------- ACK(2) ---------- |
```

If an acknowledgement is not received before the retransmission timeout expires, the sender retransmits the outstanding packet.

### Strengths

- Simple protocol state
- Minimal buffering
- Straightforward correctness reasoning
- Useful baseline for comparison

### Trade-off

Throughput is strongly limited by the round-trip time because only one packet may be outstanding.

---

## Go-Back-N

Go-Back-N uses a fixed sender window and cumulative acknowledgements.

The receiver normally accepts only the next expected packet and discards out-of-order DATA packets.

```text
Sender                         Receiver

DATA 0 ----------------------> deliver 0
DATA 1 ----------------------> deliver 1
DATA 2 --------X

DATA 3 ----------------------> discard
DATA 4 ----------------------> discard

             <--------------- cumulative ACK

DATA 2 ----------------------> deliver 2
DATA 3 ----------------------> deliver 3
DATA 4 ----------------------> deliver 4
```

When a loss causes the sender to recover, multiple outstanding packets may be retransmitted.

### Strengths

- Better utilization than Stop-and-Wait
- Fixed sender-window model
- Simpler receiver buffering than Selective Repeat

### Trade-off

Out-of-order packets are discarded, so reordering can cause unnecessary retransmissions of packets that have already reached the receiver.

---

## Selective Repeat

Selective Repeat allows the receiver to buffer valid out-of-order DATA packets.

Each received packet can be acknowledged individually.

```text
Sender                         Receiver

DATA 0 ----------------------> deliver 0
DATA 1 --------X

DATA 2 ----------------------> buffer 2
DATA 3 ----------------------> buffer 3

             <--------------- individual ACKs

DATA 1 ----------------------> deliver 1
                              |
                              +--> deliver buffered 2
                              +--> deliver buffered 3
```

Only missing packets need to be retransmitted.

### Strengths

- Efficient loss recovery
- Handles packet reordering better
- Avoids unnecessary retransmission of already received packets
- Receiver buffering preserves useful out-of-order packets

### Trade-off

Selective Repeat requires more complex sender and receiver state management.

---

# Shared Packet Format

All ARQ protocols use a common packet representation above UDP.

The packet framing contains fields for:

```text
+----------------+----------------+
| Sequence Number|     Length     |
+----------------+----------------+
|      Flags / Type              |
+--------------------------------+
|           Checksum             |
+--------------------------------+
|          Payload              ...
+--------------------------------+
```

The exact encoding is implemented in Rust and is shared by all protocol implementations.

The common framing provides:

- Sequence identification
- Packet type identification
- Payload-length validation
- Corruption detection
- Consistent protocol boundaries

The sender segments the source file into packet payloads.

The receiver reconstructs the original byte stream and verifies its integrity using SHA-256.

---

# Adaptive Retransmission Timeout

A fixed retransmission timeout does not perform consistently across different network conditions.

The timing subsystem therefore estimates RTT dynamically using the **Jacobson/Karels algorithm**.

Conceptually:

```text
SRTT   = Smoothed RTT
RTTVAR = RTT variation

RTO = SRTT + max(G, 4 × RTTVAR)
```

The implementation includes:

- RTT measurement
- Smoothed RTT (`SRTT`)
- RTT variation (`RTTVAR`)
- Adaptive RTO calculation
- Jacobson/Karels estimation
- Karn's algorithm
- Exponential timeout backoff
- RTO bounds
- Timer-generation handling

### Karn's Algorithm

RTT samples from retransmitted packets are treated as ambiguous and are not used as ordinary RTT samples.

### Timeout Backoff

Repeated timeout events increase the effective retransmission timeout to avoid repeatedly retransmitting too aggressively when the network is experiencing delay or loss.

### Timer Safety

Timer-generation identifiers prevent stale timer-expiry events from affecting newer transmissions.

---

# Deterministic Channel Emulator

The channel emulator sits between the UDP sender and receiver.

```text
                    UDP Sender
                        |
                        v
             +-----------------------+
             | Deterministic Channel |
             +-----------------------+
                |    |    |    |    |
                v    v    v    v    v
               Loss Dup  Corr Reorder Delay/Jitter
                \    |    |    |    /
                 \   |    |    |   /
                  +--+----+----+--+
                           |
                           v
                      UDP Receiver
```

The emulator operates on serialized datagrams and is deliberately **protocol-independent**.

It does not inspect whether a datagram contains:

- DATA
- ACK
- Sequence information
- ARQ state

Instead, it applies configured network impairments to datagrams.

### Supported Impairments

- Packet loss
- Packet duplication
- Packet corruption
- Packet reordering
- Base delay
- Random jitter
- Direction-specific behavior

### Deterministic Seeds

Each experiment supplies a seed to the emulator.

This makes the logical impairment decisions reproducible and allows matched protocol comparisons under controlled conditions.

Wall-clock completion time can still vary because of operating-system scheduling and machine load.

---

# Integrity Verification

Protocol completion alone does not determine whether a transfer was successful.

The receiver reconstructs the transferred file and compares its SHA-256 digest with the source file.

```text
Original File
      |
      v
   SHA-256
      |
      |       compare
      +----------+----------+
                 |
                 v
          Integrity Result
                 ^
                 |
      +----------+----------+
      |
   SHA-256
      |
Received File
```

A successful transfer must satisfy:

```text
source_sha256 == received_sha256
```

The system also tracks **unique application bytes** so retransmitted or duplicated packets cannot artificially increase delivered-byte measurements.

---

# The Experimental Claim

### Primary claim

The primary claim tested by this project is:

> **Under identical controlled conditions, Selective Repeat will require fewer data retransmissions than Go-Back-N as packet reordering increases.**

The reasoning is based on the fundamental difference between the two protocols:

- Go-Back-N discards out-of-order packets.
- Selective Repeat buffers valid out-of-order packets.
- Go-Back-N can therefore retransmit packets that have already been successfully received.
- Selective Repeat can selectively retransmit only missing packets.

The experiment is designed to measure whether this difference becomes increasingly visible as packet reordering increases.

### Secondary claim (RTO study)

Scale the computed adaptive RTO by a fixed multiplier while holding Jacobson/Karels alpha and beta constant, and measure the trade-off between **premature retransmissions** and **delayed recovery from genuine loss**. See **E4 — RTO Sensitivity** below.

---

# Experiments

Experiments are grouped by the **approved proposal** (primary and secondary claims) and **catalogue requirements** (Path A P3 core experiments).

## E1 — Retransmissions vs Packet Reordering (primary claim)

This is the **primary experiment** of the project.

### Independent Variable

Packet reordering probability.

The probability is swept from approximately:

```text
0% → 25%
```

### Controlled Variables

The following remain fixed:

- Packet loss
- File size
- Packet size
- Window size
- Base RTT
- Timeout controls
- Other channel parameters

Packet loss is held at **0%** to isolate the effect of reordering.

### Method

Each experimental condition is executed multiple times using matched seeds.

The experiment records:

- Data retransmissions
- Timeout counts
- Failure counts
- Completion time
- Goodput
- Raw per-run results

The primary reported metric is:

```text
Mean data retransmissions
```

with:

```text
95% confidence intervals
```

---

## E2 — Goodput vs Window Size (catalogue experiment)

This **catalogue-required** experiment studies how sender window size affects transfer performance for all three protocols.

Example window sizes (configurable; not fixed by the proposal):

```text
1
2
4
8
16
32
```

The exact matrix is defined by the experiment configuration.

### Measured

- Goodput
- Completion time
- Data retransmissions
- Timeout count
- Transfer outcome

The experiment allows the behavior of the three ARQ protocols to be compared as the number of outstanding packets changes.

---

## E3 — Goodput vs Packet Loss (catalogue experiment)

This **catalogue-required** experiment studies protocol behavior as the underlying network becomes increasingly lossy.

### Independent Variable

Packet loss probability.

### Measured

- Goodput
- Completion time
- Data retransmissions
- Timeout count
- Failure rate

All protocols are evaluated under matched experimental conditions.

---

## E4 — RTO Sensitivity (secondary claim)

This is the **secondary RTO study** from the proposal.

The computed adaptive RTO is scaled by a **fixed multiplier** (for example **0.5× to 3×**) while Jacobson/Karels **alpha and beta stay constant**. The goal is to measure the trade-off between premature retransmissions and delayed recovery from genuine loss.

Example multipliers (configurable):

```text
0.5×
1.0×
3.0×
```

### Measured

- Goodput
- Timeout count
- Data retransmissions
- Completion time
- Final SRTT
- Final RTO

The experiment investigates the trade-off between:

```text
RTO too small
      |
      v
Premature / spurious retransmissions
```

and:

```text
RTO too large
      |
      v
Slow recovery from genuine packet loss
```

---

# Statistical Methodology

The experiment pipeline preserves individual raw runs rather than storing only aggregated results.

For repeated experimental conditions, the analysis reports:

- Mean values
- Confidence intervals
- Failure counts
- Timeout counts
- Per-run measurements

For the primary reordering experiment, the main reported result is the **mean data retransmission count with a 95% confidence interval**.

The raw results remain available so that every aggregate and visualization can be traced back to individual runs.

---

# Metrics

The metrics system separates application, timing, protocol, channel, and experiment-level measurements.

## Application Metrics

- Source file size
- Delivered unique application bytes
- Transfer status
- SHA-256 integrity result

## Timing Metrics

- Data-transfer completion time
- RTT samples
- SRTT
- RTTVAR
- RTO values
- Timeout count
- Timeout backoff events

## Protocol Metrics

- DATA sends
- ACK sends
- DATA receives
- ACK receives
- Data retransmissions
- Timeout retransmissions
- Duplicate DATA packets
- Duplicate ACKs
- Buffered packets where applicable

## Channel Metrics

- Offered datagrams
- Dropped datagrams
- Duplicated datagrams
- Corrupted datagrams
- Reordered datagrams
- Scheduled deliveries

## Experiment Metrics

- Experiment ID
- Protocol
- Configuration
- Seed
- Trial/run ID
- Completion status
- Failure status
- Software revision

---

# Goodput

Goodput measures successfully delivered **unique application bytes** relative to data-transfer completion time.

```text
                         unique application bytes delivered
goodput = ----------------------------------------------------------
                    data-transfer completion time
```

Retransmitted and duplicated packets are not counted again in the numerator.

Failed or integrity-failed transfers are reported separately rather than silently converting them into zero-goodput measurements.

---

# Reproducibility

Every experiment produces a raw result containing enough information to reproduce and interpret the run.

A result includes information such as:

```text
run_id
experiment_id
protocol
configuration
random_seed
software_revision
source_file_hash
file_size
terminal_status
completion_time
protocol_counters
channel_counters
timing_statistics
integrity_result
```

The experiment flow is:

```text
Experiment Configuration
          |
          v
     Protocol
          |
          v
      Seed + Configuration
          |
          v
     Rust Transport
          |
          v
 Deterministic Channel
          |
          v
       Raw Result
          |
     +----+----+
     |         |
     v         v
   JSON      CSV
     |         |
     +----+----+
          |
          v
 Python Analysis
          |
     +----+----+
     |         |
     v         v
  Tables     Plots
```

### Reproducibility Boundary

The seeded channel emulator makes logical impairment decisions reproducible.

However, wall-clock completion time may vary due to:

- Operating-system scheduling
- Machine load
- Timer granularity
- Runtime scheduling

Therefore, deterministic seeds provide **reproducible experimental conditions**, not necessarily bit-identical wall-clock timing.

---

# Testing

Testing focuses on protocol correctness as well as end-to-end reliability.

## Packet Tests

- Encode/decode round trips
- Boundary payload sizes
- Malformed packets
- Invalid lengths
- Invalid flags/types
- Checksum corruption
- Truncated datagrams
- Invalid sequence information

## Timer / RTO Tests

- RTT estimation
- SRTT calculation
- RTTVAR calculation
- RTO calculation
- RTO bounds
- Karn's algorithm
- Timeout backoff
- Timer cancellation
- Timer-generation handling
- Stale timer expiry

## Channel Tests

- Loss
- Duplication
- Corruption
- Delay
- Jitter
- Reordering
- Combined impairments
- Seed reproducibility
- Direction-specific behavior

## Stop-and-Wait Tests

- Clean transfer
- Lost DATA
- Lost ACK
- Duplicate DATA
- Duplicate ACK
- Corrupted DATA
- Delayed ACK
- Timeout recovery
- Retry limits
- Final DATA loss
- Final ACK loss

## Go-Back-N Tests

- Clean transfer
- Window advancement
- Cumulative ACK handling
- Lost DATA
- Lost ACK
- Duplicate DATA
- Duplicate ACK
- Out-of-order DATA
- Timeout recovery
- Outstanding-window retransmission
- Final DATA loss
- Final ACK loss

## Selective Repeat Tests

- Clean transfer
- Sender window handling
- Receiver window handling
- Individual ACKs
- Out-of-order buffering
- Lost DATA
- Lost ACK
- Duplicate DATA
- Duplicate ACK
- Selective retransmission
- Timeout recovery
- Final DATA loss
- Final ACK loss

## End-to-End Tests

The end-to-end suite includes:

- Empty files where supported
- 1-byte files
- Exact packet-sized files
- Files slightly larger than one packet
- Binary files
- Larger transfer fixtures
- Clean transfers
- Lossy transfers
- Reordered transfers
- Corrupted transfers
- Combined impairment scenarios
- SHA-256 verification

A successful transfer must demonstrate both:

```text
Protocol completion
        AND
SHA-256 integrity
```

---

# Repository Structure

The layout below is the **target** project structure. Directories and files appear as components land according to the weekly plan (ARQ, timing, and channel work may still be in progress).

```text
.
├── README.md
├── AI-USE.md
├── Cargo.toml
│
├── src/
│   ├── main.rs
│   ├── app/
│   ├── packet/
│   ├── arq/
│   │   ├── mod.rs
│   │   ├── sw/
│   │   ├── gbn/
│   │   └── sr/
│   ├── timing/
│   ├── channel/
│   └── metrics/
│
├── tests/
│   ├── packet/
│   ├── timing/
│   ├── channel/
│   ├── arq/
│   │   ├── sw/
│   │   ├── gbn/
│   │   └── sr/
│   └── integration/
│
├── fixtures/
│
├── configs/
│   ├── base/
│   └── experiments/
│
├── experiments/
│   └── python/
│
├── scripts/
│
├── results/
│   ├── raw/
│   └── processed/
│
├── plots/
│
└── reports/
```

The repository separates:

```text
Rust Transport
      |
      +-- Packet
      +-- ARQ
      +-- Timing
      +-- Channel
      +-- Metrics
      |
      v
Raw JSON / CSV
      |
      v
Python Experiments
      |
      +-- Automation
      +-- Statistics
      +-- Aggregation
      +-- Visualization
```

This prevents experimental analysis logic from leaking into the reliable-transport implementation.

---

# Design Principles

### 1. Shared Protocol Contracts

The application and experiment runner should not need to change when switching between:

```text
Stop-and-Wait
Go-Back-N
Selective Repeat
```

### 2. Protocol-Independent Channel

The channel emulator operates on datagrams rather than understanding ARQ semantics.

### 3. Correctness Before Performance

A transfer that completes quickly but produces corrupted output is a failed transfer.

### 4. Reproducibility by Design

Configurations, seeds, software revisions, and raw per-run results are preserved.

### 5. Raw Data First

Aggregated tables and plots must always be traceable to individual experimental runs.

### 6. Controlled Experiments

The primary protocol comparison changes one experimental factor at a time while keeping the remaining relevant parameters fixed.

### 7. Core Before Stretch

Required reliability mechanisms, correctness tests, and mandatory experiments take priority over optional extensions.

---

# Scope

## Core

- [ ] Stop-and-Wait ARQ
- [ ] Go-Back-N ARQ
- [ ] Selective Repeat ARQ
- [ ] Common packet framing
- [ ] Sequence numbers
- [ ] Checksums
- [ ] Length and packet-type fields
- [ ] Cumulative acknowledgements
- [ ] Individual acknowledgements
- [ ] Duplicate packet handling
- [ ] Duplicate ACK handling
- [ ] In-order application delivery
- [ ] Adaptive RTO
- [ ] Jacobson/Karels RTT estimation
- [ ] SRTT
- [ ] RTTVAR
- [ ] Karn's algorithm
- [ ] Exponential timeout backoff
- [ ] Deterministic seeded channel emulator
- [ ] Loss
- [ ] Duplication
- [ ] Corruption
- [ ] Reordering
- [ ] Delay and jitter
- [ ] SHA-256 integrity verification
- [ ] Rust raw-result generation
- [ ] JSON/CSV result interface
- [ ] Python experiment automation
- [ ] Statistical analysis
- [ ] 95% confidence intervals
- [ ] Required performance experiments
- [ ] Result visualization
- [ ] End-to-end validation

## Stretch

Stretch features are considered only after the core system, correctness tests, and required experiments are stable.

Possible extensions (assignment catalogue stretch, after Core is solid):

- SACK blocks
- A sliding-window flow-control layer
- Connection setup and teardown
- Nagle-style coalescing

---

# What This Project Is Not

This project is intentionally **not**:

- A reimplementation of TCP
- A congestion-control implementation
- A production-grade file-transfer application
- An encrypted transport
- A GUI networking application
- An Internet-scale benchmark

The purpose is to study **transport-layer reliability and ARQ behavior under controlled and reproducible network conditions**.

---

# Technology Stack

| Technology | Purpose |
|---|---|
| **Rust** | Complete reliable-transport implementation |
| **UDP sockets** | Underlying datagram transport |
| **Rust standard library / runtime facilities** | Networking, concurrency, timers, file I/O |
| **SHA-256** | End-to-end file integrity |
| **JSON** | Configuration and structured results |
| **CSV** | Experiment result exchange |
| **Python 3.11+** | Experiment orchestration and analysis |
| **pandas** | Result processing and statistical analysis |
| **matplotlib** | Visualization |

No existing reliable-transport or ARQ library is used to implement the protocols.

---

# Getting Started

> Installation and execution commands will be added as the implementation stabilizes.

The intended workflow is:

```text
1. Configure an experiment
        ↓
2. Select ARQ protocol
        ↓
3. Configure channel conditions
        ↓
4. Run the Rust transport
        ↓
5. Transfer file over UDP
        ↓
6. Verify SHA-256 integrity
        ↓
7. Emit JSON/CSV raw result
        ↓
8. Repeat experimental condition
        ↓
9. Run Python analysis
        ↓
10. Generate tables and plots
```

---

# Project Workflow

The development plan follows the **approved proposal** (rough schedule; not a claim that each week is complete):

| Week | Target | Owner / responsibility |
|------|--------|-------------------------|
| **1** | Interfaces, packet header design, proposal, and repository setup. | All members — initial setup, repository, proposal, and interface agreement. |
| **2** | Packet codec, channel emulator, timer, and metrics foundations; Rust/Python result interface; vertical slice with one ARQ protocol end-to-end on a simple channel. | Soumyadeb — ARQ/protocol foundations and packet semantics; Kashish — timer foundations and RTO design; Ashwika — channel emulator foundations; Sahan — metrics, integration, and Rust/Python result interface. |
| **3** | Stop-and-Wait complete; timing and emulator faults integrated; first basic results. | Soumyadeb — Stop-and-Wait and protocol testing; Kashish — retransmission timers and RTO integration; Ashwika — fault injection and emulator testing; Sahan — initial results, metrics, and end-to-end validation. |
| **4** | Go-Back-N integration and testing under channel faults. | Soumyadeb — Go-Back-N window and cumulative-ACK logic; Kashish — timer/RTO integration and timeout testing; Ashwika — fault testing under loss, duplication, reordering, and delay; Sahan — integration, validation, and result collection. |
| **5** | Selective Repeat integration, combined-fault testing, and Core correctness gate. | Soumyadeb — Selective Repeat buffering, receiver window, and individual ACKs; Kashish — timing/RTO support and timeout testing; Ashwika — combined-fault testing and emulator validation; Sahan — end-to-end validation and integrity checks. |
| **6** | Experiment runner, pilot studies, and first plots. | Sahan — Python experiment automation, parameter sweeps, result processing, and initial plots; Soumyadeb and Ashwika — protocol-specific support and debugging; Kashish — timing/RTO support for experiments. |
| **7** | Final experiment matrix, statistical analysis, and regression fixes. | Sahan — final experiments, statistical analysis, confidence intervals, and plots; Soumyadeb — protocol debugging and regression testing; Kashish — timer/RTO regression testing; Ashwika — channel emulator and fault-model regression testing. |
| **8** | Final validation, documentation, AI-USE.md, demo, and individual viva practice. | All members — final integration, documentation, demo preparation, validation, and viva preparation. |

The project prioritizes a working vertical slice early so that integration problems are discovered before the full experimental phase.

---

# Risks and Fallbacks

From the approved proposal:

| Risk | Fallback |
|------|----------|
| **Timer races and stale expiry** | Use one timer-scheduling mechanism, timer-generation IDs, and controlled-clock tests; fall back to a simpler serialized event loop if needed. |
| **Combined-fault integration** | Build an early vertical slice with weekly clean and fault checkpoints; prioritize Core correctness if time is short. |
| **Rust/Python interface mismatch** | Define and test a stable JSON/CSV result schema early. |

---

# Academic Project

This project is developed as:

**P3 — Reliable Data Transfer over UDP** (Path A, catalogue)

for:

**CS-30003 • Coding Assignment 1 • Section CSE 33**

The project focuses on demonstrating:

- UDP socket programming
- Transport-layer reliability
- ARQ protocols
- Sliding-window protocols
- Sequence numbers and acknowledgements
- RTT estimation
- Retransmission timeout estimation
- Packet-level fault handling
- Deterministic network emulation
- File integrity verification
- Experimental methodology
- Statistical analysis
- Reproducible networking experiments

---

# Team

| Member | Roll | Responsibility |
|---|---|---|
| **Soumyadeb Mukherjee** | 24052329 | Protocol & ARQ — packet format and semantics, Stop-and-Wait, Go-Back-N, Selective Repeat, protocol-state and conformance testing |
| **Kashish Gupta** | 24052495 | Timing & RTO — retransmission timers, Jacobson/Karels RTO, SRTT, RTTVAR, Karn's algorithm, timing and timeout tests |
| **Ashwika Burman** | 24155095 | Channel Emulator — deterministic loss, duplication, reordering, corruption, delay/jitter, seed replay, emulator testing |
| **Sahan Maiti** | 24052247 | Evaluation & Integration — Rust/Python experiment interface, experiment automation, metrics, raw-result logging, statistical analysis, plots, integrity checks, end-to-end validation |

All members contribute to:

- Integration
- Code review
- Debugging
- Documentation
- Final validation
- Demo preparation
- Viva preparation

---

# Project Goal

The central question behind the project is:

> **How do Stop-and-Wait, Go-Back-N, and Selective Repeat behave as packet reordering increases under identical controlled network conditions, and how does adaptive timeout estimation affect their recovery efficiency?**

The primary claim is:

> **Selective Repeat will require fewer data retransmissions than Go-Back-N as packet reordering increases.**

The project tests this claim through controlled experiments rather than relying only on theoretical expectations.

The complete system combines:

```text
Raw UDP
   +
Packet Framing
   +
ARQ Reliability
   +
Adaptive RTO
   +
Deterministic Fault Injection
   +
SHA-256 Integrity
   +
Reproducible Raw Results
   +
Statistical Analysis
```

into one modular reliable data-transfer system.

---

# License

This project is released under the **MIT License**.

See [`LICENSE`](LICENSE) for the full license text.

---

<div align="center">

<br>

**Built to understand reliable transport from the ground up.**

*Reliable Data Transfer over UDP*

</div>
