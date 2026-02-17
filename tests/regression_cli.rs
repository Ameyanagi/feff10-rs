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

#[test]
fn oracle_command_runs_screen_parity_for_optional_screen_input_cases() {
    if !command_available("jq") {
        eprintln!("Skipping SCREEN oracle CLI test because jq is unavailable in PATH.");
        return;
    }

    let temp = TempDir::new().expect("tempdir should be created");
    let fixture_id = "FX-SCREEN-001";
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let capture_runner = workspace_root.join("scripts/fortran/ci-oracle-capture-runner.sh");
    assert!(
        capture_runner.is_file(),
        "capture runner should exist at '{}'",
        capture_runner.display()
    );

    let manifest_path = temp.path().join("manifest.json");
    let policy_path = workspace_root.join("tasks/numeric-tolerance-policy.json");
    let fixture_input_dir = workspace_root.join("feff10/examples/MPSE/Cu_OPCONS");
    let manifest = format!(
        r#"
        {{
          "fixtures": [
            {{
              "id": "{fixture_id}",
              "modulesCovered": ["SCREEN"],
              "inputDirectory": "{input_directory}",
              "entryFiles": ["feff.inp"]
            }}
          ]
        }}
        "#,
        fixture_id = fixture_id,
        input_directory = fixture_input_dir.to_string_lossy().replace('\\', "\\\\")
    );
    write_file(&manifest_path, &manifest);

    let workspace_root_arg = workspace_root.to_string_lossy().replace('\'', "'\"'\"'");
    let capture_runner_arg = capture_runner.to_string_lossy().replace('\'', "'\"'\"'");
    let capture_runner_command = format!(
        "GITHUB_WORKSPACE='{}' '{}'",
        workspace_root_arg, capture_runner_arg
    );

    let with_override_oracle_root = temp.path().join("oracle-root-with-override");
    let with_override_actual_root = temp.path().join("actual-root-with-override");
    let with_override_report_path = temp.path().join("report/oracle-screen-with-override.json");
    let with_override_staged_output_dir = with_override_actual_root.join(fixture_id).join("actual");
    stage_workspace_fixture_file(
        fixture_id,
        "pot.inp",
        &with_override_staged_output_dir.join("pot.inp"),
    );
    stage_workspace_fixture_file(
        fixture_id,
        "geom.dat",
        &with_override_staged_output_dir.join("geom.dat"),
    );
    stage_workspace_fixture_file(
        fixture_id,
        "ldos.inp",
        &with_override_staged_output_dir.join("ldos.inp"),
    );
    stage_workspace_fixture_file(
        fixture_id,
        "screen.inp",
        &with_override_staged_output_dir.join("screen.inp"),
    );

    let with_override_output = run_oracle_command_with_extra_args(
        &manifest_path,
        &policy_path,
        &with_override_oracle_root,
        &with_override_actual_root,
        &with_override_report_path,
        &[
            "--capture-runner",
            capture_runner_command.as_str(),
            "--run-screen",
        ],
    );

    assert_eq!(
        with_override_output.status.code(),
        Some(1),
        "SCREEN oracle parity with optional screen.inp should report mismatches against captured outputs, stderr: {}",
        String::from_utf8_lossy(&with_override_output.stderr)
    );
    assert!(
        with_override_actual_root
            .join(fixture_id)
            .join("actual")
            .join("screen.inp")
            .is_file(),
        "with-override case should include optional screen.inp for '{}'",
        fixture_id
    );
    assert!(
        with_override_actual_root
            .join(fixture_id)
            .join("actual")
            .join("wscrn.dat")
            .is_file(),
        "run-screen should materialize wscrn.dat for '{}'",
        fixture_id
    );
    assert!(
        with_override_actual_root
            .join(fixture_id)
            .join("actual")
            .join("logscreen.dat")
            .is_file(),
        "run-screen should materialize logscreen.dat for '{}'",
        fixture_id
    );

    let with_override_stdout = String::from_utf8_lossy(&with_override_output.stdout);
    assert!(
        with_override_stdout.contains("Fixture FX-SCREEN-001 mismatches"),
        "SCREEN oracle summary should include fixture mismatch details, stdout: {}",
        with_override_stdout
    );
    assert!(
        with_override_report_path.is_file(),
        "SCREEN oracle parity should emit a report for optional-screen case"
    );
    let with_override_report: Value = serde_json::from_str(
        &fs::read_to_string(&with_override_report_path).expect("report should be readable"),
    )
    .expect("report JSON should parse");
    assert_eq!(
        with_override_report["mismatch_fixture_count"],
        Value::from(1)
    );
    let with_override_mismatch_fixtures = with_override_report["mismatch_fixtures"]
        .as_array()
        .expect("mismatch_fixtures should be an array");
    let with_override_fixture = with_override_mismatch_fixtures
        .iter()
        .find(|fixture| fixture["fixture_id"].as_str() == Some(fixture_id))
        .expect("SCREEN mismatch report should include fixture");
    let with_override_artifacts = with_override_fixture["artifacts"]
        .as_array()
        .expect("fixture artifact list should be an array");
    assert!(
        with_override_artifacts.iter().all(|artifact| {
            artifact["artifact_path"]
                .as_str()
                .is_some_and(|path| !path.is_empty())
                && artifact["reason"]
                    .as_str()
                    .is_some_and(|reason| !reason.is_empty())
        }),
        "SCREEN mismatch artifacts should include deterministic path and reason fields"
    );

    let without_override_oracle_root = temp.path().join("oracle-root-without-override");
    let without_override_actual_root = temp.path().join("actual-root-without-override");
    let without_override_report_path = temp
        .path()
        .join("report/oracle-screen-without-override.json");
    let staged_output_dir = without_override_actual_root.join(fixture_id).join("actual");
    stage_workspace_fixture_file(fixture_id, "pot.inp", &staged_output_dir.join("pot.inp"));
    stage_workspace_fixture_file(fixture_id, "geom.dat", &staged_output_dir.join("geom.dat"));
    stage_workspace_fixture_file(fixture_id, "ldos.inp", &staged_output_dir.join("ldos.inp"));
    assert!(
        !staged_output_dir.join("screen.inp").exists(),
        "test setup should omit optional screen.inp override input"
    );

    let without_override_output = run_oracle_command_with_extra_args(
        &manifest_path,
        &policy_path,
        &without_override_oracle_root,
        &without_override_actual_root,
        &without_override_report_path,
        &[
            "--capture-runner",
            capture_runner_command.as_str(),
            "--run-screen",
        ],
    );

    assert_eq!(
        without_override_output.status.code(),
        Some(1),
        "SCREEN oracle parity without optional screen.inp should still run and report mismatches, stderr: {}",
        String::from_utf8_lossy(&without_override_output.stderr)
    );
    assert!(
        !without_override_actual_root
            .join(fixture_id)
            .join("actual")
            .join("screen.inp")
            .is_file(),
        "run-screen should not require screen.inp to be present for '{}'",
        fixture_id
    );
    assert!(
        without_override_actual_root
            .join(fixture_id)
            .join("actual")
            .join("wscrn.dat")
            .is_file(),
        "run-screen should materialize wscrn.dat without optional override for '{}'",
        fixture_id
    );
    assert!(
        without_override_actual_root
            .join(fixture_id)
            .join("actual")
            .join("logscreen.dat")
            .is_file(),
        "run-screen should materialize logscreen.dat without optional override for '{}'",
        fixture_id
    );

    let without_override_stdout = String::from_utf8_lossy(&without_override_output.stdout);
    assert!(
        without_override_stdout.contains("Fixture FX-SCREEN-001 mismatches"),
        "SCREEN oracle summary should include fixture mismatch details, stdout: {}",
        without_override_stdout
    );
    assert!(
        without_override_report_path.is_file(),
        "SCREEN oracle parity should emit a report for missing-optional-input case"
    );
    let without_override_report: Value = serde_json::from_str(
        &fs::read_to_string(&without_override_report_path).expect("report should be readable"),
    )
    .expect("report JSON should parse");
    assert_eq!(
        without_override_report["mismatch_fixture_count"],
        Value::from(1)
    );
    let without_override_mismatch_fixtures = without_override_report["mismatch_fixtures"]
        .as_array()
        .expect("mismatch_fixtures should be an array");
    let without_override_fixture = without_override_mismatch_fixtures
        .iter()
        .find(|fixture| fixture["fixture_id"].as_str() == Some(fixture_id))
        .expect("SCREEN mismatch report should include fixture");
    let without_override_artifacts = without_override_fixture["artifacts"]
        .as_array()
        .expect("fixture artifact list should be an array");
    let missing_optional_override_report = without_override_artifacts
        .iter()
        .find(|artifact| artifact["artifact_path"].as_str() == Some("screen.inp"))
        .expect("missing optional screen.inp should be reported deterministically");
    assert_eq!(
        missing_optional_override_report["reason"],
        Value::from("Missing actual artifact"),
        "optional screen.inp absence should map to deterministic report reason"
    );
}

#[test]
fn oracle_command_runs_crpa_parity_and_reports_tolerance_and_contract_failures() {
    if !command_available("jq") {
        eprintln!("Skipping CRPA oracle CLI test because jq is unavailable in PATH.");
        return;
    }

    let temp = TempDir::new().expect("tempdir should be created");
    let fixture_id = "FX-CRPA-001";
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let capture_runner = workspace_root.join("scripts/fortran/ci-oracle-capture-runner.sh");
    assert!(
        capture_runner.is_file(),
        "capture runner should exist at '{}'",
        capture_runner.display()
    );

    let manifest_path = temp.path().join("manifest.json");
    let policy_path = temp.path().join("policy.json");
    let oracle_root = temp.path().join("oracle-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/oracle-crpa.json");
    let fixture_input_dir = workspace_root.join("feff10/examples/CRPA");

    let manifest = format!(
        r#"
        {{
          "fixtures": [
            {{
              "id": "{fixture_id}",
              "modulesCovered": ["CRPA"],
              "inputDirectory": "{input_directory}",
              "entryFiles": ["feff.inp"]
            }}
          ]
        }}
        "#,
        fixture_id = fixture_id,
        input_directory = fixture_input_dir.to_string_lossy().replace('\\', "\\\\")
    );
    write_file(&manifest_path, &manifest);
    write_file(
        &policy_path,
        r#"
        {
          "defaultMode": "exact_text",
          "categories": [
            {
              "id": "crpa_screen_numeric",
              "mode": "numeric_tolerance",
              "fileGlobs": ["**/wscrn.dat"],
              "tolerance": {
                "absTol": 1e-8,
                "relTol": 1e-6,
                "relativeFloor": 1e-12
              }
            }
          ]
        }
        "#,
    );

    let staged_output_dir = actual_root.join(fixture_id).join("actual");
    stage_workspace_fixture_file(fixture_id, "crpa.inp", &staged_output_dir.join("crpa.inp"));
    stage_workspace_fixture_file(fixture_id, "pot.inp", &staged_output_dir.join("pot.inp"));
    stage_workspace_fixture_file(fixture_id, "geom.dat", &staged_output_dir.join("geom.dat"));
    write_file(
        &staged_output_dir.join("missing-baseline-artifact.dat"),
        "synthetic contract mismatch\n",
    );

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
            "--run-crpa",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "CRPA oracle parity should report mismatches against captured outputs, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Fixture FX-CRPA-001 mismatches"),
        "CRPA oracle summary should include fixture mismatch details, stdout: {}",
        stdout
    );
    assert!(
        actual_root
            .join(fixture_id)
            .join("actual")
            .join("wscrn.dat")
            .is_file(),
        "run-crpa should materialize wscrn.dat for '{}'",
        fixture_id
    );
    assert!(
        actual_root
            .join(fixture_id)
            .join("actual")
            .join("logscrn.dat")
            .is_file(),
        "run-crpa should materialize logscrn.dat for '{}'",
        fixture_id
    );
    assert!(
        report_path.is_file(),
        "CRPA oracle parity should emit a report"
    );

    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("report should be readable"))
            .expect("report JSON should parse");
    assert_eq!(parsed["mismatch_fixture_count"], Value::from(1));

    let fixture_reports = parsed["fixtures"]
        .as_array()
        .expect("fixtures report should be an array");
    let fixture_report = fixture_reports
        .iter()
        .find(|fixture| fixture["fixture_id"].as_str() == Some(fixture_id))
        .expect("CRPA report should include fixture");
    let artifact_reports = fixture_report["artifacts"]
        .as_array()
        .expect("fixture artifact reports should be an array");
    let wscrn_report = artifact_reports
        .iter()
        .find(|artifact| artifact["artifact_path"].as_str() == Some("wscrn.dat"))
        .expect("CRPA fixture should include wscrn.dat comparison");
    assert_eq!(
        wscrn_report["comparison"]["mode"],
        Value::from("numeric_tolerance"),
        "wscrn.dat should be compared using numeric_tolerance mode"
    );
    assert_eq!(
        wscrn_report["comparison"]["matched_category"],
        Value::from("crpa_screen_numeric"),
        "wscrn.dat should resolve the CRPA numeric policy category"
    );
    assert_eq!(
        wscrn_report["comparison"]["metrics"]["kind"],
        Value::from("numeric_tolerance"),
        "wscrn.dat comparison should emit numeric tolerance metrics"
    );
    assert!(
        wscrn_report["comparison"]["metrics"]["compared_values"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "wscrn.dat tolerance metrics should include compared values"
    );

    let mismatch_fixtures = parsed["mismatch_fixtures"]
        .as_array()
        .expect("mismatch_fixtures should be an array");
    let mismatch_fixture = mismatch_fixtures
        .iter()
        .find(|fixture| fixture["fixture_id"].as_str() == Some(fixture_id))
        .expect("mismatch_fixtures should include CRPA fixture");
    let mismatch_artifacts = mismatch_fixture["artifacts"]
        .as_array()
        .expect("mismatch artifact list should be an array");
    let contract_failure = mismatch_artifacts
        .iter()
        .find(|artifact| {
            artifact["artifact_path"].as_str() == Some("missing-baseline-artifact.dat")
        })
        .expect("CRPA parity should report synthetic contract mismatch artifact");
    assert_eq!(
        contract_failure["reason"],
        Value::from("Missing baseline artifact"),
        "contract mismatch artifact should map to deterministic missing-baseline reason"
    );
}

#[test]
fn oracle_command_runs_xsph_parity_for_required_fixtures_with_optional_wscrn_cases() {
    if !command_available("jq") {
        eprintln!("Skipping XSPH oracle CLI test because jq is unavailable in PATH.");
        return;
    }

    let temp = TempDir::new().expect("tempdir should be created");
    let fixtures = [
        ("FX-XSPH-001", "feff10/examples/XANES/Cu"),
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
    let policy_path = temp.path().join("policy.json");
    let oracle_root = temp.path().join("oracle-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/oracle-xsph.json");

    let fixture_entries = fixtures
        .iter()
        .map(|(fixture_id, input_directory)| {
            let fixture_input_dir = workspace_root.join(input_directory);
            format!(
                r#"
            {{
              "id": "{fixture_id}",
              "modulesCovered": ["XSPH"],
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
        r##"
        {
          "defaultMode": "exact_text",
          "numericParsing": {
            "commentPrefixes": ["#"]
          },
          "categories": [
            {
              "id": "xsph_cross_section_numeric",
              "mode": "numeric_tolerance",
              "fileGlobs": ["**/xsect.dat"],
              "tolerance": {
                "absTol": 1e-8,
                "relTol": 1e-6,
                "relativeFloor": 1e-12
              }
            }
          ]
        }
        "##,
    );

    for (fixture_id, _) in fixtures {
        let staged_output_dir = actual_root.join(fixture_id).join("actual");
        stage_workspace_fixture_file(fixture_id, "xsph.inp", &staged_output_dir.join("xsph.inp"));
        stage_workspace_fixture_file(fixture_id, "geom.dat", &staged_output_dir.join("geom.dat"));
        stage_workspace_fixture_file(
            fixture_id,
            "global.inp",
            &staged_output_dir.join("global.inp"),
        );
        stage_workspace_fixture_file(fixture_id, "pot.bin", &staged_output_dir.join("pot.bin"));

        if fixture_id == "FX-XSPH-001" {
            stage_workspace_fixture_file(
                fixture_id,
                "wscrn.dat",
                &staged_output_dir.join("wscrn.dat"),
            );
        }
    }
    assert!(
        actual_root
            .join("FX-XSPH-001")
            .join("actual")
            .join("wscrn.dat")
            .is_file(),
        "setup should include optional wscrn.dat for FX-XSPH-001"
    );
    assert!(
        !actual_root
            .join("FX-WORKFLOW-XAS-001")
            .join("actual")
            .join("wscrn.dat")
            .is_file(),
        "setup should omit optional wscrn.dat for FX-WORKFLOW-XAS-001"
    );

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
            "--run-xsph",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "XSPH oracle parity should report mismatches against captured outputs, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Mismatches:"),
        "XSPH oracle summary should include mismatch totals, stdout: {}",
        stdout
    );
    for (fixture_id, _) in fixtures {
        assert!(
            stdout.contains(&format!("Fixture {} mismatches", fixture_id)),
            "XSPH oracle summary should include fixture-level mismatch details for '{}', stdout: {}",
            fixture_id,
            stdout
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("phase.bin")
                .is_file(),
            "run-xsph should materialize phase.bin for fixture '{}'",
            fixture_id
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("xsect.dat")
                .is_file(),
            "run-xsph should materialize xsect.dat for fixture '{}'",
            fixture_id
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("log2.dat")
                .is_file(),
            "run-xsph should materialize log2.dat for fixture '{}'",
            fixture_id
        );
    }
    assert!(
        report_path.is_file(),
        "XSPH oracle parity should emit a report"
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

        let phase_report = artifact_reports
            .iter()
            .find(|artifact| artifact["artifact_path"].as_str() == Some("phase.bin"))
            .unwrap_or_else(|| {
                panic!(
                    "fixture '{}' should include phase.bin comparison",
                    fixture_id
                )
            });
        assert_eq!(
            phase_report["comparison"]["mode"],
            Value::from("exact_text"),
            "phase.bin should remain an exact-text/binary comparison artifact for fixture '{}'",
            fixture_id
        );
        assert_eq!(
            phase_report["comparison"]["metrics"]["kind"],
            Value::from("exact_text"),
            "phase.bin should report exact_text metrics for fixture '{}'",
            fixture_id
        );

        let xsect_report = artifact_reports
            .iter()
            .find(|artifact| artifact["artifact_path"].as_str() == Some("xsect.dat"))
            .unwrap_or_else(|| {
                panic!(
                    "fixture '{}' should include xsect.dat comparison",
                    fixture_id
                )
            });
        assert_eq!(
            xsect_report["comparison"]["mode"],
            Value::from("numeric_tolerance"),
            "xsect.dat should be compared using numeric_tolerance mode for fixture '{}'",
            fixture_id
        );
        assert_eq!(
            xsect_report["comparison"]["matched_category"],
            Value::from("xsph_cross_section_numeric"),
            "xsect.dat should resolve the XSPH numeric category for fixture '{}'",
            fixture_id
        );
        assert_eq!(
            xsect_report["comparison"]["metrics"]["kind"],
            Value::from("numeric_tolerance"),
            "xsect.dat comparison should emit numeric tolerance metrics for fixture '{}'",
            fixture_id
        );
        assert!(
            xsect_report["comparison"]["metrics"]["compared_values"]
                .as_u64()
                .is_some_and(|count| count > 0),
            "xsect.dat tolerance metrics should include compared values for fixture '{}'",
            fixture_id
        );
    }

    let mismatch_fixtures = parsed["mismatch_fixtures"]
        .as_array()
        .expect("mismatch_fixtures should be an array");
    let missing_optional_override_fixture = mismatch_fixtures
        .iter()
        .find(|fixture| fixture["fixture_id"].as_str() == Some("FX-WORKFLOW-XAS-001"))
        .expect("mismatch_fixtures should include FX-WORKFLOW-XAS-001");
    let missing_optional_override_artifacts = missing_optional_override_fixture["artifacts"]
        .as_array()
        .expect("fixture artifact list should be an array");
    let missing_wscrn_report = missing_optional_override_artifacts
        .iter()
        .find(|artifact| artifact["artifact_path"].as_str() == Some("wscrn.dat"))
        .expect("missing optional wscrn.dat should be reported deterministically");
    assert_eq!(
        missing_wscrn_report["reason"],
        Value::from("Missing actual artifact"),
        "optional wscrn.dat absence should map to deterministic report reason"
    );
}

#[test]
fn oracle_command_runs_path_parity_for_required_fixtures_and_applies_path_listing_policy() {
    if !command_available("jq") {
        eprintln!("Skipping PATH oracle CLI test because jq is unavailable in PATH.");
        return;
    }

    let temp = TempDir::new().expect("tempdir should be created");
    let fixtures = [
        ("FX-PATH-001", "feff10/examples/EXAFS/Cu"),
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
    let report_path = temp.path().join("report/oracle-path.json");

    let fixture_entries = fixtures
        .iter()
        .map(|(fixture_id, input_directory)| {
            let fixture_input_dir = workspace_root.join(input_directory);
            format!(
                r#"
            {{
              "id": "{fixture_id}",
              "modulesCovered": ["PATH"],
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
        stage_workspace_fixture_file(
            fixture_id,
            "paths.inp",
            &staged_output_dir.join("paths.inp"),
        );
        stage_workspace_fixture_file(fixture_id, "geom.dat", &staged_output_dir.join("geom.dat"));
        stage_workspace_fixture_file(
            fixture_id,
            "global.inp",
            &staged_output_dir.join("global.inp"),
        );
        stage_workspace_fixture_file(
            fixture_id,
            "phase.bin",
            &staged_output_dir.join("phase.bin"),
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
            "--run-path",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "PATH oracle parity should report mismatches against captured outputs, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Mismatches:"),
        "PATH oracle summary should include mismatch totals, stdout: {}",
        stdout
    );
    for (fixture_id, _) in fixtures {
        assert!(
            stdout.contains(&format!("Fixture {} mismatches", fixture_id)),
            "PATH oracle summary should include fixture-level mismatch details for '{}', stdout: {}",
            fixture_id,
            stdout
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("paths.dat")
                .is_file(),
            "run-path should materialize paths.dat for fixture '{}'",
            fixture_id
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("paths.bin")
                .is_file(),
            "run-path should materialize paths.bin for fixture '{}'",
            fixture_id
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("crit.dat")
                .is_file(),
            "run-path should materialize crit.dat for fixture '{}'",
            fixture_id
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("log4.dat")
                .is_file(),
            "run-path should materialize log4.dat for fixture '{}'",
            fixture_id
        );
    }
    assert!(
        report_path.is_file(),
        "PATH oracle parity should emit a report"
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

    let mismatch_fixtures = parsed["mismatch_fixtures"]
        .as_array()
        .expect("mismatch_fixtures should be an array");
    for (fixture_id, _) in fixtures {
        let mismatch_fixture = mismatch_fixtures
            .iter()
            .find(|fixture| fixture["fixture_id"].as_str() == Some(fixture_id))
            .unwrap_or_else(|| panic!("missing mismatch report for fixture '{}'", fixture_id));
        let mismatch_artifacts = mismatch_fixture["artifacts"]
            .as_array()
            .expect("fixture mismatch artifact list should be an array");
        assert!(
            !mismatch_artifacts.is_empty(),
            "fixture '{}' mismatch report should include artifact details",
            fixture_id
        );
        assert!(
            mismatch_artifacts.iter().all(|artifact| {
                artifact["artifact_path"]
                    .as_str()
                    .is_some_and(|path| !path.is_empty())
                    && artifact["reason"]
                        .as_str()
                        .is_some_and(|reason| !reason.is_empty())
            }),
            "fixture '{}' mismatch artifacts should include deterministic path and reason fields",
            fixture_id
        );
    }
}

#[test]
fn oracle_command_runs_fms_parity_for_required_fixtures_and_reports_binary_contracts() {
    if !command_available("jq") {
        eprintln!("Skipping FMS oracle CLI test because jq is unavailable in PATH.");
        return;
    }

    let temp = TempDir::new().expect("tempdir should be created");
    let fixtures = [
        ("FX-FMS-001", "feff10/examples/XANES/Cu"),
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
    let report_path = temp.path().join("report/oracle-fms.json");

    let fixture_entries = fixtures
        .iter()
        .map(|(fixture_id, input_directory)| {
            let fixture_input_dir = workspace_root.join(input_directory);
            format!(
                r#"
            {{
              "id": "{fixture_id}",
              "modulesCovered": ["FMS"],
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
        stage_workspace_fixture_file(fixture_id, "fms.inp", &staged_output_dir.join("fms.inp"));
        stage_workspace_fixture_file(fixture_id, "geom.dat", &staged_output_dir.join("geom.dat"));
        stage_workspace_fixture_file(
            fixture_id,
            "global.inp",
            &staged_output_dir.join("global.inp"),
        );
        stage_workspace_fixture_file(
            fixture_id,
            "phase.bin",
            &staged_output_dir.join("phase.bin"),
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
            "--run-fms",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "FMS oracle parity should report mismatches against captured outputs, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Mismatches:"),
        "FMS oracle summary should include mismatch totals, stdout: {}",
        stdout
    );
    for (fixture_id, _) in fixtures {
        assert!(
            stdout.contains(&format!("Fixture {} mismatches", fixture_id)),
            "FMS oracle summary should include fixture-level mismatch details for '{}', stdout: {}",
            fixture_id,
            stdout
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("gg.bin")
                .is_file(),
            "run-fms should materialize gg.bin for fixture '{}'",
            fixture_id
        );
        assert!(
            actual_root
                .join(fixture_id)
                .join("actual")
                .join("log3.dat")
                .is_file(),
            "run-fms should materialize log3.dat for fixture '{}'",
            fixture_id
        );
    }
    assert!(
        report_path.is_file(),
        "FMS oracle parity should emit a report"
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

        let gg_report = artifact_reports
            .iter()
            .find(|artifact| artifact["artifact_path"].as_str() == Some("gg.bin"))
            .unwrap_or_else(|| panic!("fixture '{}' should include gg.bin comparison", fixture_id));
        assert_eq!(
            gg_report["comparison"]["mode"],
            Value::from("exact_text"),
            "gg.bin should use exact_text comparison mode for fixture '{}'",
            fixture_id
        );
        assert_eq!(
            gg_report["comparison"]["metrics"]["kind"],
            Value::from("exact_text"),
            "gg.bin should report exact_text metrics for fixture '{}'",
            fixture_id
        );
    }
}

#[test]
fn oracle_command_runs_band_parity_with_capture_prerequisite_handling() {
    if !command_available("jq") {
        eprintln!("Skipping BAND oracle CLI test because jq is unavailable in PATH.");
        return;
    }

    let temp = TempDir::new().expect("tempdir should be created");
    let fixture_id = "FX-BAND-001";
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture_input_dir = workspace_root.join("feff10/examples/KSPACE/Cr2GeC");
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
    let report_path = temp.path().join("report/oracle-band.json");

    let baseline_archive = fixture_input_dir.join("REFERENCE.zip");
    let manifest = format!(
        r#"
        {{
          "fixtures": [
            {{
              "id": "{fixture_id}",
              "modulesCovered": ["BAND"],
              "inputDirectory": "{input_directory}",
              "entryFiles": ["feff.inp", "REFERENCE/band.inp"],
              "baselineStatus": "requires_fortran_capture",
              "baselineSources": [
                {{
                  "kind": "reference_archive",
                  "path": "{baseline_archive}"
                }}
              ]
            }}
          ]
        }}
        "#,
        fixture_id = fixture_id,
        input_directory = fixture_input_dir.to_string_lossy().replace('\\', "\\\\"),
        baseline_archive = baseline_archive.to_string_lossy().replace('\\', "\\\\")
    );
    write_file(&manifest_path, &manifest);

    let staged_output_dir = actual_root.join(fixture_id).join("actual");
    stage_band_input_with_fallback(fixture_id, &staged_output_dir.join("band.inp"));
    stage_workspace_fixture_file(fixture_id, "geom.dat", &staged_output_dir.join("geom.dat"));
    stage_workspace_fixture_file(
        fixture_id,
        "global.inp",
        &staged_output_dir.join("global.inp"),
    );
    stage_workspace_fixture_file(
        fixture_id,
        "phase.bin",
        &staged_output_dir.join("phase.bin"),
    );

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
            "--capture-allow-missing-entry-files",
            "--run-band",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "BAND oracle parity should report mismatches against captured outputs, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Mismatches:"),
        "BAND oracle summary should include mismatch totals, stdout: {}",
        stdout
    );
    assert!(
        stdout.contains("Fixture FX-BAND-001 mismatches"),
        "BAND oracle summary should include fixture mismatch details, stdout: {}",
        stdout
    );
    assert!(
        actual_root
            .join(fixture_id)
            .join("actual")
            .join("bandstructure.dat")
            .is_file(),
        "run-band should materialize bandstructure.dat"
    );
    assert!(
        actual_root
            .join(fixture_id)
            .join("actual")
            .join("logband.dat")
            .is_file(),
        "run-band should materialize logband.dat"
    );
    assert!(
        report_path.is_file(),
        "BAND oracle parity should emit a report"
    );

    let capture_metadata_path = oracle_root.join(fixture_id).join("metadata.txt");
    assert!(
        capture_metadata_path.is_file(),
        "capture metadata should be emitted for '{}'",
        fixture_id
    );
    let capture_metadata =
        fs::read_to_string(&capture_metadata_path).expect("capture metadata should be readable");
    assert!(
        capture_metadata.contains("allow_missing_entry_files=1"),
        "capture metadata should record allow-missing-entry-files usage, metadata: {}",
        capture_metadata
    );
    assert!(
        capture_metadata.contains("missing_entry_files=REFERENCE/band.inp"),
        "capture metadata should record unresolved BAND entry file prerequisite, metadata: {}",
        capture_metadata
    );

    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("report should be readable"))
            .expect("report JSON should parse");
    assert_eq!(parsed["mismatch_fixture_count"], Value::from(1));

    let fixture_reports = parsed["fixtures"]
        .as_array()
        .expect("fixtures report should be an array");
    let fixture_report = fixture_reports
        .iter()
        .find(|fixture| fixture["fixture_id"].as_str() == Some(fixture_id))
        .expect("fixture report should exist for FX-BAND-001");
    let artifact_reports = fixture_report["artifacts"]
        .as_array()
        .expect("fixture artifact reports should be an array");
    assert!(
        artifact_reports
            .iter()
            .any(|artifact| artifact["artifact_path"].as_str() == Some("bandstructure.dat")),
        "fixture report should include bandstructure.dat artifact entry"
    );
}

#[test]
fn oracle_command_run_screen_input_mismatch_emits_deterministic_diagnostic_contract() {
    if !command_available("jq") {
        eprintln!("Skipping SCREEN oracle CLI test because jq is unavailable in PATH.");
        return;
    }

    let temp = TempDir::new().expect("tempdir should be created");
    let fixture_id = "FX-SCREEN-001";
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture_input_dir = workspace_root.join("feff10/examples/MPSE/Cu_OPCONS");

    let manifest_path = temp.path().join("manifest.json");
    let policy_path = temp.path().join("policy.json");
    let oracle_root = temp.path().join("oracle-root");
    let actual_root = temp.path().join("actual-root");
    let report_path = temp.path().join("report/oracle-report.json");

    let manifest = format!(
        r#"
        {{
          "fixtures": [
            {{
              "id": "{fixture_id}",
              "modulesCovered": ["SCREEN"],
              "inputDirectory": "{input_directory}",
              "entryFiles": ["feff.inp"]
            }}
          ]
        }}
        "#,
        fixture_id = fixture_id,
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

    let output = run_oracle_command_with_extra_args(
        &manifest_path,
        &policy_path,
        &oracle_root,
        &actual_root,
        &report_path,
        &["--capture-runner", ":", "--run-screen"],
    );

    assert_eq!(
        output.status.code(),
        Some(3),
        "missing SCREEN required input should map to deterministic IO fatal exit code, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ERROR: [IO.SCREEN_INPUT_READ]"),
        "stderr should include SCREEN input-contract placeholder, stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("pot.inp"),
        "stderr should identify the missing required SCREEN input artifact, stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("FATAL EXIT CODE: 3"),
        "stderr should include fatal exit summary line, stderr: {}",
        stderr
    );
    assert!(
        !report_path.exists(),
        "fatal SCREEN input-contract failures should not emit an oracle report"
    );
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

fn stage_band_input_with_fallback(fixture_id: &str, destination: &Path) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts/fortran-baselines")
        .join(fixture_id)
        .join("baseline")
        .join("band.inp");

    if source.is_file() {
        let source_bytes = fs::read(&source).unwrap_or_else(|_| {
            panic!("fixture baseline should be readable: {}", source.display())
        });
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should be created");
        }
        fs::write(destination, source_bytes).expect("fixture baseline should be staged");
        return;
    }

    write_file(destination, default_band_input_source());
}

fn default_band_input_source() -> &'static str {
    "mband : calculate bands if = 1\n   1\nemin, emax, estep : energy mesh\n    -8.00000      6.00000      0.05000\nnkp : # points in k-path\n 121\nikpath : type of k-path\n   2\nfreeprop :  empty lattice if = T\n F\n"
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}
