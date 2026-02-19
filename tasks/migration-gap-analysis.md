# FEFF10-RS Full-Module Migration Status

## Current Status

The FEFF10-to-Rust migration closeout is complete for the full-module scope.

Release-blocking parity evidence is tracked in `tasks/oracle-16-module-migration-summary.md`.
Latest workflow-equivalent oracle gate snapshot (`2026-02-19 14:04:25 JST`):

| Metric | Value |
| --- | --- |
| `passed` | `true` |
| `fixture_count` | `21` |
| `passed_fixture_count` | `21` |
| `failed_fixture_count` | `0` |
| `mismatch_fixture_count` | `0` |
| `failed_artifact_count` | `0` |
| `mismatch_artifact_count` | `0` |

## Gap Closure Snapshot

Latest regenerated gap artifacts (`2026-02-19 13:59:11 JST`):
- `gap/fortran-to-rust-file-gap.csv`
- `gap/fortran-to-rust-gap-report.md`

| Metric | Value |
| --- | --- |
| `total_fortran_files` | `527` |
| `runtime_owned` | `272` |
| `migrated_to_rust` | `255` |
| `runtime_support_dependency` | `0` |
| `out_of_scope` | `0` |
| `remaining_gap` | `0` |

Interpretation:
- No unresolved migration-gap classes remain.
- Runtime-owned module sources and migrated support/compatibility coverage together account for all scanned Fortran files.

## Completed Migration Scope

- Rust true-compute execution remains available for all 16 module commands:
  - `rdinp`, `pot`, `xsph`, `path`, `fms`, `band`, `ldos`, `rixs`, `crpa`, `compton`, `ff2x`, `dmdw`, `screen`, `sfconv`, `eels`, `fullspectrum`.
- Deferred phase-2 support families are migrated and integrated via Rust-native support domains.
- Oracle parity and regression infrastructure remain release-blocking validation flows (`cargo run -- oracle`, `cargo run -- regression`).
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
