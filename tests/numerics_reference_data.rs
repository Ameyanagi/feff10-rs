use feff10_rs::numerics::{
    NUMERIC_TOLERANCE_POLICY_PATH, NumericTolerance, compare_with_policy_tolerance,
    deterministic_argsort, format_numeric_for_policy, interpolate_linear, linear_grid,
    load_numeric_tolerance_policy, stable_sum, stable_weighted_mean,
};
use std::fs;
use std::path::PathBuf;

#[test]
fn eels_reference_aggregates_match_approved_baseline() {
    let rows = parse_numeric_table("artifacts/fortran-baselines/FX-EELS-001/baseline/eels.dat");
    let energy = column(&rows, 0);
    let total = column(&rows, 1);
    let weights: Vec<f64> = total.iter().map(|value| value.abs()).collect();

    let tolerance = tolerance_for("columnar_spectra");
    assert_with_tolerance(
        "eels.energy.sum",
        567_416.57,
        stable_sum(&energy),
        tolerance,
    );

    let weighted_mean =
        stable_weighted_mean(&energy, &weights).expect("eels weighted mean should exist");
    assert_with_tolerance(
        "eels.energy.weighted_mean_by_abs_total",
        9_013.041_376_178_21,
        weighted_mean,
        tolerance,
    );
}

#[test]
fn ldos_reference_interpolation_and_order_match_baseline() {
    let rows = parse_numeric_table("artifacts/fortran-baselines/FX-LDOS-001/baseline/ldos00.dat");
    let energy = column(&rows, 0);
    let sdos_up = column(&rows, 1);
    let tolerance = tolerance_for("density_tables");

    assert_with_tolerance("ldos.energy.sum", -200.0, stable_sum(&energy), tolerance);

    let sample = &sdos_up[..8];
    let order = deterministic_argsort(sample);
    assert_eq!(order, vec![7, 6, 5, 4, 3, 2, 1, 0]);

    let x_grid = [rows[9][0], rows[10][0]];
    let y_grid = [rows[9][1], rows[10][1]];
    let midpoint = (x_grid[0] + x_grid[1]) / 2.0;
    let interpolated = interpolate_linear(midpoint, &x_grid, &y_grid)
        .expect("ldos midpoint interpolation should succeed");
    assert_with_tolerance(
        "ldos.sdos_up.midpoint_interpolation",
        9.531_712_5e-4,
        interpolated,
        tolerance,
    );
}

#[test]
fn emesh_reference_grid_prefix_matches_baseline() {
    let rows = parse_numeric_table("artifacts/fortran-baselines/FX-PATH-001/baseline/emesh.dat");
    let k_grid = column(&rows, 2);
    let tolerance = tolerance_for("path_scattering_tables");

    let expected_prefix = &k_grid[..11];
    let generated = linear_grid(
        expected_prefix[0],
        expected_prefix[10],
        expected_prefix.len(),
    )
    .expect("linear grid should generate");

    for (index, (expected, actual)) in expected_prefix.iter().zip(generated.iter()).enumerate() {
        assert_with_tolerance(
            &format!("emesh.k_grid.prefix[{index}]"),
            *expected,
            *actual,
            tolerance,
        );
    }

    let x_grid = [rows[9][2], rows[10][2]];
    let y_grid = [rows[9][1], rows[10][1]];
    let midpoint = (x_grid[0] + x_grid[1]) / 2.0;
    let interpolated = interpolate_linear(midpoint, &x_grid, &y_grid)
        .expect("emesh midpoint interpolation should succeed");
    assert_with_tolerance(
        "emesh.energy.midpoint_interpolation",
        -0.328_990_000_000_000_4,
        interpolated,
        tolerance,
    );
}

fn tolerance_for(category_id: &str) -> NumericTolerance {
    let policy_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(NUMERIC_TOLERANCE_POLICY_PATH);
    let policy = load_numeric_tolerance_policy(&policy_path).unwrap_or_else(|error| {
        panic!(
            "numeric tolerance policy {} should load: {}",
            policy_path.display(),
            error
        )
    });
    policy
        .tolerance_for_category(category_id)
        .unwrap_or_else(|| panic!("policy category '{category_id}' should define tolerance"))
}

fn assert_with_tolerance(label: &str, expected: f64, actual: f64, tolerance: NumericTolerance) {
    let comparison = compare_with_policy_tolerance(expected, actual, tolerance);
    assert!(
        comparison.passes,
        "{} expected={} actual={} abs_diff={} rel_diff={} abs_tol={} rel_tol={} relative_floor={}",
        label,
        format_numeric_for_policy(expected),
        format_numeric_for_policy(actual),
        format_numeric_for_policy(comparison.abs_diff),
        format_numeric_for_policy(comparison.rel_diff),
        format_numeric_for_policy(tolerance.abs_tol),
        format_numeric_for_policy(tolerance.rel_tol),
        format_numeric_for_policy(tolerance.relative_floor)
    );
}

fn parse_numeric_table(relative_path: &str) -> Vec<Vec<f64>> {
    let file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let source = fs::read_to_string(&file_path).unwrap_or_else(|error| {
        panic!(
            "numeric baseline file {} should be readable: {}",
            file_path.display(),
            error
        )
    });

    source
        .lines()
        .enumerate()
        .filter_map(|(line_index, raw_line)| {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }

            let values: Vec<f64> = line
                .split_whitespace()
                .map(|token| {
                    let normalized = normalize_numeric_token(token);
                    normalized.parse::<f64>().unwrap_or_else(|error| {
                        panic!(
                            "line {} in {} has invalid numeric token '{}': {}",
                            line_index + 1,
                            file_path.display(),
                            token,
                            error
                        )
                    })
                })
                .collect();

            Some(values)
        })
        .collect()
}

fn column(rows: &[Vec<f64>], index: usize) -> Vec<f64> {
    rows.iter()
        .enumerate()
        .map(|(row_index, row)| {
            *row.get(index)
                .unwrap_or_else(|| panic!("row {} is missing column {}", row_index + 1, index + 1))
        })
        .collect()
}

fn normalize_numeric_token(token: &str) -> String {
    token.replace('D', "E").replace('d', "e")
}
