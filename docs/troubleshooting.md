# Troubleshooting

This guide maps common FEFF10 Rust migration/runtime failures to concrete actions.

## Common Failures And Actions

| Symptom | Likely cause | Action |
| --- | --- | --- |
| `ERROR: [INPUT.CLI_WORKSPACE] failed to locate workspace root...` | Command was run outside a checkout that contains both `tasks/golden-fixture-manifest.json` and `artifacts/fortran-baselines/`. | Run from repository root (or a subdirectory inside it), and ensure baseline snapshots exist. |
| `ERROR: [IO.CLI_MANIFEST_READ] ...` | CLI manifest file is missing or unreadable. | Verify `tasks/golden-fixture-manifest.json` exists and file permissions allow reading. |
| `ERROR: [INPUT.CLI_MANIFEST_PARSE] ...` | Manifest JSON is invalid. | Validate and repair JSON syntax: `jq empty tasks/golden-fixture-manifest.json`. |
| `ERROR: [IO.REGRESSION_MANIFEST] ...` | Regression `--manifest` path is wrong. | Pass an existing manifest path (normally `tasks/golden-fixture-manifest.json`). |
| `ERROR: [RUN.<MODULE>_INPUT_MISMATCH] ...` during regression hooks | Staged module inputs do not match approved baseline inputs for that fixture. | Compare staged files under `<actual-root>/<fixture>/<actual-subdir>` against baseline files under `<baseline-root>/<fixture>/<baseline-subdir>` and resolve drift before rerunning. |
| Regression command exits `1` with fixture failures | Comparator found artifact mismatches. | Inspect `artifacts/regression/report.json` and render a diff summary with the jq command in `docs/developer-workflows.md`. |
| `WARNING: [RUN.MPI_DEFERRED] ...` from `feffmpi` | MPI parity is intentionally deferred for v1. | This warning is expected; serial compatibility execution continues. |
| `ld: library not found for -liconv` on macOS tests/lints | Default linker environment is missing required pathing for this host setup. | Run with `CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=\"$(xcrun -f clang)\"` for `cargo test`/`cargo clippy`. |

## Exit Code Quick Reference

- `0`: success
- `2`: input validation issue
- `3`: I/O or filesystem issue
- `4`: computation/parity issue
- `5`: internal failure

When troubleshooting fatal exits, use both the placeholder token in `ERROR: [TOKEN] ...` and the numeric `FATAL EXIT CODE: <n>` to locate the failing contract quickly.
