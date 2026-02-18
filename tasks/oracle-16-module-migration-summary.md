# Oracle 16-Module Migration Summary

## Latest Workflow-Equivalent Oracle Run

- Run timestamp: 2026-02-18 12:20:06 JST
- Environment: local workspace (`/Users/ryuichi/dev/feff10-rs`)
- Command path: `.github/workflows/rust-parity-gates.yml` parity step equivalent
- Command:

```bash
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

- Exit code: `0`
- Report artifact: `artifacts/regression/oracle-report.json`
- Summary artifact: `artifacts/regression/oracle-summary.txt`
- Stderr artifact: `artifacts/regression/oracle-stderr.txt`

## Gate Result Snapshot

| Metric | Value |
| --- | --- |
| passed | `true` |
| fixture_count | `17` |
| passed_fixture_count | `17` |
| failed_fixture_count | `0` |
| mismatch_fixture_count | `0` |
| failed_artifact_count | `0` |
| mismatch_artifact_count | `0` |

Note: the current workflow manifest (`tasks/golden-fixture-manifest.json`) uses canonical fixture IDs (for example `FX-SELF-001`) rather than `*-ORACLE-*` IDs. The workflow-equivalent oracle gate above is the active release-blocking parity source of truth.

## Per-Module Status (Latest Run)

| Module(s) | Fixture | Status | Failed Artifacts | Mismatched Artifacts |
| --- | --- | --- | --- | --- |
| RDINP | `FX-RDINP-001` | PASS | `0` | `0` |
| POT | `FX-POT-001` | PASS | `0` | `0` |
| PATH | `FX-PATH-001` | PASS | `0` | `0` |
| FMS | `FX-FMS-001` | PASS | `0` | `0` |
| XSPH | `FX-XSPH-001` | PASS | `0` | `0` |
| BAND | `FX-BAND-001` | PASS | `0` | `0` |
| LDOS | `FX-LDOS-001` | PASS | `0` | `0` |
| RIXS | `FX-RIXS-001` | PASS | `0` | `0` |
| CRPA | `FX-CRPA-001` | PASS | `0` | `0` |
| COMPTON | `FX-COMPTON-001` | PASS | `0` | `0` |
| DEBYE | `FX-DEBYE-001` | PASS | `0` | `0` |
| DMDW | `FX-DMDW-001` | PASS | `0` | `0` |
| SCREEN | `FX-SCREEN-001` | PASS | `0` | `0` |
| SELF | `FX-SELF-001` | PASS | `0` | `0` |
| EELS | `FX-EELS-001` | PASS | `0` | `0` |
| FULLSPECTRUM | `FX-FULLSPECTRUM-001` | PASS | `0` | `0` |
| RDINP, POT, XSPH, FMS, PATH (workflow integration) | `FX-WORKFLOW-XAS-001` | PASS | `0` | `0` |
