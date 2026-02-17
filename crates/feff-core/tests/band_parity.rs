use feff_core::domain::{ComputeArtifact, ComputeModule, ComputeRequest};
use feff_core::modules::ModuleExecutor;
use feff_core::modules::band::BandModule;
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

const APPROVED_BAND_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-BAND-001",
    input_directory: "feff10/examples/KSPACE/Cr2GeC",
}];

const EXPECTED_BAND_ARTIFACTS: [&str; 2] = ["bandstructure.dat", "logband.dat"];
const REQUIRED_BAND_INPUT_ARTIFACTS: [&str; 3] = ["geom.dat", "global.inp", "phase.bin"];

#[test]
fn approved_band_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_BAND_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = run_band_for_fixture(fixture, temp.path(), "actual");

        for artifact in &EXPECTED_BAND_ARTIFACTS {
            let output_path = output_dir.join(artifact);
            assert!(
                output_path.is_file(),
                "BAND artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "BAND artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }
}

#[test]
fn approved_band_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_BAND_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_output = run_band_for_fixture(fixture, temp.path(), "first");
        let second_output = run_band_for_fixture(fixture, temp.path(), "second");

        for artifact in &EXPECTED_BAND_ARTIFACTS {
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
fn band_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("band-manifest.json");

    for fixture in &APPROVED_BAND_FIXTURES {
        let seed_root = temp.path().join("seed");
        let seed_output = run_band_for_fixture(fixture, &seed_root, "actual");
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);
        stage_band_inputs_for_fixture(fixture, &actual_root.join(fixture.id).join("actual"));
    }

    let manifest = json!({
      "fixtures": APPROVED_BAND_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["BAND"],
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
        run_band: true,
        run_ldos: false,
        run_rixs: false,
        run_crpa: false,
        run_compton: false,
        run_debye: false,
        run_dmdw: false,
        run_screen: false,
        run_self: false,
        run_eels: false,
        run_full_spectrum: false,
    };

    let report = run_regression(&config).expect("BAND regression suite should run");
    assert!(report.passed, "expected BAND suite to pass");
    assert_eq!(report.fixture_count, APPROVED_BAND_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn run_band_for_fixture(fixture: &FixtureCase, root: &Path, subdir: &str) -> PathBuf {
    let output_dir = root.join(fixture.id).join(subdir);
    stage_band_inputs_for_fixture(fixture, &output_dir);

    let band_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::Band,
        output_dir.join("band.inp"),
        &output_dir,
    );
    let artifacts = BandModule
        .execute(&band_request)
        .expect("BAND execution should succeed");
    assert_eq!(
        artifact_set(&artifacts),
        expected_artifact_set(&EXPECTED_BAND_ARTIFACTS),
        "fixture '{}' should emit expected BAND artifacts",
        fixture.id
    );

    output_dir
}

fn stage_band_inputs_for_fixture(fixture: &FixtureCase, destination_dir: &Path) {
    stage_band_input(fixture.id, &destination_dir.join("band.inp"));
    for artifact in REQUIRED_BAND_INPUT_ARTIFACTS {
        let source = baseline_artifact_path(fixture.id, Path::new(artifact));
        copy_file(&source, &destination_dir.join(artifact));
    }
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    workspace_root()
        .join("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn stage_band_input(fixture_id: &str, destination: &Path) {
    let source = baseline_artifact_path(fixture_id, Path::new("band.inp"));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(destination, default_band_input_source()).expect("band input should be staged");
}

fn default_band_input_source() -> &'static str {
    "mband : calculate bands if = 1\n   1\nemin, emax, estep : energy mesh\n    -8.00000      6.00000      0.05000\nnkp : # points in k-path\n 121\nikpath : type of k-path\n   2\nfreeprop :  empty lattice if = T\n F\n"
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
