pub mod comparator;
pub mod regression;

use crate::domain::{PipelineArtifact, PipelineRequest, PipelineResult};

pub trait PipelineExecutor {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>>;
}

#[cfg(test)]
mod tests {
    use super::PipelineExecutor;
    use crate::domain::{
        FeffError, FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest,
    };

    struct FailingExecutor;

    impl PipelineExecutor for FailingExecutor {
        fn execute(
            &self,
            _request: &PipelineRequest,
        ) -> crate::domain::PipelineResult<Vec<PipelineArtifact>> {
            Err(FeffError::computation(
                "RUN.PIPELINE",
                "module execution failed",
            ))
        }
    }

    #[test]
    fn pipeline_executor_uses_shared_error_types() {
        let request = PipelineRequest::new("FX-001", PipelineModule::Rdinp, "feff.inp", "out");
        let error = FailingExecutor
            .execute(&request)
            .expect_err("executor should fail");
        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.exit_code(), 4);
        assert_eq!(error.placeholder(), "RUN.PIPELINE");
    }
}
