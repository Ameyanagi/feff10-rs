# FEFF10 True-Compute Migration Contract Reference

This document is the single authoritative contract index for true-compute module rewrites.

All artifacts listed below are release-blocking references for implementation and validation stories. If runtime behavior, validation behavior, or module outputs diverge from these artifacts, the change is not release-ready until the contract is updated and re-approved.

## Release-Blocking Contract Artifacts

| Contract Area | Canonical Artifact | Release-Blocking Scope |
| --- | --- | --- |
| Compatibility matrix | `tasks/feff10-compatibility-matrix.md` | Defines per-module command surfaces, required cards/options, required input/output artifacts, output directory contract, and fixture IDs. |
| Fixture manifest | `tasks/golden-fixture-manifest.json` | Defines approved fixture inventory, per-fixture comparison mode, and pass/fail thresholds used by parity flows. |
| Numeric tolerance policy (D-3) | `tasks/numeric-tolerance-policy.json` | Defines comparator modes, category matching order, and tolerance thresholds for numeric parity decisions. |
| Fortran-to-Rust boundary map | `tasks/fortran-rust-boundary-map.md` | Defines module ownership boundaries, dependency order, and migration sequencing constraints. |
| Warning/error contract (D-4) | `tasks/migration-decision-log.md#d-4-warning-and-error-compatibility-contract` | Defines fatal exit-code mapping, stderr/stdout diagnostics format, and failure-category mapping. |

## Enforcement Rules

- Module rewrite stories must implement behavior consistent with every applicable contract artifact above.
- Runtime module commands must not bypass these contracts by replaying baseline outputs.
- Any story that intentionally changes contract behavior must update the affected artifact and this index in the same change set.
