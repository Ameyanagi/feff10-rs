pub mod config;
pub mod error;
pub mod input;
pub mod output;
pub mod pipeline;
pub mod stage;

// Re-exports for convenience
pub use config::{FeffConfig, FeffConfigBuilder};
pub use error::Error;
pub use input::{Atom, FeffInput, Potential};
pub use output::XmuDat;
pub use pipeline::{FeffPipeline, PipelineResult, StageResult};
pub use stage::Stage;

use std::path::{Path, PathBuf};

/// Run a FEFF calculation from a feff.inp file.
///
/// Parses the input file, validates it, builds a default configuration
/// (all stages from CONTROL card), and runs the full pipeline.
///
/// # Example
///
/// ```no_run
/// let result = feff10::run("feff.inp", "./work")?;
/// for sr in &result.stages {
///     println!("{}: {:.3}s", sr.stage, sr.duration.as_secs_f64());
/// }
/// # Ok::<(), feff10::Error>(())
/// ```
pub fn run(
    input_path: impl AsRef<Path>,
    work_dir: impl Into<PathBuf>,
) -> Result<PipelineResult, Error> {
    let input = FeffInput::from_file(input_path)?;
    run_input(input, work_dir)
}

/// Run a FEFF calculation from a pre-built [`FeffInput`].
///
/// Validates the input, builds a default configuration, and runs the
/// full pipeline. Use this when you've constructed or modified input
/// programmatically.
///
/// # Example
///
/// ```no_run
/// let mut inp = feff10::FeffInput::from_file("feff.inp")?;
/// inp.s02 = Some(0.9);
/// let result = feff10::run_input(inp, "./work")?;
/// # Ok::<(), feff10::Error>(())
/// ```
pub fn run_input(input: FeffInput, work_dir: impl Into<PathBuf>) -> Result<PipelineResult, Error> {
    input.validate()?;
    let config = FeffConfigBuilder::new()
        .work_dir(work_dir.into())
        .input(input)
        .build()?;
    FeffPipeline::new(config).run()
}

/// Run a FEFF calculation from raw feff.inp text.
///
/// Parses the string as feff.inp content, validates it, and runs the
/// full pipeline.
///
/// # Example
///
/// ```no_run
/// let content = std::fs::read_to_string("feff.inp")?;
/// let result = feff10::run_str(&content, "./work")?;
/// # Ok::<(), feff10::Error>(())
/// ```
pub fn run_str(content: &str, work_dir: impl Into<PathBuf>) -> Result<PipelineResult, Error> {
    let input = FeffInput::parse(content)?;
    run_input(input, work_dir)
}

/// Validate a feff.inp file without running any calculations.
///
/// Parses the file and checks semantic correctness (potentials, atoms,
/// absorber site, etc.). Returns `Ok(())` if the input is valid.
///
/// # Example
///
/// ```no_run
/// feff10::validate("feff.inp")?;
/// println!("Input is valid");
/// # Ok::<(), feff10::Error>(())
/// ```
pub fn validate(input_path: impl AsRef<Path>) -> Result<(), Error> {
    let input = FeffInput::from_file(input_path)?;
    input.validate()
}

/// Validate raw feff.inp text without running any calculations.
///
/// # Example
///
/// ```no_run
/// let content = std::fs::read_to_string("feff.inp")?;
/// feff10::validate_str(&content)?;
/// # Ok::<(), feff10::Error>(())
/// ```
pub fn validate_str(content: &str) -> Result<(), Error> {
    let input = FeffInput::parse(content)?;
    input.validate()
}
