# Loss-rate goodput study

These numbers are computed from the Rust records in this directory.
Intervals are two-sided 95% Student-t intervals on the successful trials in each cell.
With five seeds, df = 4 when every seed succeeded. The interval is not truncated at zero.
Paired rows are Go-Back-N minus Selective Repeat on the shared seed.
Exploratory p-values are uncorrected. A small p-value is not treated as proof that one protocol is better.
Agreement between protocols is reported as an observation. It is not a pass condition.

## Design

- Study: `loss_goodput`
- Records loaded: 90
- Seeds: 101, 202, 303, 404, 505
- File chunk: 1400 bytes
- Reorder: 0.0
- RTO multiplier: 1.0
- min/initial/max RTO: 200 / 1000 / 60000 ms
- Delay / jitter / reorder hold: 20 / 0 / 50 ms
- Loss rates: 0.00, 0.05, 0.10, 0.15, 0.20, 0.25
- Go-Back-N and Selective Repeat use window 8. Stop-and-Wait uses window 1.
- Reorder, duplication, and corruption are 0.

## Data quality

Every loaded trial has transfer_status SUCCESS and sha256_match true.

## Cell means

| protocol | loss_rate | metric | n | mean | 95% CI |
|---|---|---|---|---|---|
| StopAndWait | 0.00 | goodput_bytes_sec | 5 | 35175.9 | [35175.9, 35175.9] |
| StopAndWait | 0.00 | data_retransmissions | 5 | 0 | [0, 0] |
| StopAndWait | 0.00 | duration_secs | 5 | 3.98 | [3.98, 3.98] |
| StopAndWait | 0.05 | goodput_bytes_sec | 5 | 28086.7 | [21803.7, 34369.7] |
| StopAndWait | 0.05 | data_retransmissions | 5 | 4.6 | [0.426088, 8.77391] |
| StopAndWait | 0.05 | duration_secs | 5 | 5.14 | [3.78664, 6.49336] |
| StopAndWait | 0.10 | goodput_bytes_sec | 5 | 19198.9 | [15390.5, 23007.2] |
| StopAndWait | 0.10 | data_retransmissions | 5 | 11.8 | [7.83444, 15.7656] |
| StopAndWait | 0.10 | duration_secs | 5 | 7.46 | [5.81649, 9.10351] |
| StopAndWait | 0.15 | goodput_bytes_sec | 5 | 14456.9 | [10542.5, 18371.3] |
| StopAndWait | 0.15 | data_retransmissions | 5 | 17.2 | [15.5811, 18.8189] |
| StopAndWait | 0.15 | duration_secs | 5 | 10.1 | [7.11174, 13.0883] |
| StopAndWait | 0.20 | goodput_bytes_sec | 5 | 10746.1 | [7425.5, 14066.7] |
| StopAndWait | 0.20 | data_retransmissions | 5 | 24.8 | [19.5761, 30.0239] |
| StopAndWait | 0.20 | duration_secs | 5 | 13.66 | [9.68823, 17.6318] |
| StopAndWait | 0.25 | goodput_bytes_sec | 5 | 8284.44 | [4504.55, 12064.3] |
| StopAndWait | 0.25 | data_retransmissions | 5 | 31.6 | [22.201, 40.999] |
| StopAndWait | 0.25 | duration_secs | 5 | 18.9 | [9.79326, 28.0067] |
| GoBackN | 0.00 | goodput_bytes_sec | 5 | 280000 | [280000, 280000] |
| GoBackN | 0.00 | data_retransmissions | 5 | 0 | [0, 0] |
| GoBackN | 0.00 | duration_secs | 5 | 0.5 | [0.5, 0.5] |
| GoBackN | 0.05 | goodput_bytes_sec | 5 | 94745.5 | [61443, 128048] |
| GoBackN | 0.05 | data_retransmissions | 5 | 27 | [11.6415, 42.3585] |
| GoBackN | 0.05 | duration_secs | 5 | 1.588 | [0.969258, 2.20674] |
| GoBackN | 0.10 | goodput_bytes_sec | 5 | 10360.9 | [206.073, 20515.7] |
| GoBackN | 0.10 | data_retransmissions | 5 | 85 | [64.223, 105.777] |
| GoBackN | 0.10 | duration_secs | 5 | 32.604 | [-18.1084, 83.3164] |
| GoBackN | 0.15 | goodput_bytes_sec | 5 | 1214.66 | [683.989, 1745.33] |
| GoBackN | 0.15 | data_retransmissions | 5 | 128.4 | [97.8137, 158.986] |
| GoBackN | 0.15 | duration_secs | 5 | 127.836 | [70.997, 184.675] |
| GoBackN | 0.20 | goodput_bytes_sec | 5 | 687.111 | [142.056, 1232.17] |
| GoBackN | 0.20 | data_retransmissions | 5 | 165 | [120.978, 209.022] |
| GoBackN | 0.20 | duration_secs | 5 | 306.812 | [32.3611, 581.263] |
| GoBackN | 0.25 | goodput_bytes_sec | 5 | 288.099 | [35.6193, 540.578] |
| GoBackN | 0.25 | data_retransmissions | 5 | 215 | [172.393, 257.607] |
| GoBackN | 0.25 | duration_secs | 5 | 674.196 | [164.601, 1183.79] |
| SelectiveRepeat | 0.00 | goodput_bytes_sec | 5 | 280000 | [280000, 280000] |
| SelectiveRepeat | 0.00 | data_retransmissions | 5 | 0 | [0, 0] |
| SelectiveRepeat | 0.00 | duration_secs | 5 | 0.5 | [0.5, 0.5] |
| SelectiveRepeat | 0.05 | goodput_bytes_sec | 5 | 111044 | [30936.4, 191152] |
| SelectiveRepeat | 0.05 | data_retransmissions | 5 | 4.6 | [0.426088, 8.77391] |
| SelectiveRepeat | 0.05 | duration_secs | 5 | 2.092 | [-0.530799, 4.7148] |
| SelectiveRepeat | 0.10 | goodput_bytes_sec | 5 | 26654.1 | [19164.7, 34143.4] |
| SelectiveRepeat | 0.10 | data_retransmissions | 5 | 11.8 | [7.83444, 15.7656] |
| SelectiveRepeat | 0.10 | duration_secs | 5 | 5.548 | [3.49917, 7.59683] |
| SelectiveRepeat | 0.15 | goodput_bytes_sec | 5 | 15142.1 | [4893.99, 25390.1] |
| SelectiveRepeat | 0.15 | data_retransmissions | 5 | 17.2 | [15.5811, 18.8189] |
| SelectiveRepeat | 0.15 | duration_secs | 5 | 11.492 | [4.95199, 18.032] |
| SelectiveRepeat | 0.20 | goodput_bytes_sec | 5 | 9027.37 | [4523.07, 13531.7] |
| SelectiveRepeat | 0.20 | data_retransmissions | 5 | 24.8 | [19.5761, 30.0239] |
| SelectiveRepeat | 0.20 | duration_secs | 5 | 18.876 | [5.10454, 32.6475] |
| SelectiveRepeat | 0.25 | goodput_bytes_sec | 5 | 4126.11 | [-491.54, 8743.76] |
| SelectiveRepeat | 0.25 | data_retransmissions | 5 | 31.6 | [22.201, 40.999] |
| SelectiveRepeat | 0.25 | duration_secs | 5 | 77.508 | [-8.18401, 163.2] |

## Paired Go-Back-N minus Selective Repeat

| loss_rate | metric | n | mean difference | 95% CI | exploratory p |
|---|---|---|---|---|---|
| 0.00 | goodput_bytes_sec | 5 | 0 | [0, 0] |  |
| 0.00 | data_retransmissions | 5 | 0 | [0, 0] |  |
| 0.00 | duration_secs | 5 | 0 | [0, 0] |  |
| 0.05 | goodput_bytes_sec | 5 | -16298.9 | [-76042.9, 43445.1] | 0.490952 |
| 0.05 | data_retransmissions | 5 | 22.4 | [10.2469, 34.5531] | 0.00689853 |
| 0.05 | duration_secs | 5 | -0.504 | [-2.63463, 1.62663] | 0.547196 |
| 0.10 | goodput_bytes_sec | 5 | -16293.2 | [-32669, 82.5908] | 0.0507224 |
| 0.10 | data_retransmissions | 5 | 73.2 | [53.8165, 92.5835] | 0.000467724 |
| 0.10 | duration_secs | 5 | 27.056 | [-24.0602, 78.1722] | 0.215613 |
| 0.15 | goodput_bytes_sec | 5 | -13927.4 | [-24432.2, -3422.62] | 0.0211843 |
| 0.15 | data_retransmissions | 5 | 111.2 | [81.5895, 140.81] | 0.000477953 |
| 0.15 | duration_secs | 5 | 116.344 | [56.321, 176.367] | 0.0057623 |
| 0.20 | goodput_bytes_sec | 5 | -8340.25 | [-12358.8, -4321.74] | 0.00450002 |
| 0.20 | data_retransmissions | 5 | 140.2 | [98.4941, 181.906] | 0.00073361 |
| 0.20 | duration_secs | 5 | 287.936 | [26.1343, 549.738] | 0.0378945 |
| 0.25 | goodput_bytes_sec | 5 | -3838.01 | [-8267.1, 591.08] | 0.0738808 |
| 0.25 | data_retransmissions | 5 | 183.4 | [147.946, 218.854] | 0.000136566 |
| 0.25 | duration_secs | 5 | 596.688 | [83.243, 1110.13] | 0.0320794 |

## Regression against the frozen primary reorder-0 cells

Compared fields: transfer status, SHA-256 match, delivered digest, data retransmissions, premature retransmissions, timeout count, virtual duration, and goodput.
Only cells with loss 0, reorder 0, and the primary window (1 for Stop-and-Wait, 8 otherwise) are compared. The primary file is read and not rewritten.
All 120 compared field values match the frozen primary records.

## Limitations

- Virtual-time driver, not the localhost UDP path.
- Five seeds per cell. Intervals can be wide.
- The window study does not cross window size with loss. The loss study holds window size fixed.
- Goodput is the value already stored on the Rust record (`goodput_bytes_sec`). This script does not redefine it.
