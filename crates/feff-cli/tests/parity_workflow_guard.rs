use std::fs;
use std::path::Path;

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read_parity_workflow() -> String {
    let workflow_path = workspace_root().join(".github/workflows/rust-parity-gates.yml");
    fs::read_to_string(&workflow_path).unwrap_or_else(|error| {
        panic!(
            "parity workflow file {} should be readable: {}",
            workflow_path.display(),
            error
        )
    })
}

fn assert_contains_once(haystack: &str, needle: &str) {
    let match_count = haystack.matches(needle).count();
    assert_eq!(
        match_count, 1,
        "parity workflow should contain exactly one '{}' snippet, found {}",
        needle, match_count
    );
}

#[test]
fn rust_parity_workflow_keeps_strict_failure_and_diagnostics_contract() {
    let workflow = read_parity_workflow();

    assert!(
        workflow.contains("push:"),
        "parity workflow should trigger on push"
    );
    assert!(
        workflow.contains("pull_request:"),
        "parity workflow should trigger on pull_request"
    );
    assert!(
        workflow.contains("cargo run --locked -- oracle \\"),
        "parity workflow should run locked oracle command"
    );
    assert!(
        workflow.contains("if [[ -f artifacts/regression/oracle-report.json ]]; then"),
        "parity workflow should render summary from oracle report json when present"
    );

    assert!(
        workflow.contains("\"Mismatched artifact details:\""),
        "parity summary should include mismatch-detail heading"
    );
    assert!(
        workflow.contains("(.mismatch_fixtures // [])[]"),
        "parity summary should iterate mismatch fixtures from report json"
    );
    assert!(
        workflow.contains(".artifacts[]"),
        "parity summary should iterate mismatch artifacts for each fixture"
    );
    assert!(
        workflow.contains("\\(.artifact_path): \\(.reason // \\\"comparison failed\\\")"),
        "parity summary should print per-artifact mismatch diagnostics"
    );

    assert_contains_once(
        &workflow,
        "echo \"parity_exit_code=${parity_exit_code}\" >> \"${GITHUB_OUTPUT}\"",
    );
    assert_contains_once(&workflow, "name: oracle-parity-failure-artifacts");
    assert!(
        workflow.contains("artifacts/regression/oracle-report.json"),
        "parity workflow upload list should include oracle-report.json"
    );
    assert!(
        workflow.contains("artifacts/regression/oracle-diff.txt"),
        "parity workflow upload list should include oracle-diff.txt"
    );
    assert!(
        workflow.contains("artifacts/regression/oracle-summary.txt"),
        "parity workflow upload list should include oracle-summary.txt"
    );
    assert!(
        workflow.contains("artifacts/regression/oracle-stderr.txt"),
        "parity workflow upload list should include oracle-stderr.txt"
    );
    assert_contains_once(&workflow, "if-no-files-found: warn");

    let parity_guard_count = workflow
        .matches("if: steps.parity.outputs.parity_exit_code != '0'")
        .count();
    assert_eq!(
        parity_guard_count, 2,
        "parity workflow should guard both upload and fail-job steps on nonzero parity exit code"
    );

    let upload_index = workflow
        .find("- name: Upload parity diff artifacts")
        .expect("parity workflow should upload parity diagnostics when gate fails");
    let fail_index = workflow
        .find("- name: Fail job on parity mismatch")
        .expect("parity workflow should fail job when parity command exits nonzero");
    assert!(
        upload_index < fail_index,
        "parity workflow should upload diagnostics before failing the job"
    );
    assert!(
        workflow.contains("exit 1"),
        "parity workflow should fail hard on parity mismatches"
    );
    assert!(
        !workflow.contains("continue-on-error: true"),
        "parity workflow must not enable continue-on-error"
    );
}
