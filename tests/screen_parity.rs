use feff10_rs::domain::{PipelineArtifact, PipelineModule, PipelineRequest};
use feff10_rs::pipelines::PipelineExecutor;
use feff10_rs::pipelines::comparator::Comparator;
use feff10_rs::pipelines::regression::{RegressionRunnerConfig, run_regression};
use feff10_rs::pipelines::screen::ScreenPipelineScaffold;
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct FixtureCase {
    id: &'static str,
    input_directory: &'static str,
}

const APPROVED_SCREEN_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-SCREEN-001",
    input_directory: "feff10/examples/MPSE/Cu_OPCONS",
}];

const SCREEN_OUTPUT_CANDIDATES: [&str; 2] = ["wscrn.dat", "logscreen.dat"];
const REQUIRED_SCREEN_INPUT_ARTIFACTS: [&str; 3] = ["pot.inp", "geom.dat", "ldos.inp"];

#[test]
fn approved_screen_fixtures_match_baseline_under_policy() {
    let comparator = Comparator::from_policy_path("tasks/numeric-tolerance-policy.json")
        .expect("policy should load");

    for fixture in &APPROVED_SCREEN_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");

        for artifact in REQUIRED_SCREEN_INPUT_ARTIFACTS {
            copy_file(
                &baseline_artifact_path(fixture.id, Path::new(artifact)),
                &output_dir.join(artifact),
            );
        }
        stage_optional_screen_override(fixture.id, &output_dir.join("screen.inp"));

        let screen_request = PipelineRequest::new(
            fixture.id,
            PipelineModule::Screen,
            output_dir.join("pot.inp"),
            &output_dir,
        );
        let artifacts = ScreenPipelineScaffold
            .execute(&screen_request)
            .expect("SCREEN execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_screen_artifact_set_for_fixture(fixture.id),
            "artifact contract should match expected SCREEN outputs"
        );

        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let baseline_path = baseline_artifact_path(fixture.id, Path::new(&relative_path));
            assert!(
                baseline_path.exists(),
                "baseline artifact '{}' should exist for fixture '{}'",
                baseline_path.display(),
                fixture.id
            );
            let actual_path = output_dir.join(&artifact.relative_path);
            let comparison = comparator
                .compare_artifact(&relative_path, &baseline_path, &actual_path)
                .expect("comparison should succeed");
            assert!(
                comparison.passed,
                "fixture '{}' artifact '{}' failed comparison: {:?}",
                fixture.id, relative_path, comparison.reason
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
        for artifact in expected_screen_artifacts_for_fixture(fixture.id) {
            let baseline_source = baseline_artifact_path(fixture.id, Path::new(&artifact));
            let baseline_target = baseline_root
                .join(fixture.id)
                .join("baseline")
                .join(&artifact);
            copy_file(&baseline_source, &baseline_target);
        }

        let baseline_fixture_dir = baseline_root.join(fixture.id).join("baseline");
        for artifact in REQUIRED_SCREEN_INPUT_ARTIFACTS {
            copy_file(
                &baseline_artifact_path(fixture.id, Path::new(artifact)),
                &baseline_fixture_dir.join(artifact),
            );
        }
        stage_optional_screen_override(fixture.id, &baseline_fixture_dir.join("screen.inp"));

        let staged_dir = actual_root.join(fixture.id).join("actual");
        for artifact in REQUIRED_SCREEN_INPUT_ARTIFACTS {
            copy_file(
                &baseline_artifact_path(fixture.id, Path::new(artifact)),
                &staged_dir.join(artifact),
            );
        }
        stage_optional_screen_override(fixture.id, &staged_dir.join("screen.inp"));
    }

    let manifest = json!({
      "fixtures": APPROVED_SCREEN_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["SCREEN"],
          "inputDirectory": fixture.input_directory,
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
        policy_path: PathBuf::from("tasks/numeric-tolerance-policy.json"),
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
        run_dmdw: false,
        run_screen: true,
        run_self: false,
    };

    let report = run_regression(&config).expect("SCREEN regression suite should run");
    assert!(report.passed, "expected SCREEN suite to pass");
    assert_eq!(report.fixture_count, APPROVED_SCREEN_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    PathBuf::from("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn expected_screen_artifact_set_for_fixture(fixture_id: &str) -> BTreeSet<String> {
    let artifacts: BTreeSet<String> = SCREEN_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_artifact_path(fixture_id, Path::new(artifact)).is_file())
        .map(|artifact| artifact.to_string())
        .collect();
    assert!(
        !artifacts.is_empty(),
        "fixture '{}' should provide at least one SCREEN output artifact",
        fixture_id
    );
    artifacts
}

fn expected_screen_artifacts_for_fixture(fixture_id: &str) -> Vec<String> {
    expected_screen_artifact_set_for_fixture(fixture_id)
        .into_iter()
        .collect()
}

fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn stage_optional_screen_override(fixture_id: &str, destination: &Path) {
    let source = baseline_artifact_path(fixture_id, Path::new("screen.inp"));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(destination, "ioverride: optional screening override\n0\n")
        .expect("screen override input should be staged");
}

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::copy(source, destination).expect("baseline artifact copy should succeed");
}
