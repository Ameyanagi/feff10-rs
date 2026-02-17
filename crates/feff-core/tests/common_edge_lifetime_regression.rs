use feff_core::common::edge::{core_hole_lifetime_ev, hole_code_from_edge_spec, is_edge};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EdgeLifetimeFixtures {
    edge_lifetime_cases: Vec<EdgeLifetimeCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EdgeLifetimeCase {
    id: String,
    atomic_number: i32,
    edge: String,
    expected_gamach_ev: f64,
    abs_tol: f64,
    rel_tol: f64,
}

#[test]
fn common_edge_lifetime_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.edge_lifetime_cases {
        let hole_code = hole_code_from_edge_spec(&case.edge)
            .unwrap_or_else(|| panic!("{} should map to a hole code", case.id));
        let actual = core_hole_lifetime_ev(case.atomic_number, hole_code);
        assert_scalar_close(
            &case.id,
            case.expected_gamach_ev,
            actual,
            case.abs_tol,
            case.rel_tol,
        );
    }
}

#[test]
fn isedge_equivalent_recognizes_common_labels_and_codes() {
    for value in ["K", "L1", "L2", "L3", "1", "2", "3", "4"] {
        assert!(
            is_edge(value),
            "'{}' should be recognized as an edge",
            value
        );
    }

    for value in ["", "L9", "Q1", "41", "-1", "abc"] {
        assert!(
            !is_edge(value),
            "'{}' should not be recognized as an edge",
            value
        );
    }
}

fn load_fixtures() -> EdgeLifetimeFixtures {
    let fixture_path = workspace_root().join("tasks/common-edge-lifetime-fixtures.json");
    let source = fs::read_to_string(&fixture_path).unwrap_or_else(|error| {
        panic!(
            "fixture file {} should be readable: {}",
            fixture_path.display(),
            error
        )
    });

    serde_json::from_str(&source).unwrap_or_else(|error| {
        panic!(
            "fixture file {} should parse as JSON: {}",
            fixture_path.display(),
            error
        )
    })
}

fn assert_scalar_close(label: &str, expected: f64, actual: f64, abs_tol: f64, rel_tol: f64) {
    let abs_diff = (actual - expected).abs();
    let rel_diff = abs_diff / expected.abs().max(1.0);

    assert!(
        abs_diff <= abs_tol || rel_diff <= rel_tol,
        "{} expected={:.15e} actual={:.15e} abs_diff={:.15e} rel_diff={:.15e} abs_tol={:.15e} rel_tol={:.15e}",
        label,
        expected,
        actual,
        abs_diff,
        rel_diff,
        abs_tol,
        rel_tol
    );
}
