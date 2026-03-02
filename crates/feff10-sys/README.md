# feff10-sys

Low-level FFI bindings and build system for the [FEFF10](https://github.com/times-software/feff10) Fortran library.

This crate compiles the FEFF10 Fortran source into a static library (`libfeff10.a`) and links it. When the Fortran source is not available (e.g. when installed from crates.io), it automatically downloads a prebuilt library from GitHub releases.

## Build modes

| Mode | When | How |
|------|------|-----|
| From source | Git checkout with submodule | Detects gfortran/ifx/flang and compiles |
| Prebuilt (auto) | `cargo install` from crates.io | Downloads `libfeff10.a` from GitHub releases |
| Prebuilt (explicit) | `--features prebuilt` | Same as auto, forced even when source is present |
| Custom library | `FEFF10_LIB_DIR=/path` | Uses your own `libfeff10.a` |

## Environment variables

| Variable | Description |
|----------|-------------|
| `FEFF_FC` | Fortran compiler path |
| `FEFF_FFLAGS` | Extra Fortran flags |
| `FEFF_BLAS` | BLAS/LAPACK linker flags |
| `FEFF10_LIB_DIR` | Path to prebuilt `libfeff10.a` |
| `FEFF10_LIB_SHA256` | Expected SHA256 of prebuilt library |

See the [main project README](https://github.com/Ameyanagi/feff10-rs) for full documentation.
