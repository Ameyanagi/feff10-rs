use feff_core::numerics::special::{
    convolve_lorentzian, eigen_decompose, eigenvalues, integrate_somm, interpolate_spectrum_linear,
    lu_factorize, lu_invert, lu_solve, spherical_h, spherical_j, spherical_n, wigner_3j, wigner_6j,
    y_lm, DenseComplexMatrix, LorentzianConvolutionInput, SommInput, SpectralInterpolationInput,
};
use feff_core::numerics::special::{Wigner3jInput, Wigner6jInput};
use num_complex::Complex64;
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
struct MathRegressionFixtures {
    bessel_cases: Vec<BesselCase>,
    harmonics_cases: Vec<HarmonicsCase>,
    wigner3j_cases: Vec<Wigner3jCase>,
    wigner6j_cases: Vec<Wigner6jCase>,
    lu_solve_cases: Vec<LuSolveCase>,
    lu_invert_cases: Vec<LuInvertCase>,
    eigen_cases: Vec<EigenCase>,
    integration_cases: Vec<IntegrationCase>,
    interpolation_cases: Vec<InterpolationCase>,
    convolution_cases: Vec<ConvolutionCase>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ComplexValue {
    re: f64,
    im: f64,
}

impl ComplexValue {
    fn as_complex(self) -> Complex64 {
        Complex64::new(self.re, self.im)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BesselCase {
    id: String,
    kind: BesselKind,
    order: usize,
    argument: ComplexValue,
    expected: ComplexValue,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum BesselKind {
    J,
    N,
    H,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HarmonicsCase {
    id: String,
    degree: i32,
    order: i32,
    theta: f64,
    phi: f64,
    expected: ComplexValue,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wigner3jInputFixture {
    two_j1: i32,
    two_j2: i32,
    two_j3: i32,
    two_m1: i32,
    two_m2: i32,
    two_m3: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wigner3jCase {
    id: String,
    input: Wigner3jInputFixture,
    expected: f64,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wigner6jInputFixture {
    two_j1: i32,
    two_j2: i32,
    two_j3: i32,
    two_j4: i32,
    two_j5: i32,
    two_j6: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wigner6jCase {
    id: String,
    input: Wigner6jInputFixture,
    expected: f64,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LuSolveCase {
    id: String,
    matrix: Vec<Vec<ComplexValue>>,
    rhs: Vec<ComplexValue>,
    expected: Vec<ComplexValue>,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LuInvertCase {
    id: String,
    matrix: Vec<Vec<ComplexValue>>,
    expected_inverse: Vec<Vec<ComplexValue>>,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EigenCase {
    id: String,
    matrix: Vec<Vec<ComplexValue>>,
    expected_eigenvalues: Vec<ComplexValue>,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntegrationCase {
    id: String,
    radial_grid: Vec<f64>,
    dp: Vec<f64>,
    dq: Vec<f64>,
    log_step: f64,
    near_zero_exponent: f64,
    power: i32,
    expected: f64,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InterpolationCase {
    id: String,
    energy: f64,
    energy_grid: Vec<f64>,
    spectrum: Vec<ComplexValue>,
    expected: ComplexValue,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvolutionCase {
    id: String,
    energy_grid: Vec<f64>,
    spectrum: Vec<ComplexValue>,
    broadening: f64,
    expected: Vec<ComplexValue>,
    abs_tol: f64,
    rel_tol: f64,
}

#[test]
fn bessel_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.bessel_cases {
        let actual = match case.kind {
            BesselKind::J => spherical_j(case.order, case.argument.as_complex()),
            BesselKind::N => spherical_n(case.order, case.argument.as_complex()),
            BesselKind::H => spherical_h(case.order, case.argument.as_complex()),
        };

        assert_complex_close(
            &case.id,
            case.expected.as_complex(),
            actual,
            case.abs_tol,
            case.rel_tol,
        );
    }
}

#[test]
fn harmonics_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.harmonics_cases {
        let actual = y_lm(case.degree, case.order, case.theta, case.phi);
        assert_complex_close(
            &case.id,
            case.expected.as_complex(),
            actual,
            case.abs_tol,
            case.rel_tol,
        );
    }
}

#[test]
fn wigner_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.wigner3j_cases {
        let input = Wigner3jInput::new(
            case.input.two_j1,
            case.input.two_j2,
            case.input.two_j3,
            case.input.two_m1,
            case.input.two_m2,
            case.input.two_m3,
        );
        let actual = wigner_3j(input);
        assert_scalar_close(&case.id, case.expected, actual, case.abs_tol, case.rel_tol);
    }

    for case in fixtures.wigner6j_cases {
        let input = Wigner6jInput::new(
            case.input.two_j1,
            case.input.two_j2,
            case.input.two_j3,
            case.input.two_j4,
            case.input.two_j5,
            case.input.two_j6,
        );
        let actual = wigner_6j(input);
        assert_scalar_close(&case.id, case.expected, actual, case.abs_tol, case.rel_tol);
    }
}

#[test]
fn linalg_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.lu_solve_cases {
        let matrix = dense_matrix(&case.matrix);
        let rhs = complex_vec(&case.rhs);
        let expected = complex_vec(&case.expected);

        let decomposition = lu_factorize(&matrix)
            .unwrap_or_else(|error| panic!("{} lu_factorize should succeed: {}", case.id, error));
        let actual = decomposition
            .solve(&rhs)
            .unwrap_or_else(|error| panic!("{} LU solve should succeed: {}", case.id, error));
        assert_complex_vector_close(&case.id, &expected, &actual, case.abs_tol, case.rel_tol);

        let direct = lu_solve(&matrix, &rhs)
            .unwrap_or_else(|error| panic!("{} lu_solve should succeed: {}", case.id, error));
        assert_complex_vector_close(
            &format!("{} (direct)", case.id),
            &expected,
            &direct,
            case.abs_tol,
            case.rel_tol,
        );
    }

    for case in fixtures.lu_invert_cases {
        let matrix = dense_matrix(&case.matrix);
        let expected = dense_matrix(&case.expected_inverse);

        let decomposition = lu_factorize(&matrix)
            .unwrap_or_else(|error| panic!("{} lu_factorize should succeed: {}", case.id, error));
        let via_decomposition = decomposition
            .invert()
            .unwrap_or_else(|error| panic!("{} LU invert should succeed: {}", case.id, error));
        assert_complex_matrix_close(
            &case.id,
            &expected,
            &via_decomposition,
            case.abs_tol,
            case.rel_tol,
        );

        let direct = lu_invert(&matrix)
            .unwrap_or_else(|error| panic!("{} lu_invert should succeed: {}", case.id, error));
        assert_complex_matrix_close(
            &format!("{} (direct)", case.id),
            &expected,
            &direct,
            case.abs_tol,
            case.rel_tol,
        );
    }

    for case in fixtures.eigen_cases {
        let matrix = dense_matrix(&case.matrix);
        let expected = complex_vec(&case.expected_eigenvalues);

        let decomposition = eigen_decompose(&matrix).unwrap_or_else(|error| {
            panic!("{} eigen_decompose should succeed: {}", case.id, error)
        });
        assert_complex_vector_close(
            &case.id,
            &expected,
            decomposition.eigenvalues(),
            case.abs_tol,
            case.rel_tol,
        );

        let direct = eigenvalues(&matrix)
            .unwrap_or_else(|error| panic!("{} eigenvalues should succeed: {}", case.id, error));
        assert_complex_vector_close(
            &format!("{} (direct)", case.id),
            &expected,
            &direct,
            case.abs_tol,
            case.rel_tol,
        );
    }
}

#[test]
fn integration_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.integration_cases {
        let actual = integrate_somm(SommInput::new(
            &case.radial_grid,
            &case.dp,
            &case.dq,
            case.log_step,
            case.near_zero_exponent,
            case.power,
        ))
        .unwrap_or_else(|error| panic!("{} integrate_somm should succeed: {}", case.id, error));

        assert_scalar_close(&case.id, case.expected, actual, case.abs_tol, case.rel_tol);
    }
}

#[test]
fn convolution_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.interpolation_cases {
        let spectrum = complex_vec(&case.spectrum);
        let actual = interpolate_spectrum_linear(SpectralInterpolationInput::new(
            case.energy,
            &case.energy_grid,
            &spectrum,
        ))
        .unwrap_or_else(|error| {
            panic!(
                "{} interpolate_spectrum_linear should succeed: {}",
                case.id, error
            )
        });

        assert_complex_close(
            &case.id,
            case.expected.as_complex(),
            actual,
            case.abs_tol,
            case.rel_tol,
        );
    }

    for case in fixtures.convolution_cases {
        let spectrum = complex_vec(&case.spectrum);
        let expected = complex_vec(&case.expected);
        let actual = convolve_lorentzian(LorentzianConvolutionInput::new(
            &case.energy_grid,
            &spectrum,
            case.broadening,
        ))
        .unwrap_or_else(|error| {
            panic!("{} convolve_lorentzian should succeed: {}", case.id, error)
        });

        assert_complex_vector_close(&case.id, &expected, &actual, case.abs_tol, case.rel_tol);
    }
}

fn load_fixtures() -> MathRegressionFixtures {
    let fixture_path = workspace_root().join("tasks/math-regression-fixtures.json");
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

fn dense_matrix(rows: &[Vec<ComplexValue>]) -> DenseComplexMatrix {
    let nrows = rows.len();
    assert!(nrows > 0, "matrix fixtures must include at least one row");

    let ncols = rows[0].len();
    assert!(
        ncols > 0,
        "matrix fixtures must include at least one column"
    );
    assert!(
        rows.iter().all(|row| row.len() == ncols),
        "matrix fixtures must be rectangular"
    );

    let mut matrix = DenseComplexMatrix::zeros(nrows, ncols);
    for (row_index, row) in rows.iter().enumerate() {
        for (col_index, value) in row.iter().enumerate() {
            matrix[(row_index, col_index)] = value.as_complex();
        }
    }

    matrix
}

fn complex_vec(values: &[ComplexValue]) -> Vec<Complex64> {
    values.iter().map(|value| value.as_complex()).collect()
}

fn assert_complex_matrix_close(
    label: &str,
    expected: &DenseComplexMatrix,
    actual: &DenseComplexMatrix,
    abs_tol: f64,
    rel_tol: f64,
) {
    assert_eq!(expected.nrows(), actual.nrows(), "{} row mismatch", label);
    assert_eq!(expected.ncols(), actual.ncols(), "{} col mismatch", label);

    for row in 0..expected.nrows() {
        for col in 0..expected.ncols() {
            assert_complex_close(
                &format!("{}[{row},{col}]", label),
                expected[(row, col)],
                actual[(row, col)],
                abs_tol,
                rel_tol,
            );
        }
    }
}

fn assert_complex_vector_close(
    label: &str,
    expected: &[Complex64],
    actual: &[Complex64],
    abs_tol: f64,
    rel_tol: f64,
) {
    assert_eq!(
        expected.len(),
        actual.len(),
        "{} vector length mismatch",
        label
    );

    for (index, (expected_value, actual_value)) in expected.iter().zip(actual).enumerate() {
        assert_complex_close(
            &format!("{}[{index}]", label),
            *expected_value,
            *actual_value,
            abs_tol,
            rel_tol,
        );
    }
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

fn assert_complex_close(
    label: &str,
    expected: Complex64,
    actual: Complex64,
    abs_tol: f64,
    rel_tol: f64,
) {
    let abs_diff = (actual - expected).norm();
    let rel_diff = abs_diff / expected.norm().max(1.0);

    assert!(
        abs_diff <= abs_tol || rel_diff <= rel_tol,
        "{} expected=({:.15e},{:.15e}) actual=({:.15e},{:.15e}) abs_diff={:.15e} rel_diff={:.15e} abs_tol={:.15e} rel_tol={:.15e}",
        label,
        expected.re,
        expected.im,
        actual.re,
        actual.im,
        abs_diff,
        rel_diff,
        abs_tol,
        rel_tol
    );
}
