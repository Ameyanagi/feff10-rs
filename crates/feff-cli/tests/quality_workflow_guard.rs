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

fn read_quality_workflow() -> String {
    let workflow_path = workspace_root().join(".github/workflows/rust-quality-gates.yml");
    fs::read_to_string(&workflow_path).unwrap_or_else(|error| {
        panic!(
            "quality workflow file {} should be readable: {}",
            workflow_path.display(),
            error
        )
    })
}

fn assert_contains_once(haystack: &str, needle: &str) {
    let match_count = haystack.matches(needle).count();
    assert_eq!(
        match_count, 1,
        "quality workflow should contain exactly one '{}' command, found {}",
        needle, match_count
    );
}

#[test]
fn rust_quality_workflow_keeps_strict_locked_gate_contract() {
    let workflow = read_quality_workflow();

    assert!(
        workflow.contains("push:"),
        "quality workflow should trigger on push"
    );
    assert!(
        workflow.contains("pull_request:"),
        "quality workflow should trigger on pull_request"
    );

    assert_contains_once(&workflow, "cargo check --locked");
    assert_contains_once(&workflow, "cargo test --locked");
    assert_contains_once(
        &workflow,
        "cargo clippy --locked --all-targets -- -D warnings",
    );
    assert_contains_once(&workflow, "cargo fmt --all -- --check");

    let check_index = workflow
        .find("cargo check --locked")
        .expect("quality workflow should define cargo check --locked");
    let test_index = workflow
        .find("cargo test --locked")
        .expect("quality workflow should define cargo test --locked");
    let clippy_index = workflow
        .find("cargo clippy --locked --all-targets -- -D warnings")
        .expect("quality workflow should define strict clippy command");
    let fmt_index = workflow
        .find("cargo fmt --all -- --check")
        .expect("quality workflow should define format check command");

    assert!(
        check_index < test_index && test_index < clippy_index && clippy_index < fmt_index,
        "quality workflow should run check -> test -> clippy -> fmt in order"
    );
    assert!(
        !workflow.contains("continue-on-error: true"),
        "quality workflow must not enable continue-on-error"
    );
    assert!(
        !workflow.contains("-A warnings"),
        "quality workflow must not relax clippy warnings"
    );
}
