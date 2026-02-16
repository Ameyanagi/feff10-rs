# Rust Cutover Rehearsal Checklist

Use this checklist before GA cutover to validate Rust-native release artifacts and smoke behavior on all GA platforms from D-1.

## GA Platform Scope

- macOS 14 arm64 (`aarch64-apple-darwin`)
- Ubuntu 22.04 x86_64 (`x86_64-unknown-linux-gnu`)

## Smoke Fixture Scope

- Fixture: `FX-WORKFLOW-XAS-001`
- Input seed: `artifacts/fortran-baselines/FX-WORKFLOW-XAS-001/baseline/feff.inp`
- Smoke command: `feff10-rs feff`
- Required smoke outputs:
  - `geom.dat`
  - `global.inp`
  - `pot.inp`
  - `xsph.inp`
  - `paths.inp`
  - `fms.inp`
  - `pot.bin`
  - `phase.bin`
  - `paths.dat`
  - `gg.bin`
  - `log.dat`
  - `log1.dat`
  - `log2.dat`
  - `log3.dat`
  - `log4.dat`

## Rehearsal Steps

1. Run local quality gates:
   - `cargo check --locked`
   - `cargo test --locked`
2. Build release artifact for macOS arm64:
   - `cargo build --locked --release`
3. Build release artifact for Ubuntu 22.04 x86_64:
   - Run in Linux environment (native or containerized `ubuntu:22.04`) with stable Rust.
   - Build command: `cargo build --locked --release`
4. For each GA platform, run smoke validation:
   - Use a workspace-descendant smoke directory so CLI workspace discovery can resolve `tasks/golden-fixture-manifest.json`.
   - Stage `feff.inp` from the fixture seed path.
   - Execute release binary: `./feff10-rs feff`
   - Verify all required smoke outputs exist.
   - Verify smoke stderr is empty.
5. Capture release artifact evidence per platform:
   - Binary checksum (`sha256`)
   - Binary size
   - Smoke command output summary
6. Record rehearsal outcomes and unresolved issues in a dated report under `tasks/`:
   - `tasks/cutover-rehearsal-YYYY-MM-DD.md`

