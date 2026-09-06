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

## Parser Behavior

- Uses FEFF-style 4-character keyword tokenization (same matching style as Fortran `itoken`).
- Accepts `include` / `load` directives in `feff.inp` files (`FeffInput::from_file`) with nested include support.
- Treats full-line comments using FEFF prefixes `;`, `*`, `%`, `#`.
- Rejects unknown keywords (Fortran-compatible strict behavior).

Programmatic helpers:

```rust
let input = feff10::FeffInput::from_file("feff.inp")?;
let resolved = input.resolve_defaults(); // explicit defaults model
let typed = input.typed_cards();         // typed per-card view
input.validate_fortran_rules()?;         // FEFF consistency.f90 card checks
```

Writers:
- `write_to()` preserves parsed card order when card stream metadata is available.
- `write_canonical()` writes normalized CONTROL/PRINT + sections layout.

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
let result = feff10::run("feff.inp", "./work")?;
let outputs = result.outputs()?;

let xmu = result.read_xmu()?;              // convenience
let paths = result.read_paths()?;          // structured paths.dat parser
let chi = outputs.read_chi()?;             // via discovered output registry

let reference = feff10::FeffTable::from_file("reference_xmu.dat")?;
let rsq = xmu.r_squared(&reference, 0, 3);
println!("R-squared = {:.4}%", rsq * 100.0);
println!("discovered outputs: {}", outputs.files().len());
println!("paths parsed: {}", paths.len());
println!("chi rows: {}", chi.nrows());
```

## Stage isolation and path output

Each FEFF stage needs a fresh process because Fortran module allocations persist
between calls. Rust applications should install the worker hook before application
initialization:

```rust,no_run
fn main() -> Result<(), feff10::Error> {
    feff10::worker::init();
    let result = feff10::run("feff.inp", "./work")?;
    println!("Outputs: {}", result.work_dir.display());
    Ok(())
}
```

`StageIsolation::Auto` forks on ordinary Unix hosts and uses workers on Windows
or in macOS hosts with an initialized `NSApplication`. A missing required worker
hook returns a configuration error before changing input files. Explicit `Worker`
also requires the hook. The CLI installs it automatically.

`InProcess` is only for a single stage in a fresh host process; it rejects
multi-stage pipelines and timeouts. Sequential calls can still retain allocations,
and a Fortran fatal error can terminate that host.

To request individual `feffNNNN.dat` files, use `PRINT 0 0 0 0 0 3` and include the
`ff2x` stage. `paths.dat` contains enumerated paths; amplitude filtering can produce
fewer individual path files. For the bundled Cu input with `RPATH 5.2`, the regression
test checks 14 files, finite amplitudes, and first-path geometry.

## Features

- `prebuilt` — Skip Fortran compilation and use a prebuilt library

See the [main project README](https://github.com/Ameyanagi/feff10-rs) for build instructions, benchmarks, and architecture details.
