use feff_core::numerics::{
    SfconvConvolutionInput, SfconvGridConvolutionInput, convolve_sfconv_grid, convolve_sfconv_point,
};
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
struct SfconvRegressionFixtures {
    point_cases: Vec<SfconvPointCase>,
    grid_cases: Vec<SfconvGridCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SfconvPointCase {
    id: String,
    photoelectron_energy: f64,
    chemical_potential: f64,
    core_hole_lifetime: f64,
    signal_energies: Vec<f64>,
    signal_values: Vec<f64>,
    spectral_energies: Vec<f64>,
    spectral_values: Vec<f64>,
    weights: Vec<f64>,
    use_asymmetric_phase: bool,
    apply_energy_cutoff: bool,
    plasma_frequency: f64,
    expected_magnitude: f64,
    expected_phase: f64,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SfconvGridCase {
    id: String,
    photoelectron_energies: Vec<f64>,
    chemical_potential: f64,
    core_hole_lifetime: f64,
    signal_energies: Vec<f64>,
    signal_values: Vec<f64>,
    spectral_energies: Vec<f64>,
    spectral_values: Vec<f64>,
    weights: Vec<f64>,
    use_asymmetric_phase: bool,
    apply_energy_cutoff: bool,
    plasma_frequency: f64,
    expected_magnitudes: Vec<f64>,
    expected_phases: Vec<f64>,
    abs_tol: f64,
    rel_tol: f64,
}

#[test]
fn sfconv_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.point_cases {
        let weights = parse_weights(&case.weights, &case.id);
        let actual = convolve_sfconv_point(SfconvConvolutionInput::new(
            case.photoelectron_energy,
            case.chemical_potential,
            case.core_hole_lifetime,
            &case.signal_energies,
            &case.signal_values,
            &case.spectral_energies,
            &case.spectral_values,
            weights,
            case.use_asymmetric_phase,
            case.apply_energy_cutoff,
            case.plasma_frequency,
        ))
        .unwrap_or_else(|error| {
            panic!(
                "{} convolve_sfconv_point should succeed: {}",
                case.id, error
            )
        });

        assert_scalar_close(
            &format!("{} magnitude", case.id),
            case.expected_magnitude,
            actual.magnitude,
            case.abs_tol,
            case.rel_tol,
        );
        assert_scalar_close(
            &format!("{} phase", case.id),
            case.expected_phase,
            actual.phase,
            case.abs_tol,
            case.rel_tol,
        );
    }

    for case in fixtures.grid_cases {
        let weights = parse_weights(&case.weights, &case.id);
        let actual = convolve_sfconv_grid(SfconvGridConvolutionInput::new(
            &case.photoelectron_energies,
            case.chemical_potential,
            case.core_hole_lifetime,
            &case.signal_energies,
            &case.signal_values,
            &case.spectral_energies,
            &case.spectral_values,
            weights,
            case.use_asymmetric_phase,
            case.apply_energy_cutoff,
            case.plasma_frequency,
        ))
        .unwrap_or_else(|error| {
            panic!("{} convolve_sfconv_grid should succeed: {}", case.id, error)
        });

        assert_eq!(
            actual.magnitudes.len(),
            case.expected_magnitudes.len(),
            "{} magnitudes length mismatch",
            case.id
        );
        assert_eq!(
            actual.phases.len(),
            case.expected_phases.len(),
            "{} phases length mismatch",
            case.id
        );

        for (index, expected) in case.expected_magnitudes.iter().enumerate() {
            assert_scalar_close(
                &format!("{} magnitude[{index}]", case.id),
                *expected,
                actual.magnitudes[index],
                case.abs_tol,
                case.rel_tol,
            );
        }
        for (index, expected) in case.expected_phases.iter().enumerate() {
            assert_scalar_close(
                &format!("{} phase[{index}]", case.id),
                *expected,
                actual.phases[index],
                case.abs_tol,
                case.rel_tol,
            );
        }
    }
}

fn load_fixtures() -> SfconvRegressionFixtures {
    let fixture_path = workspace_root().join("tasks/sfconv-regression-fixtures.json");
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

fn parse_weights(weights: &[f64], case_id: &str) -> [f64; 8] {
    assert_eq!(
        weights.len(),
        8,
        "{case_id} weights should contain 8 values"
    );
    let mut values = [0.0_f64; 8];
    values.copy_from_slice(weights);
    values
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
