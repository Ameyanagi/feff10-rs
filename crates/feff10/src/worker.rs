//! Stage-worker support for fork-unsafe hosts (GUI applications).
//!
//! FEFF stages are isolated per process. On plain Unix processes that is
//! done with `fork()`; but a forked child of a process that has initialized
//! AppKit/Metal (any macOS GUI app embedding this library) aborts in the
//! Objective-C runtime before the stage can run. Running stages in-process
//! is not an alternative for multi-stage pipelines because FEFF's Fortran
//! modules keep allocated state between stages (e.g. `lrstat` in
//! m_stkets.f90), which only a fresh process resets.
//!
//! The solution mirrors the original FEFF design of one executable per
//! stage: re-exec the host executable as a single-stage worker. Hosts opt in
//! by calling [`init`] at the very top of `main()`, before any GUI
//! initialization:
//!
//! ```no_run
//! fn main() {
//!     feff10::worker::init(); // never returns in worker processes
//!     // ... GUI setup ...
//! }
//! ```
//!
//! With the hook installed, [`StageIsolation::Auto`](crate::config::StageIsolation)
//! transparently uses worker processes when the host is fork-unsafe.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::stage::Stage;

/// Environment variable carrying the stage to run in a worker process.
pub const ENV_STAGE: &str = "FEFF10_WORKER_STAGE";
/// Environment variable carrying the working directory for the stage.
pub const ENV_DIR: &str = "FEFF10_WORKER_DIR";

static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Whether [`init`] has been called in this process.
pub fn installed() -> bool {
    INSTALLED.load(Ordering::Relaxed)
}

/// Install the stage-worker hook. Call first thing in `main()`.
///
/// When the current process was spawned as a stage worker (the worker
/// environment variables are set), this runs that single stage and exits —
/// it never returns. Otherwise it records that the hook is installed (so
/// `StageIsolation::Auto` may use worker processes) and returns.
pub fn init() {
    INSTALLED.store(true, Ordering::Relaxed);
    let (Ok(stage_name), Ok(dir)) = (std::env::var(ENV_STAGE), std::env::var(ENV_DIR)) else {
        return;
    };
    let Some(stage) = Stage::all()
        .iter()
        .copied()
        .find(|s| s.executable_name() == stage_name)
    else {
        eprintln!("feff10 worker: unknown stage '{stage_name}'");
        std::process::exit(64);
    };
    let code = run_stage(stage, Path::new(&dir));
    std::process::exit(code);
}

fn run_stage(stage: Stage, dir: &Path) -> i32 {
    if let Err(e) = std::env::set_current_dir(dir) {
        eprintln!("feff10 worker: cannot enter '{}': {e}", dir.display());
        return 66;
    }
    // Fresh process: safe to call the stage directly, exactly like the
    // original standalone FEFF executables. Fortran `stop` terminates this
    // worker only.
    unsafe { stage.call_ffi() };
    0
}
