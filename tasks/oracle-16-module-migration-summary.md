# Oracle Full-Module Migration Summary

## Latest Workflow-Equivalent Oracle Run

- Run timestamp: 2026-02-19 14:04:25 JST
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
- Diff summary artifact: `artifacts/regression/oracle-diff.txt`

## Gate Result Snapshot

| Metric | Value |
| --- | --- |
| `passed` | `true` |
| `fixture_count` | `21` |
| `passed_fixture_count` | `21` |
| `failed_fixture_count` | `0` |
| `mismatch_fixture_count` | `0` |
| `failed_artifact_count` | `0` |
| `mismatch_artifact_count` | `0` |
| `artifact_count` | `1162` |
| `passed_artifact_count` | `1162` |

## Fixture-Level Status (Latest Run)

| Fixture ID | Status | Artifact Pass Count |
| --- | --- | --- |
| `FX-RDINP-001` | PASS | `49/49` |
| `FX-POT-001` | PASS | `49/49` |
| `FX-PATH-001` | PASS | `49/49` |
| `FX-FMS-001` | PASS | `45/45` |
| `FX-XSPH-001` | PASS | `45/45` |
| `FX-BAND-001` | PASS | `61/61` |
| `FX-LDOS-001` | PASS | `97/97` |
| `FX-RIXS-001` | PASS | `5/5` |
| `FX-CRPA-001` | PASS | `39/39` |
| `FX-COMPTON-001` | PASS | `63/63` |
| `FX-DEBYE-001` | PASS | `41/41` |
| `FX-DMDW-001` | PASS | `66/66` |
| `FX-SCREEN-001` | PASS | `60/60` |
| `FX-SELF-001` | PASS | `60/60` |
| `FX-ATOM-SCF-001` | PASS | `42/42` |
| `FX-KSPACE-GENFMT-001` | PASS | `63/63` |
| `FX-EELSMDFF-001` | PASS | `76/76` |
| `FX-SELF-OPCONSAT-001` | PASS | `60/60` |
| `FX-EELS-001` | PASS | `71/71` |
| `FX-FULLSPECTRUM-001` | PASS | `76/76` |
| `FX-WORKFLOW-XAS-001` | PASS | `45/45` |

## Notes

- `artifacts/regression/oracle-stderr.txt` contains expected capture warnings for missing staged `REFERENCE/band.inp` entries in KSPACE fixtures when `--capture-allow-missing-entry-files` is enabled; this does not affect parity pass/fail.
- The full-module migration parity gate is currently green at `21/21` fixtures and `0` mismatched artifacts.
