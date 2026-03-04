# feff10

Python bindings for [FEFF10](https://github.com/times-software/feff10), a real-space multiple-scattering code for ab initio calculations of X-ray absorption spectra (EXAFS, XANES) and related properties.

Built with [PyO3](https://pyo3.rs) and [maturin](https://www.maturin.rs) for native performance with a Pythonic API.

## Quick Start

```python
import feff10

# Parse a FEFF input file
inp = feff10.FeffInput.from_file("feff.inp")
print(f"{inp.edge} edge, {inp.num_atoms} atoms, {inp.num_potentials} potentials")

# Configure and run the calculation
config = feff10.FeffConfig("./work", inp)
pipeline = feff10.FeffPipeline(config)
result = pipeline.run()
print(f"Done in {result.total_duration_secs:.1f}s")

# Parse and compare output
xmu = feff10.FeffTable.from_file("./work/xmu.dat")
reference = feff10.FeffTable.from_file("reference_xmu.dat")
rsq = xmu.r_squared(reference, col_x=0, col_y=3)
print(f"R-squared = {rsq*100:.4f}%")
```

## Features

- **Input parsing** — read, modify, and write `feff.inp` files
- **Pipeline execution** — run all 18 FEFF10 stages with progress callbacks
- **Output parsing** — parse `xmu.dat` with column access, iteration, and pandas integration
- **Spectrum comparison** — R-squared metric for comparing calculated spectra
- **GIL release** — long-running calculations release the Python GIL
- **Type stubs** — full PEP 561 type annotations for IDE support

## Next Steps

- [Installation](installation.md) — build and install the package
- [User Guide](guide/input.md) — learn how to use each component
- [API Reference](api/index.md) — complete class and method reference
