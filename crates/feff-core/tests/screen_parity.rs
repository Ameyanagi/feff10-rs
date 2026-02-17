use feff_core::domain::{ComputeArtifact, ComputeModule, ComputeRequest};
use feff_core::modules::ModuleExecutor;
use feff_core::modules::pot::PotModule;
use feff_core::modules::rdinp::RdinpModule;
use feff_core::modules::regression::{RegressionRunnerConfig, run_regression};
use feff_core::modules::screen::ScreenModule;
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

const APPROVED_SCREEN_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-SCREEN-001",
    input_directory: "feff10/examples/MPSE/Cu_OPCONS",
}];

const REQUIRED_SCREEN_INPUT_ARTIFACTS: [&str; 3] = ["pot.inp", "geom.dat", "ldos.inp"];
const EXPECTED_POT_ARTIFACTS: [&str; 5] = [
    "pot.bin",
    "pot.dat",
    "log1.dat",
    "convergence.scf",
    "convergence.scf.fine",
];
const EXPECTED_SCREEN_ARTIFACTS: [&str; 2] = ["wscrn.dat", "logscreen.dat"];

#[test]
fn approved_screen_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_SCREEN_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = run_rdinp_pot_and_screen_for_fixture(fixture, temp.path(), "actual", true);

        for artifact in &EXPECTED_SCREEN_ARTIFACTS {
            let output_path = output_dir.join(artifact);
            assert!(
                output_path.is_file(),
                "SCREEN artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "SCREEN artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }
}

#[test]
fn approved_screen_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_SCREEN_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_output =
            run_rdinp_pot_and_screen_for_fixture(fixture, temp.path(), "first", true);
        let second_output =
            run_rdinp_pot_and_screen_for_fixture(fixture, temp.path(), "second", true);

        for artifact in &EXPECTED_SCREEN_ARTIFACTS {
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
fn screen_module_supports_missing_optional_screen_input() {
    for fixture in &APPROVED_SCREEN_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = run_rdinp_pot_and_screen_for_fixture(
            fixture,
            temp.path(),
            "without-screen-override",
            false,
        );

        for artifact in &EXPECTED_SCREEN_ARTIFACTS {
            assert!(
                output_dir.join(artifact).is_file(),
                "fixture '{}' should still produce '{}' without optional screen.inp",
                fixture.id,
                artifact
            );
        }
    }
}

#[test]
fn screen_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("screen-manifest.json");

    for fixture in &APPROVED_SCREEN_FIXTURES {
        let seed_root = temp.path().join("seed");
        let seed_output = run_rdinp_pot_and_screen_for_fixture(fixture, &seed_root, "actual", true);
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);
    }

    let manifest = json!({
      "fixtures": APPROVED_SCREEN_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["RDINP", "POT", "SCREEN"],
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
        run_rdinp: true,
        run_pot: true,
        run_xsph: false,
        run_path: false,
        run_fms: false,
        run_band: false,
        run_ldos: false,
        run_rixs: false,
        run_crpa: false,
        run_compton: false,
        run_debye: false,
        run_dmdw: false,
        run_screen: true,
        run_self: false,
        run_eels: false,
        run_full_spectrum: false,
    };

    let report = run_regression(&config).expect("SCREEN regression suite should run");
    assert!(report.passed, "expected SCREEN suite to pass");
    assert_eq!(report.fixture_count, APPROVED_SCREEN_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn run_rdinp_pot_and_screen_for_fixture(
    fixture: &FixtureCase,
    root: &Path,
    subdir: &str,
    include_screen_override: bool,
) -> PathBuf {
    let output_dir = root.join(fixture.id).join(subdir);
    let rdinp_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::Rdinp,
        workspace_root().join(fixture.input_directory).join("feff.inp"),
        &output_dir,
    );
    let rdinp_artifacts = RdinpModule
        .execute(&rdinp_request)
        .expect("RDINP execution should succeed");

    let rdinp_set = artifact_set(&rdinp_artifacts);
    for artifact in REQUIRED_SCREEN_INPUT_ARTIFACTS {
        assert!(
            rdinp_set.contains(artifact),
            "fixture '{}' should include '{}' before POT/SCREEN execution",
            fixture.id,
            artifact
        );
    }

    let pot_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::Pot,
        output_dir.join("pot.inp"),
        &output_dir,
    );
    let pot_artifacts = PotModule
        .execute(&pot_request)
        .expect("POT execution should succeed");
    assert_eq!(
        artifact_set(&pot_artifacts),
        expected_artifact_set(&EXPECTED_POT_ARTIFACTS),
        "fixture '{}' should emit expected POT artifacts before SCREEN execution",
        fixture.id
    );

    if !include_screen_override {
        let screen_override_path = output_dir.join("screen.inp");
        if screen_override_path.is_file() {
            fs::remove_file(&screen_override_path).unwrap_or_else(|_| {
                panic!(
                    "failed to remove optional screen override '{}'",
                    screen_override_path.display()
                )
            });
        }
    }

    let screen_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::Screen,
        output_dir.join("pot.inp"),
        &output_dir,
    );
    let screen_artifacts = ScreenModule
        .execute(&screen_request)
        .expect("SCREEN execution should succeed");
    assert_eq!(
        artifact_set(&screen_artifacts),
        expected_artifact_set(&EXPECTED_SCREEN_ARTIFACTS),
        "fixture '{}' should emit expected SCREEN artifacts",
        fixture.id
    );

    output_dir
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
