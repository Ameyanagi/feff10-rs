# User And Operator Guide

This guide documents runtime behavior, compatibility guarantees, and known limits for FEFF10 Rust v1.

## Command Surface

Top-level commands:

- `feff10-rs feff`
- `feff10-rs feffmpi <nprocs>`
- `feff10-rs regression [options]`
- `feff10-rs <module>`

Module commands:

- `rdinp`
- `pot`
- `xsph`
- `path`
- `fms`
- `band`
- `ldos`
- `rixs`
- `crpa`
- `compton`
- `ff2x` (DEBYE)
- `dmdw`
- `screen`
- `sfconv` (SELF)
- `eels`
- `fullspectrum`

Help:

- `feff10-rs --help`
- `feff10-rs regression --help`
- `feff10-rs <module> --help`

## Compatibility Guarantees

### CLI And Output Contracts

- The canonical compatibility contract is maintained in `tasks/feff10-compatibility-matrix.md`.
- `feff` and module commands operate in the current working directory and preserve expected artifact naming per the compatibility matrix.
- Regression output is stable: human summary to stdout plus JSON report at `--report` (default `artifacts/regression/report.json`).

### Diagnostics And Exit Behavior

Fatal errors are emitted with both lines below:

- `ERROR: [TOKEN] ...`
- `FATAL EXIT CODE: <n>`

Exit code mapping:

- `0`: success
- `2`: input validation failure
- `3`: I/O or system failure
- `4`: computation/parity failure
- `5`: internal failure

## Runtime Notes

- `feffmpi <nprocs>` is available for compatibility, but MPI parity is deferred in v1.
- When `nprocs > 1`, the command emits `WARNING: [RUN.MPI_DEFERRED]` and executes the serial compatibility chain.
- Module selection and workflow resolution are driven by `tasks/golden-fixture-manifest.json`.
- Runtime commands (`feff`, `feffmpi`, and module commands) must not read from `artifacts/fortran-baselines` to generate outputs.
- Baseline snapshots are reserved for validation flows (`regression`) and test tooling only.

## Known Limits

- MPI runtime parity is out of scope for v1 and is not release-blocking.
- GA platform targets are currently:
  - macOS 14 arm64 (`aarch64-apple-darwin`)
  - Ubuntu 22.04 x86_64 (`x86_64-unknown-linux-gnu`)
- Non-GA targets (`aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`) are preview-only.
- Regression parity behavior is bounded to approved fixtures and baseline snapshots under `artifacts/fortran-baselines`.
