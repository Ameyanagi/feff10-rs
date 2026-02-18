use feff_core::domain::{ComputeArtifact, ComputeModule, ComputeRequest};
use feff_core::modules::ModuleExecutor;
use feff_core::modules::crpa::CrpaModule;
use feff_core::modules::regression::{RegressionRunnerConfig, run_regression};
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

struct FixtureCase {
    id: &'static str,
    input_directory: &'static str,
}

const APPROVED_CRPA_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-CRPA-001",
    input_directory: "feff10/examples/CRPA",
}];

const REQUIRED_CRPA_INPUT_ARTIFACTS: [&str; 3] = ["crpa.inp", "pot.inp", "geom.dat"];
const EXPECTED_CRPA_ARTIFACTS: [&str; 2] = ["wscrn.dat", "logscrn.dat"];

#[test]
fn approved_crpa_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_CRPA_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = run_crpa_for_fixture(fixture, temp.path(), "actual");

        for artifact in &EXPECTED_CRPA_ARTIFACTS {
            let output_path = output_dir.join(artifact);
            assert!(
                output_path.is_file(),
                "CRPA artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "CRPA artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }
}

#[test]
fn approved_crpa_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_CRPA_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_output = run_crpa_for_fixture(fixture, temp.path(), "first");
        let second_output = run_crpa_for_fixture(fixture, temp.path(), "second");

        for artifact in &EXPECTED_CRPA_ARTIFACTS {
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
fn crpa_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("crpa-manifest.json");

    for fixture in &APPROVED_CRPA_FIXTURES {
        let seed_root = temp.path().join("seed");
        let seed_output = run_crpa_for_fixture(fixture, &seed_root, "actual");
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);

        let staged_dir = actual_root.join(fixture.id).join("actual");
        stage_required_crpa_inputs_for_fixture(fixture.id, &staged_dir);
    }

    let manifest = json!({
      "fixtures": APPROVED_CRPA_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["CRPA"],
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
        run_crpa: true,
        run_compton: false,
        run_debye: false,
        run_dmdw: false,
        run_screen: false,
        run_self: false,
        run_eels: false,
        run_full_spectrum: false,
    };

    let report = run_regression(&config).expect("CRPA regression suite should run");
    assert!(report.passed, "expected CRPA suite to pass");
    assert_eq!(report.fixture_count, APPROVED_CRPA_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn run_crpa_for_fixture(fixture: &FixtureCase, root: &Path, subdir: &str) -> PathBuf {
    let output_dir = root.join(fixture.id).join(subdir);
    stage_required_crpa_inputs_for_fixture(fixture.id, &output_dir);

    let crpa_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::Crpa,
        output_dir.join("crpa.inp"),
        &output_dir,
    );
    let crpa_artifacts = CrpaModule
        .execute(&crpa_request)
        .expect("CRPA execution should succeed");
    assert_eq!(
        artifact_set(&crpa_artifacts),
        expected_artifact_set(&EXPECTED_CRPA_ARTIFACTS),
        "fixture '{}' should emit expected CRPA artifacts",
        fixture.id
    );

    output_dir
}

fn stage_required_crpa_inputs_for_fixture(fixture_id: &str, output_dir: &Path) {
    for artifact in REQUIRED_CRPA_INPUT_ARTIFACTS {
        copy_file(
            &baseline_artifact_path(fixture_id, Path::new(artifact)),
            &output_dir.join(artifact),
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

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::copy(source, destination).expect("artifact copy should succeed");
}
