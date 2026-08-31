# Reliable Data Transfer over UDP

A reliable file-transfer system built on top of UDP, implementing three interchangeable ARQ protocols — **Stop-and-Wait, Go-Back-N, and Selective Repeat** — with adaptive retransmission timeout estimation and a deterministic network channel emulator.

The project explores how different reliability strategies behave under controlled packet loss, duplication, reordering, corruption, delay, and jitter.

---

## Overview

UDP provides a lightweight datagram transport, but it does not guarantee:

- delivery
- ordering
- duplicate suppression
- integrity
- retransmission
- congestion or flow control

This project builds reliability above UDP without using an existing reliable-transport implementation.

The same file-transfer application can switch between three ARQ protocols:

| Protocol | Reliability Strategy | Window |
|---|---|---:|
| **Stop-and-Wait** | One outstanding packet at a time | 1 |
| **Go-Back-N** | Cumulative ACKs and retransmission of outstanding packets | Sliding |
| **Selective Repeat** | Selective ACKs with receiver buffering and selective retransmission | Sliding |

All protocols share the same packet format, channel interface, timing system, application layer, metrics system, and experiment framework.

---

## Key Features

### Reliable File Transfer

- UDP-based data transfer
- File segmentation and reconstruction
- Sequence numbers
- Packet checksums
- Duplicate detection
- Ordered delivery
- Reliable transfer completion
- SHA-256 integrity verification

### ARQ Protocols

- Stop-and-Wait ARQ
- Go-Back-N ARQ
- Selective Repeat ARQ
- Runtime protocol selection
- Protocol-independent application interface

### Adaptive Timing

- RTT measurement
- Jacobson/Karels RTT estimation
- Adaptive Retransmission Timeout (RTO)
- Karn's algorithm
- RTO bounds
- Timer generation IDs to prevent stale expirations

### Deterministic Network Emulator

The project includes a team-written channel emulator capable of injecting:

- packet loss
- packet duplication
- packet corruption
- packet reordering
- propagation delay
- jitter

Every run is controlled by a seed so that the same configuration and seed reproduce the same logical impairment decisions and event trace.

### Reproducible Experiments

The experiment framework records:

- protocol
- configuration
- random seed
- software revision
- transfer outcome
- completion time
- retransmissions
- timeout retransmissions
- channel impairment events
- RTT/RTO statistics
- SHA-256 integrity status

Raw results are preserved before aggregation so that reported graphs can be reproduced from individual runs.

---

## Architecture

```text
                    Configuration / Scenario / Seed
                                |
                                v
+-------------------------------------------------------------+
|                    Transfer Application                     |
|            File Source / File Sink / CLI / Hashing         |
+-----------------------------+-------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                         ARQ Layer                           |
|                                                             |
|       Stop-and-Wait   |   Go-Back-N   |   Selective Repeat |
+-----------------------+---------------+---------------------+
                              |
              +---------------+---------------+
              |                               |
              v                               v
       +-------------+                 +--------------+
       | Packet      |                 | Timer / RTO  |
       | Codec       |                 | Estimator    |
       +------+------+                 +------+-------+
              |                               |
              +---------------+---------------+
                              |
                              v
                  +-----------------------+
                  | UDP / Channel Layer  |
                  |                       |
                  | Deterministic Fault   |
                  | Injection Emulator    |
                  +-----------+-----------+
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
                       SHA-256 Verify

        All runtime components
                 |
                 v
        +----------------+
        | Metrics System |
        +-------+--------+
                |
                v
        Raw JSON / CSV Results
                |
                v
       Aggregation & Analysis
                |
                v
          Plots / Tables
```

The ARQ implementations never inspect emulator configuration or random decisions. The channel treats datagrams as opaque data, while the experiment framework selects protocols through a common interface.

---

## Protocols

### Stop-and-Wait

The sender transmits one packet and waits for its acknowledgement before sending the next packet.

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

If the ACK does not arrive before the RTO expires, the packet is retransmitted.

**Advantages**

- Simple state machine
- Minimal buffering
- Easy to reason about

**Disadvantages**

- Poor link utilization on high-delay paths
- Throughput is strongly RTT-limited

---

### Go-Back-N

The sender may have multiple packets outstanding simultaneously.

The receiver uses cumulative acknowledgements. If a packet is lost or arrives out of order, subsequent packets are not delivered until the missing packet is recovered.

```text
Sender                         Receiver

DATA 0 ---------------------->  0
DATA 1 ---------------------->  1
DATA 2 --------X

DATA 3 ---------------------->  discarded
DATA 4 ---------------------->  discarded

             <--------------- ACK 2

DATA 2 ---------------------->  2
DATA 3 ---------------------->  3
DATA 4 ---------------------->  4
```

A timeout for the window base can cause multiple outstanding packets to be retransmitted.

---

### Selective Repeat

Selective Repeat allows the receiver to buffer valid out-of-order packets and acknowledge them selectively.

```text
Sender                         Receiver

DATA 0 ----------------------> deliver 0
DATA 1 --------X

DATA 2 ----------------------> buffer 2
DATA 3 ----------------------> buffer 3

             <--------------- selective ACK

DATA 1 ----------------------> deliver 1

                              deliver buffered 2
                              deliver buffered 3
```

Only missing packets need to be retransmitted.

This makes Selective Repeat particularly interesting when packet reordering is high.

---

## Adaptive RTO

A fixed timeout is unsuitable across different network conditions.

The implementation therefore estimates the retransmission timeout dynamically using RTT samples.

The estimator follows the Jacobson/Karels approach:

```text
SRTT    = estimated smoothed RTT
RTTVAR  = estimated RTT variation

RTO = SRTT + max(G, 4 × RTTVAR)
```

Karn's algorithm is applied so that RTT samples from retransmitted packets are not used to update the estimator.

The timer system also uses generation identifiers to prevent an old timeout event from incorrectly expiring a newly acknowledged or rearmed packet.

---

## Deterministic Channel Emulator

Instead of relying on an uncontrolled real network, the project introduces a deterministic channel between the UDP endpoints.

Each direction can independently configure:

```text
loss probability
duplication probability
reordering probability
corruption probability
base delay
jitter
seed
```

Conceptually:

```text
UDP Sender
    |
    v
+---------------------------+
| Deterministic Channel     |
|                           |
| Loss                      |
| Duplication               |
| Corruption                |
| Delay + Jitter            |
| Reordering                |
+-------------+-------------+
              |
              v
         UDP Receiver
```

The emulator operates on serialized datagrams and does not understand ARQ semantics.

This separation allows the same network conditions to be applied fairly to all three protocols.

---

## Integrity Verification

Successful completion is not considered sufficient evidence of correctness.

After the receiver reconstructs the file, the source and received files are compared using SHA-256:

```text
Source file
    |
    +---- SHA-256 ----+
                     |
                     v
                 Compare
                     ^
                     |
    +---- SHA-256 ---+
    |
Received file
```

A successful run must satisfy:

```text
source_sha256 == received_sha256
```

The system also checks the number of delivered bytes and prevents duplicate delivery from corrupting the reconstructed file.

---

## Experiments

The project evaluates the three protocols under controlled conditions.

### E1 — Goodput vs Loss

**Independent variable**

Packet loss probability.

**Measured**

- goodput
- completion time
- retransmissions
- failure rate

The same file, packet size, delay model, window configuration, and seed set are used across protocols.

---

### E2 — Goodput vs Window Size

**Independent variable**

ARQ window size.

Example values:

```text
1, 2, 4, 8, 16, 32
```

**Measured**

- goodput
- completion time
- retransmissions

This experiment investigates how increasing the number of outstanding packets affects transfer performance.

---

### E3 — Retransmissions vs Reordering

**Independent variable**

Packet reordering probability.

Example values:

```text
0%, 5%, 10%, 20%, 30%, 40%
```

**Measured**

- retransmission count
- retransmission rate
- goodput
- duplicate ACKs

The primary hypothesis is that Go-Back-N becomes increasingly inefficient as reordering causes cumulative ACK gaps, while Selective Repeat can buffer valid out-of-order packets and retransmit only missing data.

---

### E4 — RTO Sensitivity

The effective adaptive RTO is scaled around its computed value:

```text
0.5×
0.75×
1.0×
1.5×
2.0×
```

**Measured**

- goodput
- timeout count
- retransmissions
- completion time
- final SRTT
- final RTO

This experiment demonstrates the trade-off between:

```text
Too-small RTO
    ↓
Premature / spurious retransmissions

Too-large RTO
    ↓
Slow recovery from genuine packet loss
```

The Jacobson/Karels estimator parameters remain fixed while the effective RTO scale is varied.

---

## Metrics

The experiment framework distinguishes between network impairments and protocol behavior.

### Application

- source bytes
- delivered unique bytes
- completion status
- SHA-256 result

### Timing

- data-transfer completion time
- sender completion time
- receiver completion time
- integrity-verification time
- RTT samples
- RTO values

### Protocol

- DATA sends
- ACK sends
- DATA receives
- ACK receives
- retransmissions
- timeout retransmissions
- duplicate packets
- duplicate ACKs

### Channel

- offered datagrams
- dropped datagrams
- duplicated datagrams
- corrupted datagrams
- reordered datagrams
- scheduled deliveries

### Goodput

Goodput is defined using successfully delivered unique application bytes:

```text
goodput = unique application bytes delivered
          --------------------------------
          data-transfer completion time
```

Retransmitted and duplicate bytes are not included in the numerator.

Failed or integrity-failed runs are reported separately rather than being treated as zero-goodput runs.

---

## Reproducibility

Every experiment run is associated with:

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

A run can therefore be reproduced from its configuration and seed.

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

Real wall-clock timing may vary because of operating-system scheduling and system load, but the emulator's logical impairment decisions remain deterministic.

---

## Testing

Testing is designed around protocol correctness rather than only successful transfers.

### Packet Tests

- packet round-trip encoding/decoding
- boundary payload sizes
- malformed packets
- invalid lengths
- invalid flags
- checksum corruption
- truncated datagrams

### Timer / RTO Tests

- RTT estimation
- RTO calculation
- RTO bounds
- Karn's algorithm
- timeout backoff
- timer cancellation
- stale timer generations

### Channel Tests

- loss
- duplication
- corruption
- delay
- jitter
- reordering
- combined impairments
- seed reproducibility
- directional configurations

### ARQ Tests

Each protocol is tested against:

- clean transfer
- lost DATA
- lost ACK
- duplicate DATA
- duplicate ACK
- corrupted DATA
- reordered DATA
- delayed ACK
- timeout
- retry limit
- final DATA loss
- final ACK loss

### End-to-End Tests

The end-to-end suite includes:

- empty files where supported
- 1-byte files
- exact packet-sized files
- files slightly larger than one packet
- binary files
- larger transfer fixtures
- clean transfers
- impaired transfers
- SHA-256 verification

Every successful transfer must demonstrate both correct protocol completion and byte-for-byte file integrity.

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

The repository separates protocol implementation, timing, network emulation, metrics, experimentation, and analysis so that experimental code does not leak into the transport implementation.

---

## Design Principles

The project follows several important engineering rules:

### 1. Protocols share contracts

Changing from Stop-and-Wait to Go-Back-N or Selective Repeat should not require changing the application or experiment runner.

### 2. The emulator is protocol-independent

The ARQ implementation never knows whether a packet was lost, reordered, duplicated, or corrupted intentionally.

### 3. Experiments are reproducible

Configurations and seeds are recorded with raw results.

### 4. Correctness comes before performance

A fast transfer that occasionally produces corrupted output is a failure.

### 5. Raw data is never replaced by aggregate results

Every reported graph should be traceable back to individual experimental runs.

### 6. Core before stretch

The primary implementation focuses on the required ARQ protocols, adaptive timing, deterministic emulation, integrity, testing, and experiments before considering additional features.

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

Stretch features will only be considered after the Core implementation and mandatory experiments are stable.

Potential extensions include:

- SACK blocks beyond the required selective-ACK representation
- additional flow-control mechanisms
- connection setup/teardown
- Nagle-style packet coalescing

---

## What This Project Is Not

This project is intentionally **not**:

- an implementation of TCP
- a congestion-control implementation
- a production file-transfer application
- an encrypted transport
- a GUI-based networking tool
- an Internet-scale benchmark

The experiments measure the behavior of these implementations under a controlled local/emulated network model.

---

## Project Goal

The central goal is not simply to "make UDP reliable."

It is to build a controlled experimental environment in which different reliability mechanisms can be implemented, tested, compared, and explained.

The project investigates:

> **How do Stop-and-Wait, Go-Back-N, and Selective Repeat behave when the underlying network becomes unreliable, and how does adaptive timeout estimation affect their ability to recover efficiently?**

The resulting system combines:

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

into a single modular networking project.

---

## Technology Stack

- **Language:** Python
- **Transport:** UDP sockets
- **Concurrency:** `threading` / `asyncio`
- **Integrity:** SHA-256 via `hashlib`
- **Testing:** `pytest` / standard testing tools
- **Configuration:** JSON
- **Results:** JSON / CSV
- **Analysis:** pandas
- **Visualization:** matplotlib

No existing reliable-transport library is used for the protocol implementation.

---

## Academic Project

This project is developed as **P3 — Reliable Data Transfer over UDP** for the Computer Networks coding assignment.

The implementation focuses on demonstrating:

- UDP socket programming
- transport-layer reliability concepts
- ARQ protocols
- sliding-window protocols
- sequence numbers and acknowledgements
- RTT and timeout estimation
- packet-level fault handling
- experimental methodology
- reproducible networking experiments

---

## License

This project is released under the **MIT License**.

See `LICENSE` for the full license text.

---

## Status

🚧 **Active development**

The repository is being developed incrementally, with correctness and reproducibility treated as the primary milestones before performance optimization and stretch features.
