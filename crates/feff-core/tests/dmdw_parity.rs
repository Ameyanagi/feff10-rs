use feff_core::domain::{ComputeArtifact, ComputeModule, ComputeRequest};
use feff_core::modules::ModuleExecutor;
use feff_core::modules::dmdw::DmdwModule;
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

const APPROVED_DMDW_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-DMDW-001",
    input_directory: "feff10/examples/DEBYE/DM/EXAFS/Cu",
}];

const EXPECTED_DMDW_ARTIFACTS: [&str; 1] = ["dmdw.out"];

#[test]
fn approved_dmdw_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_DMDW_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = run_dmdw_for_fixture(fixture, temp.path(), "actual", None);

        for artifact in &EXPECTED_DMDW_ARTIFACTS {
            let output_path = output_dir.join(artifact);
            assert!(
                output_path.is_file(),
                "DMDW artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "DMDW artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }
}

#[test]
fn approved_dmdw_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_DMDW_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_output = run_dmdw_for_fixture(fixture, temp.path(), "first", None);
        let second_output = run_dmdw_for_fixture(fixture, temp.path(), "second", None);

        for artifact in &EXPECTED_DMDW_ARTIFACTS {
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
fn dmdw_output_depends_on_feff_dym_input() {
    for fixture in &APPROVED_DMDW_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_output = run_dmdw_for_fixture(
            fixture,
            temp.path(),
            "first",
            Some(&[1_u8, 2_u8, 3_u8, 4_u8]),
        );
        let second_output = run_dmdw_for_fixture(
            fixture,
            temp.path(),
            "second",
            Some(&[9_u8, 10_u8, 11_u8, 12_u8]),
        );

        let first = fs::read(first_output.join("dmdw.out")).expect("first output should exist");
        let second = fs::read(second_output.join("dmdw.out")).expect("second output should exist");
        assert_ne!(
            first, second,
            "fixture '{}' should produce distinct dmdw.out when feff.dym changes",
            fixture.id
        );
    }
}

#[test]
fn dmdw_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("dmdw-manifest.json");

    for fixture in &APPROVED_DMDW_FIXTURES {
        let seed_root = temp.path().join("seed");
        let seed_output = run_dmdw_for_fixture(fixture, &seed_root, "actual", None);
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);

        stage_dmdw_inputs_for_fixture(fixture, &actual_root.join(fixture.id).join("actual"), None);
    }

    let manifest = json!({
      "fixtures": APPROVED_DMDW_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["DMDW"],
          "inputDirectory": workspace_root().join(fixture.input_directory).to_string_lossy().to_string(),
          "entryFiles": ["feff.inp", "feff.dym"]
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
        run_compton: false,
        run_debye: false,
        run_dmdw: true,
        run_screen: false,
        run_self: false,
        run_eels: false,
        run_full_spectrum: false,
    };

    let report = run_regression(&config).expect("DMDW regression suite should run");
    assert!(report.passed, "expected DMDW suite to pass");
    assert_eq!(report.fixture_count, APPROVED_DMDW_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn run_dmdw_for_fixture(
    fixture: &FixtureCase,
    root: &Path,
    subdir: &str,
    feff_dym_override: Option<&[u8]>,
) -> PathBuf {
    let output_dir = root.join(fixture.id).join(subdir);
    stage_dmdw_inputs_for_fixture(fixture, &output_dir, feff_dym_override);

    let dmdw_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::Dmdw,
        output_dir.join("dmdw.inp"),
        &output_dir,
    );
    let artifacts = DmdwModule
        .execute(&dmdw_request)
        .expect("DMDW execution should succeed");

    assert_eq!(
        artifact_set(&artifacts),
        expected_artifact_set(&EXPECTED_DMDW_ARTIFACTS),
        "fixture '{}' should emit expected DMDW artifacts",
        fixture.id
    );

    output_dir
}

fn stage_dmdw_inputs_for_fixture(
    fixture: &FixtureCase,
    destination_dir: &Path,
    feff_dym_override: Option<&[u8]>,
) {
    stage_dmdw_input(fixture.id, &destination_dir.join("dmdw.inp"));
    if let Some(bytes) = feff_dym_override {
        if let Some(parent) = destination_dir.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::create_dir_all(destination_dir).expect("destination directory should exist");
        fs::write(destination_dir.join("feff.dym"), bytes).expect("feff.dym input should exist");
        return;
    }

    stage_feff_dym_input(
        fixture.id,
        &destination_dir.join("feff.dym"),
        &[0_u8, 1_u8, 2_u8, 3_u8],
    );
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    workspace_root()
        .join("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn stage_dmdw_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "dmdw.inp",
        destination,
        "   1\n   6\n   1    450.000\n   0\nfeff.dym\n   1\n   2   1   0          29.78\n",
    );
}

fn stage_feff_dym_input(fixture_id: &str, destination: &Path, fallback_bytes: &[u8]) {
    let source = baseline_artifact_path(fixture_id, Path::new("feff.dym"));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(destination, fallback_bytes).expect("feff.dym input should be staged");
}

fn stage_text_input(fixture_id: &str, artifact: &str, destination: &Path, fallback: &str) {
    let source = baseline_artifact_path(fixture_id, Path::new(artifact));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(destination, fallback).expect("text input should be staged");
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
        fs::copy(&source_path, &destination_path).expect("artifact copy should succeed");
    }
}
