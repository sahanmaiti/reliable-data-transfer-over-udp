# Experiment runner & analysis

`run_experiments.py` invokes the Rust release binary `run-experiment` once per trial and stores the JSON the binary writes. It does **not** invent retransmissions, RTT, RTO, or goodput.

`--study` is **required**:

| Flag | Study | Default output |
|---|---|---|
| `--study primary` | E1 reordering (90 trials) | `results/raw/` |
| `--study rto` | E4 RTO sensitivity (45 trials) | `results/rto_sensitivity/` |

Bare invocation fails so the frozen primary dataset cannot be overwritten accidentally. E4 refuses to write under `results/raw/`.

## E1 configuration (frozen)

StopAndWait (W=1), GoBackN (W=8), SelectiveRepeat (W=8); reorder 0–0.25; seeds 101–505; loss/dup/corrupt 0; delay 20 ms; jitter 0; hold 50 ms; chunk 1400; RTO multiplier 1.0; min RTO 200 ms; initial 1000 ms; max 60000 ms; fixture `fixtures/transfer_medium.bin`.

Submitted hash of `results/raw/experiments_raw.json`:

`d93dc2b18b49775c35587836e042a9daf6dfc844e85427a6be927180c6f0e8be`

## E4 configuration

Same protocols/seeds/fixture; multipliers 0.5 / 1.0 / 3.0; loss 0.05; reorder 0; **min RTO 10 ms**. See `results/rto_sensitivity/README.md`.

## Analysis

```bash
python3 experiments/python/plot_results.py \
  --input results/raw/experiments_raw.json \
  --output-dir results/analysis
python3 experiments/python/interpret_results.py
python3 experiments/python/analyze_rto_sensitivity.py
```

`plot_results.py` / `interpret_results.py` / `analyze_rto_sensitivity.py` only read Rust records. They do not fall back to the archived synthetic pilot.

Unit tests:

```bash
cd experiments/python && python3 -m unittest test_run_experiments test_plot_results
```
