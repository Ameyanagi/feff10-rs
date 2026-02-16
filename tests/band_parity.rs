use feff10_rs::domain::{PipelineArtifact, PipelineModule, PipelineRequest};
use feff10_rs::pipelines::PipelineExecutor;
use feff10_rs::pipelines::band::BandPipelineScaffold;
use feff10_rs::pipelines::comparator::Comparator;
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

const APPROVED_BAND_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-BAND-001",
    input_directory: "feff10/examples/KSPACE/Cr2GeC",
}];

const BAND_OUTPUT_CANDIDATES: [&str; 4] =
    ["bandstructure.dat", "logband.dat", "list.dat", "log5.dat"];
const REQUIRED_BAND_INPUT_ARTIFACTS: [&str; 3] = ["geom.dat", "global.inp", "phase.bin"];

#[test]
fn approved_band_fixtures_match_baseline_under_policy() {
    let comparator = Comparator::from_policy_path("tasks/numeric-tolerance-policy.json")
        .expect("policy should load");

    for fixture in &APPROVED_BAND_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");

        stage_band_input(fixture.id, &output_dir.join("band.inp"));
        for artifact in REQUIRED_BAND_INPUT_ARTIFACTS {
            copy_file(
                &baseline_artifact_path(fixture.id, Path::new(artifact)),
                &output_dir.join(artifact),
            );
        }

        let band_request = PipelineRequest::new(
            fixture.id,
            PipelineModule::Band,
            output_dir.join("band.inp"),
            &output_dir,
        );
        let artifacts = BandPipelineScaffold
            .execute(&band_request)
            .expect("BAND execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_band_artifact_set_for_fixture(fixture.id),
            "artifact contract should match expected BAND outputs"
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
fn band_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("band-manifest.json");

    for fixture in &APPROVED_BAND_FIXTURES {
        for artifact in expected_band_artifacts_for_fixture(fixture.id) {
            let baseline_source = baseline_artifact_path(fixture.id, Path::new(&artifact));
            let baseline_target = baseline_root
                .join(fixture.id)
                .join("baseline")
                .join(&artifact);
            copy_file(&baseline_source, &baseline_target);
        }
        let baseline_fixture_dir = baseline_root.join(fixture.id).join("baseline");
        stage_band_input(fixture.id, &baseline_fixture_dir.join("band.inp"));
        for artifact in REQUIRED_BAND_INPUT_ARTIFACTS {
            copy_file(
                &baseline_artifact_path(fixture.id, Path::new(artifact)),
                &baseline_fixture_dir.join(artifact),
            );
        }

        let staged_dir = actual_root.join(fixture.id).join("actual");
        stage_band_input(fixture.id, &staged_dir.join("band.inp"));
        copy_file(
            &baseline_artifact_path(fixture.id, Path::new("geom.dat")),
            &staged_dir.join("geom.dat"),
        );
        copy_file(
            &baseline_artifact_path(fixture.id, Path::new("global.inp")),
            &staged_dir.join("global.inp"),
        );
        copy_file(
            &baseline_artifact_path(fixture.id, Path::new("phase.bin")),
            &staged_dir.join("phase.bin"),
        );
    }

    let manifest = json!({
      "fixtures": APPROVED_BAND_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["BAND"],
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
        run_band: true,
        run_ldos: false,
        run_rixs: false,
        run_crpa: false,
        run_compton: false,
        run_debye: false,
        run_dmdw: false,
        run_screen: false,
        run_self: false,
    };

    let report = run_regression(&config).expect("BAND regression suite should run");
    assert!(report.passed, "expected BAND suite to pass");
    assert_eq!(report.fixture_count, APPROVED_BAND_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    PathBuf::from("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn expected_band_artifact_set_for_fixture(fixture_id: &str) -> BTreeSet<String> {
    let artifacts: BTreeSet<String> = BAND_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_artifact_path(fixture_id, Path::new(artifact)).is_file())
        .map(|artifact| artifact.to_string())
        .collect();
    assert!(
        !artifacts.is_empty(),
        "fixture '{}' should provide at least one BAND output artifact",
        fixture_id
    );
    artifacts
}

fn expected_band_artifacts_for_fixture(fixture_id: &str) -> Vec<String> {
    expected_band_artifact_set_for_fixture(fixture_id)
        .into_iter()
        .collect()
}

fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
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
    fs::write(
        destination,
        "mband : calculate bands if = 1\n   0\nemin, emax, estep : energy mesh\n      0.00000      0.00000      0.00000\nnkp : # points in k-path\n   0\nikpath : type of k-path\n  -1\nfreeprop :  empty lattice if = T\n F\n",
    )
    .expect("band input should be staged");
}

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::copy(source, destination).expect("baseline artifact copy should succeed");
}
