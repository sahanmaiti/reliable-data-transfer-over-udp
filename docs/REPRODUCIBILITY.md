# Reproducibility Guide

Commands below match the current repository runners. **Do not regenerate** frozen E1/E4 datasets unless you intentionally replace them; hashes below identify the submitted measurements.

Verified toolchain (from `results/analysis/provenance.md` / Phase 11 audit):

- Rust: 1.99.0
- Python: 3.12.13
- OS used for acquisition: macOS (Darwin) arm64

---

## 1. Build Rust

```bash
cargo build --release
```

Binary: `target/release/reliable_udp` (or `$CARGO_TARGET_DIR/release/reliable_udp` if that environment variable is set).

---

## 2. Run tests

```bash
cargo test
```

Covers packet, ARQ, timing, channel, transfer (virtual-time), CLI helpers, and real UDP loopback (`tests/test_udp.rs`: StopAndWait, GoBackN, SelectiveRepeat, empty file).

Python runner unit tests (no Rust matrix):

```bash
cd experiments/python
python3 -m unittest test_run_experiments test_plot_results
```

---

## 3. Real UDP sender / receiver (localhost)

Terminal 1 — receiver:

```bash
cargo run --release -- recv \
  --bind 127.0.0.1:19001 \
  --output /tmp/rdt_out.bin \
  --protocol GoBackN \
  --window 8 \
  --chunk-size 1400
```

Terminal 2 — sender:

```bash
cargo run --release -- send \
  --file fixtures/transfer_medium.bin \
  --to 127.0.0.1:19001 \
  --protocol GoBackN \
  --window 8 \
  --chunk-size 1400
```

Protocols: `StopAndWait`, `GoBackN`, `SelectiveRepeat` (must match on both sides).  
Optional sender RTO knobs: `--rto-multiplier`, `--min-rto-ms`, `--max-rto-ms`, `--initial-rto-ms`, `--deadline-secs`.

Verify:

```bash
cargo run --release -- verify \
  --source fixtures/transfer_medium.bin \
  --received /tmp/rdt_out.bin
```

---

## 4. E1 — Primary reordering study

**Submitted dataset already exists.** Re-running overwrites `results/raw/` and must not be done for submission QA.

Configuration used for the frozen 90 trials:

- `--study primary` (required)
- protocols × reorder `{0,0.05,0.10,0.15,0.20,0.25}` × seeds `{101,202,303,404,505}`
- loss/dup/corrupt 0; delay 20 ms; jitter 0; reorder hold 50 ms
- chunk 1400; SW window 1; GBN/SR window 8
- RTO multiplier 1.0; min 200 ms; initial 1000 ms; max 60000 ms
- input `fixtures/transfer_medium.bin`

Command that produced / would reproduce the matrix:

```bash
cargo build --release
python3 experiments/python/run_experiments.py --study primary
```

Smoke (writes outside `results/raw/`):

```bash
python3 experiments/python/run_experiments.py \
  --study primary --protocol StopAndWait --reorder 0 --seed 101 --max-trials 1 \
  --output-dir /tmp/rdt-smoke-primary
```

Dataset hashes (must match submission):

- `results/raw/experiments_raw.json` → `d93dc2b18b49775c35587836e042a9daf6dfc844e85427a6be927180c6f0e8be`
- `results/raw/experiments_raw.csv` → `ac071d325a07a41e86f70348579a5ee0e7f0e494d9f1ded4809ddb2f05ffa108`

---

## 5. E4 — RTO sensitivity study

**Submitted dataset already exists** under `results/rto_sensitivity/`. Do not overwrite for QA.

Configuration:

- `--study rto`
- multipliers `{0.5, 1.0, 3.0}` × three protocols × five seeds = 45
- loss 0.05; reorder 0; min RTO **10 ms**; other timing as in E4 README

```bash
python3 experiments/python/run_experiments.py --study rto
```

Bare invocation without `--study` fails (protects the primary dataset).

---

## 5b. Window-size goodput study

Separate from E1 and E4. Do not point `--output-dir` at `results/raw/` or `results/rto_sensitivity/`.

- `--study window`
- Go-Back-N and Selective Repeat at windows `{1, 2, 4, 8, 16, 32}` × five seeds = 60
- Stop-and-Wait only at window 1 × five seeds = 5
- Total **65** trials
- loss/dup/corrupt 0; reorder 0; delay 20 ms; jitter 0; reorder hold 50 ms
- chunk 1400; RTO multiplier 1.0; min 200 ms; initial 1000 ms; max 60000 ms
- input `fixtures/transfer_medium.bin`

```bash
python3 experiments/python/run_experiments.py --study window
```

Dataset hashes:

- `results/window_goodput/experiments_raw.json` → `34a84ef13f7c059a437fead2e3f0cbd0906756fdd525d29b6250bdfcbff526b4`
- `results/window_goodput/experiments_raw.csv` → `0d9a756db6f629395193975fb93a70b9394dbdf7b95afc0d84034a0ade4c7853`

---

## 5c. Loss-rate goodput study

Separate directory. Same file, seeds, delay, and RTO bounds as E1. Window stays at 8 (Stop-and-Wait stays at 1). Reorder stays at 0.

- `--study loss`
- loss `{0, 0.05, 0.10, 0.15, 0.20, 0.25}` × three protocols × five seeds = **90**

```bash
python3 experiments/python/run_experiments.py --study loss
```

Dataset hashes:

- `results/loss_goodput/experiments_raw.json` → `b4e663644dbc4fab426322150de9755461ef0586cd4cbac9515e0ade915c526f`
- `results/loss_goodput/experiments_raw.csv` → `4488c9c874251a219df126f2c1687f0669045be927323b511cccfd3d3e6ce3a0`

The loss-0 cells, and the window-study cells at the primary window, matched the frozen E1 reorder-0 records on status, digest, retransmissions, timeouts, duration, and goodput. That check is `analysis/regression_vs_primary.csv` in each study directory.

---

## 6. Analysis

E1 (reads frozen raw JSON; does not invent measurements):

```bash
python3 experiments/python/plot_results.py \
  --input results/raw/experiments_raw.json \
  --output-dir results/analysis

python3 experiments/python/interpret_results.py
```

E4:

```bash
python3 experiments/python/analyze_rto_sensitivity.py
```

Window and loss studies:

```bash
python3 experiments/python/analyze_catalogue_sweeps.py --study window
python3 experiments/python/analyze_catalogue_sweeps.py --study loss
```

---

## 7. Where figures and tables live

| Study | Tables / text | Figures |
|---|---|---|
| E1 | `results/analysis/summary.csv`, `primary_results.csv`, `protocol_comparisons.csv`, `research_findings.md`, `primary_comparison.txt` | `results/analysis/*_vs_reorder.svg` (and related) |
| E4 | `results/rto_sensitivity/analysis/summary.csv`, `paired_by_seed.csv`, `findings.md` | `results/rto_sensitivity/analysis/*_vs_multiplier.svg` |
| Window | `results/window_goodput/analysis/summary.csv`, `paired_by_seed.csv`, `regression_vs_primary.csv`, `findings.md` | `results/window_goodput/analysis/*_vs_window.svg` |
| Loss | `results/loss_goodput/analysis/summary.csv`, `paired_by_seed.csv`, `regression_vs_primary.csv`, `findings.md` | `results/loss_goodput/analysis/*_vs_loss.svg` |

Narrative report: `docs/FINAL_REPORT.md`.
