# FEFF10-RS Migration Status (Post-Parity Closure)

## Current Status

The FEFF10-to-Rust migration closeout for the v1 scope is complete.

Release-blocking evidence is tracked in `tasks/oracle-16-module-migration-summary.md`.
Latest workflow-equivalent oracle gate snapshot (`2026-02-18 12:20:06 JST`):

| Metric | Value |
| --- | --- |
| `passed` | `true` |
| `fixture_count` | `17` |
| `passed_fixture_count` | `17` |
| `failed_fixture_count` | `0` |
| `mismatch_fixture_count` | `0` |
| `failed_artifact_count` | `0` |
| `mismatch_artifact_count` | `0` |

## Completed Migration Scope

- Rust true-compute execution is available for all 16 module commands: `rdinp`, `pot`, `xsph`, `path`, `fms`, `band`, `ldos`, `rixs`, `crpa`, `compton`, `ff2x`, `dmdw`, `screen`, `sfconv`, `eels`, `fullspectrum`.
- Oracle parity and regression infrastructure are wired as release-blocking validation flows (`cargo run -- oracle`, `cargo run -- regression`).
- CI quality gates remain strict and blocking via `.github/workflows/rust-quality-gates.yml`:
  - `cargo check --locked`
  - `cargo test --locked`
  - `cargo clippy --locked --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
- CI parity gates remain strict and blocking via `.github/workflows/rust-parity-gates.yml`:
  - locked oracle command execution
  - hard job failure on nonzero oracle exit code
  - failure-artifact uploads for diagnostics

## Remaining Non-Blocking Limits

- MPI runtime parity is still intentionally deferred for v1. `feffmpi <nprocs>` validates input, emits `WARNING: [RUN.MPI_DEFERRED]` when `nprocs > 1`, and executes the serial compatibility chain.

No open module-port migration gaps remain in the v1 closeout scope.

## Reproduction Commands (Release-Blocking Contracts)

### Quality Gate Command Set

```bash
scripts/fortran/ensure-feff10-reference.sh
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
```

### Parity Gate Command Set

```bash
scripts/fortran/ensure-feff10-reference.sh
mkdir -p artifacts/regression
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

Expected parity report artifacts:
- `artifacts/regression/oracle-report.json`
- `artifacts/regression/oracle-diff.txt`
- `artifacts/regression/oracle-summary.txt`
- `artifacts/regression/oracle-stderr.txt`

## Historical Note

This file previously tracked pre-closeout migration gaps and placeholder-implementation risks.
That historical planning context is intentionally superseded by the passing parity summary and is available in git history when needed.
