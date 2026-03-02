use feff10::stage::Stage;
use pyo3::prelude::*;

#[pyclass(name = "Stage", eq, eq_int, hash, frozen)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyStage {
    RDINP = 0,
    DMDW = 1,
    ATOMIC = 2,
    POT = 3,
    LDOS = 4,
    SCREEN = 5,
    CRPA = 6,
    OPCONSAT = 7,
    XSPH = 8,
    FMS = 9,
    MKGTR = 10,
    PATH = 11,
    GENFMT = 12,
    FF2X = 13,
    SFCONV = 14,
    COMPTON = 15,
    EELS = 16,
    RHORRP = 17,
}

impl PyStage {
    pub fn to_rust(self) -> Stage {
        match self {
            PyStage::RDINP => Stage::Rdinp,
            PyStage::DMDW => Stage::Dmdw,
            PyStage::ATOMIC => Stage::Atomic,
            PyStage::POT => Stage::Pot,
            PyStage::LDOS => Stage::Ldos,
            PyStage::SCREEN => Stage::Screen,
            PyStage::CRPA => Stage::Crpa,
            PyStage::OPCONSAT => Stage::Opconsat,
            PyStage::XSPH => Stage::Xsph,
            PyStage::FMS => Stage::Fms,
            PyStage::MKGTR => Stage::Mkgtr,
            PyStage::PATH => Stage::Path,
            PyStage::GENFMT => Stage::Genfmt,
            PyStage::FF2X => Stage::Ff2x,
            PyStage::SFCONV => Stage::Sfconv,
            PyStage::COMPTON => Stage::Compton,
            PyStage::EELS => Stage::Eels,
            PyStage::RHORRP => Stage::Rhorrp,
        }
    }

    pub fn from_rust(s: Stage) -> Self {
        match s {
            Stage::Rdinp => PyStage::RDINP,
            Stage::Dmdw => PyStage::DMDW,
            Stage::Atomic => PyStage::ATOMIC,
            Stage::Pot => PyStage::POT,
            Stage::Ldos => PyStage::LDOS,
            Stage::Screen => PyStage::SCREEN,
            Stage::Crpa => PyStage::CRPA,
            Stage::Opconsat => PyStage::OPCONSAT,
            Stage::Xsph => PyStage::XSPH,
            Stage::Fms => PyStage::FMS,
            Stage::Mkgtr => PyStage::MKGTR,
            Stage::Path => PyStage::PATH,
            Stage::Genfmt => PyStage::GENFMT,
            Stage::Ff2x => PyStage::FF2X,
            Stage::Sfconv => PyStage::SFCONV,
            Stage::Compton => PyStage::COMPTON,
            Stage::Eels => PyStage::EELS,
            Stage::Rhorrp => PyStage::RHORRP,
        }
    }
}

#[pymethods]
impl PyStage {
    /// All stages in pipeline order.
    #[staticmethod]
    fn all() -> Vec<PyStage> {
        Stage::all().iter().map(|s| PyStage::from_rust(*s)).collect()
    }

    /// Default pipeline order.
    #[staticmethod]
    fn default_pipeline() -> Vec<PyStage> {
        Stage::default_pipeline()
            .iter()
            .map(|s| PyStage::from_rust(*s))
            .collect()
    }

    /// Parse a stage name (case-insensitive).
    #[staticmethod]
    fn from_name(name: &str) -> PyResult<PyStage> {
        name.parse::<Stage>()
            .map(|s| PyStage::from_rust(s))
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))
    }

    /// Executable name for this stage.
    #[getter]
    fn executable_name(&self) -> &str {
        self.to_rust().executable_name()
    }

    /// CONTROL flag index (0-5) for this stage.
    #[getter]
    fn control_index(&self) -> usize {
        self.to_rust().control_index()
    }

    fn __repr__(&self) -> String {
        format!(
            "Stage.{}",
            self.to_rust().executable_name().to_uppercase()
        )
    }
}
