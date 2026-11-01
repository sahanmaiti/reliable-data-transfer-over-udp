# Integration Overview

Reliable transfer is assembled in two ways. Both reuse the same ARQ, packet, timing, chunking, and integrity modules.

## Deterministic experiment path

```text
File → Chunker → ARQ sender → Channel emulator → scheduler
      → ARQ receiver → reverse channel → ACK → RTO
      → Reassembler → SHA-256 → ExperimentRecord
```

- Driver: `src/app/transfer.rs`
- CLI: `reliable_udp run-experiment ...`
- Orchestration: `experiments/python/run_experiments.py --study primary|rto`

## Real UDP path

```text
File → Chunker → ARQ sender → serialize → UdpSocket
UdpSocket → deserialize → ARQ receiver → Reassembler → SHA-256
```

- Driver: `src/app/udp.rs`
- CLI: `reliable_udp send ...` / `reliable_udp recv ...`
- The channel emulator is **not** used on this path.

## Python role

Python only orchestrates trials and analyses Rust JSON/CSV. It does not invent retransmission counts or implement ARQ.
