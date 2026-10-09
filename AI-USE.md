# AI Use Statement

This project was developed by the listed student team for CS-30003. AI coding assistants (including Cursor) were used during development. This file discloses that assistance as accurately as the available evidence and confirmed team recollection allow.

The course permits AI assistance for explaining concepts, debugging, code review, generating tests, boilerplate, plotting/analysis scripts, and improving writing. It prohibits submitting AI-generated **core protocol implementation** as the team’s own work without genuine authorship and understanding. An honest disclosure is required.

## Team responsibility areas

These describe ownership areas. They do **not** claim that one person wrote every line in that area without assistance, or that every teammate has already demonstrated complete viva-ready understanding of every module.

- **Soumyadeb Mukherjee (24052329)** — Protocol & ARQ (packet semantics; Stop-and-Wait; Go-Back-N; Selective Repeat; protocol-state testing)
- **Kashish Gupta (24052495)** — Timing & RTO (retransmission timers; Jacobson/Karels; SRTT/RTTVAR; backoff; Karn; timing tests)
- **Ashwika Burman (24155095)** — Channel emulator (seeded fault injection; loss/dup/reorder/corrupt/delay; emulator testing)
- **Sahan Maiti (24052247)** — Evaluation & Integration (file integrity; metrics; experiment automation; raw results; plots; integration; end-to-end validation)

Git commit **authors** for the initial module drops attribute ARQ/packet work to Soumyadeb, timing/RTO to Kashish, and the channel emulator to Ashwika. Committer fields sometimes differ from authors; that is recorded in Git history and is not rewritten here.

## How AI was used

### 1. Explanations, debugging, review, and suggestions

AI tools were used to explain concepts, discuss design trade-offs, debug failures, and review code. Suggestions were accepted, rejected, or modified by the team.

### 2. Tests, plotting, analysis, and documentation

AI assistance was used for test drafting and refinement, experiment orchestration and analysis scripts (Python), plotting helpers, and drafting/editing documentation and reports. Generated measurements were **not** invented by AI: experimental JSON/CSV records come from the Rust `run-experiment` path.

### 3. Implementation suggestions and generated code (reviewed/tested)

AI assistance across **ARQ**, **timing/RTO**, and **channel-emulator** work was **mixed**: repository history and file headers attribute module ownership to the members above, but the exact fraction of AI-generated versus hand-written lines in those original modules **cannot be established from Git history alone**.

Later Cursor-assisted work (as recalled by Sahan) was used primarily as **suggestions** during integration, experiment automation, localhost UDP wiring, testing, and documentation workflows. Those suggestions were reviewed and tested before retention. Successful tests and code review do **not**, by themselves, prove human authorship of every line.

### 4. Human decisions and responsibility

The team chose architecture, protocol behaviour, experimental factors, and evaluation criteria. The team ran and froze experimental datasets, validated integrity hashes, and is responsible for the final submission—including defending the system in viva.

### 5. Uncertainties (explicit)

- The precise extent of AI-generated code inside the original Stop-and-Wait / Go-Back-N / Selective Repeat / packet implementations cannot be proven from the repository alone.
- The same uncertainty applies to the original timing/RTO and channel-emulator modules.
- Whether every team member can currently explain every edge case of every module has **not** been independently verified in this document; the team must ensure understanding before submission.

## What we do not claim

- AI did not invent the frozen experimental measurements under `results/raw/` or `results/rto_sensitivity/`.
- Python does not implement the core transport; Rust does.
- This file does **not** claim that AI wrote the entire project.
- This file does **not** claim that AI only ever provided suggestions for every module if substantial generation occurred—that extent remains partly uncertain and is disclosed as such above.
- This file does **not** attribute a teammate’s module ownership to Sahan.

## Tools

Cursor-based AI assistants were used during development. Exact prompt logs and dates are not retained in the repository; this disclosure is limited to confirmed recollection and available evidence.
