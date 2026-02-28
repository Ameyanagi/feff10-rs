use std::path::PathBuf;

use crate::error::Error;
use crate::input::FeffInput;
use crate::stage::Stage;

/// Configuration for a FEFF calculation.
#[derive(Debug, Clone)]
pub struct FeffConfig {
    /// Working directory for the calculation.
    pub work_dir: PathBuf,
    /// The feff.inp input.
    pub input: FeffInput,
    /// Which stages to run. If empty, derived from CONTROL card.
    pub stages: Vec<Stage>,
}

/// Builder for FeffConfig.
pub struct FeffConfigBuilder {
    work_dir: Option<PathBuf>,
    input: Option<FeffInput>,
    stages: Option<Vec<Stage>>,
}

impl FeffConfigBuilder {
    pub fn new() -> Self {
        Self {
            work_dir: None,
            input: None,
            stages: None,
        }
    }

    pub fn work_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.work_dir = Some(dir.into());
        self
    }

    pub fn input(mut self, input: FeffInput) -> Self {
        self.input = Some(input);
        self
    }

    pub fn input_file(mut self, path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        self.input = Some(FeffInput::from_file(path)?);
        Ok(self)
    }

    pub fn stages(mut self, stages: Vec<Stage>) -> Self {
        self.stages = Some(stages);
        self
    }

    pub fn build(self) -> Result<FeffConfig, Error> {
        let work_dir = self
            .work_dir
            .ok_or_else(|| Error::Config("work_dir is required".into()))?;
        let input = self
            .input
            .ok_or_else(|| Error::Config("input is required".into()))?;

        let stages = self.stages.unwrap_or_else(|| {
            // Derive from CONTROL card
            Stage::default_pipeline()
                .into_iter()
                .filter(|s| input.control[s.control_index()] != 0)
                .collect()
        });

        Ok(FeffConfig {
            work_dir,
            input,
            stages,
        })
    }
}

impl Default for FeffConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}
