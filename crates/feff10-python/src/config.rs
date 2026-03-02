use std::path::PathBuf;
use std::time::Duration;

use feff10::config::FeffConfigBuilder;
use pyo3::prelude::*;

use crate::error::to_pyerr;
use crate::input::PyFeffInput;
use crate::stage::PyStage;

#[pyclass(name = "FeffConfig")]
pub struct PyFeffConfig {
    pub(crate) inner: feff10::config::FeffConfig,
}

#[pymethods]
impl PyFeffConfig {
    /// Create a new FEFF configuration.
    ///
    /// Args:
    ///     work_dir: Working directory for the calculation.
    ///     input: Parsed FeffInput object.
    ///     stages: Optional list of stages to run (default: derived from CONTROL card).
    ///     stage_timeout: Optional timeout in seconds per stage (Unix only).
    #[new]
    #[pyo3(signature = (work_dir, input, stages=None, stage_timeout=None))]
    fn new(
        work_dir: &str,
        input: &PyFeffInput,
        stages: Option<Vec<PyStage>>,
        stage_timeout: Option<f64>,
    ) -> PyResult<Self> {
        let mut builder = FeffConfigBuilder::new()
            .work_dir(PathBuf::from(work_dir))
            .input(input.inner.clone());

        if let Some(stages) = stages {
            builder = builder.stages(stages.iter().map(|s| s.to_rust()).collect());
        }
        if let Some(timeout) = stage_timeout {
            builder = builder.stage_timeout(Duration::from_secs_f64(timeout));
        }

        builder
            .build()
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Create from an input file path.
    #[staticmethod]
    #[pyo3(signature = (work_dir, input_file, stages=None, stage_timeout=None))]
    fn from_file(
        work_dir: &str,
        input_file: &str,
        stages: Option<Vec<PyStage>>,
        stage_timeout: Option<f64>,
    ) -> PyResult<Self> {
        let mut builder = FeffConfigBuilder::new()
            .work_dir(PathBuf::from(work_dir))
            .input_file(input_file)
            .map_err(to_pyerr)?;

        if let Some(stages) = stages {
            builder = builder.stages(stages.iter().map(|s| s.to_rust()).collect());
        }
        if let Some(timeout) = stage_timeout {
            builder = builder.stage_timeout(Duration::from_secs_f64(timeout));
        }

        builder
            .build()
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    #[getter]
    fn work_dir(&self) -> PyResult<String> {
        self.inner
            .work_dir
            .to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err("work_dir contains invalid UTF-8")
            })
    }

    #[getter]
    fn stages(&self) -> Vec<PyStage> {
        self.inner
            .stages
            .iter()
            .map(|s| PyStage::from_rust(*s))
            .collect()
    }

    #[getter]
    fn stage_timeout(&self) -> Option<f64> {
        self.inner.stage_timeout.map(|d| d.as_secs_f64())
    }

    fn __repr__(&self) -> String {
        format!(
            "FeffConfig(work_dir='{}', stages={})",
            self.inner.work_dir.display(),
            self.inner.stages.len()
        )
    }
}
