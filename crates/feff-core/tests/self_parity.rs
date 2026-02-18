use feff_core::domain::{ComputeArtifact, ComputeModule, ComputeRequest};
use feff_core::modules::ModuleExecutor;
use feff_core::modules::regression::{RegressionRunnerConfig, run_regression};
use feff_core::modules::self_energy::SelfEnergyModule;
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

struct FixtureCase {
    id: &'static str,
    input_directory: &'static str,
}

const APPROVED_SELF_FIXTURES: [FixtureCase; 2] = [
    FixtureCase {
        id: "FX-SELF-001",
        input_directory: "feff10/examples/MPSE/Cu_OPCONS",
    },
    FixtureCase {
        id: "FX-SELF-ORACLE-001",
        input_directory: "feff10/examples/MPSE/Cu_OPCONS",
    },
];

const SELF_REQUIRED_OUTPUT_ARTIFACTS: [&str; 7] = [
    "selfenergy.dat",
    "sigma.dat",
    "specfunct.dat",
    "logsfconv.dat",
    "sig2FEFF.dat",
    "mpse.dat",
    "opconsCu.dat",
];
const SELF_CORE_OUTPUT_ARTIFACTS: [&str; 3] = ["selfenergy.dat", "sigma.dat", "specfunct.dat"];
const SELF_AUXILIARY_OUTPUT_ARTIFACTS: [&str; 5] = [
    "loss.dat",
    "mpse.dat",
    "opconsCu.dat",
    "sig2FEFF.dat",
    "logsfconv.dat",
];
const SELF_SPECTRUM_INPUT_CANDIDATES: [&str; 3] = ["xmu.dat", "chi.dat", "loss.dat"];

#[test]
fn approved_self_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_SELF_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (output_dir, expected_artifacts) = run_self_for_fixture(fixture, temp.path(), "actual");

        for artifact in expected_artifacts {
            let output_path = output_dir.join(&artifact);
            assert!(
                output_path.is_file(),
                "SELF artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "SELF artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }
}

#[test]
fn approved_self_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_SELF_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (first_output, first_expected) = run_self_for_fixture(fixture, temp.path(), "first");
        let (second_output, second_expected) = run_self_for_fixture(fixture, temp.path(), "second");

        assert_eq!(
            first_expected, second_expected,
            "fixture '{}' expected output contract should be stable",
            fixture.id
        );

        for artifact in first_expected {
            let first = fs::read(first_output.join(&artifact)).expect("first output should exist");
            let second =
                fs::read(second_output.join(&artifact)).expect("second output should exist");
            assert_eq!(
                first, second,
                "fixture '{}' artifact '{}' should be deterministic",
                fixture.id, artifact
            );
        }
    }
}

#[test]
fn oracle_self_fixture_core_outputs_match_committed_baseline() {
    let fixture = APPROVED_SELF_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "FX-SELF-ORACLE-001")
        .expect("oracle SELF fixture should be configured");
    let temp = TempDir::new().expect("tempdir should be created");
    let (output_dir, _) = run_self_for_fixture(fixture, temp.path(), "actual");

    assert_outputs_match_committed_baseline(fixture.id, &output_dir, &SELF_CORE_OUTPUT_ARTIFACTS);
}

#[test]
fn oracle_self_fixture_auxiliary_outputs_match_committed_baseline() {
    let fixture = APPROVED_SELF_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "FX-SELF-ORACLE-001")
        .expect("oracle SELF fixture should be configured");
    let temp = TempDir::new().expect("tempdir should be created");
    let (output_dir, _) = run_self_for_fixture(fixture, temp.path(), "actual");

    assert_outputs_match_committed_baseline(
        fixture.id,
        &output_dir,
        &SELF_AUXILIARY_OUTPUT_ARTIFACTS,
    );
}

#[test]
fn oracle_self_fixture_staging_prefers_loss_and_stages_optional_exc_deterministically() {
    let fixture_id = "FX-SELF-ORACLE-001";
    let seed_fixture_id = self_input_seed_fixture_id(fixture_id);
    let temp = TempDir::new().expect("tempdir should be created");
    let first_dir = temp.path().join("first");
    let second_dir = temp.path().join("second");

    let first_staged = stage_self_inputs_for_fixture(fixture_id, &first_dir);
    let second_staged = stage_self_inputs_for_fixture(fixture_id, &second_dir);

    assert_eq!(first_staged, vec!["loss.dat".to_string()]);
    assert_eq!(
        second_staged, first_staged,
        "oracle SELF staging should be deterministic across runs"
    );
    assert!(
        !first_dir.join("xmu.dat").exists() && !first_dir.join("chi.dat").exists(),
        "oracle SELF staging should only include loss.dat among named spectra"
    );

    for artifact in ["loss.dat", "sfconv.inp", "exc.dat"] {
        let expected = fs::read(baseline_artifact_path(seed_fixture_id, Path::new(artifact)))
            .unwrap_or_else(|_| {
                panic!(
                    "baseline '{}' should be readable for deterministic oracle staging",
                    artifact
                )
            });
        let first = fs::read(first_dir.join(artifact))
            .unwrap_or_else(|_| panic!("staged '{}' should be readable (first run)", artifact));
        let second = fs::read(second_dir.join(artifact))
            .unwrap_or_else(|_| panic!("staged '{}' should be readable (second run)", artifact));

        assert_eq!(
            first, expected,
            "oracle SELF staging should source '{}' from '{}'",
            artifact, seed_fixture_id
        );
        assert_eq!(
            second, expected,
            "oracle SELF staging should remain deterministic for '{}'",
            artifact
        );
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
        let seed_root = temp.path().join("seed");
        let (seed_output, _) = run_self_for_fixture(fixture, &seed_root, "actual");
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);
        stage_self_inputs_for_fixture(fixture.id, &actual_root.join(fixture.id).join("actual"));
    }

    let manifest = json!({
      "fixtures": APPROVED_SELF_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["SELF"],
          "inputDirectory": workspace_root().join(fixture.input_directory).to_string_lossy().to_string(),
          "entryFiles": ["feff.inp", "loss.dat", "REFERENCE/sfconv.inp"]
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
        run_full_spectrum: false,
    };

    let report = run_regression(&config).expect("SELF regression suite should run");
    assert!(report.passed, "expected SELF suite to pass");
    assert_eq!(report.fixture_count, APPROVED_SELF_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn run_self_for_fixture(
    fixture: &FixtureCase,
    root: &Path,
    subdir: &str,
) -> (PathBuf, BTreeSet<String>) {
    let output_dir = root.join(fixture.id).join(subdir);
    let staged_spectra = stage_self_inputs_for_fixture(fixture.id, &output_dir);

    let self_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::SelfEnergy,
        output_dir.join("sfconv.inp"),
        &output_dir,
    );
    let artifacts = SelfEnergyModule
        .execute(&self_request)
        .expect("SELF execution should succeed");

    let expected_artifacts = expected_self_artifact_set(&staged_spectra);
    assert_eq!(
        artifact_set(&artifacts),
        expected_artifacts,
        "fixture '{}' should emit expected SELF output contract",
        fixture.id
    );

    (output_dir, expected_artifacts)
}

fn stage_self_inputs_for_fixture(fixture_id: &str, destination_dir: &Path) -> Vec<String> {
    let input_seed_fixture_id = self_input_seed_fixture_id(fixture_id);
    stage_sfconv_input(input_seed_fixture_id, &destination_dir.join("sfconv.inp"));

    let spectrum_candidates: &[&str] = match fixture_id {
        "FX-SELF-ORACLE-001" => &["loss.dat"],
        _ => &SELF_SPECTRUM_INPUT_CANDIDATES,
    };

    let mut staged_spectra = Vec::new();
    for artifact in spectrum_candidates {
        let source = baseline_artifact_path(input_seed_fixture_id, Path::new(artifact));
        if !source.is_file() {
            continue;
        }
        copy_file(&source, &destination_dir.join(*artifact));
        staged_spectra.push((*artifact).to_string());
    }

    for artifact in collect_feff_spectrum_inputs_from_baseline(input_seed_fixture_id) {
        copy_file(
            &baseline_artifact_path(input_seed_fixture_id, Path::new(&artifact)),
            &destination_dir.join(&artifact),
        );
        staged_spectra.push(artifact);
    }

    if staged_spectra.is_empty() {
        stage_text_file(
            destination_dir.join("xmu.dat"),
            "# fallback xmu\n1.0 0.1\n2.0 0.2\n3.0 0.3\n",
        );
        staged_spectra.push("xmu.dat".to_string());
    }

    stage_optional_exc_input(input_seed_fixture_id, &destination_dir.join("exc.dat"));
    staged_spectra.sort();
    staged_spectra.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    staged_spectra
}

fn self_input_seed_fixture_id(fixture_id: &str) -> &str {
    match fixture_id {
        "FX-SELF-ORACLE-001" => "FX-SELF-001",
        _ => fixture_id,
    }
}

fn expected_self_artifact_set(staged_spectra: &[String]) -> BTreeSet<String> {
    let mut artifacts: BTreeSet<String> = SELF_REQUIRED_OUTPUT_ARTIFACTS
        .iter()
        .map(|artifact| artifact.to_string())
        .collect();
    artifacts.extend(staged_spectra.iter().cloned());
    artifacts
}

fn collect_feff_spectrum_inputs_from_baseline(fixture_id: &str) -> Vec<String> {
    let baseline_dir = baseline_artifact_path(fixture_id, Path::new(""));
    if !baseline_dir.is_dir() {
        return Vec::new();
    }

    let entries = fs::read_dir(&baseline_dir).expect("baseline directory should be readable");
    let mut artifacts = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if is_feff_spectrum_name(&file_name) {
            artifacts.push(file_name);
        }
    }
    artifacts.sort();
    artifacts
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    workspace_root()
        .join("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn assert_outputs_match_committed_baseline(
    fixture_id: &str,
    output_dir: &Path,
    artifacts: &[&str],
) {
    for artifact in artifacts {
        let baseline_path = baseline_artifact_path(fixture_id, Path::new(artifact));
        assert!(
            baseline_path.is_file(),
            "oracle baseline artifact '{}' should exist",
            baseline_path.display()
        );

        let actual_path = output_dir.join(artifact);
        let actual = fs::read_to_string(&actual_path).unwrap_or_else(|_| {
            panic!(
                "actual artifact '{}' should be readable",
                actual_path.display()
            )
        });
        let baseline = fs::read_to_string(&baseline_path).unwrap_or_else(|_| {
            panic!(
                "baseline artifact '{}' should be readable",
                baseline_path.display()
            )
        });
        assert_eq!(
            actual, baseline,
            "oracle SELF artifact '{}' should match committed baseline bytes",
            artifact
        );
    }
}

fn stage_sfconv_input(fixture_id: &str, destination: &Path) {
    let source = baseline_artifact_path(fixture_id, Path::new("sfconv.inp"));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    stage_text_file(
        destination.to_path_buf(),
        "msfconv, ipse, ipsk\n   1   0   0\nwsigk, cen\n      0.00000      0.00000\nispec, ipr6\n   1   0\ncfname\nNULL\n",
    );
}

fn stage_optional_exc_input(fixture_id: &str, destination: &Path) {
    let source = baseline_artifact_path(fixture_id, Path::new("exc.dat"));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    stage_text_file(
        destination.to_path_buf(),
        "0.1 1.0 0.8 0.9\n0.2 1.0 0.5 1.0\n",
    );
}

fn is_feff_spectrum_name(name: &str) -> bool {
    let lowercase = name.to_ascii_lowercase();
    if !lowercase.starts_with("feff") || !lowercase.ends_with(".dat") {
        return false;
    }

    let suffix = &lowercase[4..lowercase.len() - 4];
    !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
}

fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn stage_text_file(destination: PathBuf, contents: &str) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(destination, contents).expect("text file should be staged");
}

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::copy(source, destination).expect("artifact copy should succeed");
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
