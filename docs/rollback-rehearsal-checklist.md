# Rust Rollback Rehearsal Checklist

Use this checklist before GA sign-off to validate rollback from Rust release artifacts to the last stable Fortran artifact bundle.

## GA Platform Scope

- macOS 14 arm64 (`aarch64-apple-darwin`)
- Ubuntu 22.04 x86_64 (`x86_64-unknown-linux-gnu`)

## Rollback Artifact Scope

- Rust candidate artifact:
  - `target/release/feff10-rs` (or packaged equivalent)
- Last stable Fortran artifact bundle:
  - `artifacts/fortran-baselines/FX-WORKFLOW-XAS-001/baseline/`
  - `artifacts/fortran-baselines/FX-WORKFLOW-XAS-001/checksums.sha256`

## Smoke Fixture Scope

- Fixture: `FX-WORKFLOW-XAS-001`
- Seed input: `artifacts/fortran-baselines/FX-WORKFLOW-XAS-001/baseline/feff.inp`
- Required workflow artifacts after rollback:
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

1. Run release quality gates:
   - `cargo check --locked`
   - `cargo test --locked`
   - `cargo clippy --locked --all-targets -- -D warnings`
   - `cargo fmt --all -- --check`
2. Build the Rust release artifact:
   - `cargo build --locked --release`
3. Validate pre-rollback Rust smoke behavior:
   - Run `feff10-rs feff` in a workspace-descendant smoke directory seeded with the fixture `feff.inp`.
   - Verify required workflow artifacts are present and stderr is empty.
4. Stage rollback deployment roots:
   - `deployment/rust-current` for the active Rust release state.
   - `deployment/fortran-stable` for the last stable Fortran artifact bundle.
5. Simulate rollback by switching deployment pointer:
   - Before: `deployment/current -> deployment/rust-current`
   - After: `deployment/current -> deployment/fortran-stable`
6. Verify rollback smoke artifacts:
   - Confirm all required workflow artifacts exist under `deployment/current/FX-WORKFLOW-XAS-001/baseline/`.
7. Run focused rollback parity validation:
   - Generate a single-fixture manifest for `FX-WORKFLOW-XAS-001`.
   - Run `feff10-rs regression` with:
     - `--baseline-root artifacts/fortran-baselines`
     - `--actual-root <rollback deployment/current path>`
     - `--baseline-subdir baseline`
     - `--actual-subdir baseline`
   - Expect regression status `PASS`.
8. Record final rollback evidence and GA decision under `tasks/`:
   - `tasks/rollback-rehearsal-YYYY-MM-DD.md`
   - Include sign-off outcome (`GO` or `NO-GO`) and explicit rationale.
