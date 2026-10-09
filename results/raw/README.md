# Raw experiment results

`experiments_raw.json` and `experiments_raw.csv` are the primary reordering study: 90 Rust `ExperimentRecord` objects written by `experiments/python/run_experiments.py` from `reliable_udp run-experiment`.

The run completed. Every trial in that dataset has `transfer_status` `SUCCESS` and matching SHA-256 digests. Per-trial JSON files are in `trials/`. `execution_log.json` is provenance only; timestamps in that log are not experiment ids.

Input file: `fixtures/transfer_medium.bin` (140,000 bytes).

SHA-256: `8e6c7049a65aeff8211b2bbd78db47683a1b86f75565ad3362fb6e53a0a4cbbf`

The binary was built from the working tree whose `HEAD` was `3255730d0b9452bc405087f6244f5740356d63a8`. That tree had uncommitted implementation changes. Rust 1.99.0, Python 3.12.13, macOS 27.0.1 (Darwin 27.0.0 arm64). Wall time for the 90 trials was under one second because the driver uses virtual time.

`archive/synthetic_pilot/` holds the old formula-generated files. Those are not measurements. Do not cite them.

Dataset SHA-256, checked again before analysis and unchanged afterward:

- `experiments_raw.json`: `d93dc2b18b49775c35587836e042a9daf6dfc844e85427a6be927180c6f0e8be`
- `experiments_raw.csv`: `ac071d325a07a41e86f70348579a5ee0e7f0e494d9f1ded4809ddb2f05ffa108`

Analysis output is in `results/analysis/`. The statistical method is the Phase 6 Student-t interval. This directory does not contain a research conclusion.
