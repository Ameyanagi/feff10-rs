# Developer Workflows

This document is the maintainer-facing workflow reference for building, validating, and running FEFF10 Rust parity checks.

## Prerequisites

- Rust stable toolchain (edition `2024`)
- `bash`
- `jq`
- `unzip`
- `sha256sum` or `shasum`

If tests fail on macOS with `ld: library not found for -liconv`, run Rust test/lint commands with:

```bash
CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER="$(xcrun -f clang)"
```

## Build And Quality Checks

Use the same commands as CI:

```bash
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Regression Workflow

Run fixture comparisons and emit a machine-readable report:

```bash
mkdir -p artifacts/regression

cargo run --locked -- regression \
  --manifest tasks/golden-fixture-manifest.json \
  --policy tasks/numeric-tolerance-policy.json \
  --baseline-root artifacts/fortran-baselines \
  --actual-root artifacts/fortran-baselines \
  --baseline-subdir baseline \
  --actual-subdir baseline \
  --report artifacts/regression/report.json \
  > artifacts/regression/regression-summary.txt \
  2> artifacts/regression/regression-stderr.txt
```

Render a concise diff summary from the JSON report:

```bash
jq -r '
  def status(v): if v then "PASS" else "FAIL" end;
  "Regression status: \(status(.passed))",
  "Failed fixtures: \(.failed_fixture_count)",
  "Failed artifacts: \(.failed_artifact_count)",
  "",
  "Failed artifact details:",
  (
    .fixtures[]
    | select(.passed | not)
    | "Fixture \(.fixture_id):",
      (
        .artifacts[]
        | select(.passed | not)
        | "  - \(.artifact_path): \(.reason // (if .comparison then (.comparison.mode + \" mismatch\") else \"comparison failed\" end))"
      )
  )
' artifacts/regression/report.json > artifacts/regression/regression-diff.txt
```

Baseline snapshots under `artifacts/fortran-baselines` are validation-only inputs for this workflow and for tests.
Do not use baseline snapshot files as runtime output sources for `feff`/module commands.

## Baseline Snapshot Regeneration

Refresh committed Fortran baselines and checksums:

```bash
scripts/fortran/generate-baseline-snapshots.sh \
  --manifest tasks/golden-fixture-manifest.json \
  --output-root artifacts/fortran-baselines
```

## Regression Pre-Compare Hooks

The regression command supports module pre-compare execution flags such as:

- `--run-rdinp`
- `--run-pot`
- `--run-xsph`
- `--run-path`
- `--run-fms`
- `--run-band`
- `--run-ldos`
- `--run-rixs`
- `--run-crpa`
- `--run-compton`
- `--run-debye`
- `--run-dmdw`
- `--run-screen`
- `--run-self`
- `--run-eels`
- `--run-fullspectrum`

Use these flags when you need Rust pipelines to materialize module artifacts into
`<actual-root>/<fixture-id>/<actual-subdir>` before comparison.
These hooks are part of validation-only parity flows, not production runtime execution paths.

## Oracle Dual-Run Workflow

Use the `oracle` command when you need one flow that:
1. captures Fortran oracle outputs for the manifest fixture set, and
2. runs Rust pre-compare hooks plus policy comparisons against those captures.

```bash
cargo run -- oracle \
  --manifest tasks/golden-fixture-manifest.json \
  --policy tasks/numeric-tolerance-policy.json \
  --oracle-root artifacts/fortran-oracle-capture \
  --oracle-subdir outputs \
  --actual-root artifacts/oracle-actual \
  --actual-subdir actual \
  --report artifacts/regression/oracle-report.json \
  --capture-runner "<fortran capture command>" \
  --run-rdinp
```

Capture mode is required and exclusive:
- `--capture-runner "<command>"`
- `--capture-bin-dir <path>`

Optional capture behavior:
- `--capture-allow-missing-entry-files` (records unresolved entry files in capture metadata and continues)

This command is validation-only and is intentionally isolated from runtime CLI paths.

### CI Oracle Parity Gate

`.github/workflows/rust-parity-gates.yml` runs oracle parity with:
- `--actual-root artifacts/fortran-baselines --actual-subdir baseline`
- `--capture-runner scripts/fortran/ci-oracle-capture-runner.sh`
- `--capture-allow-missing-entry-files`

The default CI runner script replays committed fixture baselines into each capture output
directory so the parity lane continuously validates oracle command plumbing and report artifacts
without requiring a local Fortran toolchain on the primary Rust quality lane.
