use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult};

pub trait ModuleExecutor {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>>;
}

pub trait RuntimeModuleExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>>;
}

pub trait ValidationModuleExecutor {
    fn execute_validation(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>>;
}

impl<T> ValidationModuleExecutor for T
where
    T: ModuleExecutor,
{
    fn execute_validation(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        self.execute(request)
    }
}

#[cfg(test)]
mod tests {
    use super::{ModuleExecutor, ValidationModuleExecutor};
    use crate::domain::{
        ComputeArtifact, ComputeModule, ComputeRequest, FeffError, FeffErrorCategory,
    };

    struct FailingExecutor;

    impl ModuleExecutor for FailingExecutor {
        fn execute(
            &self,
            _request: &ComputeRequest,
        ) -> crate::domain::ComputeResult<Vec<ComputeArtifact>> {
            Err(FeffError::computation(
                "RUN.MODULE",
                "module execution failed",
            ))
        }
    }

    #[test]
    fn module_executor_uses_shared_error_types() {
        let request = ComputeRequest::new("FX-001", ComputeModule::Rdinp, "feff.inp", "out");
        let error = FailingExecutor
            .execute(&request)
            .expect_err("executor should fail");
        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.exit_code(), 4);
        assert_eq!(error.placeholder(), "RUN.MODULE");
    }

    #[test]
    fn validation_executor_trait_adapts_module_executor_types() {
        let request = ComputeRequest::new("FX-001", ComputeModule::Rdinp, "feff.inp", "out");
        let error = FailingExecutor
            .execute_validation(&request)
            .expect_err("validation adapter should preserve errors");
        assert_eq!(error.placeholder(), "RUN.MODULE");
    }
}
