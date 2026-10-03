# Packet Format & Framing Specification

**Author:** Soumyadeb Mukherjee  
**Component:** Protocol & ARQ  
**Course:** CS-30003  

---

## 1. Overview & Framing Goals

All three ARQ protocols (**Stop-and-Wait**, **Go-Back-N**, and **Selective Repeat**) share this uniform packet format. Operating directly over UDP datagrams, this framing layer guarantees:

1. **Identification**: Disambiguates `DATA` packets from `ACK` packets.
2. **Sequencing**: Tracks ordering and detects duplicates with a 32-bit sequence number.
3. **Payload Boundaries**: Explicit payload length prevents buffer overrun and garbage byte ingestion.
4. **Data Integrity**: 16-bit Internet Checksum (RFC 1071) detects bit errors induced by noisy channels or the channel emulator.

---

## 2. Packet Layout (10-Byte Header + Variable Payload)

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        Sequence Number                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Packet Type  |     Flags     |         Payload Length        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|           Checksum            |        Payload Data ...       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+                               |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### Field Breakdown

| Offset (Bytes) | Field Name | Data Type | Size (Bytes) | Description |
| :--- | :--- | :--- | :--- | :--- |
| **0 – 3** | `seq_num` | `u32` | 4 | Packet sequence number (for DATA) or acknowledged sequence number (for ACK). |
| **4** | `pkt_type` | `u8` | 1 | `0x01` = `DATA`, `0x02` = `ACK`, `0x03` = `FIN` (End of Transfer). |
| **5** | `flags` | `u8` | 1 | Control flags (e.g., bit 0: retransmitted packet indicator; default `0x00`). |
| **6 – 7** | `payload_len` | `u16` | 2 | Size of the payload in bytes ($0$ for standalone ACKs; max standard payload typically $1024$ or $1400$ bytes). |
| **8 – 9** | `checksum` | `u16` | 2 | 16-bit One's Complement Internet Checksum (RFC 1071) covering header and payload. |
| **10+** | `payload` | `[u8]` | `payload_len` | Raw chunk of source file data. |

---

## 3. Endianness & Wire Format

* **Byte Ordering**: **Big-Endian (Network Byte Order)** is strictly enforced for all multi-byte fields (`seq_num`, `payload_len`, `checksum`).
* In Rust:
  * Serialization uses `to_be_bytes()`.
  * Deserialization uses `from_be_bytes()`.

---

## 4. Packet Types & Semantics

### `DATA` (`0x01`)
* Sent by: **Sender**.
* `seq_num`: The monotonically increasing packet sequence number (0-indexed).
* `payload_len`: Number of data bytes (typically $1 \le \text{payload\_len} \le \text{MAX\_PAYLOAD}$).
* `payload`: Sliced chunk of the file.

### `ACK` (`0x02`)
* Sent by: **Receiver**.
* `seq_num`:
  * **Stop-and-Wait**: Sequence number of the received packet being acknowledged.
  * **Go-Back-N**: Cumulative ACK — highest in-order sequence number received (or next expected sequence number, agreed across protocols).
  * **Selective Repeat**: Individual ACK — exact sequence number of the specific packet being acknowledged.
* `payload_len`: Always `0`.
* `payload`: Empty.

### `FIN` (`0x03`)
* Sent by: **Sender** once all file data has been acknowledged.
* Signals to receiver that the transfer is complete, allowing receiver to finalize the SHA-256 hash and flush file buffers.

---

## 5. Checksum Specification (RFC 1071)

### Calculation Algorithm
1. Treat the entire packet (header + payload) as a sequence of 16-bit integers (in big-endian order).
2. During calculation, set the `checksum` field in the header to `0`.
3. If total byte count is odd, pad the last byte with a trailing `0x00` for the checksum computation.
4. Sum all 16-bit integers using 32-bit arithmetic to capture carry bits.
5. Fold the carry bits into the lower 16 bits: `sum = (sum >> 16) + (sum & 0xFFFF)` until no carry remains.
6. Take the bitwise NOT (one's complement) of the result: `!sum as u16`.

### Verification Algorithm
1. Compute the checksum over the received byte array, leaving the received `checksum` field intact.
2. If the data is uncorrupted, the one's complement sum of the data and its checksum will equal `0x0000` (or `0xFFFF` depending on final inversion).
3. If corruption is detected:
   * Discard the packet immediately.
   * Log the event for channel metrics.
   * Treat the packet as lost (let ARQ timeout handle recovery).
