# feff10-rs

Rust wrapper for [FEFF10](http://feffproject.org/), a real-space multiple-scattering code for ab initio calculations of X-ray absorption spectra (EXAFS, XANES), electronic structure, and related properties.

The 18 Fortran modules of FEFF10 are compiled into a single static library (`libfeff10.a`) and linked directly into the Rust binary via FFI — no external executables or runtime dependencies required.

## Features

- Single self-contained binary (~26-29 MB depending on compiler)
- All 18 FEFF10 stages linked statically (MKL, Fortran runtime included)
- Supports 3 Fortran compilers: **gfortran**, **ifx** (Intel), **flang-new** (LLVM)
- **Prebuilt binaries** — no Fortran compiler needed with `--features prebuilt`
- C library distribution (`libfeff10.a` + `feff10.h`) for use from other languages
- Built-in benchmarking and output comparison tools
- Progress bar and per-stage timing

## Quick Start (prebuilt)

No Fortran compiler required. The prebuilt `libfeff10.a` is automatically downloaded from GitHub releases:

```sh
cargo build --release --features prebuilt
```

Or provide your own prebuilt library:

```sh
FEFF10_LIB_DIR=./lib cargo build --release --features prebuilt
```

## Building from Source

Requires a Fortran compiler and a BLAS/LAPACK implementation (MKL preferred).

```sh
# Default (gfortran + auto-detected MKL or system BLAS, -march=native)
cargo build --release

# Intel Fortran (ifx) — needs oneAPI installed
FEFF_FC=/opt/intel/oneapi/compiler/latest/bin/ifx cargo build --release

# LLVM Flang
FEFF_FC=flang-new cargo build --release

# Portable binary for distribution (-march=x86-64-v3, AVX2+FMA, ~2013+ CPUs)
FEFF_PORTABLE=1 cargo build --release
```

The build system automatically detects MKL (from oneAPI) and falls back to OpenBLAS or system LAPACK. All dependencies are linked statically — the resulting binary has no special runtime requirements.

### Build environment variables

| Variable | Description |
|---|---|
| `FEFF_FC` | Fortran compiler path (default: auto-detect) |
| `FEFF_FFLAGS` | Override all Fortran flags |
| `FEFF_PORTABLE` | Use `-march=x86-64-v3` for distributable binaries |
| `FEFF_MARCH` | Explicit `-march=` value (e.g. `x86-64-v2`) |
| `FEFF_NO_NATIVE` | Disable `-march` entirely |
| `FEFF_LTO` | Enable link-time optimization |
| `FEFF10_LIB_DIR` | Path to prebuilt `libfeff10.a` (with `--features prebuilt`) |

## Usage

```sh
# Run a FEFF calculation
feff10-rs run path/to/feff.inp

# Run with explicit working directory
feff10-rs run path/to/feff.inp -w /tmp/work

# Run only specific stages
feff10-rs run path/to/feff.inp -s rdinp,pot,xsph

# Validate a feff.inp file
feff10-rs validate path/to/feff.inp

# Compare two xmu.dat files
feff10-rs compare file1.dat file2.dat

# Benchmark (3 iterations, JSON output)
feff10-rs bench path/to/feff.inp -n 3 -o results.json -l my-label
```

`validate` uses strict parsing and reports line-numbered errors for malformed
`CONTROL`, `PRINT`, `POTENTIALS`, and `ATOMS` records.

## C Library

Each release includes `libfeff10.a` (static library) and `feff10.h` (C header) for Linux, macOS, and Windows. These can be used to call FEFF10 from C, C++, Python (via ctypes/cffi), or any language with C FFI support.

```c
#include "feff10.h"

// Write feff.inp, chdir to working directory, then:
feff_rdinp();
feff_pot();
feff_xsph();
feff_fms();
feff_path();
feff_genfmt();
feff_ff2x();
```

Linker flags (platform-dependent):
- **Linux** (ifx+MKL): `-lfeff10 -lpthread -lm -ldl` (MKL and Intel runtime are merged into the `.a`)
- **macOS** (gfortran): `-lfeff10 -lgfortran -framework Accelerate`
- **Windows** (gfortran): `-lfeff10 -lgfortran -lpthread -lm`

## Compiler Benchmarks

Benchmarked on AMD Ryzen 9 7940HS (Zen4, 16 threads), Arch Linux, all using static MKL (sequential). Each test averaged over 3 iterations.

### Total Runtime

| Compiler | EXAFS Cu | XANES Cu | Binary Size |
|---|---:|---:|---:|
| ifx 2025.3 | **0.72 s** | **7.37 s** | 29 MB |
| gfortran 15.2 | 0.79 s | 9.69 s | 26 MB |
| flang-new 21.1 | 1.22 s | 15.98 s | 28 MB |

### XANES Cu Stage Breakdown (mean seconds)

| Stage | gfortran | ifx | flang-new |
|---|---:|---:|---:|
| pot | 5.903 | 4.416 | 9.354 |
| fms | 2.041 | 1.494 | 3.640 |
| screen | 1.210 | 0.929 | 2.218 |
| mkgtr | 0.247 | 0.191 | 0.347 |
| xsph | 0.172 | 0.224 | 0.302 |
| atomic | 0.072 | 0.070 | 0.064 |
| **Total** | **9.69** | **7.37** | **15.98** |

### Output Correctness

All three compilers produce identical XANES Cu xmu.dat output (0.000% deviation across all pairwise comparisons).

### Notes

- **ifx** is the fastest overall — 24% faster than gfortran, 54% faster than flang-new on XANES Cu. The advantage comes from `pot` and `fms` stages which are dominated by dense linear algebra (MKL matrix operations).
- **gfortran** produces the smallest binary and is the most portable. Good default choice.
- **flang-new** (LLVM Flang) works but is significantly slower, likely due to less mature optimization passes for Fortran-specific patterns.
- ifx requires the `-no-vec` flag as a workaround for an ICE in the VPlan vectorizer on `ff2chijas.f90` (ifx 2025.3).
- All compilers use `-fPIC` (required for Rust's PIE executables) and static MKL (sequential, no OpenMP — FEFF10 code is serial).

## Architecture

```
feff10-rs/
├── feff10/                    # FEFF10 Fortran source (git submodule)
├── crates/
│   ├── feff10-sys/            # Build system + FFI bindings
│   │   ├── build.rs           # Fortran compilation, driver patching, static linking
│   │   ├── include/feff10.h   # C header for library usage
│   │   └── src/lib.rs         # extern "C" declarations for 18 FEFF stages
│   ├── feff10/                # Safe Rust wrapper
│   │   ├── src/pipeline.rs    # Pipeline orchestration (fork per stage)
│   │   ├── src/stage.rs       # Stage enum + FFI dispatch
│   │   ├── src/input.rs       # feff.inp parser
│   │   └── src/output.rs      # xmu.dat reader + comparison
│   └── feff10-rs/            # CLI binary
└── Cargo.toml
```

### How It Works

1. **Build time** (`build.rs`): The Fortran `program` entry points are patched to `subroutine ... bind(C)` in a copy of the source. All objects are compiled and archived into `libfeff10.a`, merged with MKL and the Fortran runtime. With `--features prebuilt`, this step is skipped and a prebuilt library is used instead.

2. **Runtime**: Each FEFF stage runs in a `fork()`-ed child process to isolate Fortran module state (global allocatable arrays, I/O units). This matches the original FEFF behavior where each stage was a separate executable.

## License

The Rust wrapper code (`feff10-rs`) is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

The FEFF10 Fortran source code (in the `feff10/` submodule) is copyright (c) 2020 FEFF Project, University of Washington and SLAC National Accelerator Laboratory, and is distributed under its own [license](feff10/LICENSE). This software is based on or developed using FEFF10.0.

All licenses permit redistribution and use in source and binary forms, with or without modification, subject to their respective conditions. See [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE), and [feff10/LICENSE](feff10/LICENSE) for full terms.
