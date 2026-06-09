//! Worker-process isolation round trip. `harness = false`: this test binary
//! has its own `main` that installs `feff10::worker::init()` exactly like a
//! real (GUI) host, so the pipeline can re-exec it as single-stage workers.

use std::path::PathBuf;

use feff10::config::{FeffConfigBuilder, StageIsolation};
use feff10::input::FeffInput;
use feff10::pipeline::FeffPipeline;

fn main() {
    feff10::worker::init(); // never returns in worker invocations

    let inp_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../feff10-cli/examples/bundled/exafs-sf6.inp");
    let work_dir = tempfile::tempdir().unwrap();
    std::fs::copy(&inp_path, work_dir.path().join("feff.inp")).unwrap();
    let input = FeffInput::from_file(work_dir.path().join("feff.inp")).unwrap();

    let config = FeffConfigBuilder::new()
        .work_dir(work_dir.path())
        .input(input)
        .stage_isolation(StageIsolation::Worker)
        .build()
        .unwrap();

    FeffPipeline::new(config)
        .run()
        .expect("worker-mode pipeline failed");
    assert!(
        work_dir.path().join("chi.dat").exists() || work_dir.path().join("xmu.dat").exists(),
        "no spectra outputs produced"
    );
    println!("worker-mode pipeline ok");
}
