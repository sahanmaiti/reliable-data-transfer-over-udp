# Analysis provenance

This file records the tree that produced `results/raw/experiments_raw.json`. It is not a scientific conclusion.

## Git

- Branch: `main`
- HEAD: `3255730d0b9452bc405087f6244f5740356d63a8`
- Status: dirty. HEAD alone does not describe the binary that generated the dataset.
- Exact tracked diff: `results/analysis/git-diff.txt` (16,599 lines).
- Status snapshot taken before analysis: `results/analysis/git-status.txt`.

Tracked modifications at acquisition time included the transport driver, Go-Back-N initial-ACK fix, metrics schema, CLI, experiment runner, and the raw dataset files themselves. Untracked files included `experiments/python/README.md`, the Python tests, `tests/test_cli.rs`, `tests/test_transfer.rs`, `results/raw/trials/`, `results/raw/execution_log.json`, and `results/raw/archive/`.

`src/app/file_io.rs` and `src/app/integrity.rs` are also modified relative to HEAD. Those diffs are in `git-diff.txt`.

## Input fixture

- Path: `fixtures/transfer_medium.bin`
- Size: 140,000 bytes
- SHA-256: `8e6c7049a65aeff8211b2bbd78db47683a1b86f75565ad3362fb6e53a0a4cbbf`

## Dataset

- `results/raw/experiments_raw.json`
- SHA-256: `d93dc2b18b49775c35587836e042a9daf6dfc844e85427a6be927180c6f0e8be`
- Records: 90
- `results/raw/experiments_raw.csv`
- SHA-256: `ac071d325a07a41e86f70348579a5ee0e7f0e494d9f1ded4809ddb2f05ffa108`
- Per-trial files: `results/raw/trials/` (90 files). Each parsed trial file equals the corresponding object in the aggregate JSON.
- These hashes were the same after analysis.

## Toolchain

- Rust: 1.99.0 (b940084d7 2026-09-28)
- Python: 3.12.13
- OS: macOS 27.0.1, Darwin 27.0.0 arm64

## Analysis method

Phase 6 `plot_results.py`, unchanged for this run. For each protocol and configured reorder rate, n, mean, sample standard deviation (divisor n−1), standard error, and a 95% Student-t interval `mean ± t_{n-1, 0.975} * s / sqrt(n)`. Means use only `SUCCESS` trials with `sha256_match` true. This dataset has 5 successes in every cell, so every interval uses df = 4. No bootstrap and no smoothing.
