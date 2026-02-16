pub mod comparator;
pub mod regression;

use crate::domain::{PipelineArtifact, PipelineRequest};
use std::error::Error;
use std::fmt::{Display, Formatter};

pub trait PipelineExecutor {
    fn execute(
        &self,
        request: &PipelineRequest,
    ) -> Result<Vec<PipelineArtifact>, PipelineExecutionError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineExecutionError {
    message: String,
}

impl PipelineExecutionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for PipelineExecutionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for PipelineExecutionError {}

#[cfg(test)]
mod tests {
    use super::PipelineExecutionError;

    #[test]
    fn pipeline_execution_error_exposes_message() {
        let error = PipelineExecutionError::new("failed to run pipeline");
        assert_eq!(error.to_string(), "failed to run pipeline");
    }
}
