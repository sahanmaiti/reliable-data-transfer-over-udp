<div align="center">

# Reliable Data Transfer over UDP

**Making UDP reliable — one packet at a time.**

A modular and reproducible reliable file-transfer system built on top of UDP, implementing **Stop-and-Wait, Go-Back-N, and Selective Repeat ARQ**, with adaptive RTO estimation and a deterministic network channel emulator.

<br>

![Python](https://img.shields.io/badge/Python-3.11%2B-3776AB?style=flat-square&logo=python&logoColor=white)
![UDP](https://img.shields.io/badge/transport-UDP-111111?style=flat-square)
![ARQ](https://img.shields.io/badge/protocol-ARQ-6f42c1?style=flat-square)
![Status](https://img.shields.io/badge/status-active%20development-orange?style=flat-square)
![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)

<br>

</div>

---

## What is Reliable Data Transfer over UDP?

UDP is fast and lightweight, but it deliberately provides very few guarantees. Packets can be **lost, duplicated, reordered, corrupted, or delayed**, and UDP does not reconstruct the original byte stream for you.

This project builds a reliable file-transfer layer **on top of UDP**.

Instead of implementing one reliability strategy, the project provides three interchangeable ARQ protocols:

- **Stop-and-Wait** — one packet at a time
- **Go-Back-N** — sliding window with cumulative acknowledgements
- **Selective Repeat** — sliding window with selective acknowledgements and receiver buffering

The protocols operate over the same packet format, timing subsystem, UDP transport, deterministic channel emulator, metrics layer, and experiment framework.

The goal is not simply to make UDP reliable.

**The goal is to build a controlled environment for implementing, testing, measuring, and comparing different reliability strategies under reproducible network conditions.**

---

## Why this project?

A real network makes controlled protocol comparison difficult. If packet loss or reordering happens unpredictably, it becomes hard to determine whether a performance difference came from the protocol or from a different network event.

This project solves that problem with a **deterministic channel emulator**.

The same experiment can be executed against all three ARQ protocols using:

- the same file
- the same packet size
- the same delay model
- the same impairment probabilities
- the same window configuration
- the same random seed

This makes the resulting comparison much more meaningful and reproducible.

---

## Features

### Reliable File Transfer

- UDP socket-based file transfer
- File segmentation and reconstruction
- Sequence numbers
- Packet checksums
- Duplicate detection
- Ordered delivery
- SHA-256 end-to-end integrity verification
- Transfer completion and failure handling

### Three ARQ Protocols

- **Stop-and-Wait ARQ**
- **Go-Back-N ARQ**
- **Selective Repeat ARQ**
- Common protocol interface
- Runtime protocol selection
- Protocol-independent application layer

### Adaptive Retransmission Timeout

- RTT measurement
- Jacobson/Karels RTT estimation
- Smoothed RTT (`SRTT`)
- RTT variation (`RTTVAR`)
- Adaptive RTO
- Karn's algorithm
- RTO bounds
- Timeout backoff
- Timer generation IDs to prevent stale timer events

### Deterministic Network Emulator

Independently configurable network impairments:

- Packet loss
- Packet duplication
- Packet corruption
- Packet reordering
- Base propagation delay
- Random jitter
- Direction-specific behavior
- Seeded deterministic behavior

### Reproducible Experiments

Each run records enough information to understand and reproduce the result:

- Protocol
- Experiment ID
- Configuration
- Random seed
- Software revision
- Source-file hash
- Transfer outcome
- Completion time
- Retransmission counts
- Timeout counts
- Channel impairment events
- RTT/RTO statistics
- SHA-256 integrity status

---

## Architecture

```text
                         Experiment Configuration
                                  |
                                  v
                  +-------------------------------+
                  |       Transfer Application    |
                  |   File Source / File Sink     |
                  |   CLI / Hash Verification     |
                  +---------------+---------------+
                                  |
                                  v
              +-------------------------------------------+
              |                  ARQ Layer                 |
              |                                           |
              | Stop-and-Wait | Go-Back-N | Selective Rep |
              +----------------------+--------------------+
                                     |
                       +-------------+-------------+
                       |                           |
                       v                           v
                +-------------+             +-------------+
                | Packet Codec|             | Timer / RTO |
                | Validation  |             | Estimator   |
                +------+------+             +------+------+
                       |                           |
                       +-------------+-------------+
                                     |
                                     v
                     +-----------------------------+
                     |   Deterministic Channel    |
                     |                             |
                     | Loss / Dup / Corruption    |
                     | Reordering / Delay / Jitter|
                     +-------------+---------------+
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
                          SHA-256 Verification

                 Runtime components
                         |
                         v
                  +-------------+
                  |   Metrics   |
                  +------+------+ 
                         |
                         v
                   JSON / CSV
                         |
                         v
                Analysis & Plots
```

### Design rule

The ARQ protocols **do not know how the channel is configured**.

The channel receives serialized datagrams, applies its configured impairments, and forwards or drops them. This keeps protocol logic separate from network emulation.

---

## ARQ Protocols

### Stop-and-Wait

Stop-and-Wait permits a single outstanding data packet.

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

If an acknowledgement does not arrive before the RTO expires, the sender retransmits the packet.

**Strengths**

- Very simple state machine
- Minimal buffering
- Easy to verify

**Trade-off**

- Throughput is heavily limited by RTT
- Poor utilization on high-delay paths

---

### Go-Back-N

Go-Back-N allows multiple packets to be outstanding.

The receiver uses **cumulative acknowledgements** and normally accepts only the next packet in sequence.

```text
Sender                         Receiver

DATA 0 ---------------------->  deliver 0
DATA 1 ---------------------->  deliver 1
DATA 2 --------X

DATA 3 ---------------------->  discard
DATA 4 ---------------------->  discard

             <--------------- cumulative ACK

DATA 2 ---------------------->  deliver 2
DATA 3 ---------------------->  deliver 3
DATA 4 ---------------------->  deliver 4
```

When recovery is required, several outstanding packets may need to be retransmitted.

**Strengths**

- Better link utilization than Stop-and-Wait
- Relatively simple sender/receiver logic

**Trade-off**

- Reordering can cause unnecessary retransmissions
- Loss recovery may retransmit packets that were already received

---

### Selective Repeat

Selective Repeat allows the receiver to accept and buffer valid out-of-order packets.

```text
Sender                         Receiver

DATA 0 ----------------------> deliver 0
DATA 1 --------X

DATA 2 ----------------------> buffer 2
DATA 3 ----------------------> buffer 3

             <--------------- selective ACK

DATA 1 ----------------------> deliver 1
                              |
                              +--> deliver buffered 2
                              +--> deliver buffered 3
```

Only missing packets need to be retransmitted.

**Strengths**

- Efficient loss recovery
- Handles reordering better
- Avoids unnecessary retransmissions

**Trade-off**

- More complex state management
- Requires receiver buffering

---

## Adaptive RTO

Using a fixed retransmission timeout is unreliable across different network conditions.

The timing subsystem estimates RTT and derives an adaptive RTO using the Jacobson/Karels approach.

Conceptually:

```text
SRTT   = Smoothed RTT
RTTVAR = RTT variation

RTO = SRTT + max(G, 4 × RTTVAR)
```

The implementation also applies:

- **Karn's algorithm** — retransmitted packets are not used as ordinary RTT samples
- **RTO bounds** — prevents unreasonable timeout values
- **Backoff** — repeated timeout events increase the effective timeout
- **Timer generations** — prevents stale timer callbacks from affecting newer transmissions

The RTO system is therefore shared by the ARQ implementations rather than reimplemented independently for every protocol.

---

## Deterministic Channel Emulator

The network emulator sits between the sender and receiver.

```text
UDP Sender
    |
    v
+--------------------------------+
|     Deterministic Channel      |
|                                |
|  Loss                          |
|  Duplication                   |
|  Corruption                    |
|  Reordering                    |
|  Delay                         |
|  Jitter                        |
+----------------+---------------+
                 |
                 v
            UDP Receiver
```

A scenario can define parameters such as:

```text
loss probability
duplication probability
reordering probability
corruption probability
base delay
jitter
seed
```

Because the channel is seeded, the logical impairment decisions can be reproduced across runs.

The emulator is deliberately **protocol-independent**. It does not know whether a datagram contains DATA, ACK, sequence information, or ARQ state.

---

## Integrity Verification

A transfer is not considered successful merely because the protocol reaches its completion state.

The receiver reconstructs the file and verifies it against the original using SHA-256.

```text
Original file
      |
      v
   SHA-256
      |
      |       compare
      +----------+----------+
                 |
                 v
          Integrity result
                 ^
                 |
      +----------+----------+
      |
   SHA-256
      |
Received file
```

A successful run must satisfy:

```text
source_sha256 == received_sha256
```

The system also tracks **unique application bytes** so duplicate packets cannot inflate the reported delivered-byte count.

---

## Experiments

The project is designed around controlled experiments comparing the three ARQ protocols.

### E1 — Goodput vs Packet Loss

**Independent variable**

Packet loss probability.

**Measured**

- Goodput
- Completion time
- Retransmissions
- Failure rate

The protocol runs use the same file, configuration, and seed set.

---

### E2 — Goodput vs Window Size

**Independent variable**

ARQ window size.

Example values:

```text
1, 2, 4, 8, 16, 32
```

**Measured**

- Goodput
- Completion time
- Retransmissions

This evaluates how increasing the number of outstanding packets affects utilization and transfer performance.

---

### E3 — Retransmissions vs Reordering

**Independent variable**

Packet reordering probability.

Example values:

```text
0%, 5%, 10%, 20%, 30%, 40%
```

**Measured**

- Retransmission count
- Retransmission rate
- Goodput
- Duplicate ACKs

The main hypothesis is that Go-Back-N becomes increasingly inefficient under reordering because cumulative ACK gaps can trigger unnecessary retransmissions, while Selective Repeat can buffer valid out-of-order packets.

---

### E4 — RTO Sensitivity

The computed adaptive RTO is scaled using controlled multipliers:

```text
0.5×
0.75×
1.0×
1.5×
2.0×
```

**Measured**

- Goodput
- Timeout count
- Retransmissions
- Completion time
- Final SRTT
- Final RTO

This demonstrates the trade-off:

```text
RTO too small
      |
      v
Premature / spurious retransmissions


RTO too large
      |
      v
Slow recovery from real packet loss
```

The estimator parameters remain fixed while the effective RTO scale is varied.

---

## Metrics

The metrics system separates **application behavior**, **protocol behavior**, **timing**, and **channel behavior**.

### Application Metrics

- Source bytes
- Delivered unique bytes
- Transfer status
- SHA-256 result

### Timing Metrics

- Data-transfer completion time
- Sender completion time
- Receiver completion time
- Integrity-verification time
- RTT samples
- SRTT
- RTTVAR
- RTO values

### Protocol Metrics

- DATA sends
- ACK sends
- DATA receives
- ACK receives
- Retransmissions
- Timeout retransmissions
- Duplicate packets
- Duplicate ACKs

### Channel Metrics

- Offered datagrams
- Dropped datagrams
- Duplicated datagrams
- Corrupted datagrams
- Reordered datagrams
- Scheduled deliveries

### Goodput

Goodput is based on successfully delivered unique application bytes:

```text
goodput = unique application bytes delivered
          --------------------------------
          data-transfer completion time
```

Retransmitted and duplicate bytes are not included in the numerator.

Failed or integrity-failed runs are reported separately rather than silently treating them as zero-goodput runs.

---

## Reproducibility

Every experiment run records a reproducibility bundle containing information such as:

```text
run_id
experiment_id
protocol
configuration
seed
software revision
source-file hash
file size
terminal status
timing information
protocol counters
channel counters
integrity result
```

The experiment flow is:

```text
Experiment
    |
    +-- Protocol
    +-- Configuration
    +-- Seed
    +-- Source file
    +-- Software revision
              |
              v
        Deterministic run
              |
              v
          Raw result
              |
       +------+------+
       |             |
       v             v
     Tables        Plots
```

Logical channel impairment decisions are reproducible. Wall-clock completion time can still vary because of operating-system scheduling and machine load.

---

## Testing

Testing focuses on **protocol correctness**, not only successful transfers.

### Packet Tests

- Encode/decode round trips
- Boundary payload sizes
- Malformed packets
- Invalid lengths
- Invalid flags
- Checksum corruption
- Truncated datagrams

### Timer / RTO Tests

- RTT estimation
- RTO calculation
- RTO bounds
- Karn's algorithm
- Timeout backoff
- Timer cancellation
- Stale timer generation handling

### Channel Tests

- Loss
- Duplication
- Corruption
- Delay
- Jitter
- Reordering
- Combined impairments
- Seed reproducibility
- Direction-specific configuration

### ARQ Tests

Each protocol is tested against conditions including:

- Clean transfer
- Lost DATA
- Lost ACK
- Duplicate DATA
- Duplicate ACK
- Corrupted DATA
- Reordered DATA
- Delayed ACK
- Timeout
- Retry limit
- Final DATA loss
- Final ACK loss

### End-to-End Tests

The end-to-end suite includes:

- Empty files where supported
- 1-byte files
- Exact packet-sized files
- Files slightly larger than one packet
- Binary files
- Larger transfer fixtures
- Clean transfers
- Impaired transfers
- SHA-256 verification

A successful transfer must demonstrate both **protocol completion** and **file integrity**.

---

## Repository Structure

```text
.
├── README.md
├── AI-USE.md
├── docs/
│
├── src/
│   ├── app/
│   ├── packet/
│   ├── arq/
│   │   ├── sw/
│   │   ├── gbn/
│   │   └── sr/
│   ├── timing/
│   ├── channel/
│   └── metrics/
│
├── tests/
│   ├── unit/
│   │   ├── packet/
│   │   ├── timing/
│   │   ├── channel/
│   │   └── metrics/
│   ├── protocol/
│   │   ├── sw/
│   │   ├── gbn/
│   │   └── sr/
│   ├── integration/
│   └── fixtures/
│
├── configs/
│   ├── base/
│   └── experiments/
│
├── experiments/
├── scripts/
├── results/
├── plots/
└── reports/
```

The repository keeps protocol implementations, timing, channel emulation, metrics, experiments, and analysis separated so experimental logic does not leak into the transport implementation.

---

## Design Principles

### 1. Protocols share contracts

Switching from Stop-and-Wait to Go-Back-N or Selective Repeat should not require changing the application or experiment runner.

### 2. The emulator is protocol-independent

The ARQ layer never needs to know whether a packet was intentionally lost, reordered, duplicated, delayed, or corrupted.

### 3. Correctness comes before performance

A fast transfer that occasionally produces corrupted output is still a failed implementation.

### 4. Experiments must be reproducible

Configurations, seeds, software revisions, and raw results are preserved.

### 5. Raw data is never replaced by aggregates

Every graph and table should be traceable to individual experiment runs.

### 6. Core before stretch

Required reliability mechanisms and experiments are prioritized before optional extensions.

---

## Scope

### Core

- [ ] Stop-and-Wait ARQ
- [ ] Go-Back-N ARQ
- [ ] Selective Repeat ARQ
- [ ] Sequence numbers
- [ ] Checksums
- [ ] Cumulative acknowledgements
- [ ] Selective acknowledgements
- [ ] Adaptive RTO
- [ ] Jacobson/Karels RTT estimation
- [ ] Karn's algorithm
- [ ] Deterministic channel emulator
- [ ] SHA-256 integrity verification
- [ ] Automated correctness tests
- [ ] Reproducible experiment runner
- [ ] Required performance experiments
- [ ] Result aggregation and visualization

### Stretch

Stretch features are considered only after the core implementation and mandatory experiments are stable.

Possible extensions include:

- Additional SACK representations
- Additional flow-control mechanisms
- Connection setup/teardown
- Nagle-style packet coalescing
- Additional network impairment models

---

## What This Project Is Not

This project is intentionally **not**:

- A reimplementation of TCP
- A congestion-control implementation
- A production-grade file-transfer application
- An encrypted transport
- A GUI networking application
- An Internet-scale benchmark

The purpose is to study reliability mechanisms under a controlled and reproducible network model.

---

## Technology Stack

| Technology | Purpose |
|---|---|
| **Python** | Primary implementation language |
| **UDP sockets** | Transport |
| **threading / asyncio** | Concurrency and timers |
| **hashlib / SHA-256** | File integrity |
| **pytest** | Automated testing |
| **JSON** | Configuration and structured results |
| **CSV** | Experiment exports |
| **pandas** | Result analysis |
| **matplotlib** | Visualization |

No existing reliable-transport library is used to implement the ARQ protocols.

---

## Getting Started

> Installation and execution commands will be added here as the implementation stabilizes.

The intended workflow is:

```text
1. Configure an experiment
        ↓
2. Select ARQ protocol
        ↓
3. Configure deterministic channel
        ↓
4. Run sender and receiver
        ↓
5. Transfer file
        ↓
6. Verify SHA-256 integrity
        ↓
7. Record metrics
        ↓
8. Store raw result
        ↓
9. Aggregate and visualize
```

---

## Academic Project

This project is developed as **P3 — Reliable Data Transfer over UDP** for a Computer Networks coding assignment.

It focuses on demonstrating:

- UDP socket programming
- Transport-layer reliability
- ARQ protocols
- Sliding-window protocols
- Sequence numbers and acknowledgements
- RTT and retransmission timeout estimation
- Packet-level fault handling
- Deterministic network emulation
- Experimental methodology
- Reproducible networking experiments

---

## Project Goal

The central question behind the project is:

> **How do Stop-and-Wait, Go-Back-N, and Selective Repeat behave as the underlying network becomes unreliable, and how does adaptive timeout estimation affect their ability to recover efficiently?**

The project combines:

```text
UDP
 +
ARQ
 +
Adaptive RTO
 +
Deterministic Fault Injection
 +
Integrity Verification
 +
Reproducible Experiments
```

into one modular networking system.

---

## License

This project is released under the **MIT License**.

See [`LICENSE`](LICENSE) for the full license text.

---

<div align="center">

<br>

**Built to understand reliable transport from the ground up.**

*Reliable Data Transfer over UDP*

</div>
