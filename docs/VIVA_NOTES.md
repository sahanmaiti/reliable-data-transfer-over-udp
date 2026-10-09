# Viva Notes

Concise answers aligned with the **implemented** system. Prefer pointing at files over slogans.

---

### UDP / Networking

**1. Why UDP instead of TCP?**  
TCP already bundles reliability and congestion control in the kernel. The assignment is to build and compare ARQ mechanisms ourselves under controlled impairments.

**2. What does UDP provide?**  
Connectionless datagram delivery. No application-level retransmission, ordering, or duplicate suppression.

**3. How does your system add reliability?**  
Sequence numbers, ACKs, ARQ retransmission, timers/RTO, checksums, in-order delivery, and SHA-256 end-to-end checks (`src/arq/*`, `src/timing/*`, `src/packet.rs`, `src/app/integrity.rs`).

---

### ARQ

**4. Stop-and-Wait:** One DATA in flight; wait for ACK; timeout retransmits that packet (`src/arq/sw.rs`).

**5. Go-Back-N:** Sender window; cumulative ACK; receiver window 1 (discard early packets); timeout retransmits the outstanding window (`src/arq/gbn.rs`).

**6. Selective Repeat:** Sender/receiver windows; individual ACKs; buffer in-window out-of-order DATA; timeout retransmits only that seq (`src/arq/sr.rs`).

**7. Why SR usually retransmits fewer under reordering?**  
SR can accept and ACK early packets inside the window. GBN discards them and often recovers only after timeout by resending the window. E1 (loss=0) showed GBN retransmissions rising with reorder while SR stayed at 0.

**8. Out-of-order packet in GBN?** Discarded; duplicate cumulative ACK of last in-order seq (after seq 0 has been received).

**9. Out-of-order in SR?** If inside receive window: buffer + individual ACK; deliver when the gap fills.

---

### Packet Design

**10. Sequence numbers:** Detect duplicates, gaps, and ordering; identify ACK coverage.

**11. Checksum:** Detect corruption (including emulator bit flips); RFC 1071 over header+payload.

**12. Length:** Bound payload parsing; avoid reading past the datagram.

**13. Type/flag:** Distinguish DATA/ACK/FIN; retransmission flag supports Karn’s rule.

---

### RTO

**14. SRTT:** Smoothed RTT estimate.

**15. RTTVAR:** RTT variation estimate.

**16. Why adaptive RTO?** Fixed timeouts are too short (spurious retransmits) or too long (slow recovery) as RTT changes.

**17. Exponential backoff:** After timeout, increase RTO (doubling, clamped) to avoid timer storms.

**18. Karn’s algorithm:** Do not update RTT from ACKs of retransmitted segments (ambiguous which copy was ACKed).

**19. RTO too small?** Timer fires before the ACK could arrive → premature retransmission (seen at 0.5× in E4).

---

### Channel Emulator

**20. Deterministic seeds?** Same impairment draws for matched protocol comparisons.

**21. Why loss=0 in E1?** Isolate reordering effects from loss-driven recovery.

**22. Corruption?** Emulator can flip a bit; packet checksum fails → drop.

**23. Duplication?** Emulator may deliver an extra copy; ARQ must tolerate duplicates.

---

### Experiments

**24. Primary hypothesis?** Under controlled increasing reorder (zero loss), GBN retransmits more than SR because it cannot use early packets the way SR can.

**25. Independent variable (E1):** Configured forward reorder rate (protocol as second factor).

**26. Held constant (E1):** Loss/dup/corrupt=0, delay/jitter/hold, fixture, chunk, windows, RTO bounds/multiplier.

**27. Matched seeds?** Pair protocols on identical channel draws.

**28. Five repetitions?** n=5 per cell for means / Student-t CI (df=4); still limited power.

**29. 95% CI?** `mean ± t_{n-1,0.975}·SEM` in the Python analysis scripts.

**30. E1 showed?** SW & SR: 0 data retransmissions across reorder grid; GBN: rising timeout retransmissions and longer duration; SR remains fast. All 90 SUCCESS.

**31. E4 showed?** Premature retransmissions at 0.5× only; 3.0× tends to lengthen recovery vs 1.0× under 5% loss.

**32. Limitations?** Virtual time; fixed grids; n=5; one fixture; E1/E4 factor isolation; localhost UDP; no window study run.

---

### Implementation

**33. Real UDP vs virtual experiment?** Same ARQ/packet/RTO/chunking. Experiments use `run_transfer` + Channel + virtual clock. Demo uses `UdpSocket` wall clock without the emulator (`src/app/udp.rs`).

**34. FIN?** After DATA drained/ACKed, sender sends FIN; receivers ACK FIN; receiver finalizes file and lingers briefly for FIN retries.

**35. Integrity?** SHA-256 of source vs reconstructed/written bytes; required for SUCCESS in experiments.

**36. Duplicates?** Receivers re-ACK / ignore duplicate DATA without double-delivering to the app.

**37. Timeouts?** Timers expire → ARQ `handle_timeout` → retransmit → `RtoEstimator::on_timeout` backoff; socket path also has a hard transfer deadline.
