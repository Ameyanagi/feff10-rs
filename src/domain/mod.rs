use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionMode {
    Serial,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        Self::Serial
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineModule {
    Rdinp,
    Pot,
    Path,
    Fms,
    Xsph,
    Band,
    Ldos,
    Rixs,
    Crpa,
    Compton,
    Debye,
    Dmdw,
    Screen,
    SelfEnergy,
    Eels,
    FullSpectrum,
}

impl PipelineModule {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rdinp => "RDINP",
            Self::Pot => "POT",
            Self::Path => "PATH",
            Self::Fms => "FMS",
            Self::Xsph => "XSPH",
            Self::Band => "BAND",
            Self::Ldos => "LDOS",
            Self::Rixs => "RIXS",
            Self::Crpa => "CRPA",
            Self::Compton => "COMPTON",
            Self::Debye => "DEBYE",
            Self::Dmdw => "DMDW",
            Self::Screen => "SCREEN",
            Self::SelfEnergy => "SELF",
            Self::Eels => "EELS",
            Self::FullSpectrum => "FULLSPECTRUM",
        }
    }
}

impl Display for PipelineModule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineRequest {
    pub fixture_id: String,
    pub module: PipelineModule,
    pub execution_mode: ExecutionMode,
    pub input_path: PathBuf,
    pub output_dir: PathBuf,
}

impl PipelineRequest {
    pub fn new(
        fixture_id: impl Into<String>,
        module: PipelineModule,
        input_path: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            fixture_id: fixture_id.into(),
            module,
            execution_mode: ExecutionMode::Serial,
            input_path: input_path.into(),
            output_dir: output_dir.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineArtifact {
    pub relative_path: PathBuf,
}

impl PipelineArtifact {
    pub fn new(relative_path: impl Into<PathBuf>) -> Self {
        Self {
            relative_path: relative_path.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputDeck {
    pub cards: Vec<InputCard>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputCard {
    pub keyword: String,
    pub values: Vec<String>,
    pub source_line: usize,
}

#[cfg(test)]
mod tests {
    use super::{ExecutionMode, PipelineModule, PipelineRequest};

    #[test]
    fn pipeline_request_defaults_to_serial_mode() {
        let request = PipelineRequest::new("FX-001", PipelineModule::Rdinp, "feff.inp", "out");
        assert_eq!(request.execution_mode, ExecutionMode::Serial);
        assert_eq!(request.module.to_string(), "RDINP");
    }
}
