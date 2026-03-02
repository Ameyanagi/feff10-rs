# feff10

Python bindings for [FEFF10](https://github.com/times-software/feff10) X-ray absorption spectroscopy calculations.

Built with [PyO3](https://pyo3.rs) and [maturin](https://www.maturin.rs), wrapping the [feff10-rs](https://github.com/Ameyanagi/feff10-rs) Rust library.

## Usage

```python
import feff10

# Parse input, run calculation, compare output
inp = feff10.FeffInput.from_file("feff.inp")
config = feff10.FeffConfig("./work", inp)
result = feff10.FeffPipeline(config).run()

xmu = feff10.XmuDat.from_file("./work/xmu.dat")
ref = feff10.XmuDat.from_file("reference_xmu.dat")
print(f"R² = {xmu.r_squared(ref, col_x=0, col_y=3)*100:.4f}%")
```

## Install

```sh
git clone --recursive https://github.com/Ameyanagi/feff10-rs.git
cd feff10-rs/crates/feff10-python
uv venv && source .venv/bin/activate
uv pip install maturin && maturin develop
```

Requires a Fortran compiler (gfortran, ifx, or flang-new). See the [documentation](docs/) for details.

## License

MIT or Apache-2.0. The FEFF10 Fortran source is under its own [license](../../feff10/LICENSE).
