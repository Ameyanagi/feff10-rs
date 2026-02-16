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

fn run_regression_command(
    manifest_path: &Path,
    policy_path: &Path,
    baseline_root: &Path,
    actual_root: &Path,
    report_path: &Path,
) -> std::process::Output {
    let binary_path = env!("CARGO_BIN_EXE_feff10-rs");

    Command::new(binary_path)
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
        .arg(report_path)
        .output()
        .expect("regression command should run")
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
