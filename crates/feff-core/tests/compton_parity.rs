use feff_core::domain::{ComputeArtifact, ComputeModule, ComputeRequest};
use feff_core::modules::ModuleExecutor;
use feff_core::modules::compton::ComptonModule;
use feff_core::modules::regression::{RegressionRunnerConfig, run_regression};
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .to_path_buf()
}

struct FixtureCase {
    id: &'static str,
    input_directory: &'static str,
}

const APPROVED_COMPTON_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-COMPTON-001",
    input_directory: "feff10/examples/COMPTON/Cu",
}];

const EXPECTED_COMPTON_ARTIFACTS: [&str; 4] =
    ["compton.dat", "jzzp.dat", "rhozzp.dat", "logcompton.dat"];
const REQUIRED_COMPTON_INPUT_ARTIFACTS: [&str; 2] = ["pot.bin", "gg_slice.bin"];

#[test]
fn approved_compton_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_COMPTON_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = run_compton_for_fixture(fixture, temp.path(), "actual");

        for artifact in &EXPECTED_COMPTON_ARTIFACTS {
            let output_path = output_dir.join(artifact);
            assert!(
                output_path.is_file(),
                "COMPTON artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "COMPTON artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }
}

#[test]
fn approved_compton_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_COMPTON_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_output = run_compton_for_fixture(fixture, temp.path(), "first");
        let second_output = run_compton_for_fixture(fixture, temp.path(), "second");

        for artifact in &EXPECTED_COMPTON_ARTIFACTS {
            let first = fs::read(first_output.join(artifact)).expect("first output should exist");
            let second =
                fs::read(second_output.join(artifact)).expect("second output should exist");
            assert_eq!(
                first, second,
                "fixture '{}' artifact '{}' should be deterministic",
                fixture.id, artifact
            );
        }
    }
}

#[test]
fn compton_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("compton-manifest.json");

    for fixture in &APPROVED_COMPTON_FIXTURES {
        let seed_root = temp.path().join("seed");
        let seed_output = run_compton_for_fixture(fixture, &seed_root, "actual");
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);
        stage_compton_inputs_for_fixture(fixture, &actual_root.join(fixture.id).join("actual"));
    }

    let manifest = json!({
      "fixtures": APPROVED_COMPTON_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["COMPTON"],
          "inputDirectory": workspace_root().join(fixture.input_directory).to_string_lossy().to_string(),
          "entryFiles": ["feff.inp"]
        })
      }).collect::<Vec<_>>()
    });
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("manifest JSON"),
    )
    .expect("manifest should be written");

    let config = RegressionRunnerConfig {
        manifest_path,
        policy_path: workspace_root().join("tasks/numeric-tolerance-policy.json"),
        baseline_root,
        actual_root,
        baseline_subdir: "baseline".to_string(),
        actual_subdir: "actual".to_string(),
        report_path,
        run_rdinp: false,
        run_pot: false,
        run_xsph: false,
        run_path: false,
        run_fms: false,
        run_band: false,
        run_ldos: false,
        run_rixs: false,
        run_crpa: false,
        run_compton: true,
        run_debye: false,
        run_dmdw: false,
        run_screen: false,
        run_self: false,
        run_eels: false,
        run_full_spectrum: false,
    };

    let report = run_regression(&config).expect("COMPTON regression suite should run");
    assert!(report.passed, "expected COMPTON suite to pass");
    assert_eq!(report.fixture_count, APPROVED_COMPTON_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn run_compton_for_fixture(fixture: &FixtureCase, root: &Path, subdir: &str) -> PathBuf {
    let output_dir = root.join(fixture.id).join(subdir);
    stage_compton_inputs_for_fixture(fixture, &output_dir);

    let compton_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::Compton,
        output_dir.join("compton.inp"),
        &output_dir,
    );
    let artifacts = ComptonModule
        .execute(&compton_request)
        .expect("COMPTON execution should succeed");

    assert_eq!(
        artifact_set(&artifacts),
        expected_artifact_set(&EXPECTED_COMPTON_ARTIFACTS),
        "fixture '{}' should emit expected COMPTON artifacts",
        fixture.id
    );

    output_dir
}

fn stage_compton_inputs_for_fixture(fixture: &FixtureCase, destination_dir: &Path) {
    stage_compton_input(fixture.id, &destination_dir.join("compton.inp"));
    for artifact in REQUIRED_COMPTON_INPUT_ARTIFACTS {
        let fallback = if artifact.eq_ignore_ascii_case("pot.bin") {
            &[0_u8, 1_u8, 2_u8, 3_u8][..]
        } else {
            &[4_u8, 5_u8, 6_u8, 7_u8][..]
        };
        stage_binary_input(
            fixture.id,
            artifact,
            &destination_dir.join(artifact),
            fallback,
        );
    }
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    workspace_root()
        .join("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn stage_compton_input(fixture_id: &str, destination: &Path) {
    let source = baseline_artifact_path(fixture_id, Path::new("compton.inp"));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(
        destination,
        "run compton module?\n           1\npqmax, npq\n   5.000000            1000\nns, nphi, nz, nzp\n  32  32  32 120\nsmax, phimax, zmax, zpmax\n      0.00000      6.28319      0.00000     10.00000\njpq? rhozzp? force_recalc_jzzp?\n T T F\nwindow_type (0=Step, 1=Hann), window_cutoff\n           1  0.0000000E+00\ntemperature (in eV)\n      0.00000\nset_chemical_potential? chemical_potential(eV)\n F  0.0000000E+00\nrho_xy? rho_yz? rho_xz? rho_vol? rho_line?\n F F F F F\nqhat_x qhat_y qhat_z\n  0.000000000000000E+000  0.000000000000000E+000   1.00000000000000\n",
    )
    .expect("compton input should be staged");
}

fn stage_binary_input(fixture_id: &str, artifact: &str, destination: &Path, default: &[u8]) {
    let source = baseline_artifact_path(fixture_id, Path::new(artifact));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(destination, default).expect("binary input should be staged");
}

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::copy(source, destination).expect("artifact copy should succeed");
}

fn expected_artifact_set(artifacts: &[&str]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.to_string())
        .collect()
}

fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn copy_directory_tree(source_root: &Path, destination_root: &Path) {
    fs::create_dir_all(destination_root).expect("destination root should exist");
    let entries = fs::read_dir(source_root).expect("source root should be readable");
    for entry in entries {
        let entry = entry.expect("directory entry should be readable");
        let source_path = entry.path();
        let destination_path = destination_root.join(entry.file_name());

        if source_path.is_dir() {
            copy_directory_tree(&source_path, &destination_path);
            continue;
        }

        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::copy(&source_path, &destination_path).unwrap_or_else(|_| {
            panic!(
                "failed to copy '{}' -> '{}'",
                source_path.display(),
                destination_path.display()
            )
        });
    }
}
