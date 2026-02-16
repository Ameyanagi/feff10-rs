use feff10_rs::domain::{PipelineArtifact, PipelineModule, PipelineRequest};
use feff10_rs::pipelines::PipelineExecutor;
use feff10_rs::pipelines::comparator::Comparator;
use feff10_rs::pipelines::eels::EelsPipelineScaffold;
use feff10_rs::pipelines::regression::{RegressionRunnerConfig, run_regression};
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct FixtureCase {
    id: &'static str,
    input_directory: &'static str,
}

const APPROVED_EELS_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-EELS-001",
    input_directory: "feff10/examples/ELNES/Cu",
}];

const EELS_OUTPUT_CANDIDATES: [&str; 4] =
    ["eels.dat", "logeels.dat", "magic.dat", "reference_eels.dat"];

#[test]
fn approved_eels_fixtures_match_baseline_under_policy() {
    let comparator = Comparator::from_policy_path("tasks/numeric-tolerance-policy.json")
        .expect("policy should load");

    for fixture in &APPROVED_EELS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");

        stage_eels_inputs(fixture.id, &output_dir);

        let eels_request = PipelineRequest::new(
            fixture.id,
            PipelineModule::Eels,
            output_dir.join("eels.inp"),
            &output_dir,
        );
        let artifacts = EelsPipelineScaffold
            .execute(&eels_request)
            .expect("EELS execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_eels_artifact_set_for_fixture(fixture.id),
            "artifact contract should match expected EELS outputs"
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
fn eels_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("eels-manifest.json");

    for fixture in &APPROVED_EELS_FIXTURES {
        for artifact in expected_eels_artifacts_for_fixture(fixture.id) {
            let baseline_source = baseline_artifact_path(fixture.id, Path::new(&artifact));
            let baseline_target = baseline_root
                .join(fixture.id)
                .join("baseline")
                .join(&artifact);
            copy_file(&baseline_source, &baseline_target);
        }

        let baseline_fixture_dir = baseline_root.join(fixture.id).join("baseline");
        stage_eels_inputs(fixture.id, &baseline_fixture_dir);

        let staged_dir = actual_root.join(fixture.id).join("actual");
        stage_eels_inputs(fixture.id, &staged_dir);
    }

    let manifest = json!({
      "fixtures": APPROVED_EELS_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["EELS"],
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
        run_screen: false,
        run_self: false,
        run_eels: true,
        run_full_spectrum: false,
    };

    let report = run_regression(&config).expect("EELS regression suite should run");
    assert!(report.passed, "expected EELS suite to pass");
    assert_eq!(report.fixture_count, APPROVED_EELS_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    PathBuf::from("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn expected_eels_artifact_set_for_fixture(fixture_id: &str) -> BTreeSet<String> {
    let artifacts: BTreeSet<String> = EELS_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_artifact_path(fixture_id, Path::new(artifact)).is_file())
        .map(|artifact| artifact.to_string())
        .collect();

    assert!(
        !artifacts.is_empty(),
        "fixture '{}' should provide at least one EELS output artifact",
        fixture_id
    );
    artifacts
}

fn expected_eels_artifacts_for_fixture(fixture_id: &str) -> Vec<String> {
    expected_eels_artifact_set_for_fixture(fixture_id)
        .into_iter()
        .collect()
}

fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn stage_eels_inputs(fixture_id: &str, destination_dir: &Path) {
    copy_file(
        &baseline_artifact_path(fixture_id, Path::new("eels.inp")),
        &destination_dir.join("eels.inp"),
    );
    copy_file(
        &baseline_artifact_path(fixture_id, Path::new("xmu.dat")),
        &destination_dir.join("xmu.dat"),
    );

    let optional_magic_source = baseline_artifact_path(fixture_id, Path::new("magic.inp"));
    if optional_magic_source.is_file() {
        copy_file(&optional_magic_source, &destination_dir.join("magic.inp"));
    }
}

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::copy(source, destination).expect("baseline artifact copy should succeed");
}
