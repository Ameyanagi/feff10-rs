use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::FeffConfig;
use crate::error::{Error, PipelineError};
use crate::stage::Stage;

static FEFF_EXEC_LOCK: Mutex<()> = Mutex::new(());

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

        // Recover from poison so one failed run does not permanently disable FEFF execution.
        let _exec_lock = match FEFF_EXEC_LOCK.lock() {
            Ok(lock) => lock,
            Err(poisoned) => poisoned.into_inner(),
        };

        for &stage in &self.config.stages {
            callback(stage, StageProgress::Starting);

            let start = Instant::now();
            run_stage_isolated(stage, &self.config.work_dir, self.config.stage_timeout)?;
            let duration = start.elapsed();

            callback(stage, StageProgress::Finished { duration });

            // Check for FEFF error (written to .feff.error by the Fortran error module)
            let feff_error = fs::read_to_string(&feff_error_path)
                .ok()
                .and_then(|s| if s.trim().is_empty() { None } else { Some(s) });

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

/// Run a single FEFF stage in a forked child process (Unix) or in-process (Windows).
///
/// On Unix, each stage runs in its own process via `fork()` to isolate Fortran
/// module state, I/O unit state, and memory allocations — matching the original
/// FEFF behavior where each stage was a separate executable.
///
/// On Windows, the stage runs directly in-process since `fork()` is unavailable.
/// This means Fortran `stop` on error will terminate the host process, and global
/// state is not fully isolated between stages. In practice this is fine because
/// stages communicate via files and `stop` is only called on error paths.
#[cfg(unix)]
fn run_stage_isolated(
    stage: Stage,
    work_dir: &std::path::Path,
    timeout: Option<Duration>,
) -> Result<(), Error> {
    let _cwd_guard = CwdGuard::enter(work_dir)?;

    let pid = unsafe { libc::fork() };

    match pid {
        -1 => Err(Error::Io(std::io::Error::last_os_error())),
        0 => {
            // ── Child process ──
            // Call the Fortran subroutine. If it returns normally, exit(0).
            // If the Fortran code calls `stop`, the process terminates directly.
            unsafe { stage.call_ffi() };
            unsafe { libc::_exit(0) };
        }
        child_pid => {
            if timeout.is_some() {
                wait_with_timeout(stage, child_pid, timeout)
            } else {
                wait_blocking(stage, child_pid)
            }
        }
    }
}

/// Blocking wait (original behavior, no polling overhead).
#[cfg(unix)]
fn wait_blocking(stage: Stage, child_pid: libc::pid_t) -> Result<(), Error> {
    let mut status: libc::c_int = 0;
    loop {
        let ret = unsafe { libc::waitpid(child_pid, &mut status, 0) };
        if ret == -1 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(Error::Io(err));
        }
        break;
    }
    check_child_status(stage, status)
}

/// Non-blocking wait with timeout via WNOHANG polling.
#[cfg(unix)]
fn wait_with_timeout(
    stage: Stage,
    child_pid: libc::pid_t,
    timeout: Option<Duration>,
) -> Result<(), Error> {
    let start = Instant::now();
    loop {
        let mut status: libc::c_int = 0;
        let ret = unsafe { libc::waitpid(child_pid, &mut status, libc::WNOHANG) };

        if ret == -1 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(Error::Io(err));
        }

        if ret == child_pid {
            return check_child_status(stage, status);
        }

        // ret == 0: child still running
        if let Some(t) = timeout
            && start.elapsed() > t
        {
            unsafe { libc::kill(child_pid, libc::SIGKILL) };
            unsafe { libc::waitpid(child_pid, std::ptr::null_mut(), 0) };
            return Err(Error::Pipeline(PipelineError {
                stage: stage.executable_name().to_string(),
                exit_code: None,
                stderr: format!("timed out after {}s", t.as_secs()),
                feff_error: None,
            }));
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
fn check_child_status(stage: Stage, status: libc::c_int) -> Result<(), Error> {
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

#[cfg(not(unix))]
fn run_stage_isolated(
    stage: Stage,
    work_dir: &std::path::Path,
    _timeout: Option<Duration>,
) -> Result<(), Error> {
    let _cwd_guard = CwdGuard::enter(work_dir)?;
    unsafe { stage.call_ffi() };
    Ok(())
}

struct CwdGuard {
    old_dir: PathBuf,
}

impl CwdGuard {
    fn enter(dir: &Path) -> Result<Self, Error> {
        let old_dir = std::env::current_dir()?;
        std::env::set_current_dir(dir)?;
        Ok(Self { old_dir })
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.old_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::CwdGuard;

    #[test]
    fn cwd_guard_restores_previous_directory() {
        let original = std::env::current_dir().unwrap().canonicalize().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let tmp_canon = tmp.path().canonicalize().unwrap();

        {
            let _guard = CwdGuard::enter(tmp.path()).unwrap();
            let now = std::env::current_dir().unwrap().canonicalize().unwrap();
            assert_eq!(now, tmp_canon);
        }

        let restored = std::env::current_dir().unwrap().canonicalize().unwrap();
        assert_eq!(restored, original);
    }
}
