use feff10_rs::domain::{PipelineArtifact, PipelineModule, PipelineRequest};
use feff10_rs::pipelines::PipelineExecutor;
use feff10_rs::pipelines::comparator::Comparator;
use feff10_rs::pipelines::fullspectrum::FullSpectrumPipelineScaffold;
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

const APPROVED_FULLSPECTRUM_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-FULLSPECTRUM-001",
    input_directory: "feff10/examples/XES/Cu",
}];

const FULLSPECTRUM_OUTPUT_CANDIDATES: [&str; 9] = [
    "xmu.dat",
    "osc_str.dat",
    "eps.dat",
    "drude.dat",
    "background.dat",
    "fine_st.dat",
    "logfullspectrum.dat",
    "prexmu.dat",
    "referencexmu.dat",
];

#[test]
fn approved_fullspectrum_fixtures_match_baseline_under_policy() {
    let comparator = Comparator::from_policy_path("tasks/numeric-tolerance-policy.json")
        .expect("policy should load");

    for fixture in &APPROVED_FULLSPECTRUM_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");

        stage_fullspectrum_inputs(fixture.id, &output_dir);

        let fullspectrum_request = PipelineRequest::new(
            fixture.id,
            PipelineModule::FullSpectrum,
            output_dir.join("fullspectrum.inp"),
            &output_dir,
        );
        let artifacts = FullSpectrumPipelineScaffold
            .execute(&fullspectrum_request)
            .expect("FULLSPECTRUM execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_fullspectrum_artifact_set_for_fixture(fixture.id),
            "artifact contract should match expected FULLSPECTRUM outputs"
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
fn fullspectrum_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("fullspectrum-manifest.json");

    for fixture in &APPROVED_FULLSPECTRUM_FIXTURES {
        for artifact in expected_fullspectrum_artifacts_for_fixture(fixture.id) {
            let baseline_source = baseline_artifact_path(fixture.id, Path::new(&artifact));
            let baseline_target = baseline_root
                .join(fixture.id)
                .join("baseline")
                .join(&artifact);
            copy_file(&baseline_source, &baseline_target);
        }

        let baseline_fixture_dir = baseline_root.join(fixture.id).join("baseline");
        stage_fullspectrum_inputs(fixture.id, &baseline_fixture_dir);

        let staged_dir = actual_root.join(fixture.id).join("actual");
        stage_fullspectrum_inputs(fixture.id, &staged_dir);
    }

    let manifest = json!({
      "fixtures": APPROVED_FULLSPECTRUM_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["FULLSPECTRUM"],
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
        run_eels: false,
        run_full_spectrum: true,
    };

    let report = run_regression(&config).expect("FULLSPECTRUM regression suite should run");
    assert!(report.passed, "expected FULLSPECTRUM suite to pass");
    assert_eq!(report.fixture_count, APPROVED_FULLSPECTRUM_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    PathBuf::from("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn expected_fullspectrum_artifact_set_for_fixture(fixture_id: &str) -> BTreeSet<String> {
    let artifacts: BTreeSet<String> = FULLSPECTRUM_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_artifact_path(fixture_id, Path::new(artifact)).is_file())
        .map(|artifact| artifact.to_string())
        .collect();

    assert!(
        !artifacts.is_empty(),
        "fixture '{}' should provide at least one FULLSPECTRUM output artifact",
        fixture_id
    );
    artifacts
}

fn expected_fullspectrum_artifacts_for_fixture(fixture_id: &str) -> Vec<String> {
    expected_fullspectrum_artifact_set_for_fixture(fixture_id)
        .into_iter()
        .collect()
}

fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn stage_fullspectrum_inputs(fixture_id: &str, destination_dir: &Path) {
    copy_file(
        &baseline_artifact_path(fixture_id, Path::new("fullspectrum.inp")),
        &destination_dir.join("fullspectrum.inp"),
    );
    copy_file(
        &baseline_artifact_path(fixture_id, Path::new("xmu.dat")),
        &destination_dir.join("xmu.dat"),
    );

    for optional_artifact in ["prexmu.dat", "referencexmu.dat"] {
        let optional_source = baseline_artifact_path(fixture_id, Path::new(optional_artifact));
        if optional_source.is_file() {
            copy_file(&optional_source, &destination_dir.join(optional_artifact));
        }
    }
}

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::copy(source, destination).expect("baseline artifact copy should succeed");
}
