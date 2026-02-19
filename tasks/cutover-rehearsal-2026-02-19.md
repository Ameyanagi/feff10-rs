# Rust Cutover Rehearsal Report - 2026-02-19

## Rehearsal Metadata

- Executed at: 2026-02-19 14:00:49 JST
- Workspace: `/Users/ryuichi/dev/feff10-rs`
- Evidence root: `artifacts/cutover-rehearsal/2026-02-19/20260219-140049`
- Operator platform: macOS 14 arm64 (`aarch64-apple-darwin`)
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
  --report artifacts/cutover-rehearsal/2026-02-19/20260219-140049/parity/oracle-report.json \
  --capture-runner "${capture_runner}" \
  --capture-allow-missing-entry-files \
  > artifacts/cutover-rehearsal/2026-02-19/20260219-140049/parity/oracle-summary.txt \
  2> artifacts/cutover-rehearsal/2026-02-19/20260219-140049/parity/oracle-stderr.txt
```

## Pass/Fail Outcomes

| Step | Status | Evidence artifact |
| --- | --- | --- |
| Fetch FEFF10 reference fixtures (`scripts/fortran/ensure-feff10-reference.sh`) | PASS | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/quality/ensure-feff10-reference.log` |
| Typecheck (`cargo check --locked`) | PASS | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/quality/cargo-check.log` |
| Tests (`cargo test --locked`) | PASS | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/quality/cargo-test.log` |
| Lint (`cargo clippy --locked --all-targets -- -D warnings`) | PASS | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/quality/cargo-clippy.log` |
| Formatting (`cargo fmt --all -- --check`) | PASS | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/quality/cargo-fmt-check.log` |
| Release build (`cargo build --locked --release`) | PASS | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/quality/cargo-build-release.log` |
| Oracle parity (`cargo run --locked -- oracle ...`) | PASS (`passed=true`, `fixture_count=21`, `failed_fixture_count=0`, `mismatch_artifact_count=0`) | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/parity/oracle-report.json` |
| Oracle summary | PASS | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/parity/oracle-summary.txt` |
| Oracle stderr capture | PASS (non-empty due expected staged-entry warnings; no parity failure) | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/parity/oracle-stderr.txt` |
| Oracle mismatch detail extraction | PASS | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/parity/oracle-diff.txt` |
| macOS smoke run (`target/release/feff10-rs feff`) | PASS | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/smoke/macos-14-arm64/smoke-stdout.txt` |
| Smoke stderr check | PASS (`0` bytes) | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/smoke/macos-14-arm64/smoke-stderr.txt` |
| Required smoke output verification (`15/15` present) | PASS | `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/smoke/macos-14-arm64/smoke-output-verification.txt` |

## Artifact Verification Evidence

- Release binary digest (`sha256`):
  - Value: `b4ac27f0ec02849f1ffb4d8894c13b4a6d8505fbcb31eff039f2ac50725203d8`
  - File: `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/quality/feff10-rs.sha256`
- Release binary size:
  - Value: `3890336` bytes
  - File: `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/quality/feff10-rs.size-bytes`
- Smoke output artifact digests (`15` required files):
  - File: `artifacts/cutover-rehearsal/2026-02-19/20260219-140049/smoke/macos-14-arm64/smoke-output.sha256`

## Platform Coverage

| Platform | Outcome | Notes |
| --- | --- | --- |
| macOS 14 arm64 (`aarch64-apple-darwin`) | PASS | Build, parity, smoke, and artifact verification completed with full evidence set. |
| Ubuntu 22.04 x86_64 (`x86_64-unknown-linux-gnu`) | NOT RUN | Local rehearsal environment only; run the same command set on Linux staging/CI runner for cross-platform cutover sign-off. |

## Cutover Decision

- Local rehearsal decision: PASS for macOS rehearsal scope.
- Cross-platform GA sign-off: pending Ubuntu rehearsal execution with the same command set and evidence format.
