use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::config::FeffConfig;
use crate::error::{Error, PipelineError};
use crate::stage::Stage;

/// Result from running a single stage.
#[derive(Debug)]
pub struct StageResult {
    pub stage: Stage,
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
    Finished { duration: Duration },
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

        // Clear any stale .feff.error from a previous run
        let feff_error_path = self.config.work_dir.join(".feff.error");
        let _ = fs::remove_file(&feff_error_path);

        for &stage in &self.config.stages {
            callback(stage, StageProgress::Starting);

            let start = Instant::now();
            run_stage_forked(stage, &self.config.work_dir)?;
            let duration = start.elapsed();

            callback(stage, StageProgress::Finished { duration });

            // Check for FEFF error (written to .feff.error by the Fortran error module)
            let feff_error = fs::read_to_string(&feff_error_path).ok().and_then(|s| {
                if s.trim().is_empty() {
                    None
                } else {
                    Some(s)
                }
            });

            if feff_error.is_some() {
                return Err(Error::Pipeline(PipelineError {
                    stage: stage.executable_name().to_string(),
                    exit_code: None,
                    stderr: String::new(),
                    feff_error,
                }));
            }

            stage_results.push(StageResult { stage, duration });
        }

        Ok(PipelineResult {
            stages: stage_results,
            work_dir: self.config.work_dir.clone(),
        })
    }
}

/// Run a single FEFF stage in a forked child process.
///
/// Each stage runs in its own process to isolate Fortran module state,
/// I/O unit state, and memory allocations — matching the original FEFF
/// behavior where each stage was a separate executable.
///
/// The child process:
/// - Inherits the working directory (already set to work_dir)
/// - Calls the Fortran subroutine via FFI
/// - Exits with code 0 on success, or the process terminates via
///   Fortran `stop` on error
///
/// The parent process waits for the child and checks the exit status.
fn run_stage_forked(stage: Stage, work_dir: &std::path::Path) -> Result<(), Error> {
    // Save current directory
    let old_dir = std::env::current_dir()?;

    // Change to work directory before forking so the child inherits it
    std::env::set_current_dir(work_dir)?;

    let pid = unsafe { libc::fork() };

    match pid {
        -1 => {
            // Fork failed — restore cwd and return error
            let _ = std::env::set_current_dir(&old_dir);
            Err(Error::Io(std::io::Error::last_os_error()))
        }
        0 => {
            // ── Child process ──
            // Call the Fortran subroutine. If it returns normally, exit(0).
            // If the Fortran code calls `stop`, the process terminates directly.
            unsafe { stage.call_ffi() };
            unsafe { libc::_exit(0) };
        }
        child_pid => {
            // ── Parent process ──
            // Restore working directory immediately
            std::env::set_current_dir(&old_dir)?;

            // Wait for child to finish
            let mut status: libc::c_int = 0;
            let ret = unsafe { libc::waitpid(child_pid, &mut status, 0) };
            if ret == -1 {
                return Err(Error::Io(std::io::Error::last_os_error()));
            }

            if libc::WIFEXITED(status) {
                let exit_code = libc::WEXITSTATUS(status);
                if exit_code == 0 {
                    Ok(())
                } else {
                    Err(Error::Pipeline(PipelineError {
                        stage: stage.executable_name().to_string(),
                        exit_code: Some(exit_code),
                        stderr: String::new(),
                        feff_error: None,
                    }))
                }
            } else if libc::WIFSIGNALED(status) {
                let signal = libc::WTERMSIG(status);
                Err(Error::Pipeline(PipelineError {
                    stage: stage.executable_name().to_string(),
                    exit_code: None,
                    stderr: format!("killed by signal {signal}"),
                    feff_error: None,
                }))
            } else {
                Err(Error::Pipeline(PipelineError {
                    stage: stage.executable_name().to_string(),
                    exit_code: None,
                    stderr: "unknown child status".to_string(),
                    feff_error: None,
                }))
            }
        }
    }
}
