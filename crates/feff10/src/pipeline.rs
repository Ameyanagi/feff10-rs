use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::{FeffConfig, StageIsolation};
use crate::error::{Error, PipelineError};
use crate::output::{FeffOutputs, FeffTable, PathsDat};
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

impl PipelineResult {
    /// Discover all FEFF `*.dat` outputs in the work directory.
    pub fn outputs(&self) -> Result<FeffOutputs, Error> {
        FeffOutputs::discover(&self.work_dir)
    }

    /// Read `xmu.dat` from the work directory (permissive parsing).
    pub fn read_xmu(&self) -> Result<FeffTable, Error> {
        FeffTable::from_file(self.work_dir.join("xmu.dat"))
    }

    /// Read `xmu.dat` from the work directory (strict parsing).
    pub fn read_xmu_strict(&self) -> Result<FeffTable, Error> {
        FeffTable::from_file_strict(self.work_dir.join("xmu.dat"))
    }

    /// Read `chi.dat` from the work directory (permissive parsing).
    pub fn read_chi(&self) -> Result<FeffTable, Error> {
        FeffTable::from_file(self.work_dir.join("chi.dat"))
    }

    /// Read `chi.dat` from the work directory (strict parsing).
    pub fn read_chi_strict(&self) -> Result<FeffTable, Error> {
        FeffTable::from_file_strict(self.work_dir.join("chi.dat"))
    }

    /// Read `eels.dat` from the work directory (permissive parsing).
    pub fn read_eels(&self) -> Result<FeffTable, Error> {
        FeffTable::from_file(self.work_dir.join("eels.dat"))
    }

    /// Read `eels.dat` from the work directory (strict parsing).
    pub fn read_eels_strict(&self) -> Result<FeffTable, Error> {
        FeffTable::from_file_strict(self.work_dir.join("eels.dat"))
    }

    /// Read `ldosNN.dat` from the work directory (permissive parsing).
    pub fn read_ldos(&self, index: u32) -> Result<FeffTable, Error> {
        FeffTable::from_file(self.work_dir.join(format!("ldos{index:02}.dat")))
    }

    /// Read `ldosNN.dat` from the work directory (strict parsing).
    pub fn read_ldos_strict(&self, index: u32) -> Result<FeffTable, Error> {
        FeffTable::from_file_strict(self.work_dir.join(format!("ldos{index:02}.dat")))
    }

    /// Read `paths.dat` from the work directory.
    pub fn read_paths(&self) -> Result<PathsDat, Error> {
        PathsDat::from_file(self.work_dir.join("paths.dat"))
    }
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
    worker_command: Option<(PathBuf, Vec<std::ffi::OsString>)>,
}

impl FeffPipeline {
    pub fn new(config: FeffConfig) -> Self {
        Self {
            config,
            worker_command: None,
        }
    }

    /// Use an external worker command for every stage.
    ///
    /// This supports interpreter hosts that cannot re-execute their own `main`.
    /// The command must call [`crate::worker::init`] before application setup;
    /// it receives the stage and working directory through the worker environment
    /// variables. Each stage starts a fresh command, with the configured timeout.
    /// Selecting this command also selects [`StageIsolation::Worker`].
    pub fn with_worker_command(
        mut self,
        executable: impl Into<PathBuf>,
        args: impl IntoIterator<Item = impl Into<std::ffi::OsString>>,
    ) -> Self {
        self.worker_command = Some((
            executable.into(),
            args.into_iter().map(Into::into).collect(),
        ));
        self.config.stage_isolation = StageIsolation::Worker;
        self
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
        self.validate_isolation()?;

        // Lock before touching input or error files: another pipeline may use
        // the same directory, and in-process execution changes the host cwd.
        let _exec_lock = match FEFF_EXEC_LOCK.lock() {
            Ok(lock) => lock,
            Err(poisoned) => poisoned.into_inner(),
        };

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
            let result = if let Some((executable, args)) = &self.worker_command {
                run_stage_command(
                    stage,
                    &self.config.work_dir,
                    self.config.stage_timeout,
                    std::process::Command::new(executable).args(args),
                )
            } else {
                run_stage_isolated(
                    stage,
                    &self.config.work_dir,
                    self.config.stage_timeout,
                    self.config.stage_isolation,
                )
            };
            result.map_err(|mut error| {
                if let Error::Pipeline(ref mut failure) = error {
                    failure.feff_error = read_feff_error(&feff_error_path);
                }
                error
            })?;
            let duration = start.elapsed();

            // Check for FEFF error (written to .feff.error by the Fortran error module)
            let feff_error = read_feff_error(&feff_error_path);

            if feff_error.is_some() {
                return Err(Error::Pipeline(PipelineError {
                    stage: stage.executable_name().to_string(),
                    exit_code: None,
                    stderr: String::new(),
                    feff_error,
                }));
            }

            callback(stage, StageProgress::Finished { duration });
            stage_results.push(StageResult { stage, duration });
        }

        Ok(PipelineResult {
            stages: stage_results,
            work_dir: self.config.work_dir.clone(),
        })
    }

    fn validate_isolation(&self) -> Result<(), Error> {
        let isolation = self.config.stage_isolation;
        let needs_worker = isolation == StageIsolation::Worker
            || (isolation == StageIsolation::Auto && requires_worker());
        if needs_worker && self.worker_command.is_none() && !crate::worker::installed() {
            return Err(Error::Config(
                "stage isolation requires a worker process: call feff10::worker::init() \
                 at the top of main(), before GUI or other application initialization"
                    .into(),
            ));
        }
        if isolation == StageIsolation::Fork && !cfg!(unix) {
            return Err(Error::Config(
                "Fork isolation is only available on Unix; use Auto with feff10::worker::init()"
                    .into(),
            ));
        }
        if isolation == StageIsolation::InProcess {
            if self.config.stages.len() > 1 {
                return Err(Error::Config(
                    "InProcess isolation cannot run multiple FEFF stages: Fortran allocations \
                     persist between calls; use Auto or Worker isolation"
                        .into(),
                ));
            }
            if self.config.stage_timeout.is_some() {
                return Err(Error::Config(
                    "stage_timeout requires Fork or Worker isolation".into(),
                ));
            }
        }
        Ok(())
    }
}

fn read_feff_error(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// Run each stage in a fresh process unless InProcess was explicitly requested.
#[cfg(unix)]
fn run_stage_isolated(
    stage: Stage,
    work_dir: &std::path::Path,
    timeout: Option<Duration>,
    isolation: StageIsolation,
) -> Result<(), Error> {
    match isolation {
        StageIsolation::Fork => run_stage_forked(stage, work_dir, timeout),
        StageIsolation::Worker => run_stage_worker(stage, work_dir, timeout),
        StageIsolation::InProcess => run_stage_in_process(stage, work_dir),
        StageIsolation::Auto => {
            if !fork_unsafe_host() {
                run_stage_forked(stage, work_dir, timeout)
            } else if crate::worker::installed() {
                run_stage_worker(stage, work_dir, timeout)
            } else {
                Err(Error::Pipeline(PipelineError {
                    stage: stage.executable_name().to_string(),
                    exit_code: None,
                    stderr: "this process is fork-unsafe (GUI host): call \
                             feff10::worker::init() at the top of main(), or run the \
                             pipeline from a separate process"
                        .to_string(),
                    feff_error: None,
                }))
            }
        }
    }
}

/// Re-exec the current executable as a single-stage worker (fork+exec).
/// The host must have called [`crate::worker::init`] in `main()`.
fn run_stage_worker(
    stage: Stage,
    work_dir: &std::path::Path,
    timeout: Option<Duration>,
) -> Result<(), Error> {
    // Validate even when called outside the pipeline to avoid recursively
    // re-executing a host that has no worker hook.
    if !crate::worker::installed() {
        return Err(Error::Config(
            "Worker isolation requires feff10::worker::init() at the top of main()".into(),
        ));
    }
    let exe = std::env::current_exe()?;
    run_stage_command(
        stage,
        work_dir,
        timeout,
        &mut std::process::Command::new(exe),
    )
}

fn run_stage_command(
    stage: Stage,
    work_dir: &Path,
    timeout: Option<Duration>,
    command: &mut std::process::Command,
) -> Result<(), Error> {
    let mut child = command
        .env(crate::worker::ENV_STAGE, stage.executable_name())
        .env(crate::worker::ENV_DIR, work_dir)
        .spawn()?;
    let start = Instant::now();
    if timeout.is_none() {
        let status = child.wait()?;
        return worker_status(stage, status);
    }
    loop {
        match child.try_wait()? {
            Some(status) => {
                return worker_status(stage, status);
            }
            None => {
                if let Some(t) = timeout
                    && start.elapsed() > t
                {
                    let _ = child.kill();
                    let _ = child.wait();
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
    }
}

fn worker_status(stage: Stage, status: std::process::ExitStatus) -> Result<(), Error> {
    if status.success() {
        return Ok(());
    }
    Err(Error::Pipeline(PipelineError {
        stage: stage.executable_name().to_string(),
        exit_code: status.code(),
        stderr: format!("worker exited with {status}"),
        feff_error: None,
    }))
}

fn requires_worker() -> bool {
    #[cfg(unix)]
    {
        fork_unsafe_host()
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// `fork()` without `exec()` is not viable in every process. On macOS, a
/// process with an *initialized* GUI (NSApplication started — any AppKit/
/// gpui/winit app embedding this library) holds Objective-C runtime and
/// dispatch state that makes the forked child abort before the Fortran
/// stage can run. AppKit merely being linked is fine (e.g. test binaries of
/// GUI crates), so detect via AppKit's `NSApp` global, which stays nil
/// until `[NSApplication sharedApplication]` runs.
#[cfg(target_os = "macos")]
fn fork_unsafe_host() -> bool {
    unsafe {
        let sym = libc::dlsym(libc::RTLD_DEFAULT, c"NSApp".as_ptr());
        if sym.is_null() {
            return false; // AppKit not loaded at all
        }
        let nsapp = *(sym as *const *const std::ffi::c_void);
        !nsapp.is_null()
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn fork_unsafe_host() -> bool {
    false
}

/// Run a single stage on a dedicated big-stack thread in this process.
/// Allocated Fortran state persists; `stop` terminates the host process.
fn run_stage_in_process(stage: Stage, work_dir: &std::path::Path) -> Result<(), Error> {
    let _cwd_guard = CwdGuard::enter(work_dir)?;
    // FEFF stages assume the generous main-thread stack of a standalone
    // executable; give them one explicitly.
    let handle = std::thread::Builder::new()
        .name(format!("feff-{}", stage.executable_name()))
        .stack_size(64 * 1024 * 1024)
        .spawn(move || unsafe { stage.call_ffi() })
        .map_err(Error::Io)?;
    handle.join().map_err(|_| {
        Error::Pipeline(PipelineError {
            stage: stage.executable_name().to_string(),
            exit_code: None,
            stderr: "stage panicked".to_string(),
            feff_error: None,
        })
    })
}

#[cfg(unix)]
fn run_stage_forked(
    stage: Stage,
    work_dir: &std::path::Path,
    timeout: Option<Duration>,
) -> Result<(), Error> {
    use std::os::unix::ffi::OsStrExt;
    let directory = std::ffi::CString::new(work_dir.as_os_str().as_bytes())
        .map_err(|_| Error::Config("work_dir contains a NUL byte".into()))?;

    let pid = unsafe { libc::fork() };

    match pid {
        -1 => Err(Error::Io(std::io::Error::last_os_error())),
        0 => {
            // ── Child process ──
            // Call the Fortran subroutine. If it returns normally, exit(0).
            // If the Fortran code calls `stop`, the process terminates directly.
            // Change cwd only in the child, leaving the embedding host and
            // its other threads in their original working directory.
            if unsafe { libc::chdir(directory.as_ptr()) } != 0 {
                unsafe { libc::_exit(126) };
            }
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
    timeout: Option<Duration>,
    isolation: StageIsolation,
) -> Result<(), Error> {
    match isolation {
        StageIsolation::Worker => run_stage_worker(stage, work_dir, timeout),
        // Worker processes also isolate Fortran module state between
        // stages; prefer them when the host installed the hook.
        StageIsolation::Auto => run_stage_worker(stage, work_dir, timeout),
        StageIsolation::InProcess => run_stage_in_process(stage, work_dir),
        StageIsolation::Fork => Err(Error::Config(
            "Fork isolation is only available on Unix".into(),
        )),
    }
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
    use super::{CwdGuard, FeffPipeline, PipelineResult};
    use crate::{FeffConfigBuilder, FeffInput, Stage, config::StageIsolation};

    fn rejected_isolation(isolation: StageIsolation, stages: Vec<Stage>, timeout: bool) -> String {
        let tmp = tempfile::tempdir().unwrap();
        let inp = tmp.path().join("feff.inp");
        std::fs::write(&inp, "original input").unwrap();
        let mut builder = FeffConfigBuilder::new()
            .work_dir(tmp.path())
            .input(FeffInput::default())
            .stages(stages)
            .stage_isolation(isolation);
        if timeout {
            builder = builder.stage_timeout(std::time::Duration::from_secs(1));
        }
        let err = FeffPipeline::new(builder.build().unwrap())
            .run()
            .unwrap_err();
        assert_eq!(std::fs::read_to_string(inp).unwrap(), "original input");
        err.to_string()
    }

    #[test]
    fn worker_without_hook_is_rejected_before_writing_input() {
        assert!(
            rejected_isolation(StageIsolation::Worker, vec![Stage::Rdinp], false)
                .contains("worker::init")
        );
    }

    #[test]
    fn in_process_pipeline_is_rejected_before_ffi() {
        assert!(
            rejected_isolation(
                StageIsolation::InProcess,
                vec![Stage::Pot, Stage::Xsph],
                false
            )
            .contains("multiple FEFF stages")
        );
    }

    #[test]
    fn in_process_timeout_is_not_silently_ignored() {
        assert!(
            rejected_isolation(StageIsolation::InProcess, vec![Stage::Rdinp], true)
                .contains("stage_timeout")
        );
    }

    #[cfg(not(unix))]
    #[test]
    fn auto_requires_worker_on_windows() {
        assert!(
            rejected_isolation(StageIsolation::Auto, vec![Stage::Rdinp], false)
                .contains("worker::init")
        );
        assert!(
            rejected_isolation(StageIsolation::Fork, vec![Stage::Rdinp], false).contains("Unix")
        );
    }

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

    #[test]
    fn pipeline_result_reads_common_outputs() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("xmu.dat"), "# header\n1.0 2.0 3.0 4.0\n").unwrap();
        std::fs::write(
            tmp.path().join("paths.dat"),
            "PATH  Rmax= 5.5\n\
             1 1 1.0  index, nleg, degeneracy, r= 1.0\n\
             x y z ipot label rleg beta eta\n\
             0.0 0.0 0.0 0 'A' 1.0 180.0 0.0\n",
        )
        .unwrap();

        let result = PipelineResult {
            stages: vec![],
            work_dir: tmp.path().to_path_buf(),
        };

        let xmu = result.read_xmu().unwrap();
        assert_eq!(xmu.nrows(), 1);
        assert_eq!(xmu.ncols(), 4);

        let paths = result.read_paths().unwrap();
        assert_eq!(paths.len(), 1);

        let outputs = result.outputs().unwrap();
        assert!(outputs.file("xmu.dat").is_some());
        assert!(outputs.file("paths.dat").is_some());
    }
}
