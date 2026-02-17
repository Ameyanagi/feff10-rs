use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{Builder, TempDir};

#[test]
fn feff_command_executes_available_serial_workflow_modules() {
    let temp = fixture_tempdir();
    stage_baseline_artifact(
        "FX-WORKFLOW-XAS-001",
        "feff.inp",
        temp.path().join("feff.inp"),
    );

    let output = run_cli_command(temp.path(), &["feff"]);

    assert!(
        output.status.success(),
        "feff should succeed for serial workflow modules with runtime engines, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Completed serial workflow"),
        "feff should print serial workflow completion summary"
    );
    assert!(
        temp.path().join("pot.inp").is_file(),
        "feff should materialize RDINP/POT chain artifacts"
    );
    assert!(
        temp.path().join("phase.bin").is_file(),
        "feff should materialize XSPH outputs in serial workflow mode"
    );
    assert!(
        temp.path().join("paths.dat").is_file(),
        "feff should materialize PATH outputs in serial workflow mode"
    );
    assert!(
        temp.path().join("gg.bin").is_file(),
        "feff should materialize FMS outputs in serial workflow mode"
    );
}

#[test]
fn feffmpi_command_runs_serial_workflow_after_warning() {
    let temp = fixture_tempdir();
    stage_baseline_artifact(
        "FX-WORKFLOW-XAS-001",
        "feff.inp",
        temp.path().join("feff.inp"),
    );

    let output = run_cli_command(temp.path(), &["feffmpi", "4"]);

    assert!(
        output.status.success(),
        "feffmpi should run the serial compatibility workflow when engines are available, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("WARNING: [RUN.MPI_DEFERRED]"),
        "stderr should include deferred MPI warning, stderr: {}",
        stderr
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Completed serial workflow"),
        "feffmpi should print serial workflow completion summary"
    );
}

#[test]
fn module_commands_enforce_runtime_compute_engine_boundary() {
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
        "pot should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&pot.stderr)
    );
    assert!(
        temp.path().join("pot.bin").is_file(),
        "pot should emit pot.bin"
    );
    assert!(
        temp.path().join("pot.dat").is_file(),
        "pot should emit pot.dat"
    );
    assert!(
        temp.path().join("convergence.scf").is_file(),
        "pot should emit convergence.scf"
    );
    assert!(
        temp.path().join("convergence.scf.fine").is_file(),
        "pot should emit convergence.scf.fine"
    );

    let ldos = run_cli_command(temp.path(), &["ldos"]);
    assert!(
        ldos.status.success(),
        "ldos should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&ldos.stderr)
    );
    let has_ldos_table = fs::read_dir(temp.path())
        .expect("working directory should be readable")
        .flatten()
        .any(|entry| {
            let name = entry.file_name();
            let normalized = name.to_string_lossy().to_ascii_lowercase();
            normalized.starts_with("ldos") && normalized.ends_with(".dat")
        });
    assert!(has_ldos_table, "ldos should emit ldosNN.dat outputs");
    assert!(
        temp.path().join("logdos.dat").is_file(),
        "ldos should emit logdos.dat"
    );

    let screen = run_cli_command(temp.path(), &["screen"]);
    assert!(
        screen.status.success(),
        "screen should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&screen.stderr)
    );
    assert!(
        temp.path().join("wscrn.dat").is_file(),
        "screen should emit wscrn.dat"
    );
    assert!(
        temp.path().join("logscreen.dat").is_file(),
        "screen should emit logscreen.dat"
    );

    let xsph = run_cli_command(temp.path(), &["xsph"]);
    assert!(
        xsph.status.success(),
        "xsph should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&xsph.stderr)
    );
    assert!(
        temp.path().join("phase.bin").is_file(),
        "xsph should emit phase.bin"
    );
    assert!(
        temp.path().join("xsect.dat").is_file(),
        "xsph should emit xsect.dat"
    );
    assert!(
        temp.path().join("log2.dat").is_file(),
        "xsph should emit log2.dat"
    );

    let path = run_cli_command(temp.path(), &["path"]);
    assert!(
        path.status.success(),
        "path should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&path.stderr)
    );
    assert!(
        temp.path().join("paths.dat").is_file(),
        "path should emit paths.dat"
    );
    assert!(
        temp.path().join("paths.bin").is_file(),
        "path should emit paths.bin"
    );
    assert!(
        temp.path().join("crit.dat").is_file(),
        "path should emit crit.dat"
    );
    assert!(
        temp.path().join("log4.dat").is_file(),
        "path should emit log4.dat"
    );

    let fms = run_cli_command(temp.path(), &["fms"]);
    assert!(
        fms.status.success(),
        "fms should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&fms.stderr)
    );
    assert!(
        temp.path().join("gg.bin").is_file(),
        "fms should emit gg.bin"
    );
    assert!(
        temp.path().join("log3.dat").is_file(),
        "fms should emit log3.dat"
    );
}

#[test]
fn crpa_module_command_succeeds_with_runtime_compute_engine() {
    let temp = fixture_tempdir();
    stage_baseline_artifact("FX-CRPA-001", "crpa.inp", temp.path().join("crpa.inp"));
    stage_baseline_artifact("FX-CRPA-001", "pot.inp", temp.path().join("pot.inp"));
    stage_baseline_artifact("FX-CRPA-001", "geom.dat", temp.path().join("geom.dat"));

    let crpa = run_cli_command(temp.path(), &["crpa"]);
    assert!(
        crpa.status.success(),
        "crpa should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&crpa.stderr)
    );
    assert!(
        temp.path().join("wscrn.dat").is_file(),
        "crpa should emit wscrn.dat"
    );
    assert!(
        temp.path().join("logscrn.dat").is_file(),
        "crpa should emit logscrn.dat"
    );
}

#[test]
fn band_module_command_succeeds_with_runtime_compute_engine() {
    let temp = fixture_tempdir();
    stage_band_input(temp.path().join("band.inp"));
    stage_baseline_artifact("FX-BAND-001", "geom.dat", temp.path().join("geom.dat"));
    stage_baseline_artifact("FX-BAND-001", "global.inp", temp.path().join("global.inp"));
    stage_baseline_artifact("FX-BAND-001", "phase.bin", temp.path().join("phase.bin"));

    let band = run_cli_command(temp.path(), &["band"]);
    assert!(
        band.status.success(),
        "band should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&band.stderr)
    );
    assert!(
        temp.path().join("bandstructure.dat").is_file(),
        "band should emit bandstructure.dat"
    );
    assert!(
        temp.path().join("logband.dat").is_file(),
        "band should emit logband.dat"
    );
}

#[test]
fn compton_module_command_succeeds_with_runtime_compute_engine() {
    let temp = fixture_tempdir();
    stage_baseline_artifact(
        "FX-COMPTON-001",
        "compton.inp",
        temp.path().join("compton.inp"),
    );
    stage_baseline_artifact("FX-COMPTON-001", "pot.bin", temp.path().join("pot.bin"));
    stage_gg_slice_input(temp.path().join("gg_slice.bin"));

    let compton = run_cli_command(temp.path(), &["compton"]);
    assert!(
        compton.status.success(),
        "compton should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&compton.stderr)
    );
    assert!(
        temp.path().join("compton.dat").is_file(),
        "compton should emit compton.dat"
    );
    assert!(
        temp.path().join("jzzp.dat").is_file(),
        "compton should emit jzzp.dat"
    );
    assert!(
        temp.path().join("rhozzp.dat").is_file(),
        "compton should emit rhozzp.dat"
    );
    assert!(
        temp.path().join("logcompton.dat").is_file(),
        "compton should emit logcompton.dat"
    );
}

#[test]
fn ff2x_module_command_succeeds_with_runtime_compute_engine() {
    let temp = fixture_tempdir();
    stage_ff2x_input(temp.path().join("ff2x.inp"));
    stage_baseline_artifact("FX-DEBYE-001", "paths.dat", temp.path().join("paths.dat"));
    stage_baseline_artifact("FX-DEBYE-001", "feff.inp", temp.path().join("feff.inp"));
    stage_baseline_artifact("FX-DEBYE-001", "spring.inp", temp.path().join("spring.inp"));

    let ff2x = run_cli_command(temp.path(), &["ff2x"]);
    assert!(
        ff2x.status.success(),
        "ff2x should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&ff2x.stderr)
    );
    assert!(
        temp.path().join("s2_em.dat").is_file(),
        "ff2x should emit s2_em.dat"
    );
    assert!(
        temp.path().join("s2_rm1.dat").is_file(),
        "ff2x should emit s2_rm1.dat"
    );
    assert!(
        temp.path().join("s2_rm2.dat").is_file(),
        "ff2x should emit s2_rm2.dat"
    );
    assert!(
        temp.path().join("xmu.dat").is_file(),
        "ff2x should emit xmu.dat"
    );
    assert!(
        temp.path().join("chi.dat").is_file(),
        "ff2x should emit chi.dat"
    );
    assert!(
        temp.path().join("log6.dat").is_file(),
        "ff2x should emit log6.dat"
    );
    assert!(
        temp.path().join("spring.dat").is_file(),
        "ff2x should emit spring.dat"
    );
}

#[test]
fn dmdw_module_command_succeeds_with_runtime_compute_engine() {
    let temp = fixture_tempdir();
    stage_dmdw_input(temp.path().join("dmdw.inp"));
    stage_baseline_artifact("FX-DMDW-001", "feff.dym", temp.path().join("feff.dym"));

    let dmdw = run_cli_command(temp.path(), &["dmdw"]);
    assert!(
        dmdw.status.success(),
        "dmdw should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&dmdw.stderr)
    );
    assert!(
        temp.path().join("dmdw.out").is_file(),
        "dmdw should emit dmdw.out"
    );
}

#[test]
fn sfconv_module_command_succeeds_with_runtime_compute_engine() {
    let temp = fixture_tempdir();
    stage_sfconv_input(temp.path().join("sfconv.inp"));
    stage_self_spectrum_input(temp.path().join("xmu.dat"));

    let sfconv = run_cli_command(temp.path(), &["sfconv"]);
    assert!(
        sfconv.status.success(),
        "sfconv should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&sfconv.stderr)
    );
    assert!(
        temp.path().join("selfenergy.dat").is_file(),
        "sfconv should emit selfenergy.dat"
    );
    assert!(
        temp.path().join("sigma.dat").is_file(),
        "sfconv should emit sigma.dat"
    );
    assert!(
        temp.path().join("specfunct.dat").is_file(),
        "sfconv should emit specfunct.dat"
    );
    assert!(
        temp.path().join("logsfconv.dat").is_file(),
        "sfconv should emit logsfconv.dat"
    );
    assert!(
        temp.path().join("xmu.dat").is_file(),
        "sfconv should rewrite staged spectrum artifacts"
    );
}

#[test]
fn eels_module_command_succeeds_with_runtime_compute_engine() {
    let temp = fixture_tempdir();
    stage_eels_input(temp.path().join("eels.inp"));
    stage_eels_spectrum_input(temp.path().join("xmu.dat"));

    let eels = run_cli_command(temp.path(), &["eels"]);
    assert!(
        eels.status.success(),
        "eels should succeed once runtime compute engine is available, stderr: {}",
        String::from_utf8_lossy(&eels.stderr)
    );
    assert!(
        temp.path().join("eels.dat").is_file(),
        "eels should emit eels.dat"
    );
    assert!(
        temp.path().join("logeels.dat").is_file(),
        "eels should emit logeels.dat"
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
    assert!(stdout.contains("oracle"));
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

fn stage_band_input(destination: PathBuf) {
    let source = workspace_root()
        .join("artifacts/fortran-baselines")
        .join("FX-BAND-001")
        .join("baseline")
        .join("band.inp");
    if source.is_file() {
        let source_bytes = fs::read(&source)
            .unwrap_or_else(|_| panic!("band input should be readable: {}", source.display()));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::write(&destination, source_bytes).expect("band input should be staged");
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination parent should exist");
    }
    fs::write(
        &destination,
        "mband : calculate bands if = 1\n   1\nemin, emax, estep : energy mesh\n    -8.00000      6.00000      0.05000\nnkp : # points in k-path\n 121\nikpath : type of k-path\n   2\nfreeprop :  empty lattice if = T\n F\n",
    )
    .expect("band input should be staged");
}

fn stage_gg_slice_input(destination: PathBuf) {
    let source = workspace_root()
        .join("artifacts/fortran-baselines")
        .join("FX-COMPTON-001")
        .join("baseline")
        .join("gg_slice.bin");
    if source.is_file() {
        let source_bytes = fs::read(&source)
            .unwrap_or_else(|_| panic!("gg_slice input should be readable: {}", source.display()));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::write(&destination, source_bytes).expect("gg_slice input should be staged");
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination parent should exist");
    }
    fs::write(&destination, [4_u8, 5_u8, 6_u8, 7_u8]).expect("gg_slice input should be staged");
}

fn stage_sfconv_input(destination: PathBuf) {
    let source = workspace_root()
        .join("artifacts/fortran-baselines")
        .join("FX-SELF-001")
        .join("baseline")
        .join("sfconv.inp");
    if source.is_file() {
        let source_bytes = fs::read(&source)
            .unwrap_or_else(|_| panic!("sfconv input should be readable: {}", source.display()));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::write(&destination, source_bytes).expect("sfconv input should be staged");
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination parent should exist");
    }
    fs::write(
        &destination,
        "msfconv, ipse, ipsk\n   1   0   0\nwsigk, cen\n      0.00000      0.00000\nispec, ipr6\n   1   0\ncfname\nNULL\n",
    )
    .expect("sfconv input should be staged");
}

fn stage_self_spectrum_input(destination: PathBuf) {
    let source = workspace_root()
        .join("artifacts/fortran-baselines")
        .join("FX-SELF-001")
        .join("baseline")
        .join("xmu.dat");
    if source.is_file() {
        let source_bytes = fs::read(&source)
            .unwrap_or_else(|_| panic!("xmu input should be readable: {}", source.display()));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::write(&destination, source_bytes).expect("xmu input should be staged");
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination parent should exist");
    }
    fs::write(
        &destination,
        "# fallback xmu\n1.0 0.0 0.0 0.01\n2.0 0.0 0.0 0.02\n3.0 0.0 0.0 0.03\n",
    )
    .expect("xmu input should be staged");
}

fn stage_eels_input(destination: PathBuf) {
    let source = workspace_root()
        .join("artifacts/fortran-baselines")
        .join("FX-EELS-001")
        .join("baseline")
        .join("eels.inp");
    if source.is_file() {
        let source_bytes = fs::read(&source)
            .unwrap_or_else(|_| panic!("eels input should be readable: {}", source.display()));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::write(&destination, source_bytes).expect("eels input should be staged");
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination parent should exist");
    }
    fs::write(
        &destination,
        "calculate ELNES?\n   1\naverage? relativistic? cross-terms? Which input?\n   0   1   1   1   4\npolarizations to be used ; min step max\n   1   1   9\nbeam energy in eV\n 300000.00000\nbeam direction in arbitrary units\n      0.00000      1.00000      0.00000\ncollection and convergence semiangle in rad\n      0.00240      0.00000\nqmesh - radial and angular grid size\n   5   3\ndetector positions - two angles in rad\n      0.00000      0.00000\ncalculate magic angle if magic=1\n   0\nenergy for magic angle - eV above threshold\n      0.00000\n",
    )
    .expect("eels input should be staged");
}

fn stage_eels_spectrum_input(destination: PathBuf) {
    let source = workspace_root()
        .join("artifacts/fortran-baselines")
        .join("FX-EELS-001")
        .join("baseline")
        .join("xmu.dat");
    if source.is_file() {
        let source_bytes = fs::read(&source)
            .unwrap_or_else(|_| panic!("xmu input should be readable: {}", source.display()));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::write(&destination, source_bytes).expect("xmu input should be staged");
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination parent should exist");
    }
    fs::write(
        &destination,
        "# omega e k mu mu0 chi\n8979.411 -16.773 -1.540 5.56205E-06 6.25832E-06 -6.96262E-07\n8980.979 -15.204 -1.400 6.61771E-06 7.52318E-06 -9.05473E-07\n8982.398 -13.786 -1.260 7.99662E-06 9.19560E-06 -1.19897E-06\n",
    )
    .expect("xmu input should be staged");
}

fn stage_ff2x_input(destination: PathBuf) {
    let source = workspace_root()
        .join("artifacts/fortran-baselines")
        .join("FX-DEBYE-001")
        .join("baseline")
        .join("ff2x.inp");
    if source.is_file() {
        let source_bytes = fs::read(&source)
            .unwrap_or_else(|_| panic!("ff2x input should be readable: {}", source.display()));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::write(&destination, source_bytes).expect("ff2x input should be staged");
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination parent should exist");
    }
    fs::write(
        &destination,
        "mchi, ispec, idwopt, ipr6, mbconv, absolu, iGammaCH\n   1   0   2   0   0   0   0\nvrcorr, vicorr, s02, critcw\n      0.00000      0.00000      1.00000      4.00000\ntk, thetad, alphat, thetae, sig2g\n    450.00000    315.00000      0.00000      0.00000      0.00000\nmomentum transfer\n      0.00000      0.00000      0.00000\n the number of decomposi\n   -1\n",
    )
    .expect("ff2x input should be staged");
}

fn stage_dmdw_input(destination: PathBuf) {
    let source = workspace_root()
        .join("artifacts/fortran-baselines")
        .join("FX-DMDW-001")
        .join("baseline")
        .join("dmdw.inp");
    if source.is_file() {
        let source_bytes = fs::read(&source)
            .unwrap_or_else(|_| panic!("dmdw input should be readable: {}", source.display()));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::write(&destination, source_bytes).expect("dmdw input should be staged");
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination parent should exist");
    }
    fs::write(
        &destination,
        "   1\n   6\n   1    450.000\n   0\nfeff.dym\n   1\n   2   1   0          29.78\n",
    )
    .expect("dmdw input should be staged");
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_feff10-rs")
}
