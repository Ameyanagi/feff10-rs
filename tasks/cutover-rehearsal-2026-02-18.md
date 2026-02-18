# Rust Cutover Rehearsal Report - 2026-02-18

## Rehearsal Metadata

- Executed at: 2026-02-18 12:42:42 JST
- Workspace: `/Users/ryuichi/dev/feff10-rs`
- Evidence root: `artifacts/cutover-rehearsal/2026-02-18/20260218-124242`
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
  --report artifacts/regression/oracle-report.json \
  --capture-runner "${capture_runner}" \
  --capture-allow-missing-entry-files \
  > artifacts/regression/oracle-summary.txt \
  2> artifacts/regression/oracle-stderr.txt
```

Note: this rehearsal used the same command arguments and wrote parity outputs to the dated evidence root to preserve an immutable run snapshot.

## Pass/Fail Outcomes

| Step | Status | Evidence artifact |
| --- | --- | --- |
| Fetch FEFF10 reference fixtures (`scripts/fortran/ensure-feff10-reference.sh`) | PASS | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/quality/ensure-feff10-reference.log` |
| Typecheck (`cargo check --locked`) | PASS | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/quality/cargo-check.log` |
| Tests (`cargo test --locked`) | PASS | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/quality/cargo-test.log` |
| Lint (`cargo clippy --locked --all-targets -- -D warnings`) | PASS | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/quality/cargo-clippy.log` |
| Formatting (`cargo fmt --all -- --check`) | PASS | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/quality/cargo-fmt-check.log` |
| Release build (`cargo build --locked --release`) | PASS | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/quality/cargo-build-release.log` |
| Oracle parity (`cargo run --locked -- oracle ...`) | PASS (`passed=true`, `failed_fixture_count=0`, `mismatch_fixture_count=0`) | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/parity/oracle-report.json` |
| Oracle summary | PASS | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/parity/oracle-summary.txt` |
| Oracle stderr capture | PASS (non-empty due expected capture warnings; no parity failure) | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/parity/oracle-stderr.txt` |
| Oracle mismatch detail extraction | PASS | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/parity/oracle-diff.txt` |
| macOS smoke run (`target/release/feff10-rs feff`) | PASS | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/smoke/macos-14-arm64/smoke-stdout.txt` |
| Smoke stderr check | PASS (`0` bytes) | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/smoke/macos-14-arm64/smoke-stderr.txt` |
| Required smoke output verification (`15/15` present) | PASS | `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/smoke/macos-14-arm64/smoke-output-verification.txt` |

## Artifact Verification Evidence

- Release binary digest (`sha256`):
  - Value: `b40d311115190ba84023b1cd5a8cc2ef657986697c4d0a33a4feeaf21b737074`
  - File: `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/quality/feff10-rs.sha256`
- Release binary size:
  - Value: `3743712` bytes
  - File: `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/quality/feff10-rs.size-bytes`
- Smoke output artifact digests (`15` required files):
  - File: `artifacts/cutover-rehearsal/2026-02-18/20260218-124242/smoke/macos-14-arm64/smoke-output.sha256`

## Platform Coverage

| Platform | Outcome | Notes |
| --- | --- | --- |
| macOS 14 arm64 (`aarch64-apple-darwin`) | PASS | Build, smoke, and artifact verification completed with full evidence set. |
| Ubuntu 22.04 x86_64 (`x86_64-unknown-linux-gnu`) | NOT RUN | Local rehearsal environment only; run the same command set on the parity/quality CI runner or Linux staging host for cross-platform cutover sign-off. |

## Cutover Decision

- Local rehearsal decision: PASS for macOS rehearsal scope.
- Cross-platform GA sign-off: pending Ubuntu rehearsal execution with the same command set and evidence format.
