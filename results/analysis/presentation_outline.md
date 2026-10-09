# Presentation outline (10–12 slides)

Numbers are the cell means and 95% Student-t intervals in `primary_results.csv`. Error bars on the figures are those intervals. Do not cite `results/raw/archive/synthetic_pilot/`.

## 1. Title

Reliable Data Transfer over UDP. Stop-and-Wait, Go-Back-N, and Selective Repeat under controlled reordering. CS-30003, Coding Assignment 1.

## 2. Problem

UDP does not recover reordered datagrams. A file transfer still has to deliver the original bytes, in order, and show that the receiver’s SHA-256 matches the source.

## 3. Research question

How do the three ARQ protocols behave as configured forward reordering increases from 0% to 25%, with loss held at zero? Timeout state is recorded; in **E1** the RTO multiplier was not a separate factor. (E4 varies multipliers separately under loss—do not conflate the two studies.)

## 4. Architecture

Virtual-time loop (measured path): chunk the file, ARQ sender, seeded forward channel, delivery scheduler, ARQ receiver, clean reverse channel, Jacobson/Karels RTO, reassembly, SHA-256, JSON record. No sleep. No UDP socket on the **measured** path. Separately, `send`/`recv` demonstrate the same ARQ over localhost UDP without the emulator. Python only runs trials and summarizes the JSON.

## 5. Protocols

- Stop-and-Wait: one outstanding packet.
- Go-Back-N: sender window 8, cumulative ACKs, receiver discards anything that is not next, timeout retransmits the outstanding window.
- Selective Repeat: window 8 on both sides, receiver buffers an early packet and ACKs it, timeout retransmits that packet only.

## 6. Setup

90 trials. Reorder 0, 5, 10, 15, 20, 25%. Seeds 101, 202, 303, 404, 505, paired across protocols. Loss, duplication, corruption 0. Delay 20 ms, jitter 0, hold 50 ms. File 140,000 bytes, chunks 1,400. RTO multiplier 1, floor 200 ms, initial 1,000 ms, ceiling 60,000 ms. Five successes in every cell.

## 7. Metrics

Data retransmissions, retransmission ratio, goodput (unique bytes / virtual duration), virtual duration, mean accepted RTT, final RTO, timeout count, premature retransmissions. Configured reorder rate is not the measured reorder fraction.

## 8. Retransmissions

Show `data_retransmissions_vs_reorder.svg`.

Stop-and-Wait and Selective Repeat: mean 0 at every rate, interval [0, 0]. Go-Back-N: 0 at 0%; then 55.2, 102.4, 133, and 164.8 packets at 10%, 15%, 20%, and 25%. Those intervals lie above 0. The paired difference versus Selective Repeat is the same, because Selective Repeat is 0 on every seed.

## 9. Goodput and duration

Show `goodput_vs_reorder.svg` and `duration_vs_reorder.svg`.

At 0%, Go-Back-N and Selective Repeat: 280,000 bytes/s and 0.50 s. Stop-and-Wait: about 35,176 bytes/s and 3.98 s. At 25%, Selective Repeat: about 152,804 bytes/s and 0.918 s. Stop-and-Wait: about 27,082 bytes/s and 5.17 s. Go-Back-N: about 811 bytes/s and 311 s. Say that the 10%, 15%, and 25% Go-Back-N duration intervals include negative numbers even though every recorded duration is positive: untruncated t interval, n = 5, large spread.

## 10. RTO and recovery

Show `final_rto_vs_reorder.svg` and `timeout_count_vs_reorder.svg`.

Stop-and-Wait and Selective Repeat stay at 200 ms with zero timeouts. Go-Back-N mean final RTO at 10%, 15%, 20%, and 25% is 440 ms, 6,800 ms, 25,520 ms, and 16,640 ms. Some trials hit 60,000 ms. Mean timeout counts there are 7.2, 13.6, 18, and 22. Timeout retransmissions equal data retransmissions. Premature retransmissions are 0. State that **in E1** RTO was not independently varied; point to E4 only if asked about multiplier sensitivity.

## 11. Findings and limits

One sentence from the conclusion in `docs/FINAL_REPORT.md`. Then the boundaries: loss only 0, one delay and one hold, five seeds, virtual time, one file, this implementation, RTO not a factor.

## 12. Conclusion

Under these zero-loss reordering conditions, Go-Back-N’s recovery cost and completion time grew with the configured reorder rate, Selective Repeat did not retransmit and kept the higher goodput, and Stop-and-Wait did not retransmit but stayed slower because only one packet is outstanding. Not a universal ranking.
