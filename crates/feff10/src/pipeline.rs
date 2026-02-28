use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::config::FeffConfig;
use crate::error::{Error, PipelineError};
use crate::stage::Stage;

/// Result from running a single stage.
#[derive(Debug)]
pub struct StageResult {
    pub stage: Stage,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}

/// Result from running the full pipeline.
#[derive(Debug)]
pub struct PipelineResult {
    pub stages: Vec<StageResult>,
    pub work_dir: PathBuf,
}

/// Progress information for a stage.
#[derive(Debug)]
pub enum StageProgress {
    Starting,
    Finished { exit_code: i32, duration: Duration },
}

/// Orchestrates FEFF executable pipeline.
pub struct FeffPipeline {
    config: FeffConfig,
}

impl FeffPipeline {
    pub fn new(config: FeffConfig) -> Self {
        Self { config }
    }

    /// Run the full pipeline.
    pub fn run(&self) -> Result<PipelineResult, Error> {
        self.run_with_progress(|_, _| {})
    }

    /// Run with a progress callback invoked before/after each stage.
    pub fn run_with_progress<F>(&self, mut callback: F) -> Result<PipelineResult, Error>
    where
        F: FnMut(Stage, StageProgress),
    {
        // Ensure working directory exists
        fs::create_dir_all(&self.config.work_dir)?;

        // Write feff.inp
        let inp_path = self.config.work_dir.join("feff.inp");
        let mut file = fs::File::create(&inp_path)?;
        self.config.input.write_to(&mut file)?;

        let mut stage_results = Vec::new();

        for &stage in &self.config.stages {
            callback(stage, StageProgress::Starting);

            let exe = feff10_sys::executable(stage.executable_name());
            if !exe.exists() {
                return Err(Error::Pipeline(PipelineError {
                    stage: stage.executable_name().to_string(),
                    exit_code: None,
                    stderr: format!("Executable not found: {}", exe.display()),
                    feff_error: None,
                }));
            }

            let start = Instant::now();
            let output = Command::new(&exe)
                .current_dir(&self.config.work_dir)
                .output()
                .map_err(|e| {
                    Error::Pipeline(PipelineError {
                        stage: stage.executable_name().to_string(),
                        exit_code: None,
                        stderr: format!("Failed to execute: {e}"),
                        feff_error: None,
                    })
                })?;
            let duration = start.elapsed();

            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            callback(
                stage,
                StageProgress::Finished {
                    exit_code,
                    duration,
                },
            );

            if exit_code != 0 {
                // Check for .feff.error file
                let feff_error_path = self.config.work_dir.join(".feff.error");
                let feff_error = fs::read_to_string(&feff_error_path).ok().and_then(|s| {
                    if s.trim().is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                });

                return Err(Error::Pipeline(PipelineError {
                    stage: stage.executable_name().to_string(),
                    exit_code: Some(exit_code),
                    stderr,
                    feff_error,
                }));
            }

            stage_results.push(StageResult {
                stage,
                exit_code,
                stdout,
                stderr,
                duration,
            });
        }

        Ok(PipelineResult {
            stages: stage_results,
            work_dir: self.config.work_dir.clone(),
        })
    }
}
