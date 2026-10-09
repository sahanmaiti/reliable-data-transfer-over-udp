# Viva questions

Answers follow the implementation in `src/` and the 90-trial records. They are not claims about every possible network.

## Why UDP instead of TCP?

TCP already retransmits, reorders, and estimates RTO. The assignment is to build that reliability and to see how three ARQ rules differ. UDP is the datagram service underneath. The **measured E1/E4 experiments** use a virtual-time channel (no socket) so every protocol sees a seeded impairment pattern. A separate **localhost UDP demo** (`send`/`recv`, `src/app/udp.rs`) uses the same ARQ code over real sockets and is not the scientific experiment path.

## Why implement three ARQ protocols?

Stop-and-Wait, Go-Back-N, and Selective Repeat are the standard alternatives for what the receiver does with an early packet and what the sender retransmits. One implementation of each, on one packet format, is what makes the comparison about that rule rather than about three unrelated programs.

## Why does Go-Back-N react strongly to reordering?

The receiver accepts only the next expected sequence. A later packet that arrives first is discarded. The sender still owes that packet. Recovery waits for the timer on the oldest outstanding segment, then `handle_timeout` retransmits the outstanding window. In the 90 trials, every Go-Back-N data retransmission at nonzero reorder was one of those timeout retransmissions.

## Why does Selective Repeat behave differently?

The receiver buffers a packet inside its window and acknowledges that packet. The sender’s timer for a buffered packet is cancelled when the acknowledgement arrives. With a 50 ms hold and a 200 ms RTO floor, those acknowledgements returned before a timeout. The records show zero data retransmissions at every reorder rate.

## Why does Stop-and-Wait avoid the observed retransmissions?

Only one packet is in flight. There is no later packet that can arrive first and be discarded. Reordering only adds the 50 ms hold. That hold is below the 200 ms floor, and the records show zero data retransmissions. The protocol is still slower because the next packet waits for the round trip.

## What is the purpose of the reorder hold?

`reorder_extra_ms` is the extra delay the forward channel adds when it decides to reorder a datagram. The scheduler delivers by that delay, so a later packet can overtake an earlier one. In this study the hold is 50 ms. It is not the configured probability. The probability is `configured_reorder_rate`.

## Why five seeds?

Each protocol × reorder cell has five matched seeds: 101, 202, 303, 404, and 505. That is enough to compute a sample standard deviation and a t interval, and the shared seeds make protocol differences paired. It is still a small sample. Go-Back-N’s duration intervals are wide because of that.

## Why 95% t intervals?

The summary is a mean of five trials, not a known population. The interval uses the Student t critical value at df = 4, two-sided 95%: mean ± t × s / sqrt(n), with s divided by n − 1. A normal z interval would treat the sample as large. It is not. Intervals are not cut off at zero.

## Why paired comparisons?

The same seed is used for every protocol at a given reorder rate. The virtual channel’s random draws start from that seed. Subtracting seed by seed removes the part of the variation that is common to that seed. The paired interval is on those five differences, still with df = 4.

## Why is n only 5?

The study was specified as five repeats per cell. More seeds would narrow the Go-Back-N intervals. Five is what was run. The wide intervals are reported, not hidden.

## What is RTO?

The retransmission timeout. After a packet is sent, the sender waits this long for an acknowledgement before treating the packet as lost and sending again. The estimator updates it from accepted RTT samples. The value used in the study is clamped between 200 ms and 60,000 ms.

## What is Karn’s rule here?

If a data segment has been retransmitted, the acknowledgement that arrives might refer to the first copy or a later copy. The RTT is ambiguous. `RtoEstimator::update_rtt` takes a flag, `is_retransmission`, and rejects the sample when that flag is set. The packet’s retransmit bit is how the driver knows. Rejected samples increment `karn_rejected_count` and are not part of `total_rtt_samples`.

## Why did Go-Back-N’s RTO reach 60 seconds?

Each timeout calls `on_timeout`, which doubles the current RTO up to `max_rto`. Go-Back-N trials at 20% and 25% reordering include seeds whose repeated timeouts hit that 60,000 ms ceiling. Stop-and-Wait and Selective Repeat recorded no timeouts, so their final RTO stayed at the 200 ms floor.

## Why can’t we claim that adaptive RTO caused the E1 result?

In **this** 90-trial reordering matrix every trial used multiplier 1 and the same floor, initial value, and ceiling. The records show that Go-Back-N’s timeouts, retransmissions, rising RTO, and long duration occur together. They do not, by themselves, show what would have happened under another timeout rule. A **separate** secondary study (E4, 45 trials under `results/rto_sensitivity/`) does vary the RTO multiplier under fixed 5% loss; that study must not be mixed into E1 causal claims.

## What are the main limitations?

For E1: loss was only zero; delay, jitter, and the reorder hold were fixed; five seeds; virtual time, not a live network; one 140,000-byte file; this program’s windows and receiver rules; the RTO multiplier was not a factor **in E1**. The extra paired t tests are uncorrected. Real UDP localhost demos exist but are not this dataset.

## Why use virtual time?

Wall-clock timers move when the operating system schedules the process. A seeded channel plus an explicit clock makes the same seed produce the same deliveries and the same retransmission count. `duration_secs` is that simulated time. It is the right clock for this comparison. It is not a measurement of a campus network.

## How is SHA-256 used?

The sender hashes the source bytes. The receiver hashes the bytes it delivered in order. `sha256_match` is true only when those digests are equal. All 90 trials matched, and delivered length equalled 140,000 bytes. A completed protocol trace with a bad digest would have been `INTEGRITY_FAIL`, not a success with zero goodput hiding the failure.

## How is reproducibility ensured?

The fixture path and its SHA-256 are recorded. Each trial has an experiment id, protocol, reorder rate, seed, and trial id. The runner passes those to `run-experiment`. The raw JSON is the Rust record. Analysis reads that JSON and does not resample. Dataset hashes are in `results/analysis/provenance.md`. The synthetic archive is a different file and is not an input to the analysis.

## What would you test next?

Cross a nonzero loss rate with the same reorder sweep (a full loss×reorder grid was not run). Loss and window were swept separately: `results/loss_goodput/` (90 trials, reorder 0) and `results/window_goodput/` (65 trials, loss 0). A separate RTO-multiplier matrix **does** already exist as E4 (`results/rto_sensitivity/`, 45 trials, multipliers 0.5 / 1.0 / 3.0 under loss 0.05 and reorder 0); it answers timeout aggressiveness under loss, not reorder isolation. Further work could enlarge seeds or fixtures, or cross loss with reorder.
