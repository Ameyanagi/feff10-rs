use feff10_rs::domain::{PipelineArtifact, PipelineModule, PipelineRequest};
use feff10_rs::pipelines::PipelineExecutor;
use feff10_rs::pipelines::comparator::Comparator;
use feff10_rs::pipelines::regression::{RegressionRunnerConfig, run_regression};
use feff10_rs::pipelines::rixs::RixsPipelineScaffold;
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct FixtureCase {
    id: &'static str,
    input_directory: &'static str,
}

const APPROVED_RIXS_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-RIXS-001",
    input_directory: "feff10/examples/RIXS",
}];

const RIXS_OUTPUT_CANDIDATES: [&str; 10] = [
    "rixs0.dat",
    "rixs1.dat",
    "rixsET.dat",
    "rixsEE.dat",
    "rixsET-sat.dat",
    "rixsEE-sat.dat",
    "logrixs.dat",
    "referenceherfd.dat",
    "referenceherfd-sat.dat",
    "referencerixsET.dat",
];

#[test]
fn approved_rixs_fixtures_match_baseline_under_policy() {
    let comparator = Comparator::from_policy_path("tasks/numeric-tolerance-policy.json")
        .expect("policy should load");

    for fixture in &APPROVED_RIXS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");

        stage_required_rixs_inputs(fixture.id, &output_dir);

        let rixs_request = PipelineRequest::new(
            fixture.id,
            PipelineModule::Rixs,
            output_dir.join("rixs.inp"),
            &output_dir,
        );
        let artifacts = RixsPipelineScaffold
            .execute(&rixs_request)
            .expect("RIXS execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_rixs_artifact_set_for_fixture(fixture.id),
            "artifact contract should match expected RIXS outputs"
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
fn rixs_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("rixs-manifest.json");

    for fixture in &APPROVED_RIXS_FIXTURES {
        for artifact in expected_rixs_artifacts_for_fixture(fixture.id) {
            let baseline_source = baseline_artifact_path(fixture.id, Path::new(&artifact));
            let baseline_target = baseline_root
                .join(fixture.id)
                .join("baseline")
                .join(&artifact);
            copy_file(&baseline_source, &baseline_target);
        }
        let baseline_fixture_dir = baseline_root.join(fixture.id).join("baseline");
        stage_required_rixs_inputs(fixture.id, &baseline_fixture_dir);

        let staged_dir = actual_root.join(fixture.id).join("actual");
        stage_required_rixs_inputs(fixture.id, &staged_dir);
    }

    let manifest = json!({
      "fixtures": APPROVED_RIXS_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["RIXS"],
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
        run_rixs: true,
        run_crpa: false,
        run_compton: false,
        run_debye: false,
    };

    let report = run_regression(&config).expect("RIXS regression suite should run");
    assert!(report.passed, "expected RIXS suite to pass");
    assert_eq!(report.fixture_count, APPROVED_RIXS_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    PathBuf::from("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn expected_rixs_artifact_set_for_fixture(fixture_id: &str) -> BTreeSet<String> {
    let artifacts: BTreeSet<String> = RIXS_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_artifact_path(fixture_id, Path::new(artifact)).is_file())
        .map(|artifact| artifact.to_string())
        .collect();
    assert!(
        !artifacts.is_empty(),
        "fixture '{}' should provide at least one RIXS output artifact",
        fixture_id
    );
    artifacts
}

fn expected_rixs_artifacts_for_fixture(fixture_id: &str) -> Vec<String> {
    expected_rixs_artifact_set_for_fixture(fixture_id)
        .into_iter()
        .collect()
}

fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn stage_required_rixs_inputs(fixture_id: &str, destination_dir: &Path) {
    stage_text_input(
        fixture_id,
        "rixs.inp",
        &destination_dir.join("rixs.inp"),
        "nenergies\n3\nemin emax estep\n-10.0 10.0 0.5\n",
    );
    stage_binary_input(
        fixture_id,
        "phase_1.bin",
        &destination_dir.join("phase_1.bin"),
        &[0_u8, 1_u8, 2_u8, 3_u8],
    );
    stage_binary_input(
        fixture_id,
        "phase_2.bin",
        &destination_dir.join("phase_2.bin"),
        &[4_u8, 5_u8, 6_u8, 7_u8],
    );
    stage_text_input(
        fixture_id,
        "wscrn_1.dat",
        &destination_dir.join("wscrn_1.dat"),
        "0.0 0.0 0.0\n",
    );
    stage_text_input(
        fixture_id,
        "wscrn_2.dat",
        &destination_dir.join("wscrn_2.dat"),
        "0.0 0.0 0.0\n",
    );
    stage_text_input(
        fixture_id,
        "xsect_2.dat",
        &destination_dir.join("xsect_2.dat"),
        "0.0 0.0 0.0\n",
    );
}

fn stage_text_input(fixture_id: &str, artifact: &str, destination: &Path, default: &str) {
    let source = baseline_artifact_path(fixture_id, Path::new(artifact));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(destination, default).expect("text input should be staged");
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
    fs::copy(source, destination).expect("baseline artifact copy should succeed");
}
