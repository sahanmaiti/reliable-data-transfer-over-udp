# UDP Integration Loop (Sahan)

The reliable transfer path is wired in `src/app/transfer.rs`. ARQ state machines, timers, and the channel emulator remain owned by teammates and plug in through traits once merged.

## Data flow (Day 7 target)

```text
File -> Chunker -> ArqSender -> bytes -> ChannelEmulator -> UdpSocket
                                                              |
UdpSocket -> ChannelEmulator -> ArqReceiver -> Reassembler -> SHA-256 -> ExperimentRecord
```

## CLI ( `reliable_udp` )

| Command | Purpose |
|---------|---------|
| `send` | Sender side: source file, bind/connect addresses, protocol, channel + RTO config |
| `recv` | Receiver side: output file, listen address, same protocol config |
| `run-experiment` | Single trial: runs send+recv (or documented loopback setup), writes JSON/CSV metrics |

Until `arq`, `timing`, and `channel` modules are linked, these commands return a clear **integration not ready** error listing missing pieces.

## Python harness

`experiments/python/run_experiments.py` invokes:

```bash
cargo run --release -- run-experiment --file … --protocol … --seed … \
  --loss-rate … --reorder-rate … --rto-multiplier … --json-out … --csv-out …
```

No synthetic retransmission formulas in Python.
