use pyo3::prelude::*;

mod config;
mod error;
mod input;
mod output;
mod pipeline;
mod stage;

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
    m.add_class::<output::PyXmuDat>()?;

    Ok(())
}
