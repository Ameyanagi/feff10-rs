use feff10_rs::domain::{ComputeArtifact, ComputeModule, ComputeRequest};
use feff10_rs::modules::ModuleExecutor;
use feff10_rs::modules::eels::EelsModule;
use feff10_rs::modules::regression::{RegressionRunnerConfig, run_regression};
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

const EELS_REQUIRED_OUTPUT_ARTIFACTS: [&str; 2] = ["eels.dat", "logeels.dat"];

#[test]
fn approved_eels_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_EELS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (output_dir, expected_artifacts) =
            run_eels_for_fixture(fixture, temp.path(), "actual", false);

        for artifact in expected_artifacts {
            let output_path = output_dir.join(&artifact);
            assert!(
                output_path.is_file(),
                "EELS artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "EELS artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }
}

#[test]
fn approved_eels_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_EELS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (first_output, first_expected) =
            run_eels_for_fixture(fixture, temp.path(), "first", false);
        let (second_output, second_expected) =
            run_eels_for_fixture(fixture, temp.path(), "second", false);

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
fn eels_optional_magic_input_is_supported() {
    for fixture in &APPROVED_EELS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (_, with_magic_expected) =
            run_eels_for_fixture(fixture, temp.path(), "with-magic", true);
        let (_, without_magic_expected) =
            run_eels_for_fixture(fixture, temp.path(), "without-magic", false);

        assert!(
            with_magic_expected.contains("magic.dat"),
            "fixture '{}' should emit magic.dat when optional magic input is staged",
            fixture.id
        );
        assert!(
            !without_magic_expected.contains("magic.dat"),
            "fixture '{}' should not emit magic.dat when optional magic input is absent",
            fixture.id
        );
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
        let seed_root = temp.path().join("seed");
        let (seed_output, _) = run_eels_for_fixture(fixture, &seed_root, "actual", true);
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);
        stage_eels_inputs_for_fixture(
            fixture.id,
            &actual_root.join(fixture.id).join("actual"),
            true,
        );
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

fn run_eels_for_fixture(
    fixture: &FixtureCase,
    root: &Path,
    subdir: &str,
    include_magic: bool,
) -> (PathBuf, BTreeSet<String>) {
    let output_dir = root.join(fixture.id).join(subdir);
    stage_eels_inputs_for_fixture(fixture.id, &output_dir, include_magic);

    let eels_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::Eels,
        output_dir.join("eels.inp"),
        &output_dir,
    );
    let artifacts = EelsModule
        .execute(&eels_request)
        .expect("EELS execution should succeed");

    let expected_artifacts = expected_eels_artifact_set(include_magic);
    assert_eq!(
        artifact_set(&artifacts),
        expected_artifacts,
        "fixture '{}' should emit expected EELS output contract",
        fixture.id
    );

    (output_dir, expected_artifacts)
}

fn stage_eels_inputs_for_fixture(fixture_id: &str, destination_dir: &Path, include_magic: bool) {
    stage_eels_input(fixture_id, &destination_dir.join("eels.inp"));
    stage_xmu_input(fixture_id, &destination_dir.join("xmu.dat"));
    if include_magic {
        stage_optional_magic_input(fixture_id, &destination_dir.join("magic.inp"));
    }
}

fn expected_eels_artifact_set(include_magic: bool) -> BTreeSet<String> {
    let mut outputs = expected_artifact_set(&EELS_REQUIRED_OUTPUT_ARTIFACTS);
    if include_magic {
        outputs.insert("magic.dat".to_string());
    }
    outputs
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    PathBuf::from("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn stage_eels_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "eels.inp",
        destination,
        "calculate ELNES?\n   1\naverage? relativistic? cross-terms? Which input?\n   0   1   1   1   4\npolarizations to be used ; min step max\n   1   1   9\nbeam energy in eV\n 300000.00000\nbeam direction in arbitrary units\n      0.00000      1.00000      0.00000\ncollection and convergence semiangle in rad\n      0.00240      0.00000\nqmesh - radial and angular grid size\n   5   3\ndetector positions - two angles in rad\n      0.00000      0.00000\ncalculate magic angle if magic=1\n   0\nenergy for magic angle - eV above threshold\n      0.00000\n",
    );
}

fn stage_xmu_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "xmu.dat",
        destination,
        "# omega e k mu mu0 chi\n8979.411 -16.773 -1.540 5.56205E-06 6.25832E-06 -6.96262E-07\n8980.979 -15.204 -1.400 6.61771E-06 7.52318E-06 -9.05473E-07\n8982.398 -13.786 -1.260 7.99662E-06 9.19560E-06 -1.19897E-06\n",
    );
}

fn stage_optional_magic_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "magic.inp",
        destination,
        "magic energy offset\n12.5\nangular tweak\n0.45\n",
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
