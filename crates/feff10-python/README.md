# feff10-rs

Python bindings for [FEFF10](https://github.com/times-software/feff10), a real-space multiple-scattering code for ab initio calculations of X-ray absorption spectra (EXAFS, XANES) and related properties.

Built with [PyO3](https://pyo3.rs) and [maturin](https://www.maturin.rs) for native performance with a Pythonic API.

## Installation

```sh
pip install feff10-rs
```

**No compiler needed** — prebuilt wheels are available for:

| Platform | Architecture | Python |
|----------|-------------|--------|
| Linux | x86_64, aarch64 | 3.9 – 3.14+ |
| macOS | Intel, Apple Silicon | 3.9 – 3.14+ |
| Windows | x86_64 | 3.9 – 3.14+ |

## Quick Start

```python
import feff10

# One-liner: parse, validate, and run
result = feff10.run("feff.inp", "./work")
print(f"Done in {result.total_duration_secs:.1f}s")

# Parse and compare output
xmu = feff10.XmuDat.from_file("./work/xmu.dat")
reference = feff10.XmuDat.from_file("reference_xmu.dat")
rsq = xmu.r_squared(reference, col_x=0, col_y=3)
print(f"R-squared = {rsq*100:.4f}%")
```

You can also pass raw feff.inp text or a `FeffInput` object:

```python
# From raw text
result = feff10.run(open("feff.inp").read(), "./work")

# From a FeffInput object
inp = feff10.FeffInput.from_file("feff.inp")
inp.s02 = 0.9
result = feff10.run(inp, "./work")
```

## Working with Input Files

### Parsing

```python
# From file
inp = feff10.FeffInput.from_file("feff.inp")

# From string
inp = feff10.FeffInput.parse(content)

# Strict mode — raises FeffParseError on malformed input
inp = feff10.FeffInput.from_file_strict("feff.inp")
```

### Inspecting

```python
inp.edge           # "K", "L3", etc.
inp.s02            # amplitude reduction factor
inp.num_atoms      # number of atoms
inp.num_potentials # number of unique potentials
inp.control        # CONTROL flags (6-element list)
inp.other_cards    # other cards (EXAFS, RPATH, etc.)

for pot in inp.potentials:
    print(f"ipot={pot.ipot}, Z={pot.z}, tag={pot.tag}")

for atom in inp.atoms:
    print(f"({atom.x}, {atom.y}, {atom.z}) ipot={atom.ipot}")
```

### Creating from Scratch

```python
inp = feff10.FeffInput(
    title=["Cu K-edge EXAFS"],
    edge="K",
    s02=1.0,
    potentials=[
        feff10.Potential(ipot=0, z=29, tag="Cu"),
        feff10.Potential(ipot=1, z=29, tag="Cu"),
    ],
    atoms=[
        feff10.Atom(x=0.0, y=0.0, z=0.0, ipot=0, tag="Cu"),
        feff10.Atom(x=0.0, y=1.805, z=1.805, ipot=1, tag="Cu"),
    ],
    other_cards=["EXAFS 20.0", "RPATH 5.5"],
)
```

### Modifying and Writing

```python
inp.edge = "L3"
inp.s02 = 0.85
inp.control = [1, 1, 1, 1, 0, 0]
inp.write_to_file("modified.inp")
```

## Validation

```python
# Validate without running (raises FeffConfigError if invalid)
feff10.validate("feff.inp")

# Or validate a FeffInput object
inp = feff10.FeffInput.from_file("feff.inp")
inp.validate()
```

Checks: absorber potential (ipot=0) exists, atoms reference valid potentials, no duplicate ipot values, atomic numbers in range, and more.

## Running Calculations

### Simple (Recommended)

```python
# From file path — validates input automatically
result = feff10.run("feff.inp", "./work")

# From FeffInput object
result = feff10.run(inp, "./work")
```

### Full Control

```python
config = feff10.FeffConfig("./work", inp)
result = feff10.FeffPipeline(config).run()

for sr in result.stages:
    print(f"{sr.stage.executable_name}: {sr.duration_secs:.3f}s")
print(f"Total: {result.total_duration_secs:.3f}s")
```

### Running Specific Stages

```python
config = feff10.FeffConfig(
    "./work", inp,
    stages=[feff10.Stage.RDINP, feff10.Stage.POT, feff10.Stage.XSPH],
)
```

### Progress Callbacks

```python
def on_progress(stage, progress):
    if progress.kind == "starting":
        print(f"  Running {stage.executable_name}...", end="", flush=True)
    else:
        print(f" done ({progress.duration_secs:.2f}s)")

result = feff10.FeffPipeline(config).run_with_progress(on_progress)
```

### Pipeline Stages

FEFF10 has 18 stages, each a separate computational step:

```python
for stage in feff10.Stage.all():
    print(f"{stage.executable_name} (control index {stage.control_index})")
```

## Parsing Output

### Reading xmu.dat

```python
xmu = feff10.XmuDat.from_file("./work/xmu.dat")

print(xmu.ncols)    # number of columns
print(xmu.nrows)    # number of data points
print(xmu.header)   # comment lines from file header
print(xmu)          # shows first 5 rows
```

### Accessing Columns

```python
energy = xmu.column(0)  # or xmu[0]
mu = xmu.column(3)      # or xmu[3]
last = xmu[-1]           # negative indexing

for col in xmu:          # iterate over columns
    print(f"{len(col)} points")
```

### Comparing Spectra

```python
calculated = feff10.XmuDat.from_file("./work/xmu.dat")
reference = feff10.XmuDat.from_file("reference_xmu.dat")

rsq = calculated.r_squared(reference, col_x=0, col_y=3)
print(f"R-squared = {rsq*100:.4f}%")  # lower is better
```

### Pandas Integration

```python
pip install 'feff10-rs[pandas]'
```

```python
df = xmu.to_dataframe()
print(df.describe())
```

## Error Handling

```python
try:
    result = feff10.FeffPipeline(config).run()
except feff10.FeffPipelineError as e:
    print(f"Pipeline failed: {e}")
except feff10.FeffConfigError as e:
    print(f"Configuration error: {e}")
except feff10.FeffParseError as e:
    print(f"Parse error: {e}")
except feff10.FeffIOError as e:
    print(f"I/O error: {e}")
```

Exception hierarchy:

- `FeffError` — base exception
  - `FeffIOError` — file I/O errors
  - `FeffParseError` — input/output parsing errors
  - `FeffPipelineError` — pipeline execution errors
  - `FeffConfigError` — configuration validation errors

## GIL Behavior

Both `run()` and `run_with_progress()` release the Python GIL during FEFF stage execution, allowing other Python threads to run concurrently.

## API Summary

| Function / Class | Description |
|-----------------|-------------|
| `run(input, work_dir)` | Run a FEFF calculation (accepts file path, raw text, or FeffInput) |
| `validate(input)` | Validate input without running (accepts file path, raw text, or FeffInput) |
| `FeffInput` | Parse, create, modify, and write feff.inp files |
| `Potential` | Scattering potential (ipot, Z, tag, l_scmt, l_fms, stoich) |
| `Atom` | Atomic position (x, y, z, ipot, tag, distance) |
| `FeffConfig` | Calculation configuration (work_dir, input, stages, timeout) |
| `Stage` | Pipeline stage enum (18 stages: RDINP through RHORRP) |
| `FeffPipeline` | Execute FEFF calculations with optional progress callbacks |
| `PipelineResult` | Execution results (stages, work_dir, total_duration_secs) |
| `StageResult` | Per-stage timing (stage, duration_secs) |
| `StageProgress` | Progress callback data (kind, duration_secs) |
| `XmuDat` | Parse xmu.dat output with column access and pandas integration |

## License

MIT or Apache-2.0. The FEFF10 Fortran source is under its own [license](https://github.com/times-software/feff10/blob/main/LICENSE).

## Links

- [Source Code](https://github.com/Ameyanagi/feff10-rs)
- [FEFF10 (upstream Fortran)](https://github.com/times-software/feff10)
- [Rust crate (feff10-sys)](https://crates.io/crates/feff10-sys)
