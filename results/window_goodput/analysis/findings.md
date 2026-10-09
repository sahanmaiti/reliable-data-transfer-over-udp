# Window-size goodput study

These numbers are computed from the Rust records in this directory.
Intervals are two-sided 95% Student-t intervals on the successful trials in each cell.
With five seeds, df = 4 when every seed succeeded. The interval is not truncated at zero.
Paired rows are Go-Back-N minus Selective Repeat on the shared seed.
Exploratory p-values are uncorrected. A small p-value is not treated as proof that one protocol is better.
Agreement between protocols is reported as an observation. It is not a pass condition.

## Design

- Study: `window_goodput`
- Records loaded: 65
- Seeds: 101, 202, 303, 404, 505
- File chunk: 1400 bytes
- Reorder: 0.0
- RTO multiplier: 1.0
- min/initial/max RTO: 200 / 1000 / 60000 ms
- Delay / jitter / reorder hold: 20 / 0 / 50 ms
- Windows for Go-Back-N and Selective Repeat: 1, 2, 4, 8, 16, 32
- Stop-and-Wait is included only at window 1.
- Loss, duplication, and corruption are 0.

## Data quality

Every loaded trial has transfer_status SUCCESS and sha256_match true.

## Cell means

| protocol | window_size | metric | n | mean | 95% CI |
|---|---|---|---|---|---|
| StopAndWait | 1 | goodput_bytes_sec | 5 | 35175.9 | [35175.9, 35175.9] |
| StopAndWait | 1 | data_retransmissions | 5 | 0 | [0, 0] |
| StopAndWait | 1 | duration_secs | 5 | 3.98 | [3.98, 3.98] |
| GoBackN | 1 | goodput_bytes_sec | 5 | 35175.9 | [35175.9, 35175.9] |
| GoBackN | 1 | data_retransmissions | 5 | 0 | [0, 0] |
| GoBackN | 1 | duration_secs | 5 | 3.98 | [3.98, 3.98] |
| GoBackN | 2 | goodput_bytes_sec | 5 | 70707.1 | [70707.1, 70707.1] |
| GoBackN | 2 | data_retransmissions | 5 | 0 | [0, 0] |
| GoBackN | 2 | duration_secs | 5 | 1.98 | [1.98, 1.98] |
| GoBackN | 4 | goodput_bytes_sec | 5 | 142857 | [142857, 142857] |
| GoBackN | 4 | data_retransmissions | 5 | 0 | [0, 0] |
| GoBackN | 4 | duration_secs | 5 | 0.98 | [0.98, 0.98] |
| GoBackN | 8 | goodput_bytes_sec | 5 | 280000 | [280000, 280000] |
| GoBackN | 8 | data_retransmissions | 5 | 0 | [0, 0] |
| GoBackN | 8 | duration_secs | 5 | 0.5 | [0.5, 0.5] |
| GoBackN | 16 | goodput_bytes_sec | 5 | 538462 | [538462, 538462] |
| GoBackN | 16 | data_retransmissions | 5 | 0 | [0, 0] |
| GoBackN | 16 | duration_secs | 5 | 0.26 | [0.26, 0.26] |
| GoBackN | 32 | goodput_bytes_sec | 5 | 1e+06 | [1e+06, 1e+06] |
| GoBackN | 32 | data_retransmissions | 5 | 0 | [0, 0] |
| GoBackN | 32 | duration_secs | 5 | 0.14 | [0.14, 0.14] |
| SelectiveRepeat | 1 | goodput_bytes_sec | 5 | 35175.9 | [35175.9, 35175.9] |
| SelectiveRepeat | 1 | data_retransmissions | 5 | 0 | [0, 0] |
| SelectiveRepeat | 1 | duration_secs | 5 | 3.98 | [3.98, 3.98] |
| SelectiveRepeat | 2 | goodput_bytes_sec | 5 | 70707.1 | [70707.1, 70707.1] |
| SelectiveRepeat | 2 | data_retransmissions | 5 | 0 | [0, 0] |
| SelectiveRepeat | 2 | duration_secs | 5 | 1.98 | [1.98, 1.98] |
| SelectiveRepeat | 4 | goodput_bytes_sec | 5 | 142857 | [142857, 142857] |
| SelectiveRepeat | 4 | data_retransmissions | 5 | 0 | [0, 0] |
| SelectiveRepeat | 4 | duration_secs | 5 | 0.98 | [0.98, 0.98] |
| SelectiveRepeat | 8 | goodput_bytes_sec | 5 | 280000 | [280000, 280000] |
| SelectiveRepeat | 8 | data_retransmissions | 5 | 0 | [0, 0] |
| SelectiveRepeat | 8 | duration_secs | 5 | 0.5 | [0.5, 0.5] |
| SelectiveRepeat | 16 | goodput_bytes_sec | 5 | 538462 | [538462, 538462] |
| SelectiveRepeat | 16 | data_retransmissions | 5 | 0 | [0, 0] |
| SelectiveRepeat | 16 | duration_secs | 5 | 0.26 | [0.26, 0.26] |
| SelectiveRepeat | 32 | goodput_bytes_sec | 5 | 1e+06 | [1e+06, 1e+06] |
| SelectiveRepeat | 32 | data_retransmissions | 5 | 0 | [0, 0] |
| SelectiveRepeat | 32 | duration_secs | 5 | 0.14 | [0.14, 0.14] |

## Paired Go-Back-N minus Selective Repeat

| window_size | metric | n | mean difference | 95% CI | exploratory p |
|---|---|---|---|---|---|
| 1 | goodput_bytes_sec | 5 | 0 | [0, 0] |  |
| 1 | data_retransmissions | 5 | 0 | [0, 0] |  |
| 1 | duration_secs | 5 | 0 | [0, 0] |  |
| 2 | goodput_bytes_sec | 5 | 0 | [0, 0] |  |
| 2 | data_retransmissions | 5 | 0 | [0, 0] |  |
| 2 | duration_secs | 5 | 0 | [0, 0] |  |
| 4 | goodput_bytes_sec | 5 | 0 | [0, 0] |  |
| 4 | data_retransmissions | 5 | 0 | [0, 0] |  |
| 4 | duration_secs | 5 | 0 | [0, 0] |  |
| 8 | goodput_bytes_sec | 5 | 0 | [0, 0] |  |
| 8 | data_retransmissions | 5 | 0 | [0, 0] |  |
| 8 | duration_secs | 5 | 0 | [0, 0] |  |
| 16 | goodput_bytes_sec | 5 | 0 | [0, 0] |  |
| 16 | data_retransmissions | 5 | 0 | [0, 0] |  |
| 16 | duration_secs | 5 | 0 | [0, 0] |  |
| 32 | goodput_bytes_sec | 5 | 0 | [0, 0] |  |
| 32 | data_retransmissions | 5 | 0 | [0, 0] |  |
| 32 | duration_secs | 5 | 0 | [0, 0] |  |

## Regression against the frozen primary reorder-0 cells

Compared fields: transfer status, SHA-256 match, delivered digest, data retransmissions, premature retransmissions, timeout count, virtual duration, and goodput.
Only cells with loss 0, reorder 0, and the primary window (1 for Stop-and-Wait, 8 otherwise) are compared. The primary file is read and not rewritten.
All 120 compared field values match the frozen primary records.

## Limitations

- Virtual-time driver, not the localhost UDP path.
- Five seeds per cell. Intervals can be wide.
- The window study does not cross window size with loss. The loss study holds window size fixed.
- Goodput is the value already stored on the Rust record (`goodput_bytes_sec`). This script does not redefine it.
