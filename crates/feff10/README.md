# feff10

Rust wrapper for [FEFF10](https://github.com/times-software/feff10) X-ray absorption spectroscopy calculations.

Provides a safe Rust interface to the FEFF10 Fortran pipeline, including input parsing, output parsing, and pipeline orchestration.

## Usage

```rust
use feff10::input::FeffInput;
use feff10::config::FeffConfigBuilder;
use feff10::pipeline::FeffPipeline;

let input = FeffInput::from_file("feff.inp")?;
let config = FeffConfigBuilder::new()
    .work_dir(".")
    .input(input)
    .build()?;

let result = FeffPipeline::new(config).run()?;
for stage in &result.stages {
    println!("{}: {:.2}s", stage.stage, stage.duration.as_secs_f64());
}
```

## Features

- `prebuilt` - Skip Fortran compilation and use a prebuilt library

See the [main project README](https://github.com/Ameyanagi/feff10-rs) for full documentation.
