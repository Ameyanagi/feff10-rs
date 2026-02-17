pub mod exchange;
pub mod radial;
pub mod special;

pub use exchange::{
    evaluate_exchange_potential, ExchangeEvaluation, ExchangeEvaluationInput, ExchangeModel,
    ExchangePotential, ExchangePotentialApi,
};
pub use radial::{
    atom_s02_from_overlap, atom_total_energy_from_terms, compute_atom_scf_outputs,
    estimate_atom_total_energy_terms, solve_atom_scf, solve_bound_state_dirac,
    update_atom_charge_density, update_muffin_tin_potential, AtomChargeDensityUpdate,
    AtomRadialOrbitalInput, AtomS02Input, AtomScfInput, AtomScfIteration, AtomScfKernelError,
    AtomScfOrbitalSpec, AtomScfOutputInput, AtomScfOutputs, AtomScfResult, AtomTotalEnergyTerms,
    BoundStateSolverState, ComplexEnergySolverState, MuffinTinPotentialUpdate, RadialDiracError,
    RadialDiracInput, RadialDiracSolution, RadialExtent, RadialGrid,
};

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const NUMERIC_TOLERANCE_POLICY_PATH: &str = "tasks/numeric-tolerance-policy.json";

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct NumericTolerance {
    #[serde(rename = "absTol")]
    pub abs_tol: f64,
    #[serde(rename = "relTol")]
    pub rel_tol: f64,
    #[serde(rename = "relativeFloor")]
    pub relative_floor: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolicyToleranceComparison {
    pub abs_diff: f64,
    pub rel_diff: f64,
    pub passes: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct NumericTolerancePolicy {
    #[serde(rename = "policyVersion")]
    pub policy_version: String,
    #[serde(rename = "defaultMode")]
    pub default_mode: String,
    #[serde(rename = "matchStrategy")]
    pub match_strategy: String,
    #[serde(default)]
    pub categories: Vec<NumericToleranceCategory>,
}

impl NumericTolerancePolicy {
    pub fn tolerance_for_category(&self, category_id: &str) -> Option<NumericTolerance> {
        self.categories
            .iter()
            .find(|category| category.id == category_id)
            .and_then(|category| category.tolerance)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct NumericToleranceCategory {
    pub id: String,
    pub mode: String,
    #[serde(rename = "fileGlobs", default)]
    pub file_globs: Vec<String>,
    pub tolerance: Option<NumericTolerance>,
}

#[derive(Debug, thiserror::Error)]
pub enum NumericTolerancePolicyError {
    #[error("failed to read numeric tolerance policy '{}': {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse numeric tolerance policy '{}': {source}", path.display())]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

pub fn load_numeric_tolerance_policy(
    policy_path: impl AsRef<Path>,
) -> Result<NumericTolerancePolicy, NumericTolerancePolicyError> {
    let policy_path = policy_path.as_ref();
    let source =
        fs::read_to_string(policy_path).map_err(|source| NumericTolerancePolicyError::Read {
            path: policy_path.to_path_buf(),
            source,
        })?;
    serde_json::from_str(&source).map_err(|source| NumericTolerancePolicyError::Parse {
        path: policy_path.to_path_buf(),
        source,
    })
}

pub fn compare_with_policy_tolerance(
    baseline: f64,
    actual: f64,
    tolerance: NumericTolerance,
) -> PolicyToleranceComparison {
    let abs_diff = (actual - baseline).abs();
    let rel_denominator = baseline.abs().max(tolerance.relative_floor);
    let rel_diff = abs_diff / rel_denominator;
    let passes = abs_diff <= tolerance.abs_tol || abs_diff <= tolerance.rel_tol * rel_denominator;

    PolicyToleranceComparison {
        abs_diff,
        rel_diff,
        passes,
    }
}

pub fn format_numeric_for_policy(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }

    if value == f64::INFINITY {
        return "inf".to_string();
    }

    if value == f64::NEG_INFINITY {
        return "-inf".to_string();
    }

    format!("{value:.15E}")
}

fn kahan_add(sum: &mut f64, correction: &mut f64, value: f64) {
    let corrected = value - *correction;
    let next = *sum + corrected;
    *correction = (next - *sum) - corrected;
    *sum = next;
}

pub fn stable_sum(values: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;

    for &value in values {
        kahan_add(&mut sum, &mut correction, value);
    }

    sum
}

pub fn stable_weighted_sum(values: &[f64], weights: &[f64]) -> Option<f64> {
    if values.len() != weights.len() {
        return None;
    }

    let mut sum = 0.0;
    let mut correction = 0.0;
    for (&value, &weight) in values.iter().zip(weights) {
        kahan_add(&mut sum, &mut correction, value * weight);
    }

    Some(sum)
}

pub fn stable_weighted_mean(values: &[f64], weights: &[f64]) -> Option<f64> {
    if values.len() != weights.len() {
        return None;
    }

    let total_weight = stable_sum(weights);
    if total_weight == 0.0 {
        return None;
    }

    let weighted_sum = stable_weighted_sum(values, weights)?;
    Some(weighted_sum / total_weight)
}

pub fn squared_distance3(lhs: [f64; 3], rhs: [f64; 3]) -> f64 {
    let dx = lhs[0] - rhs[0];
    let dy = lhs[1] - rhs[1];
    let dz = lhs[2] - rhs[2];
    dx * dx + dy * dy + dz * dz
}

pub fn distance3(lhs: [f64; 3], rhs: [f64; 3]) -> f64 {
    squared_distance3(lhs, rhs).sqrt()
}

pub fn deterministic_argsort(values: &[f64]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..values.len()).collect();
    indices.sort_unstable_by(|lhs, rhs| {
        values[*lhs]
            .total_cmp(&values[*rhs])
            .then_with(|| lhs.cmp(rhs))
    });
    indices
}

pub fn linear_grid(start: f64, end: f64, count: usize) -> Option<Vec<f64>> {
    if count < 2 {
        return None;
    }

    let step = (end - start) / ((count - 1) as f64);
    let mut grid = Vec::with_capacity(count);
    for index in 0..count {
        grid.push(start + step * (index as f64));
    }

    if let Some(last) = grid.last_mut() {
        *last = end;
    }

    Some(grid)
}

pub fn interpolate_linear(x: f64, x_grid: &[f64], y_grid: &[f64]) -> Option<f64> {
    if x_grid.len() < 2 || x_grid.len() != y_grid.len() {
        return None;
    }

    if !x_grid.windows(2).all(|window| window[0] <= window[1]) {
        return None;
    }

    if x <= x_grid[0] {
        return Some(y_grid[0]);
    }

    let last_index = x_grid.len() - 1;
    if x >= x_grid[last_index] {
        return Some(y_grid[last_index]);
    }

    let upper = x_grid
        .windows(2)
        .position(|window| x <= window[1])
        .map(|index| index + 1)?;
    let lower = upper - 1;
    let x0 = x_grid[lower];
    let x1 = x_grid[upper];
    if x1 == x0 {
        return Some(y_grid[upper]);
    }

    let interpolation = (x - x0) / (x1 - x0);
    Some(y_grid[lower] + interpolation * (y_grid[upper] - y_grid[lower]))
}

pub fn relative_difference(lhs: f64, rhs: f64, relative_floor: f64) -> f64 {
    let scale = lhs.abs().max(rhs.abs()).max(relative_floor);
    (lhs - rhs).abs() / scale
}

pub fn within_tolerance(
    lhs: f64,
    rhs: f64,
    abs_tol: f64,
    rel_tol: f64,
    relative_floor: f64,
) -> bool {
    let abs_diff = (lhs - rhs).abs();
    abs_diff <= abs_tol || relative_difference(lhs, rhs, relative_floor) <= rel_tol
}

#[cfg(test)]
mod tests {
    use super::{
        compare_with_policy_tolerance, deterministic_argsort, distance3, format_numeric_for_policy,
        interpolate_linear, linear_grid, load_numeric_tolerance_policy, relative_difference,
        stable_sum, stable_weighted_mean, stable_weighted_sum, within_tolerance, NumericTolerance,
        NUMERIC_TOLERANCE_POLICY_PATH,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn stable_sum_reduces_order_loss_for_large_and_small_values() {
        let input = [1.0e16, 1.0, -1.0e16];
        assert_eq!(stable_sum(&input), 0.0);
    }

    #[test]
    fn stable_weighted_sum_validates_shape() {
        assert_eq!(stable_weighted_sum(&[1.0, 2.0], &[0.25]), None);
        let weighted = stable_weighted_sum(&[2.0, 4.0], &[0.5, 0.5]).expect("sum");
        assert!((weighted - 3.0).abs() < 1.0e-12);
    }

    #[test]
    fn stable_weighted_mean_requires_non_zero_total_weight() {
        assert_eq!(stable_weighted_mean(&[1.0, 2.0], &[0.0, 0.0]), None);
        let mean = stable_weighted_mean(&[10.0, 20.0, 40.0], &[1.0, 2.0, 1.0]).expect("mean");
        assert!((mean - 22.5).abs() < 1.0e-12);
    }

    #[test]
    fn distance_helpers_handle_three_dimensional_geometry() {
        let distance = distance3([0.0, 0.0, 0.0], [2.0, 3.0, 6.0]);
        assert!((distance - 7.0).abs() < 1.0e-12);
    }

    #[test]
    fn deterministic_argsort_orders_by_value_then_index() {
        let values = [2.0, 1.0, f64::NAN, 1.0, -0.0, 0.0];
        let order = deterministic_argsort(&values);
        assert_eq!(order, vec![4, 5, 1, 3, 0, 2]);
    }

    #[test]
    fn linear_grid_is_inclusive_and_rejects_invalid_counts() {
        assert_eq!(linear_grid(0.0, 1.0, 1), None);
        let grid = linear_grid(0.0, 2.0, 5).expect("grid");
        assert_eq!(grid, vec![0.0, 0.5, 1.0, 1.5, 2.0]);
    }

    #[test]
    fn interpolate_linear_clamps_and_interpolates() {
        let x_grid = [0.0, 1.0, 2.0];
        let y_grid = [10.0, 20.0, 30.0];

        assert_eq!(interpolate_linear(-1.0, &x_grid, &y_grid), Some(10.0));
        assert_eq!(interpolate_linear(3.0, &x_grid, &y_grid), Some(30.0));
        assert_eq!(interpolate_linear(0.5, &x_grid, &y_grid), Some(15.0));
    }

    #[test]
    fn interpolate_linear_rejects_invalid_grids() {
        assert_eq!(interpolate_linear(0.5, &[0.0], &[1.0]), None);
        assert_eq!(interpolate_linear(0.5, &[0.0, 1.0], &[1.0]), None);
        assert_eq!(
            interpolate_linear(0.5, &[0.0, 2.0, 1.0], &[0.0, 2.0, 1.0]),
            None
        );
    }

    #[test]
    fn relative_difference_uses_relative_floor() {
        let diff = relative_difference(0.0, 1.0e-10, 1.0e-6);
        assert!((diff - 1.0e-4).abs() < 1.0e-12);
    }

    #[test]
    fn within_tolerance_accepts_abs_or_relative_match() {
        assert!(within_tolerance(10.0, 10.001, 1.0e-2, 1.0e-6, 1.0e-12));
        assert!(within_tolerance(1000.0, 1000.2, 1.0e-6, 5.0e-4, 1.0e-12));
        assert!(!within_tolerance(1.0, 1.1, 1.0e-3, 1.0e-3, 1.0e-12));
    }

    #[test]
    fn compare_with_policy_tolerance_uses_baseline_relative_scale() {
        let tolerance = NumericTolerance {
            abs_tol: 1.0e-9,
            rel_tol: 0.1,
            relative_floor: 1.0e-12,
        };
        let comparison = compare_with_policy_tolerance(2.0, 2.19, tolerance);
        assert!((comparison.abs_diff - 0.19).abs() < 1.0e-12);
        assert!((comparison.rel_diff - 0.095).abs() < 1.0e-12);
        assert!(comparison.passes);
    }

    #[test]
    fn format_numeric_for_policy_handles_special_values() {
        assert_eq!(format_numeric_for_policy(f64::NAN), "NaN");
        assert_eq!(format_numeric_for_policy(f64::INFINITY), "inf");
        assert_eq!(format_numeric_for_policy(f64::NEG_INFINITY), "-inf");
        assert_eq!(format_numeric_for_policy(12.5), "1.250000000000000E1");
    }

    #[test]
    fn load_numeric_tolerance_policy_reads_category_tolerance() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let policy_path = workspace_root.join(NUMERIC_TOLERANCE_POLICY_PATH);
        let policy = load_numeric_tolerance_policy(&policy_path).expect("policy should load");
        let tolerance = policy
            .tolerance_for_category("columnar_spectra")
            .expect("columnar_spectra tolerance should exist");

        assert_eq!(tolerance.abs_tol, 1.0e-8);
        assert_eq!(tolerance.rel_tol, 1.0e-6);
        assert_eq!(tolerance.relative_floor, 1.0e-12);
    }
}
