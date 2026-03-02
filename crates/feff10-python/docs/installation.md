# Installation

## Prerequisites

- **Python** 3.9 or later
- **Rust** toolchain (1.75+) — install via [rustup](https://rustup.rs)
- **Fortran compiler** — gfortran, ifx (Intel), or flang-new (LLVM)
- **Make** — required for building the Fortran source

## Install from Source

Clone the repository with submodules and build with maturin:

```sh
git clone --recursive https://github.com/Ameyanagi/feff10-rs.git
cd feff10-rs/crates/feff10-python

# Create a virtual environment
uv venv
source .venv/bin/activate  # or `.venv\Scripts\activate` on Windows

# Install in development mode
uv pip install maturin
maturin develop
```

## Verify Installation

```python
import feff10
print(feff10.__version__)
print(feff10.Stage.all())
```

## Optional Dependencies

Install extras for additional functionality:

```sh
# pandas integration (XmuDat.to_dataframe())
uv pip install "feff10[pandas]"

# Development (tests)
uv pip install "feff10[dev]"

# Documentation
uv pip install "feff10[docs]"
```

## Environment Variables

The Fortran build can be configured with environment variables:

| Variable | Description |
|---|---|
| `FEFF_FC` | Fortran compiler path (default: auto-detect gfortran/ifx/flang) |
| `FEFF_FFLAGS` | Override all Fortran compiler flags |
| `FEFF_PORTABLE` | Use `-march=x86-64-v3` for portable binaries |

## Using a Prebuilt Library

Skip Fortran compilation entirely by providing a prebuilt `libfeff10.a`:

```sh
FEFF10_LIB_DIR=/path/to/lib maturin develop --features prebuilt
```
