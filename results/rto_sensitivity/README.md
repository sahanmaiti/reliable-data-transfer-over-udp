# E4 RTO Sensitivity Study

Secondary experiment required by the project proposal. Completely separate from the
frozen primary 90-trial reordering study under `results/raw/`.

## Matrix

| Factor | Values |
| --- | --- |
| Protocols | StopAndWait, GoBackN, SelectiveRepeat |
| RTO multipliers | 0.5, 1.0, 3.0 |
| Seeds | 101, 202, 303, 404, 505 |
| Loss | 0.05 |
| Reorder / duplicate / corrupt | 0 |
| Base delay / jitter / reorder hold | 20 ms / 0 / 50 ms |
| File | `fixtures/transfer_medium.bin` (140,000 bytes) |
| Chunk | 1,400 bytes |
| Windows | StopAndWait=1; GBN/SR=8 |
| min RTO | **10 ms (E4 only)** |
| initial / max RTO | 1000 ms / 60000 ms |

Total: **45 trials**.

## How to reproduce

```bash
cargo build --release
python3 experiments/python/run_experiments.py --study rto
python3 experiments/python/analyze_rto_sensitivity.py
```

`--study` is required. A bare runner invocation does not execute or overwrite anything.
E4 never writes under `results/raw/`.

## Layout

```
results/rto_sensitivity/
  experiments_raw.json
  experiments_raw.csv
  trials/*.json
  smoke/          # optional two-trial clamp check
  smoke_m3/
  analysis/
    summary.csv
    paired_by_seed.csv
    findings.md
    *.svg
```

## Analysis outputs

See `analysis/findings.md` for measured behaviour. Do not treat exploratory p-values
as a claim that one multiplier is universally better.
