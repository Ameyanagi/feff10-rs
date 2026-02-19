# Rust Rollback Rehearsal Report - 2026-02-19

## Rehearsal Metadata

- Executed at: 2026-02-19 14:02:28 JST
- Workspace: `/Users/ryuichi/dev/feff10-rs`
- Evidence root: `artifacts/rollback-rehearsal/2026-02-19/20260219-140228`
- Operator platform: macOS 14 arm64 (`aarch64-apple-darwin`)
- Checklist references:
  - `docs/cutover-rehearsal-checklist.md`
  - `docs/rollback-rehearsal-checklist.md`
- CI workflow references:
  - `.github/workflows/rust-quality-gates.yml`
  - `.github/workflows/rust-parity-gates.yml`

## Gate Commands (CI-Equivalent)

### Quality gate command set

```bash
scripts/fortran/ensure-feff10-reference.sh
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
```

### Parity gate command set

```bash
capture_runner="${ORACLE_CAPTURE_RUNNER:-$(pwd)/scripts/fortran/ci-oracle-capture-runner.sh}"

cargo run --locked -- oracle \
  --manifest tasks/golden-fixture-manifest.json \
  --policy tasks/numeric-tolerance-policy.json \
  --oracle-root artifacts/fortran-oracle-capture \
  --oracle-subdir outputs \
  --actual-root artifacts/fortran-baselines \
  --actual-subdir baseline \
  --report artifacts/rollback-rehearsal/2026-02-19/20260219-140228/parity/oracle-report.json \
  --capture-runner "${capture_runner}" \
  --capture-allow-missing-entry-files \
  > artifacts/rollback-rehearsal/2026-02-19/20260219-140228/parity/oracle-summary.txt \
  2> artifacts/rollback-rehearsal/2026-02-19/20260219-140228/parity/oracle-stderr.txt
```

## Rollback Validation Command

```bash
cargo run --locked -- regression \
  --manifest artifacts/rollback-rehearsal/2026-02-19/20260219-140228/workflow-manifest.json \
  --policy tasks/numeric-tolerance-policy.json \
  --baseline-root artifacts/fortran-baselines \
  --actual-root artifacts/rollback-rehearsal/2026-02-19/20260219-140228/deployment/current \
  --baseline-subdir baseline \
  --actual-subdir baseline \
  --report artifacts/rollback-rehearsal/2026-02-19/20260219-140228/regression-report.json \
  > artifacts/rollback-rehearsal/2026-02-19/20260219-140228/regression-summary.txt \
  2> artifacts/rollback-rehearsal/2026-02-19/20260219-140228/regression-stderr.txt
```

## Pass/Fail Outcomes

| Step | Status | Evidence artifact |
| --- | --- | --- |
| Fetch FEFF10 reference fixtures (`scripts/fortran/ensure-feff10-reference.sh`) | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/quality/ensure-feff10-reference.log` |
| Typecheck (`cargo check --locked`) | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/quality/cargo-check.log` |
| Tests (`cargo test --locked`) | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/quality/cargo-test.log` |
| Lint (`cargo clippy --locked --all-targets -- -D warnings`) | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/quality/cargo-clippy.log` |
| Formatting (`cargo fmt --all -- --check`) | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/quality/cargo-fmt-check.log` |
| Release build (`cargo build --locked --release`) | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/quality/cargo-build-release.log` |
| Oracle parity (`cargo run --locked -- oracle ...`) | PASS (`passed=true`, `fixture_count=21`, `failed_fixture_count=0`, `mismatch_artifact_count=0`) | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/parity/oracle-report.json` |
| Oracle mismatch detail extraction | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/parity/oracle-diff.txt` |
| Oracle stderr capture | PASS (contains expected staged-entry warnings with `--capture-allow-missing-entry-files`) | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/parity/oracle-stderr.txt` |
| Pre-rollback smoke (`target/release/feff10-rs feff`) | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/rust-smoke/macos-14-arm64/smoke-stdout.txt` |
| Pre-rollback smoke stderr check | PASS (`0` bytes) | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/rust-smoke/macos-14-arm64/smoke-stderr.txt` |
| Pre-rollback smoke required-output verification (`15/15`) | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/rust-smoke/macos-14-arm64/smoke-output-verification.txt` |
| Deployment pointer before rollback (`current -> rust-current`) | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/deployment/deployment-current-before.txt` |
| Deployment pointer after rollback (`current -> fortran-stable`) | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/deployment/deployment-current-after.txt` |
| Post-rollback required-output verification (`15/15`) | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/deployment/rollback-output-verification.txt` |
| Focused rollback regression (`FX-WORKFLOW-XAS-001`) | PASS (`1/1` fixtures, `45/45` artifacts) | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/regression-report.json` |
| Focused rollback regression summary | PASS | `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/regression-summary.txt` |

## Artifact Verification Evidence

- Release binary digest (`sha256`):
  - Value: `b4ac27f0ec02849f1ffb4d8894c13b4a6d8505fbcb31eff039f2ac50725203d8`
  - File: `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/quality/feff10-rs.sha256`
- Release binary size:
  - Value: `3890336` bytes
  - File: `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/quality/feff10-rs.size-bytes`
- Smoke output artifact digests (`15` required files):
  - File: `artifacts/rollback-rehearsal/2026-02-19/20260219-140228/rust-smoke/macos-14-arm64/smoke-output.sha256`

## Platform Coverage

| Platform | Outcome | Notes |
| --- | --- | --- |
| macOS 14 arm64 (`aarch64-apple-darwin`) | PASS | Quality + parity gates, release build, rollback drill, and focused rollback regression all passed. |
| Ubuntu 22.04 x86_64 (`x86_64-unknown-linux-gnu`) | NOT RUN | Local rehearsal environment only; rerun the same command set on Linux staging/CI runner for cross-platform rollback sign-off. |

## Rollback Decision

- Local rehearsal decision: PASS for macOS rollback rehearsal scope.
- Cross-platform GA sign-off: pending Ubuntu rollback rehearsal with the same evidence format and command set.
