use feff10_rs::domain::{PipelineArtifact, PipelineModule, PipelineRequest};
use feff10_rs::pipelines::PipelineExecutor;
use feff10_rs::pipelines::pot::PotPipelineScaffold;
use feff10_rs::pipelines::rdinp::RdinpPipelineScaffold;
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

const APPROVED_POT_FIXTURES: [FixtureCase; 2] = [
    FixtureCase {
        id: "FX-POT-001",
        input_directory: "feff10/examples/EXAFS/Cu",
    },
    FixtureCase {
        id: "FX-WORKFLOW-XAS-001",
        input_directory: "feff10/examples/XANES/Cu",
    },
];

const EXPECTED_RDINP_ARTIFACTS: [&str; 13] = [
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

const EXPECTED_POT_ARTIFACTS: [&str; 5] = [
    "pot.bin",
    "pot.dat",
    "log1.dat",
    "convergence.scf",
    "convergence.scf.fine",
];

#[test]
fn approved_pot_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_POT_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = run_rdinp_and_pot_for_fixture(fixture, temp.path(), "actual");
        let artifacts = EXPECTED_POT_ARTIFACTS
            .iter()
            .map(|artifact| output_dir.join(artifact))
            .collect::<Vec<_>>();

        for output_path in artifacts {
            assert!(
                output_path.is_file(),
                "POT artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            let bytes = fs::read(&output_path).expect("artifact should be readable");
            assert!(
                !bytes.is_empty(),
                "POT artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }
}

#[test]
fn approved_pot_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_POT_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_output = run_rdinp_and_pot_for_fixture(fixture, temp.path(), "first");
        let second_output = run_rdinp_and_pot_for_fixture(fixture, temp.path(), "second");

        for artifact in &EXPECTED_POT_ARTIFACTS {
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
fn pot_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("pot-manifest.json");

    for fixture in &APPROVED_POT_FIXTURES {
        let seed_root = temp.path().join("seed");
        let seed_output = run_rdinp_and_pot_for_fixture(fixture, &seed_root, "actual");
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);
    }

    let manifest = json!({
      "fixtures": APPROVED_POT_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["RDINP", "POT"],
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
        run_full_spectrum: false,
    };

    let report = run_regression(&config).expect("POT regression suite should run");
    assert!(report.passed, "expected POT suite to pass");
    assert_eq!(report.fixture_count, APPROVED_POT_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn run_rdinp_and_pot_for_fixture(fixture: &FixtureCase, root: &Path, subdir: &str) -> PathBuf {
    let output_dir = root.join(fixture.id).join(subdir);
    let rdinp_request = PipelineRequest::new(
        fixture.id,
        PipelineModule::Rdinp,
        Path::new(fixture.input_directory).join("feff.inp"),
        &output_dir,
    );
    let rdinp_artifacts = RdinpPipelineScaffold
        .execute(&rdinp_request)
        .expect("RDINP execution should succeed");
    let rdinp_set = artifact_set(&rdinp_artifacts);
    for artifact in EXPECTED_RDINP_ARTIFACTS {
        assert!(
            rdinp_set.contains(artifact),
            "fixture '{}' should include RDINP artifact '{}' before POT execution",
            fixture.id,
            artifact
        );
    }
    assert!(
        rdinp_set.contains("geom.dat"),
        "fixture '{}' should include geom.dat before POT execution",
        fixture.id
    );

    let pot_request = PipelineRequest::new(
        fixture.id,
        PipelineModule::Pot,
        output_dir.join("pot.inp"),
        &output_dir,
    );
    let pot_artifacts = PotPipelineScaffold
        .execute(&pot_request)
        .expect("POT execution should succeed");
    assert_eq!(
        artifact_set(&pot_artifacts),
        expected_artifact_set(&EXPECTED_POT_ARTIFACTS),
        "fixture '{}' should emit expected POT artifacts",
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

fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
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
