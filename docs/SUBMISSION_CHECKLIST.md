# Submission Checklist

Honest status at Phase 11 documentation/QA close.

## Implementation

- [x] Packet framing (`src/packet.rs`)
- [x] Stop-and-Wait (`src/arq/sw.rs`)
- [x] Go-Back-N (`src/arq/gbn.rs`)
- [x] Selective Repeat (`src/arq/sr.rs`)
- [x] Chunking/reassembly (`src/app/file_io.rs`)
- [x] Checksums (RFC 1071 in packet module)
- [x] Duplicate handling (ARQ receivers/senders)
- [x] In-order delivery (ARQ + reassembler)
- [x] Adaptive RTO (`src/timing/rto.rs`)
- [x] Karn’s algorithm
- [x] Exponential backoff
- [x] Channel emulator (`src/channel/`)
- [x] SHA-256 (`src/app/integrity.rs`)
- [x] Real UDP path (`src/app/udp.rs`, CLI `send`/`recv`)

## Testing

- [x] Unit tests (packet, ARQ, timing, channel, file I/O)
- [x] Integration / transfer tests (`tests/test_transfer.rs`)
- [x] Three-protocol UDP loopback (`tests/test_udp.rs`)
- [x] Empty-file UDP test
- [x] Release build (`cargo build --release`)

## Experiments

- [x] E1 primary reordering study (90 trials, frozen)
- [x] E4 RTO sensitivity (45 trials, separate directory)
- [x] Raw results preserved (`results/raw/`, `results/rto_sensitivity/`)
- [x] Analysis generated
- [x] Confidence intervals (Student-t)
- [x] Failure accounting (E1: zero failures recorded)
- [x] Reproducibility information (`docs/REPRODUCIBILITY.md`, provenance)
- [x] Window-size study (65 trials, `results/window_goodput/`)
- [x] Loss-rate study (90 trials, `results/loss_goodput/`)

## Documentation

- [x] README
- [x] Final report (`docs/FINAL_REPORT.md`)
- [x] Proposal compliance (`docs/PROPOSAL_COMPLIANCE.md`)
- [x] Reproducibility (`docs/REPRODUCIBILITY.md`)
- [x] Demo (`docs/DEMO.md`)
- [x] AI-USE (`AI-USE.md`)
- [x] Viva notes (`docs/VIVA_NOTES.md`)
- [x] Submission checklist (this file)

## Repository hygiene

- [x] No secrets / API keys found in audited docs
- [x] Synthetic pilot clearly archived and labelled non-evidence
- [x] Primary JSON hash documented
- [x] E4 outputs separated from `results/raw/`
- [ ] Full-repo `cargo fmt --check` clean — **pre-existing unrelated formatting diffs remain** (not mass-reformatted in Phase 11)
- [ ] Working tree is dirty relative to last commit — **human should decide what to commit** before submission
- [ ] Confirm evaluator machine uses relative paths from README (no `/Users/...` in committed docs)

## Human attention before hand-in

1. Review and commit documentation + code intentionally (do not overwrite `results/raw/` or E4 data).
2. Decide whether optional E2/E3 are required by local marking; they are not present.
3. Optionally run the 3–5 minute demo from `docs/DEMO.md` once on the submission machine.
4. Re-check primary hash: `d93dc2b18b49775c35587836e042a9daf6dfc844e85427a6be927180c6f0e8be`.
