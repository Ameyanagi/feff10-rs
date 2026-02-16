use feff10_rs::domain::{PipelineArtifact, PipelineModule, PipelineRequest};
use feff10_rs::pipelines::PipelineExecutor;
use feff10_rs::pipelines::comparator::Comparator;
use feff10_rs::pipelines::ldos::LdosPipelineScaffold;
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

const APPROVED_LDOS_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-LDOS-001",
    input_directory: "feff10/examples/HUBBARD/CeO2",
}];

const REQUIRED_LDOS_INPUT_ARTIFACTS: [&str; 3] = ["geom.dat", "pot.bin", "reciprocal.inp"];

#[test]
fn approved_ldos_fixtures_match_baseline_under_policy() {
    let comparator = Comparator::from_policy_path("tasks/numeric-tolerance-policy.json")
        .expect("policy should load");

    for fixture in &APPROVED_LDOS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");

        stage_ldos_input(fixture.id, &output_dir.join("ldos.inp"));
        for artifact in REQUIRED_LDOS_INPUT_ARTIFACTS {
            copy_file(
                &baseline_artifact_path(fixture.id, Path::new(artifact)),
                &output_dir.join(artifact),
            );
        }

        let ldos_request = PipelineRequest::new(
            fixture.id,
            PipelineModule::Ldos,
            output_dir.join("ldos.inp"),
            &output_dir,
        );
        let artifacts = LdosPipelineScaffold
            .execute(&ldos_request)
            .expect("LDOS execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_ldos_artifact_set_for_fixture(fixture.id),
            "artifact contract should match expected LDOS outputs"
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
fn ldos_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("ldos-manifest.json");

    for fixture in &APPROVED_LDOS_FIXTURES {
        for artifact in expected_ldos_artifacts_for_fixture(fixture.id) {
            let baseline_source = baseline_artifact_path(fixture.id, Path::new(&artifact));
            let baseline_target = baseline_root
                .join(fixture.id)
                .join("baseline")
                .join(&artifact);
            copy_file(&baseline_source, &baseline_target);
        }
        let baseline_fixture_dir = baseline_root.join(fixture.id).join("baseline");
        stage_ldos_input(fixture.id, &baseline_fixture_dir.join("ldos.inp"));
        for artifact in REQUIRED_LDOS_INPUT_ARTIFACTS {
            copy_file(
                &baseline_artifact_path(fixture.id, Path::new(artifact)),
                &baseline_fixture_dir.join(artifact),
            );
        }

        let staged_dir = actual_root.join(fixture.id).join("actual");
        stage_ldos_input(fixture.id, &staged_dir.join("ldos.inp"));
        for artifact in REQUIRED_LDOS_INPUT_ARTIFACTS {
            copy_file(
                &baseline_artifact_path(fixture.id, Path::new(artifact)),
                &staged_dir.join(artifact),
            );
        }
    }

    let manifest = json!({
      "fixtures": APPROVED_LDOS_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["LDOS"],
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
        run_ldos: true,
        run_rixs: false,
        run_crpa: false,
    };

    let report = run_regression(&config).expect("LDOS regression suite should run");
    assert!(report.passed, "expected LDOS suite to pass");
    assert_eq!(report.fixture_count, APPROVED_LDOS_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    baseline_fixture_dir(fixture_id).join(relative_path)
}

fn baseline_fixture_dir(fixture_id: &str) -> PathBuf {
    PathBuf::from("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
}

fn expected_ldos_artifact_set_for_fixture(fixture_id: &str) -> BTreeSet<String> {
    let baseline_dir = baseline_fixture_dir(fixture_id);
    let entries = fs::read_dir(&baseline_dir).expect("baseline directory should be readable");

    let mut artifacts = BTreeSet::new();
    for entry in entries {
        let entry = entry.expect("directory entry should be readable");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        if is_ldos_output_file_name(name) {
            artifacts.insert(name.to_string());
        }
    }

    assert!(
        !artifacts.is_empty(),
        "fixture '{}' should provide at least one LDOS output artifact",
        fixture_id
    );

    artifacts
}

fn expected_ldos_artifacts_for_fixture(fixture_id: &str) -> Vec<String> {
    expected_ldos_artifact_set_for_fixture(fixture_id)
        .into_iter()
        .collect()
}

fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn stage_ldos_input(fixture_id: &str, destination: &Path) {
    copy_file(
        &baseline_artifact_path(fixture_id, Path::new("ldos.inp")),
        destination,
    );
}

fn is_ldos_output_file_name(file_name: &str) -> bool {
    if file_name.eq_ignore_ascii_case("logdos.dat") {
        return true;
    }

    let lowered = file_name.to_ascii_lowercase();
    lowered.starts_with("ldos") && lowered.ends_with(".dat")
}

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::copy(source, destination).expect("baseline artifact copy should succeed");
}
