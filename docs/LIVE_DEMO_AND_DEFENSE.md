# Live Demo Guide & Viva Defense Strategy

**Course:** CS-30003: Computer Networks  
**Project:** Reliable Data Transfer over UDP  
**Presenter:** Sahan Maiti (Team Lead) & Team  

---

## 🚀 1. The 3-Minute Live Viva Demo Script

During the lab evaluation or viva, follow this exact sequence to demonstrate the system live.

### Demo Setup (Open Two Terminals)

Both commands run within the project root directory: `c:\Users\KIIT\Desktop\CN_Project`.

---

### Step 1: Benchmark Pipeline In-Memory (Instant Verification)
Run this command in Terminal 1 to demonstrate all three ARQ protocols running against synthetic channel faults with SHA-256 validation:

```bash
cargo run -- bench-pipeline --bytes 140000 --loss 0.05 --reorder 0.10
```

**What the professor sees:**
- Packet chunking into 1400-byte segments.
- Stop-and-Wait vs. Go-Back-N vs. Selective Repeat running under 5% packet loss and 10% packet reordering.
- Real-time adaptive RTO calculations.
- Final output showing `SHA-256 Match: PASSED` for all three protocols, with live throughput and retransmission metrics.

---

### Step 2: Live File Transfer over Raw UDP Sockets (Receiver & Sender)

#### Terminal 1 (Receiver):
Start the UDP receiver listening on port `9001`, saving incoming data to `received_output.bin`:
```bash
cargo run -- recv --addr 127.0.0.1:9001 --protocol sr --output received_output.bin
```

#### Terminal 2 (Sender):
Send a file over UDP with 5% packet drop and 10% reordering injected:
```bash
cargo run -- send --addr 127.0.0.1:9001 --protocol sr --input README.md --loss 0.05 --reorder 0.10 --seed 42
```

#### Verification:
Verify the cryptographic SHA-256 match between the original and reconstructed file:
```bash
cargo run -- verify --file1 README.md --file2 received_output.bin
```
**Expected Output:**
```
[+] SHA-256 Checksum Verification:
    File 1: README.md -> a1b2c3...
    File 2: received_output.bin -> a1b2c3...
[+] INTEGRITY VERIFIED: Bytes match identically (100% fidelity).
```

---

### Step 3: Show Experimental Parameter Sweep & Plots
Show the generated publication-quality plots:
```bash
python experiments/python/plot_results.py
```
Open the plots folder to show the examiners:
- `plots/retransmissions_vs_reordering.svg`
- `plots/goodput_vs_reordering.svg`
- `plots/goodput_vs_loss.svg`

Point to the graph and state your key thesis:
> *"As seen here, Go-Back-N retransmissions spike to over 70 packets because its receiver discards out-of-order packets. Selective Repeat buffers them in a BTreeMap, keeping retransmissions near zero."*

---

## 🎯 2. High-Frequency Viva Questions & Model Answers

### Q1: Why 1400 bytes chunk size?
**Answer:**  
Standard Ethernet MTU is 1500 bytes. Subtracting IPv4 header (20 bytes), UDP header (8 bytes), and our custom header (10 bytes) gives $1500 - 38 = 1462$ bytes max payload. Choosing **1400 bytes** leaves ample headroom for network tunneling (e.g., PPPoE, IPSec, VLAN tags) and prevents intermediate IP-level fragmentation.

### Q2: Why RFC 1071 16-bit Internet Checksum if UDP already has a checksum?
**Answer:**  
1. In IPv4, UDP checksums are optional and often offloaded to hardware (NIC offloading).
2. More importantly, our project includes a deterministic application-layer channel emulator that intentionally flips bits. Having an explicit packet checksum allows us to test and demonstrate error detection and packet discard behavior deterministically.

### Q3: Why does Selective Repeat require window size $W \le \frac{1}{2} \text{SeqSpace}$?
**Answer:**  
If the window size exceeds half the sequence number space, the sender's retransmission window can overlap with the receiver's next cycle window. If all ACKs in a window are lost, the sender retransmits packet 0. If $W > \text{SeqSpace}/2$, the receiver cannot disambiguate whether packet 0 is a duplicate retransmission from the previous window or a new packet from the next sequence cycle.

### Q4: Why is Karn's Algorithm essential in Jacobson/Karels RTO estimation?
**Answer:**  
When a packet times out and is retransmitted, receiving an ACK creates **acknowledgment ambiguity**: we cannot know whether the ACK corresponds to the first transmission or the second retransmission. Including this sample would corrupt the Smoothed RTT ($SRTT$) and variance ($RTTVAR$). Karn's algorithm strictly ignores RTT samples from retransmitted packets and instead doubles the RTO (exponential backoff).

### Q5: Why is Go-Back-N so vulnerable to packet reordering?
**Answer:**  
GBN has a receiver window of size 1 ($W_{recv} = 1$). It accepts only the strictly expected sequence number (`expected_seq`). If packet 3 arrives before packet 2 due to reordering, GBN drops packet 3 and returns a duplicate ACK for packet 1. When the sender's timer expires, it must retransmit packet 2 **and** packet 3, causing an $O(W \times P_{\text{reorder}})$ cascade of redundant retransmissions.

---

## 🛠️ 3. Handling Live Code Modification Requests

If an examiner asks you to modify the code on the spot:

| Examiner Request | File to Edit | Exact Modification |
|---|---|---|
| **Change default window size from 8 to 16** | [`src/arq/gbn.rs`](file:///c:/Users/KIIT/Desktop/CN_Project/src/arq/gbn.rs) & [`src/arq/sr.rs`](file:///c:/Users/KIIT/Desktop/CN_Project/src/arq/sr.rs) | Change `const DEFAULT_WINDOW_SIZE: usize = 8;` to `16`. |
| **Change Initial RTO from 1.0s to 500ms** | [`src/timing/rto.rs`](file:///c:/Users/KIIT/Desktop/CN_Project/src/timing/rto.rs) | Modify `RtoEstimator::new()` to initialize `rto: Duration::from_millis(500)`. |
| **Disable Karn's Algorithm (sample retransmissions)** | [`src/timing/rto.rs`](file:///c:/Users/KIIT/Desktop/CN_Project/src/timing/rto.rs) | In `sample_rtt(sample, is_retransmission)`, remove the `if is_retransmission { return; }` check. |
| **Change Chunk Size from 1400 to 1024** | [`src/app/file_io.rs`](file:///c:/Users/KIIT/Desktop/CN_Project/src/app/file_io.rs) | Change `const DEFAULT_CHUNK_SIZE: usize = 1400;` to `1024`. |
