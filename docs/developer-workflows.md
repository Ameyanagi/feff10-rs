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
