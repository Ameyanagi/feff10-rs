use feff_core::domain::{ComputeArtifact, ComputeModule, ComputeRequest};
use feff_core::modules::ModuleExecutor;
use feff_core::modules::regression::{RegressionRunnerConfig, run_regression};
use feff_core::modules::rixs::RixsModule;
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

const APPROVED_RIXS_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-RIXS-001",
    input_directory: "feff10/examples/RIXS",
}];

const RIXS_REQUIRED_OUTPUT_ARTIFACTS: [&str; 8] = [
    "rixs0.dat",
    "rixs1.dat",
    "rixsET.dat",
    "rixsEE.dat",
    "rixsET-sat.dat",
    "rixsEE-sat.dat",
    "logrixs.dat",
    "rixs.sh",
];

#[test]
fn approved_rixs_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_RIXS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (output_dir, expected_artifacts) =
            run_rixs_for_fixture(fixture, temp.path(), "actual", false);

        for artifact in expected_artifacts {
            let output_path = output_dir.join(&artifact);
            assert!(
                output_path.is_file(),
                "RIXS artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "RIXS artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }
}

#[test]
fn approved_rixs_fixture_emits_committed_shell_script() {
    for fixture in &APPROVED_RIXS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (output_dir, _) = run_rixs_for_fixture(fixture, temp.path(), "actual", false);

        let generated_script = fs::read(output_dir.join("rixs.sh"))
            .expect("generated rixs.sh should exist and be readable");
        let baseline_script = fs::read(baseline_artifact_path(fixture.id, Path::new("rixs.sh")))
            .expect("committed baseline rixs.sh should be readable");

        assert_eq!(
            generated_script, baseline_script,
            "fixture '{}' should emit deterministic rixs.sh content matching committed baseline",
            fixture.id
        );
    }
}

#[test]
fn approved_rixs_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_RIXS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (first_output, first_expected) =
            run_rixs_for_fixture(fixture, temp.path(), "first", false);
        let (second_output, second_expected) =
            run_rixs_for_fixture(fixture, temp.path(), "second", false);

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
fn rixs_multi_edge_inputs_influence_outputs() {
    for fixture in &APPROVED_RIXS_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let (baseline_output, baseline_expected) =
            run_rixs_for_fixture(fixture, temp.path(), "baseline", false);
        let (modified_output, modified_expected) =
            run_rixs_for_fixture(fixture, temp.path(), "modified", true);

        assert_eq!(
            baseline_expected, modified_expected,
            "fixture '{}' output contract should remain stable when edge-2 inputs change",
            fixture.id
        );

        let baseline_rixs1 =
            fs::read(baseline_output.join("rixs1.dat")).expect("baseline rixs1.dat should exist");
        let modified_rixs1 =
            fs::read(modified_output.join("rixs1.dat")).expect("modified rixs1.dat should exist");
        assert_ne!(
            baseline_rixs1, modified_rixs1,
            "fixture '{}' rixs1.dat should change when second-edge inputs are altered",
            fixture.id
        );

        let baseline_rixsee =
            fs::read(baseline_output.join("rixsEE.dat")).expect("baseline rixsEE.dat should exist");
        let modified_rixsee =
            fs::read(modified_output.join("rixsEE.dat")).expect("modified rixsEE.dat should exist");
        assert_ne!(
            baseline_rixsee, modified_rixsee,
            "fixture '{}' rixsEE.dat should change when second-edge inputs are altered",
            fixture.id
        );
    }
}

#[test]
fn rixs_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("rixs-manifest.json");

    for fixture in &APPROVED_RIXS_FIXTURES {
        let seed_root = temp.path().join("seed");
        let (seed_output, _) = run_rixs_for_fixture(fixture, &seed_root, "actual", false);
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);

        stage_rixs_inputs_for_fixture(
            fixture.id,
            &actual_root.join(fixture.id).join("actual"),
            false,
        );
    }

    let manifest = json!({
      "fixtures": APPROVED_RIXS_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["RIXS"],
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
        run_rixs: true,
        run_crpa: false,
        run_compton: false,
        run_debye: false,
        run_dmdw: false,
        run_screen: false,
        run_self: false,
        run_eels: false,
        run_full_spectrum: false,
    };

    let report = run_regression(&config).expect("RIXS regression suite should run");
    assert!(report.passed, "expected RIXS suite to pass");
    assert_eq!(report.fixture_count, APPROVED_RIXS_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn run_rixs_for_fixture(
    fixture: &FixtureCase,
    root: &Path,
    subdir: &str,
    alter_second_edge: bool,
) -> (PathBuf, BTreeSet<String>) {
    let output_dir = root.join(fixture.id).join(subdir);
    stage_rixs_inputs_for_fixture(fixture.id, &output_dir, alter_second_edge);

    let rixs_request = ComputeRequest::new(
        fixture.id,
        ComputeModule::Rixs,
        output_dir.join("rixs.inp"),
        &output_dir,
    );
    let artifacts = RixsModule
        .execute(&rixs_request)
        .expect("RIXS execution should succeed");

    let expected_artifacts = expected_artifact_set(&RIXS_REQUIRED_OUTPUT_ARTIFACTS);
    assert_eq!(
        artifact_set(&artifacts),
        expected_artifacts,
        "fixture '{}' should emit expected RIXS output contract",
        fixture.id
    );

    (output_dir, expected_artifacts)
}

fn stage_rixs_inputs_for_fixture(
    fixture_id: &str,
    destination_dir: &Path,
    alter_second_edge: bool,
) {
    stage_rixs_input(fixture_id, &destination_dir.join("rixs.inp"));
    stage_phase_1_input(fixture_id, &destination_dir.join("phase_1.bin"));
    stage_phase_2_input(fixture_id, &destination_dir.join("phase_2.bin"));
    stage_wscrn_1_input(fixture_id, &destination_dir.join("wscrn_1.dat"));
    stage_wscrn_2_input(fixture_id, &destination_dir.join("wscrn_2.dat"));
    stage_xsect_2_input(fixture_id, &destination_dir.join("xsect_2.dat"));

    if alter_second_edge {
        stage_binary(
            destination_dir.join("phase_2.bin"),
            &[255_u8, 254_u8, 0_u8, 8_u8, 21_u8, 34_u8, 55_u8, 89_u8],
        );
        stage_text(
            destination_dir.join("wscrn_2.dat"),
            "# altered edge 2 screening\n-5.0 0.40 2.10\n0.0 0.55 2.25\n5.0 0.70 2.40\n",
        );
    }
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    workspace_root()
        .join("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn stage_rixs_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "rixs.inp",
        destination,
        "m_run\n1\ngam_ch, gam_exp(1), gam_exp(2)\n0.0001350512 0.0001450512 0.0001550512\nEMinI, EMaxI, EMinF, EMaxF\n-12.0 18.0 -4.0 16.0\nxmu\n-367493090.02742821\nReadpoles, SkipCalc, MBConv, ReadSigma\nT F F T\nnEdges\n2\nEdge 1\nL3\nEdge 2\nL2\n",
    );
}

fn stage_phase_1_input(fixture_id: &str, destination: &Path) {
    stage_binary_input(
        fixture_id,
        "phase_1.bin",
        destination,
        &[3_u8, 5_u8, 8_u8, 13_u8, 21_u8, 34_u8, 55_u8, 89_u8],
    );
}

fn stage_phase_2_input(fixture_id: &str, destination: &Path) {
    stage_binary_input(
        fixture_id,
        "phase_2.bin",
        destination,
        &[2_u8, 7_u8, 1_u8, 8_u8, 2_u8, 8_u8, 1_u8, 8_u8],
    );
}

fn stage_wscrn_1_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "wscrn_1.dat",
        destination,
        "# edge 1 screening profile\n-6.0  0.11  0.95\n-2.0  0.16  1.05\n0.0  0.18  1.15\n3.5  0.23  1.30\n8.0  0.31  1.45\n",
    );
}

fn stage_wscrn_2_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "wscrn_2.dat",
        destination,
        "# edge 2 screening profile\n-5.0  0.09  0.85\n-1.5  0.14  0.95\n1.0  0.17  1.05\n4.0  0.21  1.22\n9.0  0.28  1.36\n",
    );
}

fn stage_xsect_2_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "xsect_2.dat",
        destination,
        "# xsect_2 seed table\n0.0 1.2 0.1\n2.0 1.0 0.2\n4.0 0.9 0.3\n6.0 0.8 0.4\n8.0 0.7 0.5\n",
    );
}

fn stage_text_input(fixture_id: &str, artifact: &str, destination: &Path, fallback: &str) {
    let source = baseline_artifact_path(fixture_id, Path::new(artifact));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    stage_text(destination.to_path_buf(), fallback);
}

fn stage_binary_input(fixture_id: &str, artifact: &str, destination: &Path, fallback: &[u8]) {
    let source = baseline_artifact_path(fixture_id, Path::new(artifact));
    if source.is_file() {
        copy_file(&source, destination);
        return;
    }

    stage_binary(destination.to_path_buf(), fallback);
}

fn stage_text(destination: PathBuf, contents: &str) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(destination, contents).expect("text input should be staged");
}

fn stage_binary(destination: PathBuf, bytes: &[u8]) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination directory should exist");
    }
    fs::write(destination, bytes).expect("binary input should be staged");
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
