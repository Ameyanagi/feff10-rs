# feff10

Rust wrapper for [FEFF10](https://github.com/times-software/feff10), a real-space multiple-scattering code for ab initio calculations of X-ray absorption spectra (EXAFS, XANES) and related properties.

## Quick Start

```rust
// One-liner: parse, validate, and run
let result = feff10::run("feff.inp", "./work")?;
for sr in &result.stages {
    println!("{}: {:.3}s", sr.stage, sr.duration.as_secs_f64());
}
```

Also accepts raw feff.inp text or a `FeffInput` object:

```rust
// From raw text
let content = std::fs::read_to_string("feff.inp")?;
let result = feff10::run_str(&content, "./work")?;

// From a FeffInput object
let mut inp = feff10::FeffInput::from_file("feff.inp")?;
inp.s02 = Some(0.9);
let result = feff10::run_input(inp, "./work")?;
```

## Validation

```rust
// Validate a file without running
feff10::validate("feff.inp")?;

// Validate a FeffInput object
let inp = feff10::FeffInput::from_file("feff.inp")?;
inp.validate()?;
```

Checks: absorber potential (ipot=0) exists, atoms reference valid potentials, no duplicate ipot values, atomic numbers in range (1-103), and more.

## Full Control

For advanced use cases (custom stages, timeouts, progress callbacks), use the builder API:

```rust
use feff10::{FeffInput, FeffConfigBuilder, FeffPipeline, Stage};
use std::time::Duration;

let input = FeffInput::from_file("feff.inp")?;
let config = FeffConfigBuilder::new()
    .work_dir("./work")
    .input(input)
    .stages(vec![Stage::Rdinp, Stage::Pot, Stage::Xsph])
    .stage_timeout(Duration::from_secs(120))
    .build()?;

let result = FeffPipeline::new(config).run_with_progress(|stage, progress| {
    println!("{stage}: {progress:?}");
})?;
```

## Parsing Output

```rust
let xmu = feff10::XmuDat::from_file("./work/xmu.dat")?;
let reference = feff10::XmuDat::from_file("reference_xmu.dat")?;
let rsq = xmu.r_squared(&reference, 0, 3);
println!("R-squared = {:.4}%", rsq * 100.0);
```

## Features

- `prebuilt` — Skip Fortran compilation and use a prebuilt library

See the [main project README](https://github.com/Ameyanagi/feff10-rs) for build instructions, benchmarks, and architecture details.
