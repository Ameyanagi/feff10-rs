use std::path::PathBuf;

use feff10::config::FeffConfigBuilder;
use feff10::input::FeffInput;
use feff10::output::FeffTable;
use feff10::pipeline::FeffPipeline;

fn feff10_examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../feff10/examples")
}

/// Run a FEFF calculation and compare xmu.dat against reference.
fn run_and_compare(example_subdir: &str, reference_file: &str, col_x: usize, col_y: usize) {
    let example_dir = feff10_examples_dir().join(example_subdir);
    let inp_path = example_dir.join("feff.inp");
    if !inp_path.exists() {
        eprintln!(
            "skipping {example_subdir}: feff10 submodule not available at {}",
            inp_path.display()
        );
        return;
    }

    let work_dir = tempfile::tempdir().unwrap();
    // Copy feff.inp to work dir
    std::fs::copy(&inp_path, work_dir.path().join("feff.inp")).unwrap();

    let input = FeffInput::from_file(work_dir.path().join("feff.inp")).unwrap();

    let config = FeffConfigBuilder::new()
        .work_dir(work_dir.path())
        .input(input)
        .build()
        .unwrap();

    let pipeline = FeffPipeline::new(config);
    let result = pipeline.run();

    match result {
        Ok(_res) => {
            // All stages completed successfully (FFI call returned normally)
        }
        Err(e) => {
            panic!("Pipeline failed: {e}");
        }
    }

    // Compare output with reference
    let output_file = work_dir
        .path()
        .join(reference_file.replace("reference", ""));
    let ref_file = example_dir.join(reference_file);

    if !output_file.exists() {
        panic!(
            "Expected output file {} not found in work dir",
            output_file.display()
        );
    }

    let output = FeffTable::from_file(&output_file).unwrap();
    let reference = FeffTable::from_file(&ref_file).unwrap();

    let rsq = output.r_squared(&reference, col_x, col_y);
    let pct = rsq * 100.0;
    assert!(
        pct < 0.1,
        "R-squared {pct:.6}% exceeds 0.1% threshold for {example_subdir}"
    );
    eprintln!("{example_subdir}: R-squared = {pct:.6}% -- PASS");
}

// ========================
// Fast tests (run by default)
// ========================

#[test]
fn exafs_cu() {
    run_and_compare("EXAFS/Cu", "referencexmu.dat", 0, 3);
}

#[test]
fn xanes_cu() {
    run_and_compare("XANES/Cu", "referencexmu.dat", 0, 3);
}

#[test]
fn xanes_bn() {
    run_and_compare("XANES/BN", "referencexmu.dat", 0, 3);
}

// ========================
// Slower tests (run with --ignored)
// ========================

#[test]
#[ignore]
fn exafs_gecl4() {
    run_and_compare("EXAFS/GeCl_4", "referencexmu.dat", 0, 3);
}

#[test]
#[ignore]
fn exafs_sf6() {
    run_and_compare("EXAFS/SF6", "referencexmu.dat", 0, 3);
}

#[test]
#[ignore]
fn exafs_ybco() {
    run_and_compare("EXAFS/YBCO", "referencexmu.dat", 0, 3);
}

#[test]
#[ignore]
fn exafs_cu_scf() {
    run_and_compare("EXAFS/Cu_SCF", "referencexmu.dat", 0, 3);
}

#[test]
#[ignore]
fn xanes_gecl4() {
    run_and_compare("XANES/GeCl_4", "referencexmu.dat", 0, 3);
}

#[test]
#[ignore]
fn danes_cu() {
    run_and_compare("DANES/Cu", "referencexmu.dat", 0, 3);
}

#[test]
#[ignore]
fn danes_bn() {
    run_and_compare("DANES/BN", "referencexmu.dat", 0, 3);
}

#[test]
#[ignore]
fn fprime_gecl4() {
    run_and_compare("FPRIME/GeCl4", "referencexmu.dat", 0, 3);
}

#[test]
#[ignore]
fn warn_ion_cu() {
    run_and_compare("WARN_ION/Cu", "referencexmu.dat", 0, 3);
}

#[test]
#[ignore]
fn mpse_cu() {
    run_and_compare("MPSE/Cu", "referencexmu.dat", 0, 3);
}

#[test]
#[ignore]
fn xes_cu() {
    run_and_compare("XES/Cu", "referencexmu.dat", 0, 3);
}

#[test]
#[ignore]
fn xes_bn() {
    run_and_compare("XES/BN", "referencexmu.dat", 0, 3);
}

/// In-process isolation is only reliable for single-stage runs (Fortran
/// module state persists within a process); verify rdinp works that way.
#[test]
fn in_process_isolation_runs_single_stage() {
    use feff10::config::StageIsolation;
    use feff10::stage::Stage;

    // Keep the test runner free of Fortran state. Subsequent Auto/Fork tests
    // inherit its memory, including any allocations and I/O units left by RDINP.
    // InProcess is supported only for a single call in a fresh process.
    const CHILD: &str = "FEFF10_TEST_IN_PROCESS_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "in_process_isolation_runs_single_stage",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .status()
            .expect("failed to launch the isolated InProcess test");
        assert!(status.success(), "InProcess test exited with {status}");
        return;
    }

    let inp_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../feff10-cli/examples/bundled/exafs-sf6.inp");
    let work_dir = tempfile::tempdir().unwrap();
    std::fs::copy(&inp_path, work_dir.path().join("feff.inp")).unwrap();
    let input = FeffInput::from_file(work_dir.path().join("feff.inp")).unwrap();

    let config = FeffConfigBuilder::new()
        .work_dir(work_dir.path())
        .input(input)
        .stages(vec![Stage::Rdinp])
        .stage_isolation(StageIsolation::InProcess)
        .build()
        .unwrap();

    FeffPipeline::new(config)
        .run()
        .expect("in-process rdinp failed");
}
