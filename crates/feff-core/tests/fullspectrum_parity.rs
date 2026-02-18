use feff_core::domain::{ComputeArtifact, ComputeModule, ComputeRequest};
use feff_core::modules::fullspectrum::FullSpectrumModule;
use feff_core::modules::regression::{run_regression, RegressionRunnerConfig};
use feff_core::modules::ModuleExecutor;
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

const APPROVED_FULLSPECTRUM_FIXTURES: [FixtureCase; 2] = [
    FixtureCase {
        id: "FX-FULLSPECTRUM-001",
        input_directory: "feff10/examples/XES/Cu",
    },
    FixtureCase {
        id: "FX-FULLSPECTRUM-ORACLE-001",
        input_directory: "feff10/examples/XES/Cu",
    },
];

const FULLSPECTRUM_REQUIRED_OUTPUT_ARTIFACTS: [&str; 7] = [
    "xmu.dat",
    "osc_str.dat",
    "eps.dat",
    "drude.dat",
    "background.dat",
    "fine_st.dat",
    "logfullspectrum.dat",
];
const FULLSPECTRUM_TABLE_OUTPUT_ARTIFACTS: [&str; 6] = [
    "xmu.dat",
    "osc_str.dat",
    "eps.dat",
    "drude.dat",
    "background.dat",
    "fine_st.dat",
];
const FULLSPECTRUM_DIAGNOSTIC_OUTPUT_ARTIFACTS: [&str; 1] = ["logfullspectrum.dat"];

#[test]
fn approved_fullspectrum_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_FULLSPECTRUM_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (output_dir, expected_artifacts) =
            run_fullspectrum_for_fixture(fixture, temp.path(), "actual", true);

        for artifact in expected_artifacts {
            let output_path = output_dir.join(&artifact);
            assert!(
                output_path.is_file(),
                "FULLSPECTRUM artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "FULLSPECTRUM artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }
}

#[test]
fn approved_fullspectrum_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_FULLSPECTRUM_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (first_output, first_expected) =
            run_fullspectrum_for_fixture(fixture, temp.path(), "first", true);
        let (second_output, second_expected) =
            run_fullspectrum_for_fixture(fixture, temp.path(), "second", true);

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
fn fullspectrum_optional_component_inputs_are_supported() {
    for fixture in &APPROVED_FULLSPECTRUM_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (with_optional_output, with_optional_expected) =
            run_fullspectrum_for_fixture(fixture, temp.path(), "with-optional", true);
        let (without_optional_output, without_optional_expected) =
            run_fullspectrum_for_fixture(fixture, temp.path(), "without-optional", false);

        assert_eq!(
            with_optional_expected,
            expected_artifact_set(&FULLSPECTRUM_REQUIRED_OUTPUT_ARTIFACTS),
            "fixture '{}' should emit the FULLSPECTRUM required output contract with optional inputs",
            fixture.id
        );
        assert_eq!(
            without_optional_expected,
            expected_artifact_set(&FULLSPECTRUM_REQUIRED_OUTPUT_ARTIFACTS),
            "fixture '{}' should emit the FULLSPECTRUM required output contract without optional inputs",
            fixture.id
        );

        let with_optional_xmu =
            fs::read(with_optional_output.join("xmu.dat")).expect("xmu.dat should exist");
        let without_optional_xmu =
            fs::read(without_optional_output.join("xmu.dat")).expect("xmu.dat should exist");
        assert_ne!(
            with_optional_xmu, without_optional_xmu,
            "fixture '{}' xmu.dat should change when prexmu/referencexmu inputs are staged",
            fixture.id
        );
    }
}

#[test]
fn oracle_fullspectrum_fixture_table_outputs_match_committed_baseline() {
    let fixture = APPROVED_FULLSPECTRUM_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "FX-FULLSPECTRUM-ORACLE-001")
        .expect("oracle FULLSPECTRUM fixture should be configured");
    let temp = TempDir::new().expect("tempdir should be created");
    let (output_dir, _) = run_fullspectrum_for_fixture(fixture, temp.path(), "actual", true);

    assert_outputs_match_committed_baseline(
        fixture.id,
        &output_dir,
        &FULLSPECTRUM_TABLE_OUTPUT_ARTIFACTS,
    );
}

#[test]
fn oracle_fullspectrum_fixture_diagnostic_log_matches_committed_baseline() {
    let fixture = APPROVED_FULLSPECTRUM_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "FX-FULLSPECTRUM-ORACLE-001")
        .expect("oracle FULLSPECTRUM fixture should be configured");
    let temp = TempDir::new().expect("tempdir should be created");
    let (output_dir, _) = run_fullspectrum_for_fixture(fixture, temp.path(), "actual", true);

    assert_outputs_match_committed_baseline(
        fixture.id,
        &output_dir,
        &FULLSPECTRUM_DIAGNOSTIC_OUTPUT_ARTIFACTS,
    );
}

#[test]
fn oracle_fullspectrum_fixture_staging_uses_seed_fixture_inputs_deterministically() {
    let fixture_id = "FX-FULLSPECTRUM-ORACLE-001";
    let seed_fixture_id = fullspectrum_input_seed_fixture_id(fixture_id);
    let temp = TempDir::new().expect("tempdir should be created");
    let first_dir = temp.path().join("first");
    let second_dir = temp.path().join("second");

    stage_fullspectrum_inputs_for_fixture(fixture_id, &first_dir, true);
    stage_fullspectrum_inputs_for_fixture(fixture_id, &second_dir, true);

    for artifact in [
        "fullspectrum.inp",
        "xmu.dat",
        "prexmu.dat",
        "referencexmu.dat",
    ] {
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
            "oracle FULLSPECTRUM staging should source '{}' from '{}'",
            artifact, seed_fixture_id
        );
        assert_eq!(
            second, expected,
            "oracle FULLSPECTRUM staging should remain deterministic for '{}'",
            artifact
        );
    }
}

#[test]
fn oracle_fullspectrum_fixture_has_zero_mismatches_against_committed_baseline() {
    let fixture = APPROVED_FULLSPECTRUM_FIXTURES
        .iter()
        .find(|fixture| fixture.id == "FX-FULLSPECTRUM-ORACLE-001")
        .expect("oracle FULLSPECTRUM fixture should be configured");
    let temp = TempDir::new().expect("tempdir should be created");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("fullspectrum-oracle-manifest.json");
    let actual_dir = actual_root.join(fixture.id).join("actual");
    let input_seed_fixture_id = fullspectrum_input_seed_fixture_id(fixture.id);

    stage_fullspectrum_inputs_for_fixture(fixture.id, &actual_dir, true);
    stage_feff_input(input_seed_fixture_id, &actual_dir.join("feff.inp"));

    let manifest = json!({
      "fixtures": [
        {
          "id": fixture.id,
          "modulesCovered": ["FULLSPECTRUM"],
          "inputDirectory": workspace_root().join(fixture.input_directory).to_string_lossy().to_string(),
          "entryFiles": ["feff.inp"]
        }
      ]
    });
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("manifest JSON"),
    )
    .expect("manifest should be written");

    let config = RegressionRunnerConfig {
        manifest_path,
        policy_path: workspace_root().join("tasks/numeric-tolerance-policy.json"),
        baseline_root: workspace_root().join("artifacts/fortran-baselines"),
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

    let report = run_regression(&config)
        .expect("oracle FULLSPECTRUM committed-baseline regression should run");
    assert!(
        report.passed,
        "expected oracle FULLSPECTRUM fixture to pass"
    );
    assert_eq!(report.fixture_count, 1);
    assert_eq!(report.failed_fixture_count, 0);
    assert_eq!(report.mismatch_fixture_count, 0);
    assert_eq!(report.mismatch_artifact_count, 0);
}

#[test]
fn fullspectrum_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("fullspectrum-manifest.json");

    for fixture in &APPROVED_FULLSPECTRUM_FIXTURES {
        let seed_root = temp.path().join("seed");
        let (seed_output, _) = run_fullspectrum_for_fixture(fixture, &seed_root, "actual", true);
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);

        stage_fullspectrum_inputs_for_fixture(
            fixture.id,
            &actual_root.join(fixture.id).join("actual"),
            true,
        );
    }

    let manifest = json!({
      "fixtures": APPROVED_FULLSPECTRUM_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["FULLSPECTRUM"],
          "inputDirectory": workspace_root().join(fixture.input_directory).to_string_lossy().to_string(),
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
        run_self: false,
        run_eels: false,
        run_full_spectrum: true,
    };

    let report = run_regression(&config).expect("FULLSPECTRUM regression suite should run");
    assert!(report.passed, "expected FULLSPECTRUM suite to pass");
    assert_eq!(report.fixture_count, APPROVED_FULLSPECTRUM_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn run_fullspectrum_for_fixture(
    fixture: &FixtureCase,
    root: &Path,
    subdir: &str,
    include_optional: bool,
) -> (PathBuf, BTreeSet<String>) {
    let output_dir = root.join(fixture.id).join(subdir);
    stage_fullspectrum_inputs_for_fixture(fixture.id, &output_dir, include_optional);

    let fullspectrum_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::FullSpectrum,
        output_dir.join("fullspectrum.inp"),
        &output_dir,
    );
    let artifacts = FullSpectrumModule
        .execute(&fullspectrum_request)
        .expect("FULLSPECTRUM execution should succeed");

    let expected_artifacts = expected_artifact_set(&FULLSPECTRUM_REQUIRED_OUTPUT_ARTIFACTS);
    assert_eq!(
        artifact_set(&artifacts),
        expected_artifacts,
        "fixture '{}' should emit expected FULLSPECTRUM output contract",
        fixture.id
    );

    (output_dir, expected_artifacts)
}

fn stage_fullspectrum_inputs_for_fixture(
    fixture_id: &str,
    destination_dir: &Path,
    include_optional: bool,
) {
    let input_seed_fixture_id = fullspectrum_input_seed_fixture_id(fixture_id);
    stage_fullspectrum_input(
        input_seed_fixture_id,
        &destination_dir.join("fullspectrum.inp"),
    );
    stage_xmu_input(input_seed_fixture_id, &destination_dir.join("xmu.dat"));

    if include_optional {
        stage_prexmu_input(input_seed_fixture_id, &destination_dir.join("prexmu.dat"));
        stage_referencexmu_input(
            input_seed_fixture_id,
            &destination_dir.join("referencexmu.dat"),
        );
    }
}

fn fullspectrum_input_seed_fixture_id(fixture_id: &str) -> &str {
    match fixture_id {
        "FX-FULLSPECTRUM-ORACLE-001" => "FX-FULLSPECTRUM-001",
        _ => fixture_id,
    }
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    workspace_root()
        .join("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn stage_fullspectrum_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "fullspectrum.inp",
        destination,
        " mFullSpectrum\n           1\n broadening drude\n     0.45000     1.25000\n oscillator epsilon_shift\n     1.10000     0.25000\n",
    );
}

fn stage_xmu_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "xmu.dat",
        destination,
        "# omega e k mu mu0 chi\n8956.1761 -40.0000 -2.9103 9.162321E-02 9.102713E-02 5.960831E-04\n8956.6084 -39.5677 -2.8908 7.595159E-02 7.534298E-02 6.086083E-04\n8957.0407 -39.1354 -2.8711 6.248403E-02 6.186194E-02 6.220848E-04\n8957.4730 -38.7031 -2.8512 5.166095E-02 5.102360E-02 6.373535E-04\n",
    );
}

fn stage_prexmu_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "prexmu.dat",
        destination,
        "-1.4699723600E+00 -5.2212753390E-04 1.1530407310E-05\n-1.4540857260E+00 -5.1175235060E-04 9.5436958570E-06\n-1.4381990910E+00 -5.0195981330E-04 7.8360530260E-06\n",
    );
}

fn stage_referencexmu_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "referencexmu.dat",
        destination,
        "# omega e k mu mu0 chi\n8956.1761 -40.0000 -2.9103 9.162321E-02 9.102713E-02 5.960831E-04\n8956.6084 -39.5677 -2.8908 7.595159E-02 7.534298E-02 6.086083E-04\n8957.0407 -39.1354 -2.8711 6.248403E-02 6.186194E-02 6.220848E-04\n",
    );
}

fn stage_feff_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "feff.inp",
        destination,
        "TITLE fallback FULLSPECTRUM fixture\n",
    );
}

fn stage_text_input(fixture_id: &str, artifact: &str, destination: &Path, fallback: &str) {
    let source = baseline_artifact_path(fixture_id, Path::new(artifact));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(destination, fallback).expect("text input should be staged");
}

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::copy(source, destination).expect("artifact copy should succeed");
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
            "oracle FULLSPECTRUM artifact '{}' should match committed baseline bytes",
            artifact
        );
    }
}

fn expected_artifact_set(artifacts: &[&str]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.to_string())
        .collect()
}

fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
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
