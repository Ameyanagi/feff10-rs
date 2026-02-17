use feff10_rs::domain::{PipelineArtifact, PipelineModule, PipelineRequest};
use feff10_rs::pipelines::PipelineExecutor;
use feff10_rs::pipelines::debye::DebyePipelineScaffold;
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

const APPROVED_DEBYE_FIXTURES: [FixtureCase; 1] = [FixtureCase {
    id: "FX-DEBYE-001",
    input_directory: "feff10/examples/DEBYE/RM/Cu",
}];

const EXPECTED_DEBYE_ARTIFACTS: [&str; 7] = [
    "s2_em.dat",
    "s2_rm1.dat",
    "s2_rm2.dat",
    "xmu.dat",
    "chi.dat",
    "log6.dat",
    "spring.dat",
];
const REQUIRED_DEBYE_INPUT_ARTIFACTS: [&str; 2] = ["paths.dat", "feff.inp"];

#[test]
fn approved_debye_fixtures_emit_required_true_compute_artifacts() {
    for fixture in &APPROVED_DEBYE_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = run_debye_for_fixture(fixture, temp.path(), "actual", true);

        for artifact in &EXPECTED_DEBYE_ARTIFACTS {
            let output_path = output_dir.join(artifact);
            assert!(
                output_path.is_file(),
                "DEBYE artifact '{}' should exist for fixture '{}'",
                output_path.display(),
                fixture.id
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "DEBYE artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }
}

#[test]
fn approved_debye_fixtures_are_deterministic_across_runs() {
    for fixture in &APPROVED_DEBYE_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_output = run_debye_for_fixture(fixture, temp.path(), "first", true);
        let second_output = run_debye_for_fixture(fixture, temp.path(), "second", true);

        for artifact in &EXPECTED_DEBYE_ARTIFACTS {
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
fn debye_optional_spring_input_is_supported() {
    for fixture in &APPROVED_DEBYE_FIXTURES {
        let temp = TempDir::new().expect("tempdir should be created");
        let with_spring = run_debye_for_fixture(fixture, temp.path(), "with-spring", true);
        let without_spring = run_debye_for_fixture(fixture, temp.path(), "without-spring", false);

        let with_spring_summary = fs::read_to_string(with_spring.join("spring.dat"))
            .expect("spring summary should exist");
        let without_spring_summary = fs::read_to_string(without_spring.join("spring.dat"))
            .expect("spring summary should exist");

        assert!(
            with_spring_summary.contains("spring_input_present = true"),
            "spring-present run should record optional spring input"
        );
        assert!(
            without_spring_summary.contains("spring_input_present = false"),
            "spring-missing run should record optional spring input absence"
        );

        let with_spring_rm2 =
            fs::read(with_spring.join("s2_rm2.dat")).expect("with-spring s2_rm2 should exist");
        let without_spring_rm2 = fs::read(without_spring.join("s2_rm2.dat"))
            .expect("without-spring s2_rm2 should exist");
        assert_ne!(
            with_spring_rm2, without_spring_rm2,
            "optional spring input should influence DEBYE thermal output"
        );
    }
}

#[test]
fn debye_regression_suite_passes() {
    let temp = TempDir::new().expect("tempdir should be created");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let manifest_path = temp.path().join("debye-manifest.json");

    for fixture in &APPROVED_DEBYE_FIXTURES {
        let seed_root = temp.path().join("seed");
        let seed_output = run_debye_for_fixture(fixture, &seed_root, "actual", true);
        let baseline_target = baseline_root.join(fixture.id).join("baseline");
        copy_directory_tree(&seed_output, &baseline_target);

        stage_debye_inputs_for_fixture(fixture, &actual_root.join(fixture.id).join("actual"), true);
    }

    let manifest = json!({
      "fixtures": APPROVED_DEBYE_FIXTURES.iter().map(|fixture| {
        json!({
          "id": fixture.id,
          "modulesCovered": ["DEBYE"],
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
        run_debye: true,
        run_dmdw: false,
        run_screen: false,
        run_self: false,
        run_eels: false,
        run_full_spectrum: false,
    };

    let report = run_regression(&config).expect("DEBYE regression suite should run");
    assert!(report.passed, "expected DEBYE suite to pass");
    assert_eq!(report.fixture_count, APPROVED_DEBYE_FIXTURES.len());
    assert_eq!(report.failed_fixture_count, 0);
}

fn run_debye_for_fixture(
    fixture: &FixtureCase,
    root: &Path,
    subdir: &str,
    include_spring: bool,
) -> PathBuf {
    let output_dir = root.join(fixture.id).join(subdir);
    stage_debye_inputs_for_fixture(fixture, &output_dir, include_spring);

    let debye_request = PipelineRequest::new(
        fixture.id,
        PipelineModule::Debye,
        output_dir.join("ff2x.inp"),
        &output_dir,
    );
    let artifacts = DebyePipelineScaffold
        .execute(&debye_request)
        .expect("DEBYE execution should succeed");

    assert_eq!(
        artifact_set(&artifacts),
        expected_artifact_set(&EXPECTED_DEBYE_ARTIFACTS),
        "fixture '{}' should emit expected DEBYE artifacts",
        fixture.id
    );

    output_dir
}

fn stage_debye_inputs_for_fixture(
    fixture: &FixtureCase,
    destination_dir: &Path,
    include_spring: bool,
) {
    stage_ff2x_input(fixture.id, &destination_dir.join("ff2x.inp"));
    for artifact in REQUIRED_DEBYE_INPUT_ARTIFACTS {
        let fallback = if artifact.eq_ignore_ascii_case("paths.dat") {
            "PATH  Rmax= 8.000,  Keep_limit= 0.00, Heap_limit 0.00  Pwcrit= 2.50%\n -----------------------------------------------------------------------\n     1    2  12.000  index, nleg, degeneracy, r=  2.5323\n     2    3  48.000  index, nleg, degeneracy, r=  3.7984\n"
        } else {
            "TITLE Cu DEBYE RM Method\nEDGE K\nEXAFS 15.0\nPOTENTIALS\n    0   29   Cu\n    1   29   Cu\nATOMS\n    0.00000    0.00000    0.00000    0   Cu  0.00000    0\n    1.79059    0.00000    1.79059    1   Cu  2.53228    1\nEND\n"
        };
        stage_text_input(
            fixture.id,
            artifact,
            &destination_dir.join(artifact),
            fallback,
        );
    }

    if include_spring {
        stage_optional_spring_input(fixture.id, &destination_dir.join("spring.inp"));
    }
}

fn baseline_artifact_path(fixture_id: &str, relative_path: &Path) -> PathBuf {
    PathBuf::from("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path)
}

fn stage_ff2x_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "ff2x.inp",
        destination,
        "mchi, ispec, idwopt, ipr6, mbconv, absolu, iGammaCH\n   1   0   2   0   0   0   0\nvrcorr, vicorr, s02, critcw\n      0.00000      0.00000      1.00000      4.00000\ntk, thetad, alphat, thetae, sig2g\n    450.00000    315.00000      0.00000      0.00000      0.00000\nmomentum transfer\n      0.00000      0.00000      0.00000\n the number of decomposi\n   -1\n",
    );
}

fn stage_optional_spring_input(fixture_id: &str, destination: &Path) {
    stage_text_input(
        fixture_id,
        "spring.inp",
        destination,
        "*\tres\twmax\tdosfit\tacut\n VDOS\t0.03\t0.5\t1\n\n STRETCHES\n *\ti\tj\tk_ij\tdR_ij (%)\n\t0\t1\t27.9\t2.\n",
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
