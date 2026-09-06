use feff10::input::FeffInput;
use pyo3::prelude::*;

mod config;
mod error;
mod input;
mod output;
mod pipeline;
mod stage;

use error::to_pyerr;
use input::PyFeffInput;

/// Resolve a Python input argument to a Rust FeffInput.
///
/// Accepts:
///   - `FeffInput` object → use directly
///   - `str` with newlines → parse as raw feff.inp content
///   - `str` without newlines → treat as file path
fn resolve_input(input: &Bound<'_, PyAny>) -> PyResult<FeffInput> {
    if let Ok(inp) = input.extract::<PyFeffInput>() {
        return Ok(inp.inner.clone());
    }
    if let Ok(s) = input.extract::<String>() {
        if s.contains('\n') {
            return FeffInput::parse(&s).map_err(to_pyerr);
        } else {
            return FeffInput::from_file(&s).map_err(to_pyerr);
        }
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "input must be a str (file path or feff.inp content) or FeffInput object",
    ))
}

/// Run a FEFF calculation.
///
/// Args:
///     input: Path to feff.inp file (str), raw feff.inp content (str with
///         newlines), or a FeffInput object.
///     work_dir: Working directory for intermediate and output files.
///
/// Returns:
///     PipelineResult with per-stage timings and the work directory.
///
/// Raises:
///     FeffParseError: If the input cannot be parsed.
///     FeffConfigError: If the input fails validation.
///     FeffPipelineError: If a FEFF stage fails during execution.
#[pyfunction]
#[pyo3(signature = (input, work_dir))]
fn run(
    py: Python<'_>,
    input: &Bound<'_, PyAny>,
    work_dir: &str,
) -> PyResult<pipeline::PyPipelineResult> {
    let feff_input = resolve_input(input)?;
    feff_input.validate().map_err(to_pyerr)?;
    let config = feff10::FeffConfigBuilder::new()
        .input(feff_input)
        .work_dir(work_dir)
        .build()
        .map_err(to_pyerr)?;
    let pipeline = pipeline::python_pipeline(py, config)?;
    let result = py.detach(|| pipeline.run());
    pipeline::convert_result(result)
}

/// Validate a FEFF input without running any calculations.
///
/// Args:
///     input: Path to feff.inp file (str), raw feff.inp content (str with
///         newlines), or a FeffInput object.
///
/// Raises:
///     FeffParseError: If the input cannot be parsed.
///     FeffConfigError: If validation finds errors.
#[pyfunction]
#[pyo3(signature = (input,))]
fn validate(input: &Bound<'_, PyAny>) -> PyResult<()> {
    let feff_input = resolve_input(input)?;
    feff_input.validate().map_err(to_pyerr)
}

/// Private entry point used only by `python -m feff10._worker`.
#[pyfunction]
fn _worker_init() {
    feff10::worker::init();
}

#[pymodule]
fn _feff10(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register exception types
    error::register(m)?;

    // Input types
    m.add_class::<input::PyPotential>()?;
    m.add_class::<input::PyAtom>()?;
    m.add_class::<input::PyFeffInput>()?;

    // Configuration
    m.add_class::<config::PyFeffConfig>()?;

    // Stage enum
    m.add_class::<stage::PyStage>()?;

    // Pipeline
    m.add_class::<pipeline::PyFeffPipeline>()?;
    m.add_class::<pipeline::PyPipelineResult>()?;
    m.add_class::<pipeline::PyStageResult>()?;
    m.add_class::<pipeline::PyStageProgress>()?;

    // Output
    m.add_class::<output::PyFeffTable>()?;
    m.add_class::<output::PyPathLeg>()?;
    m.add_class::<output::PyPathEntry>()?;
    m.add_class::<output::PyPathsDat>()?;
    m.add_class::<output::PyOutputFileInfo>()?;
    m.add_class::<output::PyFeffOutputs>()?;

    // Convenience functions
    m.add_function(wrap_pyfunction!(run, m)?)?;
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(_worker_init, m)?)?;

    Ok(())
}
