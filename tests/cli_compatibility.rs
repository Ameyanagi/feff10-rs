use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{Builder, TempDir};

#[test]
fn feff_command_runs_workflow_fixture_chain() {
    let temp = fixture_tempdir();
    stage_baseline_artifact(
        "FX-WORKFLOW-XAS-001",
        "feff.inp",
        temp.path().join("feff.inp"),
    );

    let output = run_cli_command(temp.path(), &["feff"]);

    assert!(
        output.status.success(),
        "feff command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for artifact in [
        "geom.dat",
        "global.inp",
        "pot.inp",
        "xsph.inp",
        "paths.inp",
        "fms.inp",
        "pot.bin",
        "phase.bin",
        "paths.dat",
        "gg.bin",
        "log.dat",
        "log1.dat",
        "log2.dat",
        "log3.dat",
        "log4.dat",
    ] {
        assert!(
            temp.path().join(artifact).is_file(),
            "core workflow artifact '{}' should exist in current directory",
            artifact
        );
    }
    assert!(
        !temp.path().join("FX-WORKFLOW-XAS-001").exists(),
        "workflow outputs should be written directly into current directory, not nested fixture directories"
    );
}

#[test]
fn feffmpi_command_accepts_nprocs_and_runs_serial_fallback() {
    let temp = fixture_tempdir();
    stage_baseline_artifact(
        "FX-WORKFLOW-XAS-001",
        "feff.inp",
        temp.path().join("feff.inp"),
    );

    let output = run_cli_command(temp.path(), &["feffmpi", "4"]);

    assert!(
        output.status.success(),
        "feffmpi command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("WARNING: [RUN.MPI_DEFERRED]"),
        "stderr should include deferred MPI warning, stderr: {}",
        stderr
    );
    assert!(
        temp.path().join("phase.bin").is_file(),
        "serial fallback should materialize core outputs"
    );
}

#[test]
fn module_commands_support_fixture_workflows() {
    let temp = fixture_tempdir();
    stage_baseline_artifact(
        "FX-WORKFLOW-XAS-001",
        "feff.inp",
        temp.path().join("feff.inp"),
    );

    let rdinp = run_cli_command(temp.path(), &["rdinp"]);
    assert!(
        rdinp.status.success(),
        "rdinp should succeed, stderr: {}",
        String::from_utf8_lossy(&rdinp.stderr)
    );
    assert!(
        temp.path().join("pot.inp").is_file(),
        "rdinp should emit pot.inp"
    );

    let pot = run_cli_command(temp.path(), &["pot"]);
    assert!(
        pot.status.success(),
        "pot should succeed, stderr: {}",
        String::from_utf8_lossy(&pot.stderr)
    );
    assert!(
        temp.path().join("pot.bin").is_file(),
        "pot should emit pot.bin"
    );
}

#[test]
fn cli_argument_validation_matches_contract() {
    let temp = fixture_tempdir();

    let invalid_mpi = run_cli_command(temp.path(), &["feffmpi", "not-an-integer"]);
    assert_eq!(
        invalid_mpi.status.code(),
        Some(2),
        "invalid feffmpi argument should exit with input-validation code"
    );
    assert!(
        String::from_utf8_lossy(&invalid_mpi.stderr).contains("INPUT.CLI_USAGE"),
        "invalid usage should be surfaced through compatibility error mapping"
    );
    let invalid_mpi_stderr = String::from_utf8_lossy(&invalid_mpi.stderr);
    assert!(
        invalid_mpi_stderr.contains("ERROR: [INPUT.CLI_USAGE]"),
        "fatal usage failures should include ERROR diagnostic prefix, stderr: {}",
        invalid_mpi_stderr
    );
    assert!(
        invalid_mpi_stderr.contains("FATAL EXIT CODE: 2"),
        "fatal usage failures should include fatal exit summary line, stderr: {}",
        invalid_mpi_stderr
    );

    let invalid_module_args = run_cli_command(temp.path(), &["pot", "unexpected"]);
    assert_eq!(
        invalid_module_args.status.code(),
        Some(2),
        "module command with extra args should fail usage validation"
    );
}

#[test]
fn regression_command_missing_manifest_emits_io_diagnostic_contract() {
    let temp = fixture_tempdir();

    let output = run_cli_command(temp.path(), &["regression", "--manifest", "missing.json"]);
    assert_eq!(
        output.status.code(),
        Some(3),
        "missing manifest should map to IO fatal exit code, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ERROR: [IO.REGRESSION_MANIFEST]"),
        "stderr should include IO diagnostic prefix contract, stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("FATAL EXIT CODE: 3"),
        "stderr should include fatal exit summary line, stderr: {}",
        stderr
    );
}

#[test]
fn top_level_help_lists_compatibility_commands() {
    let temp = fixture_tempdir();
    let output = run_cli_command(temp.path(), &["help"]);

    assert!(
        output.status.success(),
        "help command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("feff"));
    assert!(stdout.contains("feffmpi <nprocs>"));
    assert!(stdout.contains("rdinp"));
}

#[cfg(unix)]
#[test]
fn executable_name_alias_dispatches_module_command() {
    use std::os::unix::process::CommandExt;

    let temp = fixture_tempdir();
    stage_baseline_artifact(
        "FX-WORKFLOW-XAS-001",
        "feff.inp",
        temp.path().join("feff.inp"),
    );

    let mut command = Command::new(binary_path());
    command.arg0("rdinp").current_dir(temp.path());
    let output = command
        .output()
        .expect("rdinp executable alias command should run");

    assert!(
        output.status.success(),
        "rdinp alias should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        temp.path().join("pot.inp").is_file(),
        "rdinp alias should materialize outputs"
    );
}

fn fixture_tempdir() -> TempDir {
    let target_root = workspace_root().join("target");
    fs::create_dir_all(&target_root).expect("target dir should exist");
    Builder::new()
        .prefix("cli-compat-")
        .tempdir_in(target_root)
        .expect("fixture tempdir should be created")
}

fn run_cli_command(cwd: &Path, args: &[&str]) -> Output {
    Command::new(binary_path())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("CLI command should run")
}

fn stage_baseline_artifact(fixture_id: &str, artifact: &str, destination: PathBuf) {
    let source = workspace_root()
        .join("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(artifact);
    let source_bytes = fs::read(&source)
        .unwrap_or_else(|_| panic!("baseline artifact should be readable: {}", source.display()));
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination parent should exist");
    }
    fs::write(&destination, source_bytes).expect("baseline artifact should be staged");
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_feff10-rs")
}
