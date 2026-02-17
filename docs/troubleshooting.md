# Troubleshooting

This guide maps common FEFF10 Rust migration/runtime failures to concrete actions.

## Common Failures And Actions

| Symptom | Likely cause | Action |
| --- | --- | --- |
| `ERROR: [INPUT.CLI_WORKSPACE] failed to locate workspace root...` | A workspace-required command (`feff`, `feffmpi`, or `oracle`) was run outside a checkout containing `tasks/golden-fixture-manifest.json`. | Run from repository root (or a subdirectory inside it). For module-only runtime commands, workspace discovery is optional. |
| `ERROR: [IO.CLI_MANIFEST_READ] ...` | CLI manifest file is missing or unreadable. | Verify `tasks/golden-fixture-manifest.json` exists and file permissions allow reading. |
| `ERROR: [INPUT.CLI_MANIFEST_PARSE] ...` | Manifest JSON is invalid. | Validate and repair JSON syntax: `jq empty tasks/golden-fixture-manifest.json`. |
| `ERROR: [IO.REGRESSION_MANIFEST] ...` | Regression `--manifest` path is wrong. | Pass an existing manifest path (normally `tasks/golden-fixture-manifest.json`). |
| `Use either '--capture-runner' or '--capture-bin-dir', not both.` | Oracle command was invoked with both capture modes. | Choose exactly one capture mode and rerun `feff10-rs oracle ...`. |
| `Missing required oracle capture mode ('--capture-runner' or '--capture-bin-dir').` | Oracle command was invoked without a capture mode. | Add one required option: `--capture-runner "<cmd>"` or `--capture-bin-dir <path>`. |
| `ERROR: [IO.ORACLE_CAPTURE_SCRIPT] ...` | `scripts/fortran/capture-baselines.sh` is missing from the workspace. | Restore the script and run from repository workspace root. |
| `ERROR: [IO.ORACLE_CAPTURE_EXEC] ...` | Oracle capture script could not be launched (bad path/permissions/environment). | Verify execute permissions and runner/bin-dir command correctness, then rerun. |
| `ERROR: [RUN.ORACLE_CAPTURE] ...` | Oracle capture script ran but returned non-zero status. | Check capture command prerequisites (Fortran binaries/runner environment), inspect capture logs, and rerun after fixing the failing fixture/capture setup. |
| `ERROR: [RUN.<MODULE>_INPUT_MISMATCH] ...` during regression hooks | Staged module inputs do not match approved baseline inputs for that fixture. | Compare staged files under `<actual-root>/<fixture>/<actual-subdir>` against baseline files under `<baseline-root>/<fixture>/<baseline-subdir>` and resolve drift before rerunning. |
| Regression or oracle command exits `1` with fixture failures | Comparator found artifact mismatches (command completed and report was written). | Inspect the report JSON (`--report`) and render a diff summary with the jq command in `docs/developer-workflows.md`. |
| `WARNING: [RUN.MPI_DEFERRED] ...` from `feffmpi` | MPI parity is intentionally deferred for v1. | This warning is expected; serial compatibility execution continues. |
| `ld: library not found for -liconv` on macOS tests/lints | Default linker environment is missing required pathing for this host setup. | Run with `CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=\"$(xcrun -f clang)\"` for `cargo test`/`cargo clippy`. |

## Exit Code Quick Reference

- `0`: success
- `2`: input validation issue
- `3`: I/O or filesystem issue
- `4`: computation/parity issue
- `5`: internal failure

When troubleshooting fatal exits, use both the placeholder token in `ERROR: [TOKEN] ...` and the numeric `FATAL EXIT CODE: <n>` to locate the failing contract quickly.
