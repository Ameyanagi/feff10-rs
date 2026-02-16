use feff10_rs::domain::{PipelineArtifact, PipelineModule, PipelineRequest};
use feff10_rs::pipelines::PipelineExecutor;
use feff10_rs::pipelines::comparator::Comparator;
use feff10_rs::pipelines::pot::PotPipelineScaffold;
use feff10_rs::pipelines::rdinp::RdinpPipelineScaffold;
use feff10_rs::pipelines::regression::{RegressionRunnerConfig, run_regression};
use feff10_rs::pipelines::xsph::XsphPipelineScaffold;
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct FixtureCase {
    id: &'static str,
    input_directory: &'static str,
}

const APPROVED_XSPH_FIXTURES: [FixtureCase; 2] = [
    FixtureCase {
        id: "FX-XSPH-001",
        input_directory: "feff10/examples/XANES/Cu",
    },
    FixtureCase {
        id: "FX-WORKFLOW-XAS-001",
        input_directory: "feff10/examples/XANES/Cu",
    },
];

const EXPECTED_RDINP_ARTIFACTS: [&str; 14] = [
    "geom.dat",
    "global.inp",
    "reciprocal.inp",
    "pot.inp",
    "ldos.inp",
    "xsph.inp",
    "fms.inp",
    "paths.inp",
    "genfmt.inp",
    "ff2x.inp",
    "sfconv.inp",
    "eels.inp",
    "dmdw.inp",
    "log.dat",
];

const EXPECTED_POT_ARTIFACTS: [&str; 2] = ["pot.bin", "log1.dat"];
const EXPECTED_XSPH_ARTIFACTS: [&str; 3] = ["phase.bin", "xsect.dat", "log2.dat"];

#[test]
fn approved_xsph_fixtures_match_baseline_under_policy() {
    let comparator = Comparator::from_policy_path("tasks/numeric-tolerance-policy.json")
        .expect("policy should load");

    for fixture in &APPROVED_XSPH_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");

        let rdinp_request = PipelineRequest::new(
            fixture.id,
            PipelineModule::Rdinp,
            Path::new(fixture.input_directory).join("feff.inp"),
            &output_dir,
        );
        RdinpPipelineScaffold
            .execute(&rdinp_request)
            .expect("RDINP execution should succeed");

        let pot_request = PipelineRequest::new(
            fixture.id,
            PipelineModule::Pot,
            output_dir.join("pot.inp"),
            &output_dir,
        );
        PotPipelineScaffold
            .execute(&pot_request)
            .expect("POT execution should succeed");

        let xsph_request = PipelineRequest::new(
            fixture.id,
            PipelineModule::Xsph,
            output_dir.join("xsph.inp"),
            &output_dir,
        );
        let artifacts = XsphPipelineScaffold
            .execute(&xsph_request)
            .expect("XSPH execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&EXPECTED_XSPH_ARTIFACTS),
            "artifact contract should match expected XSPH outputs"
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
fn xsph_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("xsph-manifest.json");

    for fixture in &APPROVED_XSPH_FIXTURES {
        for artifact in EXPECTED_RDINP_ARTIFACTS
            .iter()
            .chain(EXPECTED_POT_ARTIFACTS.iter())
            .chain(EXPECTED_XSPH_ARTIFACTS.iter())
        {
            let baseline_source = baseline_artifact_path(fixture.id, Path::new(artifact));
            let baseline_target = baseline_root
                .join(fixture.id)
                .join("baseline")
                .join(artifact);
            copy_file(&baseline_source, &baseline_target);
        }
    }

    let manifest = json!({
      "fixtures": APPROVED_XSPH_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["RDINP", "POT", "XSPH"],
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
        run_rdinp: true,
        run_pot: true,
        run_xsph: true,
        run_path: false,
        run_fms: false,
        run_band: false,
        run_ldos: false,
        run_rixs: false,
    };

    let report = run_regression(&config).expect("XSPH regression suite should run");
    assert!(report.passed, "expected XSPH suite to pass");
    assert_eq!(report.fixture_count, APPROVED_XSPH_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    PathBuf::from("artifacts/fortran-baselines")
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

fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::copy(source, destination).expect("baseline artifact copy should succeed");
}
