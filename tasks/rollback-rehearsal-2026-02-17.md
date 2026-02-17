# Rollback Rehearsal And GA Sign-Off Report - 2026-02-17

Story: `US-045`  
Cutover checklist: `docs/cutover-rehearsal-checklist.md`  
Rollback checklist: `docs/rollback-rehearsal-checklist.md`  
Prior cutover report: `tasks/cutover-rehearsal-2026-02-17.md`  
Execution date: 2026-02-17 (JST)

## Checklist Execution

- [x] Ran release quality gates (`cargo check --locked`, `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --all -- --check`).
- [x] Built the Rust release artifact.
- [x] Validated pre-rollback Rust smoke behavior for `FX-WORKFLOW-XAS-001`.
- [x] Simulated rollback by switching `deployment/current` from Rust artifact state to stable Fortran bundle state.
- [x] Verified required rollback workflow artifacts after pointer switch.
- [x] Ran focused rollback parity validation (`1/1` fixtures, `45/45` artifacts, PASS).
- [x] Ran full end-to-end regression gate (`17/17` fixtures, `921/921` artifacts, PASS).

## Rehearsal Inputs

### Rust Candidate Artifact

- Path: `artifacts/rollback-rehearsal/2026-02-17/rust-release/feff10-rs`
- SHA-256: `33ce8c7f85b1b4b2cdf65567d9868ec7efff4527f2d5efc56b2976cfbed6f436`
- Size: `2,483,296` bytes

### Last Stable Fortran Artifact Bundle

- Bundle path: `artifacts/fortran-baselines/FX-WORKFLOW-XAS-001/baseline/`
- Integrity file: `artifacts/fortran-baselines/FX-WORKFLOW-XAS-001/checksums.sha256`
- Integrity file SHA-256: `cc977d1173f9e5ae2d245788aa8380aaf2d0e88ec37ffcf88335e31da3a1b9e0`
- Integrity file size: `3,399` bytes

## Rollback Execution

### Pre-Rollback Rust Smoke (Cutover State)

- Working directory: `artifacts/rollback-rehearsal/2026-02-17/rust-smoke/workdir`
- Command: `target/release/feff10-rs feff`
- Result: PASS
- stdout summary:
  - `Running RDINP...`
  - `Running POT...`
  - `Running XSPH...`
  - `Running PATH...`
  - `Running FMS...`
  - `Completed serial workflow for fixture 'FX-WORKFLOW-XAS-001'.`
- stderr: empty

### Artifact Reversion Drill

- Deployment pointer before rollback:
  - `deployment/current -> /Users/ryuichi/dev/feff10-rs/artifacts/rollback-rehearsal/2026-02-17/deployment/rust-current`
- Deployment pointer after rollback:
  - `deployment/current -> /Users/ryuichi/dev/feff10-rs/artifacts/rollback-rehearsal/2026-02-17/deployment/fortran-stable`
- Rollback operation:
  - Restored `FX-WORKFLOW-XAS-001` from stable Fortran bundle into `deployment/fortran-stable`.
  - Switched active deployment symlink to `deployment/fortran-stable`.

## Rollback Smoke Verification

- Required workflow artifacts verified after rollback: `15/15` present under:
  - `artifacts/rollback-rehearsal/2026-02-17/deployment/current/FX-WORKFLOW-XAS-001/baseline/`
- Focused rollback regression validation:
  - Manifest: `artifacts/rollback-rehearsal/2026-02-17/workflow-manifest.json`
  - Command: `cargo run --locked -- regression ... --actual-root artifacts/rollback-rehearsal/2026-02-17/deployment/current ...`
  - Result: PASS
  - Fixtures: `1/1` passed
  - Artifacts: `45/45` passed
  - Summary: `artifacts/rollback-rehearsal/2026-02-17/regression-summary.txt`
  - Report: `artifacts/rollback-rehearsal/2026-02-17/regression-report.json`

## Final End-To-End Regression Gate

- Command: `cargo run --locked -- regression --manifest tasks/golden-fixture-manifest.json --policy tasks/numeric-tolerance-policy.json --baseline-root artifacts/fortran-baselines --actual-root artifacts/fortran-baselines --baseline-subdir baseline --actual-subdir baseline --report artifacts/rollback-rehearsal/2026-02-17/final-regression-report.json`
- Result: PASS
- Fixtures: `17/17` passed
- Artifacts: `921/921` passed
- Summary: `artifacts/rollback-rehearsal/2026-02-17/final-regression-summary.txt`
- Report: `artifacts/rollback-rehearsal/2026-02-17/final-regression-report.json`

## GA Readiness Sign-Off

- Decision timestamp: 2026-02-17 13:29 JST
- Decision: `GO`
- Rationale:
  - Cutover rehearsal evidence is recorded in `tasks/cutover-rehearsal-2026-02-17.md`.
  - Rollback artifact reversion and smoke verification completed successfully.
  - Focused rollback parity validation passed with zero fixture/artifact failures.
  - Final end-to-end regression gate passed across the full approved fixture set.
  - Rust quality gates remain release-blocking in CI (`.github/workflows/rust-quality-gates.yml` and `.github/workflows/rust-parity-gates.yml`).

## Follow-Up (Non-Blocking)

- Repeat the GA Linux smoke steps from `docs/cutover-rehearsal-checklist.md` on a native Ubuntu 22.04 x86_64 runner during release execution to mirror production hardware directly.
