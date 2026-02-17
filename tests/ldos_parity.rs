use feff10_rs::domain::{ComputeArtifact, ComputeModule, ComputeRequest};
use feff10_rs::modules::ModuleExecutor;
use feff10_rs::modules::ldos::LdosModule;
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

const APPROVED_LDOS_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-LDOS-001",
    input_directory: "feff10/examples/HUBBARD/CeO2",
}];

const REQUIRED_LDOS_INPUT_ARTIFACTS: [&str; 3] = ["geom.dat", "pot.bin", "reciprocal.inp"];

#[test]
fn approved_ldos_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_LDOS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = run_ldos_for_fixture(fixture, temp.path(), "actual");

        let ldos_outputs = list_ldos_outputs(&output_dir);
        assert!(
            !ldos_outputs.is_empty(),
            "fixture '{}' should emit ldosNN.dat outputs",
            fixture.id
        );
        for artifact in &ldos_outputs {
            let output_path = output_dir.join(artifact);
            assert!(
                output_path.is_file(),
                "LDOS artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "LDOS artifact '{}' should not be empty",
                output_path.display()
            );
        }

        let log_path = output_dir.join("logdos.dat");
        assert!(
            log_path.is_file(),
            "fixture '{}' should emit logdos.dat",
            fixture.id
        );
        assert!(
            !fs::read(&log_path)
                .expect("log should be readable")
                .is_empty(),
            "fixture '{}' logdos.dat should not be empty",
            fixture.id
        );
    }
}

#[test]
fn approved_ldos_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_LDOS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_output = run_ldos_for_fixture(fixture, temp.path(), "first");
        let second_output = run_ldos_for_fixture(fixture, temp.path(), "second");

        let first_artifacts = list_ldos_outputs(&first_output);
        let second_artifacts = list_ldos_outputs(&second_output);
        assert_eq!(
            first_artifacts, second_artifacts,
            "fixture '{}' should emit the same LDOS artifact set across runs",
            fixture.id
        );

        let mut artifacts_to_compare = first_artifacts;
        artifacts_to_compare.push("logdos.dat".to_string());
        for artifact in artifacts_to_compare {
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
fn ldos_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("ldos-manifest.json");

    for fixture in &APPROVED_LDOS_FIXTURES {
        let seed_root = temp.path().join("seed");
        let seed_output = run_ldos_for_fixture(fixture, &seed_root, "actual");
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);

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
        run_compton: false,
        run_debye: false,
        run_dmdw: false,
        run_screen: false,
        run_self: false,
        run_eels: false,
        run_full_spectrum: false,
    };

    let report = run_regression(&config).expect("LDOS regression suite should run");
    assert!(report.passed, "expected LDOS suite to pass");
    assert_eq!(report.fixture_count, APPROVED_LDOS_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn run_ldos_for_fixture(fixture: &FixtureCase, root: &Path, subdir: &str) -> PathBuf {
    let output_dir = root.join(fixture.id).join(subdir);

    stage_ldos_input(fixture.id, &output_dir.join("ldos.inp"));
    for artifact in REQUIRED_LDOS_INPUT_ARTIFACTS {
        copy_file(
            &baseline_artifact_path(fixture.id, Path::new(artifact)),
            &output_dir.join(artifact),
        );
    }

    let ldos_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::Ldos,
        output_dir.join("ldos.inp"),
        &output_dir,
    );
    let artifacts = LdosModule
        .execute(&ldos_request)
        .expect("LDOS execution should succeed");

    let artifact_names = artifact_set(&artifacts);
    assert!(
        artifact_names.contains("logdos.dat"),
        "fixture '{}' should emit logdos.dat",
        fixture.id
    );
    assert!(
        artifact_names.iter().any(|artifact| {
            artifact.starts_with("ldos") && artifact.ends_with(".dat") && artifact != "logdos.dat"
        }),
        "fixture '{}' should emit ldosNN.dat outputs",
        fixture.id
    );

    output_dir
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    baseline_fixture_dir(fixture_id).join(relative_path)
}

fn baseline_fixture_dir(fixture_id: &str) -> PathBuf {
    PathBuf::from("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
}

fn stage_ldos_input(fixture_id: &str, destination: &Path) {
    copy_file(
        &baseline_artifact_path(fixture_id, Path::new("ldos.inp")),
        destination,
    );
}

fn list_ldos_outputs(output_dir: &Path) -> Vec<String> {
    let entries = fs::read_dir(output_dir).expect("output directory should be readable");
    let mut outputs = Vec::new();

    for entry in entries {
        let entry = entry.expect("directory entry should be readable");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let normalized = name.to_ascii_lowercase();
        if normalized.starts_with("ldos")
            && normalized.ends_with(".dat")
            && normalized != "logdos.dat"
        {
            outputs.push(name.to_string());
        }
    }

    outputs.sort();
    outputs
}

fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
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
