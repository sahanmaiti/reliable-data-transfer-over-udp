# Demo Script (≈ 3–5 minutes)

## Team contributions

Responsibility areas (not a claim that every listed task was completed solely by one person):

- **Soumyadeb Mukherjee (24052329)** — Protocol & ARQ
- **Kashish Gupta (24052495)** — Timing & RTO
- **Ashwika Burman (24155095)** — Channel Emulator
- **Sahan Maiti (24052247)** — Evaluation & Integration

Full wording and AI disclosure: `README.md`, `AI-USE.md`.

## 1. Introduction (30 s)

UDP delivers datagrams without reliability. This project adds a reliable file-transfer layer with three runtime-selectable ARQ protocols (Stop-and-Wait, Go-Back-N, Selective Repeat), adaptive Jacobson/Karels RTO, SHA-256 integrity, a deterministic channel emulator for experiments, and a real localhost UDP demo.

## 2. Architecture (30 s)

Show two paths (whiteboard or `docs/FINAL_REPORT.md` §4):

- **Experiment path:** virtual-time driver + seeded channel → JSON metrics (E1/E4).
- **Demo path:** real `UdpSocket` send/recv using the **same** ARQ machines (no emulator).

## 3. Build / test (45 s)

```bash
cargo build --release
cargo test --test test_udp
```

Say: full `cargo test` covers packet, ARQ, RTO, channel, virtual transfer, and UDP loopback. Do **not** rerun the 90-trial matrix live.

## 4. Real UDP demonstration (90 s)

**Terminal 1 — receiver**

```bash
cargo run --release -- recv \
  --bind 127.0.0.1:19001 \
  --output /tmp/rdt_demo_out.bin \
  --protocol GoBackN \
  --window 8 \
  --chunk-size 1400
```

**Terminal 2 — sender**

```bash
cargo run --release -- send \
  --file fixtures/transfer_medium.bin \
  --to 127.0.0.1:19001 \
  --protocol GoBackN \
  --window 8 \
  --chunk-size 1400
```

Point out SUCCESS on both sides. Mention the same engine supports `--protocol StopAndWait` and `SelectiveRepeat` (both sides must match).

## 5. Integrity (30 s)

```bash
cargo run --release -- verify \
  --source fixtures/transfer_medium.bin \
  --received /tmp/rdt_demo_out.bin
```

Expect matching SHA-256.

## 6. Research results (60–90 s)

Open (do not regenerate):

1. `results/analysis/data_retransmissions_vs_reorder.svg` — GBN rises with reorder; SR stays at 0 under E1’s zero-loss design.
2. `results/analysis/goodput_vs_reorder.svg` or duration plot — SR stays fast; GBN slows at high reorder.
3. `results/rto_sensitivity/analysis/premature_retransmissions_vs_multiplier.svg` — premature counts at 0.5× only in E4.

One-sentence primary claim: *Under our zero-loss reordering study, Go-Back-N retransmitted more than Selective Repeat as reorder increased.*

## 7. Closing (20 s)

We built interchangeable ARQ over shared framing and RTO, measured them under a deterministic emulator, and demonstrated the same logic on real localhost UDP. Limitations: virtual-time experiments ≠ socket wall-clock; E1 uses loss=0; five seeds; localhost UDP focus.
