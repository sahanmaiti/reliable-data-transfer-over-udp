# Research Findings

These findings describe the 90 recorded trials. They do not replace the Phase 8 tables, and they do not come from the archived synthetic pilot.

## Experimental scope

The study compared Stop-and-Wait, Go-Back-N, and Selective Repeat while the configured forward reorder rate took the values 0.00, 0.05, 0.10, 0.15, 0.20, and 0.25. Each protocol was run at each rate with the same five seeds, 101, 202, 303, 404, and 505. That is 90 trials.

Forward loss, duplication, and corruption were 0. The input was `fixtures/transfer_medium.bin` (140,000 bytes; SHA-256 `8e6c7049a65aeff8211b2bbd78db47683a1b86f75565ad3362fb6e53a0a4cbbf`). Chunk size was 1,400 bytes. Stop-and-Wait used window 1. Go-Back-N and Selective Repeat used window 8. One-way base delay was 20 ms, jitter was 0, and the reorder hold was 50 ms. The RTO multiplier was 1, with a 200 ms floor, a 1,000 ms initial timeout, and a 60,000 ms ceiling. The reverse path was configured with no loss, reorder, duplication, or corruption.

The same seeds were used for every protocol, so protocol comparisons below are paired by seed. A positive difference means the first-named protocol had the larger recorded value. Intervals are two-sided 95% Student-t intervals on the five paired differences, with df = 4. They are not truncated at zero.

The experiment changes the protocol and the reorder rate. It does not independently change the RTO algorithm or the RTO multiplier. RTO numbers below are measurements recorded during those runs, not the result of an RTO experiment.

## Data quality

All 90 trials have `transfer_status` `SUCCESS`, matching SHA-256 digests, and `delivered_unique_bytes` equal to 140,000. Every protocol × reorder cell has five successful trials and zero failures. No primary-study record was excluded.

Raw hashes, unchanged at interpretation time:

- `results/raw/experiments_raw.json`: `d93dc2b18b49775c35587836e042a9daf6dfc844e85427a6be927180c6f0e8be`
- `results/raw/experiments_raw.csv`: `ac071d325a07a41e86f70348579a5ee0e7f0e494d9f1ded4809ddb2f05ffa108`

## Stop-and-Wait

Stop-and-Wait recorded zero data retransmissions at every reorder rate, including 0.25, and the five seeds agreed (the interval is the point [0, 0]). Timeout count and premature retransmissions were 0. Final RTO stayed at the 200 ms floor.

Virtual duration rose from 3.98 s at reorder 0 to 5.17 s at reorder 0.25. The intervals at the nonzero rates lie above the 3.98 s value recorded at reorder 0. Goodput fell from about 3.52×10^4 bytes/s to about 2.71×10^4 bytes/s over the same span. Mean RTT changed with the reorder rate, and on every paired seed it matched the Selective Repeat mean RTT exactly.

## Go-Back-N

At reorder 0, Go-Back-N recorded zero retransmissions, zero timeouts, final RTO 200 ms, and virtual duration 0.50 s, identical to Selective Repeat on those fields.

At every reorder rate above 0, all five seeds recorded data retransmissions greater than 0. The means were 30.8, 55.2, 102.4, 133, and 164.8 packets at 0.05, 0.10, 0.15, 0.20, and 0.25. Each of those intervals lies above 0. In every one of those trials, `timeout_retransmissions` equalled `data_retransmissions`, and `premature_retransmissions` was 0. Mean timeout count rose from 4.4 to 22 across those five rates, and those intervals also lie above 0.

Duration and final RTO did not increase in a straight line. Mean duration was 2.42 s, 6.20 s, 46.7 s, 168 s, and 311 s. Mean final RTO was 600 ms, 440 ms, 6,800 ms, 25,520 ms, and 16,640 ms. The 0.25 mean is lower than the 0.20 mean. Individual trials reached the 60,000 ms ceiling at both 0.20 and 0.25, and the lowest final RTO in those cells was 400 ms and 800 ms. The intervals for duration at 0.10, 0.15, and 0.25, and for final RTO at every nonzero rate, include values below the recorded minimum, including values below zero for some duration and goodput intervals. Every recorded duration and goodput in those cells is positive. The interval is the untruncated t interval. With five seeds and a large seed-to-seed spread, that interval is wide.

## Selective Repeat

Selective Repeat recorded zero data retransmissions, zero timeouts, and zero premature retransmissions at every reorder rate. The five seeds agreed. Final RTO stayed at 200 ms.

Virtual duration rose from 0.50 s at reorder 0 to 0.918 s at reorder 0.25. Goodput fell from 2.80×10^5 bytes/s to about 1.53×10^5 bytes/s. The intervals for those two series at nonzero rates do not contain the reorder-0 point. Measured reorder fraction was not identical to the configured rate. Across Selective Repeat trials it ranged from 0 to 0.25.

## Cross-protocol comparison

Paired differences use the five shared seeds. The sign is A − B as named in `protocol_comparisons.csv`.

At reorder 0, Go-Back-N and Selective Repeat recorded the same retransmissions, duration, goodput, and final RTO on every seed. Stop-and-Wait also recorded zero retransmissions, but its duration was 3.48 s longer than Selective Repeat on every seed, and its goodput was lower by about 2.45×10^5 bytes/s.

Above reorder 0:

- Go-Back-N minus Selective Repeat data retransmissions has a 95% interval above 0 at each of 0.05, 0.10, 0.15, 0.20, and 0.25. The mean differences are the Go-Back-N means, because Selective Repeat's count was 0 on every paired seed. The retransmission count ratio is undefined, because the Selective Repeat denominator is 0. No substitute ratio was computed.
- Go-Back-N minus Selective Repeat goodput has an interval entirely below 0 at each of those five rates. Mean differences are about −1.64×10^5, −1.64×10^5, −1.66×10^5, −1.60×10^5, and −1.52×10^5 bytes/s.
- Go-Back-N minus Selective Repeat duration has a positive mean at each nonzero rate (1.83 s, 5.52 s, 45.9 s, 167 s, and 311 s). The interval lies above 0 at 0.05 and 0.20. It includes 0 at 0.10, 0.15, and 0.25. The mean of the seed-wise duration ratios is about 4.0, 8.8, 56, 194, and 330. Those ratio intervals are wide and also include values below 1 at 0.10, 0.15, and 0.25.
- Stop-and-Wait minus Selective Repeat retransmissions is 0 at every rate. Stop-and-Wait duration exceeds Selective Repeat duration by about 3.5 s to 4.3 s, and those intervals lie above 0. Stop-and-Wait goodput is lower, and those intervals lie below 0.

Uncorrected two-sided paired t tests of a zero mean difference are stored in `protocol_comparisons.csv` for every comparison that was computed (7 metrics × 6 rates × 3 pairs). They are exploratory. No multiplicity adjustment was applied, and the findings above are not based on selecting significant p-values. Where the five paired differences are identical, the t statistic is undefined and the p-value is left blank. The interval in those cases is the shared value itself.

## RTO behavior

Final RTO, timeout count, and duration were recorded under one RTO configuration. The multiplier, floor, initial value, and ceiling were not varied, and no other timeout rule was run as a control. These records therefore do not show that the adaptive estimator caused the protocol differences, or that a different timeout rule would have changed them.

What the records do show is an association inside the Go-Back-N runs. At reorder 0, final RTO is 200 ms and the timeout count is 0. At nonzero reorder, every Go-Back-N trial has a positive timeout count, `timeout_retransmissions` equals `data_retransmissions`, and premature retransmissions remain 0. Mean final RTO leaves the 200 ms floor and, in some trials, reaches the 60,000 ms ceiling. The largest mean durations sit in those same nonzero-reorder Go-Back-N cells. Stop-and-Wait and Selective Repeat recorded no timeouts, and their final RTO stayed at 200 ms while reorder increased. Mean RTT for Stop-and-Wait and Selective Repeat moved together and stayed far below that floor.

## Practical interpretation

Under this zero-loss schedule, with a 50 ms reorder hold and a 200 ms RTO floor, the three protocols did not respond to configured reordering in the same way.

Stop-and-Wait completed every trial without a data retransmission. Its virtual transfer time stayed near 4–5 s. That is longer than the other two protocols at reorder 0, where Go-Back-N and Selective Repeat both finished in 0.50 s.

Selective Repeat also completed every trial without a data retransmission. Its duration stayed below 1 s. Its goodput declined as the configured reorder rate increased, while remaining above the Stop-and-Wait goodput at every rate in this table.

Go-Back-N matched Selective Repeat at reorder 0. At the nonzero rates it retransmitted, and those retransmissions were the packets resubmitted after timeouts. The retransmission means increase across 0.05 to 0.25. Duration and final RTO spread out sharply from 0.15 upward, with large seed-to-seed differences, so the duration intervals at 0.10, 0.15, and 0.25 still include 0 even though every recorded duration is positive.

The measured fraction of datagrams the forward channel marked reordered is not the configured probability. It ranged from 0 to 0.25 for Stop-and-Wait and Selective Repeat, and from 0 to about 0.283 for Go-Back-N.

## Limitations

- Forward loss was only 0. A loss setting was not crossed with reorder.
- Base delay, jitter, and reorder hold were each fixed at one value.
- Five seeds give df = 4. Several Go-Back-N intervals are wide enough to cross a physical boundary even though the observations do not.
- The clock is the driver's virtual time, not a physical network deployment.
- In **this E1 matrix** the RTO multiplier and bounds were not independently varied, and no alternative timeout rule was included. A separate E4 study under `results/rto_sensitivity/` varies the multiplier under fixed loss; it is not part of these 90 records.
- The input was one 140,000-byte file, chunked at 1,400 bytes.
- The results describe this implementation's state machines, window sizes, and channel wiring. They are not a statement about every possible Go-Back-N or Selective Repeat implementation.
- Paired t tests in the comparison file are uncorrected and exploratory.
