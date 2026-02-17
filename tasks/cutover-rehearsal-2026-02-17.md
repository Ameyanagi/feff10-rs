# Cutover Rehearsal Report - 2026-02-17

Story: `US-045`  
Checklist: `docs/cutover-rehearsal-checklist.md`  
Execution date: 2026-02-17 (JST)

## Checklist Execution

- [x] Documented cutover checklist for GA platform rehearsal.
- [x] Built Rust release artifacts for both GA targets.
- [x] Ran fixture smoke command (`feff10-rs feff`) on both GA targets.
- [x] Verified required core workflow outputs for both smoke runs.
- [x] Recorded artifact checksums, outcomes, and unresolved issues.

## Rehearsal Environment

### GA Target 1: macOS 14 arm64 (`aarch64-apple-darwin`)

- Host kernel: `Darwin ferris 25.2.0 Darwin Kernel Version 25.2.0: Tue Nov 18 21:08:48 PST 2025; root:xnu-12377.61.12~1/RELEASE_ARM64_T8132 arm64 arm Darwin`
- Toolchain: `rustc 1.93.0`, `cargo 1.93.0`
- Build command:
  - `cargo build --locked --release`
- Artifact:
  - Path: `artifacts/cutover-rehearsal/2026-02-17/macos-smoke/feff10-rs`
  - SHA-256: `33ce8c7f85b1b4b2cdf65567d9868ec7efff4527f2d5efc56b2976cfbed6f436`
  - Size: `2,483,296` bytes
- Smoke run:
  - Working directory: `artifacts/cutover-rehearsal/2026-02-17/macos-smoke/workdir`
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
  - Kernel (container runtime): `Linux b20d7059867f 6.8.0-64-generic #67-Ubuntu SMP PREEMPT_DYNAMIC Sun Jun 15 20:23:40 UTC 2025 x86_64 x86_64 x86_64 GNU/Linux`
  - Stable Rust installed in-container via rustup
- Toolchain: `rustc 1.93.1`, `cargo 1.93.1`
- Build command:
  - `cargo build --locked --release`
- Artifact:
  - Path: `artifacts/cutover-rehearsal/2026-02-17/linux-smoke/feff10-rs`
  - SHA-256: `51960c44e1d35d44c6d57e2b7746196bbd146fc6664b4641462d4d2f1780980e`
  - Size: `3,136,520` bytes
- Smoke run:
  - Working directory: `artifacts/cutover-rehearsal/2026-02-17/linux-smoke/workdir`
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
