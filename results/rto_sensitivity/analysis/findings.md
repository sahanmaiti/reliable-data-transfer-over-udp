# E4 RTO Sensitivity — Findings

## Experimental design

Secondary study required by the project proposal: scale the adaptive RTO
while holding Jacobson/Karels α and β at their Rust defaults.

- Protocols: StopAndWait, GoBackN, SelectiveRepeat
- RTO multipliers: 0.5×, 1.0×, 3.0×
- Seeds: 101, 202, 303, 404, 505 (n = 5 per cell)
- Channel: loss = 0.05, reorder = 0, duplicate = 0, corrupt = 0
- Delay: base 20 ms, jitter 0, reorder hold 50 ms
- File: fixtures/transfer_medium.bin (140,000 bytes), chunk 1,400 bytes
- Windows: StopAndWait = 1; GoBackN / SelectiveRepeat = 8
- RTO: initial 1000 ms, min 10 ms (E4 only), max 60000 ms
- Matrix: 3 × 3 × 5 = 45 trials; all SUCCESS with SHA-256 match

## Why loss was introduced

The frozen primary study uses loss = 0 and varies reorder. E4 instead
introduces a fixed 5% loss with reorder = 0 so that genuine loss events
exist for the timeout path. Without loss, a multiplier study would mostly
observe idle RTO behaviour rather than recovery under loss.

## Why min_rto differs from the primary study

The primary study floors RTO at 200 ms. Under measured RTTs near 40 ms,
a 0.5× multiplier would still clamp to 200 ms and look identical to 1.0×.
E4 uses min_rto = 10 ms only so the 0.5× arm can express a shorter timer.
This is a deliberate E4-only change; the primary floor remains 200 ms.

## Observed RTO behaviour

- **StopAndWait** mean final RTO: 0.5× → 24.6 ms; 1.0× → 41.0 ms; 3.0× → 123.0 ms
- **GoBackN** mean final RTO: 0.5× → 61.5 ms; 1.0× → 82.0 ms; 3.0× → 246.0 ms
- **SelectiveRepeat** mean final RTO: 0.5× → 168.1 ms; 1.0× → 49.2 ms; 3.0× → 147.6 ms

Smoke trials (GoBackN, seed 101) already showed distinct final RTO for
0.5× (82 ms) versus 3.0× (123 ms), confirming the multiplier reaches the
estimator and is not silently clamped to one shared value.

Note on SelectiveRepeat: mean final RTO at 0.5× is higher than at 1.0×.
That is consistent with aggressive timeouts causing more backoff before
the transfer ends; the endpoint RTO is not a pure scaled SRTT snapshot.

## Premature retransmission behaviour

- **StopAndWait**: 0.5× mean=45.00; 1.0× mean=0.00; 3.0× mean=0.00
- **GoBackN**: 0.5× mean=27.20; 1.0× mean=0.00; 3.0× mean=0.00
- **SelectiveRepeat**: 0.5× mean=40.40; 1.0× mean=0.00; 3.0× mean=0.00

Across all three protocols, premature retransmissions are concentrated at
0.5× and are exactly zero at 1.0× and 3.0× in this matrix (all five seeds).
That is a measured outcome, not a tuned requirement.

## Recovery duration and retransmissions

- **StopAndWait** duration: 0.5× 4.160s; 1.0× 4.220s; 3.0× 4.699s
- **StopAndWait** data retransmissions: 0.5× 49.8; 1.0× 4.6; 3.0× 4.6
- **GoBackN** duration: 0.5× 0.599s; 1.0× 0.671s; 3.0× 1.157s
- **GoBackN** data retransmissions: 0.5× 52.6; 1.0× 27.0; 3.0× 27.0
- **SelectiveRepeat** duration: 0.5× 6.228s; 1.0× 0.702s; 3.0× 1.472s
- **SelectiveRepeat** data retransmissions: 0.5× 45.6; 1.0× 4.6; 3.0× 4.6

Paired-by-seed differences versus the 1.0× baseline are in
`paired_by_seed.csv`. Exploratory two-sided paired t p-values are reported
for transparency only; a significant p-value is not treated as proof that
a multiplier is 'better'.

## Paired contrasts (mean difference, n = 5)

- StopAndWait [0.50 - 1.00] premature_retransmissions: mean Δ = 45, exploratory p = 5.84891e-08
- StopAndWait [0.50 - 1.00] duration_secs: mean Δ = -0.05903, exploratory p = 0.524072
- StopAndWait [0.50 - 1.00] data_retransmissions: mean Δ = 45.2, exploratory p = 4.32747e-06
- StopAndWait [3.00 - 1.00] premature_retransmissions: mean Δ = 0
- StopAndWait [3.00 - 1.00] duration_secs: mean Δ = 0.479, exploratory p = 0.0755436
- StopAndWait [3.00 - 1.00] data_retransmissions: mean Δ = 0
- GoBackN [0.50 - 1.00] premature_retransmissions: mean Δ = 27.2, exploratory p = 0.000553447
- GoBackN [0.50 - 1.00] duration_secs: mean Δ = -0.0717, exploratory p = 0.0680911
- GoBackN [0.50 - 1.00] data_retransmissions: mean Δ = 25.6, exploratory p = 0.0174559
- GoBackN [3.00 - 1.00] premature_retransmissions: mean Δ = 0
- GoBackN [3.00 - 1.00] duration_secs: mean Δ = 0.4862, exploratory p = 0.00912213
- GoBackN [3.00 - 1.00] data_retransmissions: mean Δ = 0
- SelectiveRepeat [0.50 - 1.00] premature_retransmissions: mean Δ = 40.4, exploratory p = 3.55742e-06
- SelectiveRepeat [0.50 - 1.00] duration_secs: mean Δ = 5.526, exploratory p = 0.0284776
- SelectiveRepeat [0.50 - 1.00] data_retransmissions: mean Δ = 41, exploratory p = 6.09525e-06
- SelectiveRepeat [3.00 - 1.00] premature_retransmissions: mean Δ = 0
- SelectiveRepeat [3.00 - 1.00] duration_secs: mean Δ = 0.7706, exploratory p = 0.166754
- SelectiveRepeat [3.00 - 1.00] data_retransmissions: mean Δ = 0

## Limitations

- Virtual-time channel; not a real UDP path or OS stack.
- Fixed 5% Bernoulli loss; no burst-loss model.
- Only three multipliers; no sweep between 0.5× and 3.0×.
- E4 min_rto = 10 ms differs from the primary 200 ms floor, so absolute
  RTO values are not directly comparable to the frozen reordering study.
- n = 5 seeds per cell; intervals are wide and exploratory p-values are
  uncorrected across many comparisons.
- Endpoint final_rto can be inflated by timeout backoff, especially when
  the multiplier is too aggressive.

## Bottom line

In this matrix, shortening the RTO (0.5×) produces clear premature
retransmissions and generally more retransmission work, while lengthening
it (3.0×) removes premature retransmissions and tends to lengthen recovery
relative to 1.0×. The expected premature-versus-delayed-recovery trade-off
is visible for duration and retransmission load; premature counts are
nonzero only on the aggressive arm under these conditions.
