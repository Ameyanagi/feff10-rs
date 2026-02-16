use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn regression_command_succeeds_when_all_artifacts_match() {
    let temp = TempDir::new().expect("tempdir should be created");
    let fixture_id = "FX-CLI-PASS-001";

    let manifest_path = temp.path().join("manifest.json");
    let policy_path = temp.path().join("policy.json");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");

    write_file(
        &manifest_path,
        r#"
        {
          "fixtures": [
            { "id": "FX-CLI-PASS-001" }
          ]
        }
        "#,
    );
    write_file(
        &policy_path,
        r#"
        {
          "defaultMode": "exact_text"
        }
        "#,
    );

    write_fixture_file(
        &baseline_root,
        fixture_id,
        "baseline",
        "xmu.dat",
        "1.0 2.0 3.0\n",
    );
    write_fixture_file(
        &actual_root,
        fixture_id,
        "actual",
        "xmu.dat",
        "1.0 2.0 3.0\n",
    );

    let output = run_regression_command(
        &manifest_path,
        &policy_path,
        &baseline_root,
        &actual_root,
        &report_path,
    );

    assert!(
        output.status.success(),
        "command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Regression status: PASS"),
        "stdout should contain pass status"
    );
    assert!(report_path.exists(), "report file should be created");

    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("report should be readable"))
            .expect("report JSON should parse");
    assert_eq!(parsed["passed"], Value::Bool(true));
}

#[test]
fn regression_command_exits_non_zero_when_fixture_fails() {
    let temp = TempDir::new().expect("tempdir should be created");
    let fixture_id = "FX-CLI-FAIL-001";

    let manifest_path = temp.path().join("manifest.json");
    let policy_path = temp.path().join("policy.json");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");

    write_file(
        &manifest_path,
        r#"
        {
          "fixtures": [
            { "id": "FX-CLI-FAIL-001" }
          ]
        }
        "#,
    );
    write_file(
        &policy_path,
        r#"
        {
          "defaultMode": "exact_text"
        }
        "#,
    );

    write_fixture_file(
        &baseline_root,
        fixture_id,
        "baseline",
        "log.dat",
        "baseline\n",
    );
    write_fixture_file(&actual_root, fixture_id, "actual", "log.dat", "actual\n");

    let output = run_regression_command(
        &manifest_path,
        &policy_path,
        &baseline_root,
        &actual_root,
        &report_path,
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "command should exit with status 1 on regression failure, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Regression status: FAIL"),
        "stdout should contain fail status"
    );
    assert!(report_path.exists(), "report file should be created");

    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("report should be readable"))
            .expect("report JSON should parse");
    assert_eq!(parsed["passed"], Value::Bool(false));
    assert_eq!(parsed["failed_fixture_count"], Value::from(1));
}

#[test]
fn regression_command_writes_core_workflow_outputs_to_fixture_subdirectory_contract() {
    let temp = TempDir::new().expect("tempdir should be created");
    let fixture_id = "FX-WORKFLOW-XAS-001";

    let manifest_path = temp.path().join("manifest.json");
    let policy_path = temp.path().join("policy.json");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let fixture_input_dir = temp.path().join("fixtures/workflow");

    stage_workspace_fixture_file(fixture_id, "feff.inp", &fixture_input_dir.join("feff.inp"));

    let manifest = format!(
        r#"
        {{
          "fixtures": [
            {{
              "id": "{fixture_id}",
              "modulesCovered": ["RDINP", "POT", "XSPH", "PATH", "FMS"],
              "inputDirectory": "{input_directory}",
              "entryFiles": ["feff.inp"]
            }}
          ]
        }}
        "#,
        input_directory = fixture_input_dir.to_string_lossy().replace('\\', "\\\\")
    );
    write_file(&manifest_path, &manifest);
    write_file(
        &policy_path,
        r#"
        {
          "defaultMode": "exact_text"
        }
        "#,
    );

    let output = run_regression_command_with_extra_args(
        &manifest_path,
        &policy_path,
        &baseline_root,
        &actual_root,
        &report_path,
        &[
            "--run-rdinp",
            "--run-pot",
            "--run-xsph",
            "--run-path",
            "--run-fms",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "empty baseline root should fail comparison after hook execution, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let fixture_actual_dir = actual_root.join(fixture_id).join("actual");
    for artifact in ["geom.dat", "pot.bin", "phase.bin", "paths.dat", "gg.bin"] {
        assert!(
            fixture_actual_dir.join(artifact).is_file(),
            "core workflow artifact '{}' should be written under fixture actual directory",
            artifact
        );
    }
    assert!(
        !actual_root.join(fixture_id).join("geom.dat").is_file(),
        "core outputs should not be written directly under fixture root without actual-subdir"
    );
}

#[test]
fn regression_command_run_pot_input_mismatch_emits_computation_diagnostic_contract() {
    let temp = TempDir::new().expect("tempdir should be created");
    let fixture_id = "FX-POT-001";

    let manifest_path = temp.path().join("manifest.json");
    let policy_path = temp.path().join("policy.json");
    let baseline_root = temp.path().join("baseline-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/report.json");
    let staged_output_dir = actual_root.join(fixture_id).join("actual");

    write_file(
        &manifest_path,
        r#"
        {
          "fixtures": [
            {
              "id": "FX-POT-001",
              "modulesCovered": ["POT"]
            }
          ]
        }
        "#,
    );
    write_file(
        &policy_path,
        r#"
        {
          "defaultMode": "exact_text"
        }
        "#,
    );

    stage_workspace_fixture_file(fixture_id, "pot.inp", &staged_output_dir.join("pot.inp"));
    stage_workspace_fixture_file(fixture_id, "geom.dat", &staged_output_dir.join("geom.dat"));
    write_file(&staged_output_dir.join("pot.inp"), "BROKEN POT INPUT\n");

    let output = run_regression_command_with_extra_args(
        &manifest_path,
        &policy_path,
        &baseline_root,
        &actual_root,
        &report_path,
        &["--run-pot"],
    );

    assert_eq!(
        output.status.code(),
        Some(4),
        "POT contract mismatch should map to computation fatal exit code, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ERROR: [RUN.POT_INPUT_MISMATCH]"),
        "stderr should include computation diagnostic prefix, stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("FATAL EXIT CODE: 4"),
        "stderr should include fatal exit summary line, stderr: {}",
        stderr
    );
    assert!(
        !report_path.exists(),
        "fatal pre-compare hook failures should not write a regression report"
    );
}

#[test]
fn oracle_command_runs_capture_and_rust_generation_for_same_fixture_set() {
    if !command_available("jq") {
        eprintln!("Skipping oracle CLI test because jq is unavailable in PATH.");
        return;
    }

    let temp = TempDir::new().expect("tempdir should be created");
    let fixtures = [
        ("FX-RDINP-001", "feff10/examples/EXAFS/Cu"),
        ("FX-WORKFLOW-XAS-001", "feff10/examples/XANES/Cu"),
    ];

    let manifest_path = temp.path().join("manifest.json");
    let policy_path = temp.path().join("policy.json");
    let oracle_root = temp.path().join("oracle-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/oracle-report.json");
    let fixture_entries = fixtures
        .iter()
        .map(|(fixture_id, input_directory)| {
            let fixture_input_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(input_directory);
            format!(
                r#"
            {{
              "id": "{fixture_id}",
              "modulesCovered": ["RDINP"],
              "inputDirectory": "{input_directory}",
              "entryFiles": ["feff.inp"]
            }}
            "#,
                fixture_id = fixture_id,
                input_directory = fixture_input_dir.to_string_lossy().replace('\\', "\\\\")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let manifest = format!(
        r#"
        {{
          "fixtures": [{}]
        }}
        "#,
        fixture_entries
    );
    write_file(&manifest_path, &manifest);
    write_file(
        &policy_path,
        r#"
        {
          "defaultMode": "exact_text"
        }
        "#,
    );

    let output = run_oracle_command_with_extra_args(
        &manifest_path,
        &policy_path,
        &oracle_root,
        &actual_root,
        &report_path,
        &["--capture-runner", ":", "--run-rdinp"],
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "oracle command should return regression mismatch status when oracle outputs differ, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Mismatches:"),
        "oracle summary should include mismatch totals, stdout: {}",
        stdout
    );
    for (fixture_id, _) in fixtures {
        assert!(
            stdout.contains(&format!("Fixture {} mismatches", fixture_id)),
            "oracle summary should include fixture-level mismatch details for '{}', stdout: {}",
            fixture_id,
            stdout
        );
        assert!(
            oracle_root
                .join(fixture_id)
                .join("outputs")
                .join("feff.inp")
                .is_file(),
            "oracle capture should materialize fixture inputs for '{}'",
            fixture_id
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("pot.inp")
                .is_file(),
            "run-rdinp should materialize Rust outputs under actual-root/<fixture>/actual for '{}'",
            fixture_id
        );
    }
    assert!(
        report_path.is_file(),
        "oracle command should emit a regression report"
    );
    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("report should be readable"))
            .expect("report JSON should parse");
    assert_eq!(parsed["mismatch_fixture_count"], Value::from(2));
    assert!(
        parsed["mismatch_artifact_count"]
            .as_u64()
            .map(|count| count > 0)
            .unwrap_or(false),
        "oracle report should include artifact-level mismatch entries"
    );
    let mismatch_fixtures = parsed["mismatch_fixtures"]
        .as_array()
        .expect("mismatch_fixtures should be an array");
    for (fixture_id, _) in fixtures {
        let fixture_report = mismatch_fixtures
            .iter()
            .find(|fixture| fixture["fixture_id"].as_str() == Some(fixture_id))
            .unwrap_or_else(|| panic!("missing mismatch report for fixture '{}'", fixture_id));
        let artifact_reports = fixture_report["artifacts"]
            .as_array()
            .expect("fixture artifact list should be an array");
        assert!(
            !artifact_reports.is_empty(),
            "fixture '{}' mismatch report should include artifact details",
            fixture_id
        );
        assert!(
            artifact_reports.iter().all(|artifact| {
                artifact["artifact_path"]
                    .as_str()
                    .is_some_and(|path| !path.is_empty())
                    && artifact["reason"]
                        .as_str()
                        .is_some_and(|reason| !reason.is_empty())
            }),
            "fixture '{}' artifact mismatches should include artifact path and reason",
            fixture_id
        );
    }
}

#[test]
fn oracle_command_runs_pot_parity_for_required_fixtures_and_applies_policy_modes() {
    if !command_available("jq") {
        eprintln!("Skipping POT oracle CLI test because jq is unavailable in PATH.");
        return;
    }

    let temp = TempDir::new().expect("tempdir should be created");
    let fixtures = [
        ("FX-POT-001", "feff10/examples/EXAFS/Cu"),
        ("FX-WORKFLOW-XAS-001", "feff10/examples/XANES/Cu"),
    ];
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let capture_runner = workspace_root.join("scripts/fortran/ci-oracle-capture-runner.sh");
    assert!(
        capture_runner.is_file(),
        "capture runner should exist at '{}'",
        capture_runner.display()
    );

    let manifest_path = temp.path().join("manifest.json");
    let policy_path = workspace_root.join("tasks/numeric-tolerance-policy.json");
    let oracle_root = temp.path().join("oracle-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/oracle-report.json");
    let fixture_entries = fixtures
        .iter()
        .map(|(fixture_id, input_directory)| {
            let fixture_input_dir = workspace_root.join(input_directory);
            format!(
                r#"
            {{
              "id": "{fixture_id}",
              "modulesCovered": ["RDINP", "POT"],
              "inputDirectory": "{input_directory}",
              "entryFiles": ["feff.inp"]
            }}
            "#,
                fixture_id = fixture_id,
                input_directory = fixture_input_dir.to_string_lossy().replace('\\', "\\\\")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let manifest = format!(
        r#"
        {{
          "fixtures": [{}]
        }}
        "#,
        fixture_entries
    );
    write_file(&manifest_path, &manifest);

    for (fixture_id, _) in fixtures {
        let staged_output_dir = actual_root.join(fixture_id).join("actual");
        stage_workspace_fixture_file(fixture_id, "xmu.dat", &staged_output_dir.join("xmu.dat"));
        stage_workspace_fixture_file(
            fixture_id,
            "paths.dat",
            &staged_output_dir.join("paths.dat"),
        );
    }

    let workspace_root_arg = workspace_root.to_string_lossy().replace('\'', "'\"'\"'");
    let capture_runner_arg = capture_runner.to_string_lossy().replace('\'', "'\"'\"'");
    let capture_runner_command = format!(
        "GITHUB_WORKSPACE='{}' '{}'",
        workspace_root_arg, capture_runner_arg
    );
    let output = run_oracle_command_with_extra_args(
        &manifest_path,
        &policy_path,
        &oracle_root,
        &actual_root,
        &report_path,
        &[
            "--capture-runner",
            capture_runner_command.as_str(),
            "--run-rdinp",
            "--run-pot",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "oracle POT parity should report mismatches against captured Fortran outputs, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Mismatches:"),
        "oracle summary should include mismatch totals, stdout: {}",
        stdout
    );

    for (fixture_id, _) in fixtures {
        assert!(
            stdout.contains(&format!("Fixture {} mismatches", fixture_id)),
            "oracle summary should include fixture-level mismatch details for '{}', stdout: {}",
            fixture_id,
            stdout
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("pot.bin")
                .is_file(),
            "run-pot should materialize pot.bin for fixture '{}'",
            fixture_id
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("convergence.scf")
                .is_file(),
            "run-pot should materialize convergence.scf for fixture '{}'",
            fixture_id
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("convergence.scf.fine")
                .is_file(),
            "run-pot should materialize convergence.scf.fine for fixture '{}'",
            fixture_id
        );
    }

    assert!(
        report_path.is_file(),
        "oracle command should emit a POT parity report"
    );
    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("report should be readable"))
            .expect("report JSON should parse");
    assert_eq!(parsed["mismatch_fixture_count"], Value::from(2));

    let fixture_reports = parsed["fixtures"]
        .as_array()
        .expect("fixtures report should be an array");
    for (fixture_id, _) in fixtures {
        let fixture_report = fixture_reports
            .iter()
            .find(|fixture| fixture["fixture_id"].as_str() == Some(fixture_id))
            .unwrap_or_else(|| panic!("missing fixture report for '{}'", fixture_id));
        let artifact_reports = fixture_report["artifacts"]
            .as_array()
            .expect("fixture artifact reports should be an array");

        let xmu_report = artifact_reports
            .iter()
            .find(|artifact| artifact["artifact_path"].as_str() == Some("xmu.dat"))
            .unwrap_or_else(|| {
                panic!("fixture '{}' should include xmu.dat comparison", fixture_id)
            });
        assert_eq!(
            xmu_report["comparison"]["mode"],
            Value::from("numeric_tolerance"),
            "xmu.dat should use numeric_tolerance policy mode for fixture '{}'",
            fixture_id
        );
        assert_eq!(
            xmu_report["comparison"]["matched_category"],
            Value::from("columnar_spectra"),
            "xmu.dat should resolve columnar_spectra category for fixture '{}'",
            fixture_id
        );

        let paths_report = artifact_reports
            .iter()
            .find(|artifact| artifact["artifact_path"].as_str() == Some("paths.dat"))
            .unwrap_or_else(|| {
                panic!(
                    "fixture '{}' should include paths.dat comparison",
                    fixture_id
                )
            });
        assert_eq!(
            paths_report["comparison"]["mode"],
            Value::from("exact_text"),
            "paths.dat should use exact_text policy mode for fixture '{}'",
            fixture_id
        );
        assert_eq!(
            paths_report["comparison"]["matched_category"],
            Value::from("path_listing_reports"),
            "paths.dat should resolve path_listing_reports category for fixture '{}'",
            fixture_id
        );
    }
}

fn run_regression_command(
    manifest_path: &Path,
    policy_path: &Path,
    baseline_root: &Path,
    actual_root: &Path,
    report_path: &Path,
) -> std::process::Output {
    run_regression_command_with_extra_args(
        manifest_path,
        policy_path,
        baseline_root,
        actual_root,
        report_path,
        &[],
    )
}

fn run_regression_command_with_extra_args(
    manifest_path: &Path,
    policy_path: &Path,
    baseline_root: &Path,
    actual_root: &Path,
    report_path: &Path,
    extra_args: &[&str],
) -> std::process::Output {
    let binary_path = env!("CARGO_BIN_EXE_feff10-rs");

    let mut command = Command::new(binary_path);
    command
        .arg("regression")
        .arg("--manifest")
        .arg(manifest_path)
        .arg("--policy")
        .arg(policy_path)
        .arg("--baseline-root")
        .arg(baseline_root)
        .arg("--actual-root")
        .arg(actual_root)
        .arg("--baseline-subdir")
        .arg("baseline")
        .arg("--actual-subdir")
        .arg("actual")
        .arg("--report")
        .arg(report_path);
    command.args(extra_args);
    command.output().expect("regression command should run")
}

fn run_oracle_command_with_extra_args(
    manifest_path: &Path,
    policy_path: &Path,
    oracle_root: &Path,
    actual_root: &Path,
    report_path: &Path,
    extra_args: &[&str],
) -> std::process::Output {
    let binary_path = env!("CARGO_BIN_EXE_feff10-rs");

    let mut command = Command::new(binary_path);
    command
        .arg("oracle")
        .arg("--manifest")
        .arg(manifest_path)
        .arg("--policy")
        .arg(policy_path)
        .arg("--oracle-root")
        .arg(oracle_root)
        .arg("--actual-root")
        .arg(actual_root)
        .arg("--oracle-subdir")
        .arg("outputs")
        .arg("--actual-subdir")
        .arg("actual")
        .arg("--report")
        .arg(report_path);
    command.args(extra_args);
    command.output().expect("oracle command should run")
}

fn write_fixture_file(
    root: &Path,
    fixture_id: &str,
    subdir: &str,
    relative_path: &str,
    content: &str,
) {
    let path = root.join(fixture_id).join(subdir).join(relative_path);
    write_file(&path, content);
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should be created");
    }
    fs::write(path, content).expect("file should be written");
}

fn stage_workspace_fixture_file(fixture_id: &str, relative_path: &str, destination: &Path) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join(relative_path);
    let source_bytes = fs::read(&source)
        .unwrap_or_else(|_| panic!("fixture baseline should be readable: {}", source.display()));
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination parent should be created");
    }
    fs::write(destination, source_bytes).expect("fixture baseline should be staged");
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}
