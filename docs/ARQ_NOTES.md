# ARQ Protocols Study & Architecture Notes

**Author:** Soumyadeb Mukherjee  
**Component:** Protocol & ARQ  
**Course:** CS-30003  

---

## 1. Fundamentals of Reliability over UDP

UDP provides unreliable, connectionless datagram delivery:
* Packets may be **lost** in transit.
* Packets may arrive **out of order**.
* Packets may be **duplicated**.
* Packets may suffer **bit corruption**.

To provide reliable file transfer, the ARQ (Automatic Repeat reQuest) layer introduces:
1. **Sequence Numbers**: Identifying each packet uniquely.
2. **Acknowledgements (ACKs)**: Feedback from receiver to sender.
3. **Timeouts & Retransmission**: Triggering re-sends when ACKs are missing.
4. **Buffering & Flow Control**: Managing in-flight data.

---

## 2. Stop-and-Wait ARQ

### Concept & Purpose
The simplest sliding window protocol where the sender window size is $W = 1$. The sender transmits one packet and waits for its corresponding ACK before sending the next packet.

```text
Sender                         Receiver
  |                              |
  | -------- DATA(seq=0) ------> |  (store, deliver)
  |                              |
  | <--------- ACK(0) ---------- |  (advance to 1)
  |                              |
  | -------- DATA(seq=1) ------> |
```

### Sender State
* `current_seq`: Sequence number currently being transmitted.
* `waiting_for_ack`: Boolean flag indicating packet in flight.
* `timer`: Active timer for the current packet.
* `retransmit_count`: Number of times the current packet has been re-sent.

### Receiver State
* `expected_seq`: The sequence number the receiver is waiting for (0, 1, 2, ...).

### Behavior & Rules
1. **Sender Behavior**:
   * Send packet with sequence `current_seq`.
   * Start timer.
   * On receiving `ACK(ack_num)` where `ack_num == current_seq`:
     * Cancel timer.
     * Increment `current_seq`.
     * Reset `retransmit_count`.
     * Ready to send next chunk.
   * On Timeout:
     * Retransmit packet `current_seq`.
     * Restart timer.
     * Increment `retransmit_count`.
2. **Receiver Behavior**:
   * On receiving `DATA(seq_num)`:
     * If `seq_num == expected_seq`:
       * Accept data into output stream.
       * Send `ACK(seq_num)`.
       * Increment `expected_seq`.
     * If `seq_num < expected_seq` (Duplicate packet due to lost or delayed ACK):
       * Discard duplicate payload.
       * Re-send `ACK(seq_num)` so sender stops retransmitting!
     * If `seq_num > expected_seq` (Out of order):
       * In Stop-and-Wait, this should theoretically not occur unless corrupted or delayed from an earlier cycle. Discard and send `ACK(expected_seq - 1)`.

### Core Invariant
$$\text{At any time, at most one unacknowledged data packet exists in the network.}$$

---

## 3. Go-Back-N (GBN) ARQ

### Concept & Purpose
Pipelined transmission with sender window size $N > 1$. The sender can transmit up to $N$ packets before needing an ACK. The receiver accepts packets **strictly in order** and discards out-of-order packets.

```text
Sender Window: [base ... base + N - 1]
               ^
              base (oldest unacknowledged)
```

```text
Sender                                Receiver
DATA 0 -----------------------------> Deliver 0, ACK 0
DATA 1 -----------------------------> Deliver 1, ACK 1
DATA 2 --------X (Lost)
DATA 3 -----------------------------> Discard! (out of order, re-ACK 1)
DATA 4 -----------------------------> Discard! (out of order, re-ACK 1)
[Timer for DATA 2 expires!]
DATA 2 -----------------------------> Deliver 2, ACK 2
DATA 3 -----------------------------> Deliver 3, ACK 3
DATA 4 -----------------------------> Deliver 4, ACK 4
```

### Sender State
* `window_size` ($N$): Maximum allowed in-flight packets.
* `send_base`: Oldest unacknowledged sequence number.
* `next_seqnum`: Next sequence number to be assigned to outgoing data.
* `buffer`: Queue/Vector of unacknowledged packets $[send\_base \dots next\_seqnum - 1]$.
* `timer`: Single timer tracking the oldest in-flight packet (`send_base`).

### Receiver State
* `expected_seqnum`: Next in-order sequence number expected.
* `last_ack_sent`: Highest sequence number acknowledged so far.

### Behavior & Rules
1. **Cumulative ACKs**: An `ACK(n)` confirms that **all** packets up to and including $n$ have been successfully received.
2. **Sender ACK Receipt**:
   * If received $ack\_num \ge send\_base$:
     * Slide window: `send_base = ack_num + 1`.
     * Remove acknowledged packets from buffer.
     * If $send\_base == next\_seqnum$, stop timer (all packets ACKed).
     * Else, restart timer for the new `send_base`.
3. **Sender Timeout**:
   * On timeout, sender retransmits **ALL** unacknowledged packets currently in the window: $[send\_base \dots next\_seqnum - 1]$.
   * Restart timer.
4. **Receiver Handling**:
   * If packet arrives with `seq_num == expected_seqnum`:
     * Deliver data to application.
     * Increment `expected_seqnum`.
     * Send cumulative `ACK(expected_seqnum - 1)`.
   * If packet arrives with `seq_num != expected_seqnum`:
     * Discard packet!
     * Re-send `ACK` for `last_ack_sent` (or `expected_seqnum - 1`).

### Strengths & Trade-offs
* **Strengths**: High link utilization; minimal receiver memory (no buffering needed).
* **Trade-off**: High penalty under packet reordering or isolated packet loss; all subsequent packets are retransmitted unnecessarily.

---

## 4. Selective Repeat (SR) ARQ

### Concept & Purpose
Pipelined protocol where **both** sender and receiver maintain sliding windows of size $N$. The receiver buffers out-of-order packets and sends **individual ACKs**. The sender retransmits **only** the specific packets that timed out.

```text
Sender Window:   [base ... base + N - 1]
Receiver Window: [base ... base + N - 1]
```

```text
Sender                                Receiver
DATA 0 -----------------------------> Deliver 0, ACK 0
DATA 1 --------X (Lost)
DATA 2 -----------------------------> Buffer 2, ACK 2 (Individual ACK!)
DATA 3 -----------------------------> Buffer 3, ACK 3 (Individual ACK!)
[Timer for DATA 1 expires!]
DATA 1 -----------------------------> Deliver 1, Deliver buffered 2 & 3, ACK 1
```

### Sender State
* `window_size` ($N$): Max allowed window size.
* `send_base`: Oldest unacknowledged sequence number.
* `next_seqnum`: Next sequence number to assign.
* `slots`: Buffer of entries, each containing:
  * Packet data
  * Status: `Unsent`, `Sent(timestamp)`, `Acked`
  * Retransmit timer for that specific packet

### Receiver State
* `rcv_base`: Smallest sequence number expected and not yet delivered.
* `rcv_window_size` ($N$).
* `buffer`: Array/Map of buffered out-of-order packets within $[rcv\_base \dots rcv\_base + N - 1]$.

### Sequence Number Space Invariant
$$\text{Max Window Size } N \le \frac{1}{2} \times \text{Sequence Number Space}$$
* Prevents ambiguity where the receiver cannot distinguish between a new packet and a duplicate retransmission of an old packet when sequence numbers wrap around.

---

## 5. Comparison Matrix

| Property | Stop-and-Wait | Go-Back-N | Selective Repeat |
| :--- | :--- | :--- | :--- |
| **Sender Window Size ($W_s$)** | $1$ | $N > 1$ | $N > 1$ |
| **Receiver Window Size ($W_r$)** | $1$ | $1$ | $N$ |
| **ACK Type** | Individual / Next expected | Cumulative | Individual |
| **Receiver Buffering** | None | None (discards out-of-order) | Buffers out-of-order within window |
| **Retransmissions on Loss** | Single packet | Entire window from lost packet | Only the missing packet |
| **Impact of Packet Reordering**| Minor (delays progress) | **Severe** (causes spurious drops & retransmissions) | **Minimal** (buffered and assembled) |
| **Implementation Complexity** | Low | Medium | High |
