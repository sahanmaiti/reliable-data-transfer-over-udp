# Viva Preparation & Defense Notes

**Author:** Soumyadeb Mukherjee  
**Component:** Protocol & ARQ  
**Course:** CS-30003  

---

## 1. Core Concepts & "Why" Questions

### Q1: Why build reliable data transfer over UDP instead of just using TCP?
**A:**  
* TCP bundles together reliability, in-order delivery, connection management, and congestion control into an unmodifiable kernel implementation.
* Building reliability over raw UDP allows us to inspect, isolate, and test individual mechanisms (e.g., comparing Stop-and-Wait vs. Go-Back-N vs. Selective Repeat under identical synthetic packet loss and reordering) without kernel interference or congestion backoff skewing the experimental results.

### Q2: Why do we need sequence numbers?
**A:**  
* UDP is connectionless and does not preserve packet order or prevent duplicates.
* Sequence numbers enable the receiver to:
  1. Detect duplicate packets (due to retransmissions or network duplication).
  2. Detect missing packets.
  3. Reorder out-of-order packets before delivering them to the application.

### Q3: Why is 16-bit Internet Checksum used when UDP already has a checksum?
**A:**  
* UDP checksums are often computed or verified by hardware (checksum offloading) or can be disabled in IPv4.
* More importantly, our project includes a **deterministic channel emulator** that injects bit errors at the application layer to test corruption handling. Having our own packet-level checksum ensures we test and verify error detection explicitly.

### Q4: What is the difference between Cumulative ACK and Individual ACK?
**A:**  
* **Cumulative ACK (used in GBN)**: An ACK with sequence $k$ asserts: *"I have successfully received all packets with sequence numbers $\le k$"*. If an ACK for $k-1$ is lost but ACK for $k$ arrives, the sender knows both $k-1$ and $k$ are safe.
* **Individual ACK (used in SR)**: An ACK with sequence $k$ asserts: *"I have received packet $k$ specifically"*. It says nothing about packets $0 \dots k-1$.

### Q5: Why does Go-Back-N perform poorly when packets are reordered?
**A:**  
* GBN has a receiver window of size 1 and will **discard** any packet that is not strictly the `expected_seqnum`.
* If packets 2 and 3 are swapped (3 arrives before 2), GBN drops packet 3 and sends a duplicate ACK for packet 1. Even when 2 arrives later, 3 has to be retransmitted from scratch when the sender's timer expires.

### Q6: Why must the window size in Selective Repeat be at most half the sequence number space ($W \le \frac{1}{2} \text{SeqSpace}$)?
**A:**  
* If $W > \frac{1}{2} \text{SeqSpace}$, the receiver's window overlaps with an unacknowledged past window.
* **Example**: If sequence numbers are $0, 1, 2, 3$ (space = 4) and window size $W = 3$:
  1. Sender sends 0, 1, 2.
  2. Receiver accepts all and advances window to $[3, 0, 1]$.
  3. All ACKs are lost.
  4. Sender retransmits packet 0.
  5. The receiver cannot determine if sequence 0 is a duplicate retransmission or a brand new packet from the next cycle!
* Constraining $W \le \text{SeqSpace} / 2$ guarantees that the current receiver window never overlaps with the sender's possible retransmission window.

---

## 2. Likely Live Modifications Asked by Professors

### Modification 1: Change ACK semantics in Go-Back-N
* **Question**: *"Right now your ACK represents 'highest received seq'. Change it so that the ACK contains 'next expected seq'."*
* **Where to modify**: In `src/go_back_n.rs`:
  * Sender: `send_base = ack_num` instead of `send_base = ack_num + 1`.
  * Receiver: Send `ACK(expected_seqnum)` instead of `ACK(expected_seqnum - 1)`.

### Modification 2: Change Checksum algorithm
* **Question**: *"Show how you would swap the 16-bit One's Complement sum for an XOR-based checksum or CRC."*
* **Where to modify**: In `src/packet.rs`, modify the `calculate_checksum(&self)` function. Because framing encapsulates the checksum in bytes 8-9, the rest of the protocol remains completely unaffected.

### Modification 3: Toggle receiver drop behavior
* **Question**: *"In Go-Back-N, log whenever an out-of-order packet is dropped and count the wasted bandwidth."*
* **Where to modify**: In `GbnReceiver::handle_packet`, inside the `else` block where `pkt.seq_num != self.expected_seqnum`.

---

## 3. Common Bugs & Gotchas to Avoid

1. **Forgetting to send ACKs for duplicate packets**:
   * If a receiver receives an old packet ($seq < expected$), it *must* re-send the ACK. If it silently ignores it, the sender will time out indefinitely because its previous ACK was lost!
2. **Timer reset bugs in Go-Back-N**:
   * In GBN, when an ACK arrives, do not stop the timer unless the buffer is completely empty (`send_base == next_seqnum`). If packets are still in flight, you must **restart** the timer for the oldest remaining packet!
3. **Integer underflow on `expected_seq - 1`**:
   * If sequence starts at 0 and no packet has arrived yet, `expected_seq - 1` can underflow a `u32` if not checked! Use an explicit `Option<u32>` or safe initial state.
