# Cutover Rehearsal Report - 2026-02-16

Story: `US-044`  
Checklist: `docs/cutover-rehearsal-checklist.md`  
Execution date: 2026-02-16 (JST)

## Checklist Execution

- [x] Documented cutover checklist for GA platform rehearsal.
- [x] Built Rust release artifacts for both GA targets.
- [x] Ran fixture smoke command (`feff10-rs feff`) on both GA targets.
- [x] Verified required core workflow outputs for both smoke runs.
- [x] Recorded artifact checksums, outcomes, and unresolved issues.

## Rehearsal Environment

### GA Target 1: macOS 14 arm64 (`aarch64-apple-darwin`)

- Host kernel: `Darwin 25.2.0 arm64`
- Toolchain: `rustc 1.93.0`, `cargo 1.93.0`
- Build command:
  - `cargo build --locked --release`
- Artifact:
  - Path: `target/release/feff10-rs`
  - SHA-256: `a6e406d51a62885453bd60db743c430e5746a2e2640b25700672f48ad8a351b5`
  - Size: `2,115,744` bytes
- Smoke run:
  - Working directory: `artifacts/cutover-rehearsal/2026-02-16/macos-smoke`
  - Command: `target/release/feff10-rs feff`
  - Result: PASS
  - stderr: empty
  - stdout summary:
    - `Running RDINP...`
    - `Running POT...`
    - `Running XSPH...`
    - `Running PATH...`
    - `Running FMS...`
    - `Completed serial workflow for fixture 'FX-WORKFLOW-XAS-001'.`

### GA Target 2: Ubuntu 22.04 x86_64 (`x86_64-unknown-linux-gnu`)

- Environment:
  - `docker run --rm --platform linux/amd64 ubuntu:22.04 ...`
  - Kernel (container runtime): `Linux 6.8.0-64-generic x86_64`
  - Stable Rust installed in-container via rustup
- Build command:
  - `cargo build --locked --release`
- Artifact:
  - Path: `artifacts/cutover-rehearsal/2026-02-16/linux-smoke/feff10-rs`
  - SHA-256: `485d3dc7bf41697f73fc0fcbb6fe5243e5da512bc4e545f0ae7a52a08a5fb7cc`
  - Size: `2,635,064` bytes
- Smoke run:
  - Working directory: `artifacts/cutover-rehearsal/2026-02-16/linux-smoke/workdir`
  - Command: `feff10-rs feff`
  - Result: PASS
  - stderr: empty
  - stdout summary:
    - `Running RDINP...`
    - `Running POT...`
    - `Running XSPH...`
    - `Running PATH...`
    - `Running FMS...`
    - `Completed serial workflow for fixture 'FX-WORKFLOW-XAS-001'.`

## Outcome

- Cutover rehearsal status: PASS for documented checklist execution, release artifact generation, and smoke validation on both GA targets.

## Unresolved Issues

- Linux validation was executed in an Ubuntu 22.04 `linux/amd64` container under emulation on an arm64 host. Re-run the same checklist on a native Ubuntu 22.04 x86_64 runner before final GA sign-off.

