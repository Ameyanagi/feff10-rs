use std::sync::Mutex;

use feff10::pipeline::{FeffPipeline, StageProgress};
use pyo3::prelude::*;

use crate::config::PyFeffConfig;
use crate::error::to_pyerr;
use crate::output::{PyFeffOutputs, PyFeffTable, PyPathsDat, discover_outputs_from_work_dir};
use crate::stage::PyStage;

pub(crate) fn python_pipeline(
    py: Python<'_>,
    config: feff10::config::FeffConfig,
) -> PyResult<FeffPipeline> {
    let executable: String = py.import("sys")?.getattr("executable")?.extract()?;
    if executable.is_empty() {
        return Err(to_pyerr(feff10::Error::Config(
            "Python worker requires a nonempty sys.executable".into(),
        )));
    }
    Ok(FeffPipeline::new(config).with_worker_command(executable, ["-m", "feff10._worker"]))
}

#[pyclass(name = "StageProgress", from_py_object)]
#[derive(Clone)]
pub struct PyStageProgress {
    #[pyo3(get)]
    kind: String,
    #[pyo3(get)]
    duration_secs: Option<f64>,
}

#[pymethods]
impl PyStageProgress {
    fn __repr__(&self) -> String {
        match self.duration_secs {
            Some(d) => format!("StageProgress(kind='finished', duration_secs={d:.3})"),
            None => "StageProgress(kind='starting')".to_string(),
        }
    }

    fn __str__(&self) -> String {
        match self.duration_secs {
            Some(d) => format!("finished ({d:.3}s)"),
            None => "starting".to_string(),
        }
    }
}

#[pyclass(name = "StageResult", from_py_object)]
#[derive(Clone)]
pub struct PyStageResult {
    #[pyo3(get)]
    stage: PyStage,
    #[pyo3(get)]
    duration_secs: f64,
}

#[pymethods]
impl PyStageResult {
    fn __repr__(&self) -> String {
        format!(
            "StageResult(stage=Stage.{}, duration_secs={:.3})",
            self.stage.to_rust().executable_name().to_uppercase(),
            self.duration_secs
        )
    }

    fn __str__(&self) -> String {
        format!(
            "{}: {:.3}s",
            self.stage.to_rust().executable_name(),
            self.duration_secs
        )
    }
}

#[pyclass(name = "PipelineResult")]
pub struct PyPipelineResult {
    #[pyo3(get)]
    stages: Vec<PyStageResult>,
    #[pyo3(get)]
    work_dir: String,
}

#[pymethods]
impl PyPipelineResult {
    /// Total duration of all stages.
    #[getter]
    fn total_duration_secs(&self) -> f64 {
        self.stages.iter().map(|s| s.duration_secs).sum()
    }

    /// Discover FEFF output files in the pipeline work directory.
    fn outputs(&self) -> PyResult<PyFeffOutputs> {
        discover_outputs_from_work_dir(&self.work_dir)
    }

    /// Read xmu.dat from the pipeline work directory.
    #[pyo3(signature = (strict=false))]
    fn read_xmu(&self, strict: bool) -> PyResult<PyFeffTable> {
        let path = std::path::Path::new(&self.work_dir).join("xmu.dat");
        let table = if strict {
            feff10::output::FeffTable::from_file_strict(&path)
        } else {
            feff10::output::FeffTable::from_file(&path)
        };
        table.map(|inner| PyFeffTable { inner }).map_err(to_pyerr)
    }

    /// Read chi.dat from the pipeline work directory.
    #[pyo3(signature = (strict=false))]
    fn read_chi(&self, strict: bool) -> PyResult<PyFeffTable> {
        let path = std::path::Path::new(&self.work_dir).join("chi.dat");
        let table = if strict {
            feff10::output::FeffTable::from_file_strict(&path)
        } else {
            feff10::output::FeffTable::from_file(&path)
        };
        table.map(|inner| PyFeffTable { inner }).map_err(to_pyerr)
    }

    /// Read eels.dat from the pipeline work directory.
    #[pyo3(signature = (strict=false))]
    fn read_eels(&self, strict: bool) -> PyResult<PyFeffTable> {
        let path = std::path::Path::new(&self.work_dir).join("eels.dat");
        let table = if strict {
            feff10::output::FeffTable::from_file_strict(&path)
        } else {
            feff10::output::FeffTable::from_file(&path)
        };
        table.map(|inner| PyFeffTable { inner }).map_err(to_pyerr)
    }

    /// Read ldosNN.dat from the pipeline work directory.
    #[pyo3(signature = (index, strict=false))]
    fn read_ldos(&self, index: u32, strict: bool) -> PyResult<PyFeffTable> {
        let path = std::path::Path::new(&self.work_dir).join(format!("ldos{index:02}.dat"));
        let table = if strict {
            feff10::output::FeffTable::from_file_strict(&path)
        } else {
            feff10::output::FeffTable::from_file(&path)
        };
        table.map(|inner| PyFeffTable { inner }).map_err(to_pyerr)
    }

    /// Read paths.dat from the pipeline work directory.
    fn read_paths(&self) -> PyResult<PyPathsDat> {
        let path = std::path::Path::new(&self.work_dir).join("paths.dat");
        feff10::output::PathsDat::from_file(&path)
            .map(|inner| PyPathsDat { inner })
            .map_err(to_pyerr)
    }

    fn __repr__(&self) -> String {
        format!(
            "PipelineResult(stages={}, work_dir='{}')",
            self.stages.len(),
            self.work_dir
        )
    }
}

#[pyclass(name = "FeffPipeline")]
pub struct PyFeffPipeline {
    pipeline: FeffPipeline,
}

#[pymethods]
impl PyFeffPipeline {
    #[new]
    fn new(py: Python<'_>, config: &PyFeffConfig) -> PyResult<Self> {
        Ok(Self {
            pipeline: python_pipeline(py, config.inner.clone())?,
        })
    }

    /// Run the full pipeline without progress reporting.
    ///
    /// Releases the GIL during computation so other Python threads can run.
    fn run(&self, py: Python<'_>) -> PyResult<PyPipelineResult> {
        let result = py.detach(|| self.pipeline.run());
        convert_result(result)
    }

    /// Run with a progress callback: callback(stage: Stage, progress: StageProgress).
    ///
    /// The GIL is released during FEFF worker execution and only
    /// re-acquired to invoke the callback between stages. If the callback
    /// raises an exception, it is captured and re-raised after the pipeline
    /// completes (or the current stage finishes).
    #[pyo3(signature = (callback))]
    fn run_with_progress(&self, py: Python<'_>, callback: Py<PyAny>) -> PyResult<PyPipelineResult> {
        // Store the first callback error so we can re-raise it after the pipeline.
        let callback_err: Mutex<Option<PyErr>> = Mutex::new(None);

        let result = py.detach(|| {
            self.pipeline.run_with_progress(|stage, progress| {
                Python::attach(|py| {
                    // Don't invoke callback if a previous call already failed.
                    let Ok(guard) = callback_err.lock() else {
                        return;
                    };
                    if guard.is_some() {
                        return;
                    }
                    drop(guard);

                    let py_stage = PyStage::from_rust(stage);
                    let py_progress = match progress {
                        StageProgress::Starting => PyStageProgress {
                            kind: "starting".to_string(),
                            duration_secs: None,
                        },
                        StageProgress::Finished { duration } => PyStageProgress {
                            kind: "finished".to_string(),
                            duration_secs: Some(duration.as_secs_f64()),
                        },
                    };
                    if let Err(e) = callback.call1(py, (py_stage, py_progress)) {
                        if let Ok(mut guard) = callback_err.lock() {
                            *guard = Some(e);
                        }
                    }
                })
            })
        });

        // Propagate callback error (takes priority over pipeline result).
        match callback_err.into_inner() {
            Ok(Some(err)) => return Err(err),
            Ok(None) => {}
            Err(_poisoned) => {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "internal error: callback mutex poisoned",
                ));
            }
        }

        convert_result(result)
    }
}

pub(crate) fn convert_result(
    result: Result<feff10::pipeline::PipelineResult, feff10::error::Error>,
) -> PyResult<PyPipelineResult> {
    let r = result.map_err(to_pyerr)?;
    Ok(PyPipelineResult {
        stages: r
            .stages
            .into_iter()
            .map(|sr| PyStageResult {
                stage: PyStage::from_rust(sr.stage),
                duration_secs: sr.duration.as_secs_f64(),
            })
            .collect(),
        work_dir: r
            .work_dir
            .to_str()
            .ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err("work_dir contains invalid UTF-8")
            })?
            .to_string(),
    })
}
