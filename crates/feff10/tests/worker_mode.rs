//! A real embedding host: each worker enters `init` before application setup.
mod common;

use feff10::config::{FeffConfigBuilder, StageIsolation};
use feff10::pipeline::FeffPipeline;
use feff10::{Error, Stage};

fn main() {
    feff10::worker::init();

    // GENFMT cannot run without the phase calculation. A Fortran fatal
    // error must terminate only the worker and reach the host as an error.
    let missing_phase = tempfile::tempdir().unwrap();
    let config = FeffConfigBuilder::new()
        .work_dir(missing_phase.path())
        .input(common::copper_input())
        .stages(vec![Stage::Rdinp, Stage::Genfmt])
        .stage_isolation(StageIsolation::Worker)
        .build()
        .unwrap();
    match FeffPipeline::new(config).run().unwrap_err() {
        Error::Pipeline(error) => {
            assert_eq!(error.stage, "genfmt");
            assert_eq!(error.exit_code, Some(1));
            assert!(error.feff_error.is_some());
        }
        error => panic!("unexpected worker error: {error}"),
    }

    // Exercise both explicit Worker and default Auto. Repeat with fresh output
    // directories in the same host to expose any retained Fortran allocations.
    for isolation in [StageIsolation::Worker, StageIsolation::Auto] {
        for _ in 0..2 {
            let work_dir = tempfile::tempdir().unwrap();
            let config = FeffConfigBuilder::new()
                .work_dir(work_dir.path())
                .input(common::copper_input())
                .stage_isolation(isolation)
                .build()
                .unwrap();
            FeffPipeline::new(config).run().expect("Cu pipeline failed");
            common::assert_copper_paths(work_dir.path());
        }
    }
    println!("Cu paths verified with Worker and Auto isolation, including repeat runs");
}
