use feff10_rs::domain::{PipelineArtifact, PipelineModule, PipelineRequest};
use feff10_rs::pipelines::PipelineExecutor;
use feff10_rs::pipelines::comparator::Comparator;
use feff10_rs::pipelines::regression::{RegressionRunnerConfig, run_regression};
use feff10_rs::pipelines::self_energy::SelfEnergyPipelineScaffold;
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct FixtureCase {
    id: &'static str,
    input_directory: &'static str,
}

const APPROVED_SELF_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-SELF-001",
    input_directory: "feff10/examples/MPSE/Cu_OPCONS",
}];

const SELF_OUTPUT_CANDIDATES: [&str; 9] = [
    "selfenergy.dat",
    "sigma.dat",
    "specfunct.dat",
    "xmu.dat",
    "chi.dat",
    "logsfconv.dat",
    "sig2FEFF.dat",
    "mpse.dat",
    "opconsCu.dat",
];
const SELF_SPECTRUM_INPUT_CANDIDATES: [&str; 3] = ["xmu.dat", "chi.dat", "loss.dat"];
const OPTIONAL_SELF_INPUTS: [&str; 1] = ["exc.dat"];

#[test]
fn approved_self_fixtures_match_baseline_under_policy() {
    let comparator = Comparator::from_policy_path("tasks/numeric-tolerance-policy.json")
        .expect("policy should load");

    for fixture in &APPROVED_SELF_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");
        stage_self_inputs(fixture.id, &output_dir);

        let self_request = PipelineRequest::new(
            fixture.id,
            PipelineModule::SelfEnergy,
            output_dir.join("sfconv.inp"),
            &output_dir,
        );
        let artifacts = SelfEnergyPipelineScaffold
            .execute(&self_request)
            .expect("SELF execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_self_artifact_set_for_fixture(fixture.id),
            "artifact contract should match expected SELF outputs"
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
fn self_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("self-manifest.json");

    for fixture in &APPROVED_SELF_FIXTURES {
        for artifact in expected_self_artifacts_for_fixture(fixture.id) {
            let baseline_source = baseline_artifact_path(fixture.id, Path::new(&artifact));
            let baseline_target = baseline_root
                .join(fixture.id)
                .join("baseline")
                .join(&artifact);
            copy_file(&baseline_source, &baseline_target);
        }

        let baseline_fixture_dir = baseline_root.join(fixture.id).join("baseline");
        stage_self_inputs(fixture.id, &baseline_fixture_dir);

        let staged_dir = actual_root.join(fixture.id).join("actual");
        stage_self_inputs(fixture.id, &staged_dir);
    }

    let manifest = json!({
      "fixtures": APPROVED_SELF_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["SELF"],
          "inputDirectory": fixture.input_directory,
          "entryFiles": ["feff.inp", "loss.dat", "sfconv.inp"]
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
        run_self: true,
        run_eels: false,
    };

    let report = run_regression(&config).expect("SELF regression suite should run");
    assert!(report.passed, "expected SELF suite to pass");
    assert_eq!(report.fixture_count, APPROVED_SELF_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    PathBuf::from("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn expected_self_artifact_set_for_fixture(fixture_id: &str) -> BTreeSet<String> {
    let mut artifacts: BTreeSet<String> = SELF_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_artifact_path(fixture_id, Path::new(artifact)).is_file())
        .map(|artifact| artifact.to_string())
        .collect();

    let baseline_dir = baseline_artifact_path(fixture_id, Path::new(""));
    let entries = fs::read_dir(&baseline_dir).expect("baseline directory should be readable");
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if is_feff_spectrum_name(&file_name) {
            artifacts.insert(file_name);
        }
    }

    assert!(
        !artifacts.is_empty(),
        "fixture '{}' should provide at least one SELF output artifact",
        fixture_id
    );
    artifacts
}

fn expected_self_artifacts_for_fixture(fixture_id: &str) -> Vec<String> {
    expected_self_artifact_set_for_fixture(fixture_id)
        .into_iter()
        .collect()
}

fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn stage_self_inputs(fixture_id: &str, destination_dir: &Path) {
    copy_file(
        &baseline_artifact_path(fixture_id, Path::new("sfconv.inp")),
        &destination_dir.join("sfconv.inp"),
    );

    let staged_spectrum_count = stage_self_spectrum_inputs(fixture_id, destination_dir);
    assert!(
        staged_spectrum_count > 0,
        "fixture '{}' should stage at least one SELF spectrum input",
        fixture_id
    );

    stage_optional_exc_input(fixture_id, &destination_dir.join(OPTIONAL_SELF_INPUTS[0]));
}

fn stage_self_spectrum_inputs(fixture_id: &str, destination_dir: &Path) -> usize {
    let mut staged_count = 0usize;

    for artifact in SELF_SPECTRUM_INPUT_CANDIDATES {
        let source = baseline_artifact_path(fixture_id, Path::new(artifact));
        if !source.is_file() {
            continue;
        }

        copy_file(&source, &destination_dir.join(artifact));
        staged_count += 1;
    }

    let baseline_dir = baseline_artifact_path(fixture_id, Path::new(""));
    let entries = fs::read_dir(&baseline_dir).expect("baseline directory should be readable");
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if !is_feff_spectrum_name(&file_name) {
            continue;
        }

        copy_file(
            &baseline_artifact_path(fixture_id, Path::new(&file_name)),
            &destination_dir.join(&file_name),
        );
        staged_count += 1;
    }

    staged_count
}

fn stage_optional_exc_input(fixture_id: &str, destination: &Path) {
    let source = baseline_artifact_path(fixture_id, Path::new(OPTIONAL_SELF_INPUTS[0]));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(destination, "0.0 0.0\n").expect("exc.dat should be staged");
}

fn is_feff_spectrum_name(name: &str) -> bool {
    let lowercase = name.to_ascii_lowercase();
    if !lowercase.starts_with("feff") || !lowercase.ends_with(".dat") {
        return false;
    }

    let suffix = &lowercase[4..lowercase.len() - 4];
    !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
}

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::copy(source, destination).expect("baseline artifact copy should succeed");
}
