use std::path::PathBuf;
use std::time::Duration;

use crate::error::Error;
use crate::input::FeffInput;
use crate::stage::Stage;

/// How each Fortran stage is isolated from the host process.
///
/// FEFF stages are compiled into this library; per-stage isolation matches
/// the original FEFF behavior of separate executables. On Unix the default
/// is to `fork()` a child per stage — but a forked child of a fork-unsafe
/// host (such as a macOS process with an initialized NSApplication) may
/// abort before the stage runs. Windows and fork-unsafe Unix hosts require
/// a worker process and the [`crate::worker::init`] hook in `main()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StageIsolation {
    /// Fork per stage on Unix unless the host is fork-unsafe. Use workers
    /// on Windows and in fork-unsafe hosts; return an error if the worker
    /// hook is missing.
    #[default]
    Auto,
    /// Always fork per stage (returns an error on Windows).
    Fork,
    /// Re-exec the host executable as a single-stage worker process
    /// (fork+exec — safe in GUI hosts). Requires the host to call
    /// [`crate::worker::init`] at the top of `main()`.
    Worker,
    /// Run each stage on a dedicated big-stack thread in this process.
    /// WARNING: FEFF's Fortran modules keep allocated state between stages,
    /// so multi-stage pipelines can fail (e.g. "allocate already allocated
    /// variable"). Multiple stages and `stage_timeout` are rejected. Even
    /// single-stage calls can leave state behind; use a fresh process for
    /// each call. A Fortran `stop` terminates the host process.
    InProcess,
}

/// Configuration for a FEFF calculation.
#[derive(Debug, Clone)]
pub struct FeffConfig {
    /// Working directory for the calculation.
    pub work_dir: PathBuf,
    /// The feff.inp input.
    pub input: FeffInput,
    /// Which stages to run. If empty, derived from CONTROL card.
    pub stages: Vec<Stage>,
    /// Maximum time per stage before killing it (Fork and Worker isolation).
    pub stage_timeout: Option<Duration>,
    /// How stages are isolated from the host process.
    pub stage_isolation: StageIsolation,
}

/// Builder for FeffConfig.
pub struct FeffConfigBuilder {
    work_dir: Option<PathBuf>,
    input: Option<FeffInput>,
    stages: Option<Vec<Stage>>,
    stage_timeout: Option<Duration>,
    stage_isolation: StageIsolation,
}

impl FeffConfigBuilder {
    pub fn new() -> Self {
        Self {
            work_dir: None,
            input: None,
            stages: None,
            stage_timeout: None,
            stage_isolation: StageIsolation::default(),
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

    pub fn stage_timeout(mut self, timeout: Duration) -> Self {
        self.stage_timeout = Some(timeout);
        self
    }

    pub fn stage_isolation(mut self, isolation: StageIsolation) -> Self {
        self.stage_isolation = isolation;
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
            stage_timeout: self.stage_timeout,
            stage_isolation: self.stage_isolation,
        })
    }
}

impl Default for FeffConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_input() -> FeffInput {
        FeffInput::parse(
            "\
TITLE test
CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
",
        )
        .unwrap()
    }

    #[test]
    fn builder_requires_work_dir() {
        let err = FeffConfigBuilder::new()
            .input(minimal_input())
            .build()
            .unwrap_err();
        match err {
            Error::Config(msg) => assert!(msg.contains("work_dir")),
            _ => panic!("expected Config error"),
        }
    }

    #[test]
    fn builder_requires_input() {
        let err = FeffConfigBuilder::new()
            .work_dir("/tmp")
            .build()
            .unwrap_err();
        match err {
            Error::Config(msg) => assert!(msg.contains("input")),
            _ => panic!("expected Config error"),
        }
    }

    #[test]
    fn builder_derives_stages_from_control() {
        let config = FeffConfigBuilder::new()
            .work_dir("/tmp")
            .input(minimal_input())
            .build()
            .unwrap();
        // CONTROL 1 1 1 1 1 1 means all stages enabled
        assert!(!config.stages.is_empty());
        assert!(config.stages.contains(&Stage::Rdinp));
        assert!(config.stages.contains(&Stage::Pot));
        assert!(config.stages.contains(&Stage::Ff2x));
    }

    #[test]
    fn builder_derives_stages_skips_disabled_control() {
        let mut inp = minimal_input();
        // Disable control group 1 (pot, atomic, etc.)
        inp.control = [1, 0, 1, 1, 1, 1];
        let config = FeffConfigBuilder::new()
            .work_dir("/tmp")
            .input(inp)
            .build()
            .unwrap();
        assert!(config.stages.contains(&Stage::Rdinp)); // control[0] = 1
        assert!(!config.stages.contains(&Stage::Pot)); // control[1] = 0
        assert!(!config.stages.contains(&Stage::Atomic)); // control[1] = 0
    }

    #[test]
    fn builder_with_explicit_stages() {
        let config = FeffConfigBuilder::new()
            .work_dir("/tmp")
            .input(minimal_input())
            .stages(vec![Stage::Rdinp, Stage::Pot])
            .build()
            .unwrap();
        assert_eq!(config.stages, vec![Stage::Rdinp, Stage::Pot]);
    }

    #[test]
    fn builder_with_timeout() {
        let config = FeffConfigBuilder::new()
            .work_dir("/tmp")
            .input(minimal_input())
            .stage_timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        assert_eq!(config.stage_timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn builder_default_no_timeout() {
        let config = FeffConfigBuilder::new()
            .work_dir("/tmp")
            .input(minimal_input())
            .build()
            .unwrap();
        assert_eq!(config.stage_timeout, None);
        assert_eq!(config.stage_isolation, StageIsolation::Auto);
    }

    #[test]
    fn builder_sets_stage_isolation() {
        let config = FeffConfigBuilder::new()
            .work_dir("/tmp")
            .input(minimal_input())
            .stage_isolation(StageIsolation::InProcess)
            .build()
            .unwrap();
        assert_eq!(config.stage_isolation, StageIsolation::InProcess);
    }
}
