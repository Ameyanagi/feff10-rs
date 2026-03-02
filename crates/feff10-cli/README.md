# feff10-cli

Command-line interface for running [FEFF10](https://github.com/times-software/feff10) X-ray absorption spectroscopy calculations.

## Installation

```bash
cargo install feff10-cli
```

The binary is called `feff10-rs`.

## Quick start

```bash
# List bundled examples
feff10-rs examples

# Copy an example to the current directory
feff10-rs examples exafs-cu

# Run a calculation
feff10-rs run feff.inp

# Validate an input file
feff10-rs validate feff.inp

# Compare output spectra
feff10-rs compare xmu.dat reference.dat
```

## Features

- `prebuilt` - Skip Fortran compilation and use a prebuilt library (default for crates.io installs)

See the [main project README](https://github.com/Ameyanagi/feff10-rs) for full documentation.
