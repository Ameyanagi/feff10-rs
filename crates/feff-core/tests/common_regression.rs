use feff_core::common::config::{
    atomic_number_for_symbol, configuration_for_atomic_number, element_symbol,
    getorb_for_atomic_number, shell_orbitals_for_atomic_number, ConfigurationRecipe, ElectronShell,
};
use feff_core::common::constants::{ALPHFS, ALPINV, BOHR, FA, HART, PI, PI2, RADDEG, RYD};
use feff_core::common::edge::{
    core_hole_lifetime_ev, edge_label_from_hole_code, hole_code_from_edge_spec, is_edge,
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
struct CommonRegressionFixtures {
    constant_cases: Vec<ConstantCase>,
    configuration_cases: Vec<ConfigurationCase>,
    orbital_cases: Vec<OrbitalCase>,
    edge_lifetime_cases: Vec<EdgeLifetimeCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConstantCase {
    id: String,
    constant: CommonConstant,
    expected: f64,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CommonConstant {
    Pi,
    Pi2,
    Fa,
    Bohr,
    Ryd,
    Hart,
    Alpinv,
    Alphfs,
    Raddeg,
}

impl CommonConstant {
    fn value(self) -> f64 {
        match self {
            Self::Pi => PI,
            Self::Pi2 => PI2,
            Self::Fa => FA,
            Self::Bohr => BOHR,
            Self::Ryd => RYD,
            Self::Hart => HART,
            Self::Alpinv => ALPINV,
            Self::Alphfs => ALPHFS,
            Self::Raddeg => RADDEG,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigurationCase {
    id: String,
    atomic_number: usize,
    recipe: RecipeFixture,
    expected_symbol: Option<String>,
    expected_total_occupation: f64,
    expected_total_valence: Option<f64>,
    expected_occupied_orbital_count: Option<usize>,
    expected_spin_sum: Option<f64>,
    expected_first_occupations: Option<Vec<f64>>,
    expected_first_valence: Option<Vec<f64>>,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RecipeFixture {
    Feff9,
    Feff7,
}

impl RecipeFixture {
    fn as_recipe(self) -> ConfigurationRecipe {
        match self {
            Self::Feff9 => ConfigurationRecipe::Feff9,
            Self::Feff7 => ConfigurationRecipe::Feff7,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrbitalCase {
    id: String,
    atomic_number: usize,
    recipe: RecipeFixture,
    shell: ShellFixture,
    expected_orbital_indices: Vec<usize>,
    expected_occupations: Vec<f64>,
    #[serde(default)]
    projection_expectations: Vec<ProjectionExpectation>,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ShellFixture {
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
}

impl ShellFixture {
    fn as_shell(self) -> ElectronShell {
        match self {
            Self::K => ElectronShell::K,
            Self::L => ElectronShell::L,
            Self::M => ElectronShell::M,
            Self::N => ElectronShell::N,
            Self::O => ElectronShell::O,
            Self::P => ElectronShell::P,
            Self::Q => ElectronShell::Q,
            Self::R => ElectronShell::R,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectionExpectation {
    kappa: i32,
    expected_orbital_index: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EdgeLifetimeCase {
    id: String,
    atomic_number: i32,
    edge_spec: String,
    expected_hole_code: i32,
    expected_edge_label: String,
    expected_gamach_ev: f64,
    abs_tol: f64,
    rel_tol: f64,
}

#[test]
fn common_constant_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.constant_cases {
        let actual = case.constant.value();
        assert_scalar_close(&case.id, case.expected, actual, case.abs_tol, case.rel_tol);
    }
}

#[test]
fn common_constant_relationships_match_feff_common_invariants() {
    assert_scalar_close("PI2=2PI", 2.0 * PI, PI2, 1.0e-15, 1.0e-15);
    assert_scalar_close("HART=2RYD", 2.0 * RYD, HART, 1.0e-15, 1.0e-15);
    assert_scalar_close("ALPHFS=1/ALPINV", 1.0 / ALPINV, ALPHFS, 1.0e-15, 1.0e-15);
    assert_scalar_close("RADDEG*PI=180", 180.0, RADDEG * PI, 1.0e-12, 1.0e-12);
}

#[test]
fn common_configuration_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.configuration_cases {
        let recipe = case.recipe.as_recipe();
        let configuration = configuration_for_atomic_number(case.atomic_number, recipe)
            .unwrap_or_else(|| panic!("{} configuration lookup should succeed", case.id));

        assert_scalar_close(
            &format!("{}.totalOccupation", case.id),
            case.expected_total_occupation,
            configuration.total_occupation(),
            case.abs_tol,
            case.rel_tol,
        );

        if let Some(expected_total_valence) = case.expected_total_valence {
            assert_scalar_close(
                &format!("{}.totalValence", case.id),
                expected_total_valence,
                configuration.total_valence(),
                case.abs_tol,
                case.rel_tol,
            );
        }

        if let Some(expected_orbital_count) = case.expected_occupied_orbital_count {
            assert_eq!(
                configuration.occupied_orbital_count(),
                expected_orbital_count,
                "{} occupied orbital count should match",
                case.id
            );
        }

        if let Some(expected_spin_sum) = case.expected_spin_sum {
            let actual_spin_sum: f64 = configuration.spin().iter().sum();
            assert_scalar_close(
                &format!("{}.spinSum", case.id),
                expected_spin_sum,
                actual_spin_sum,
                case.abs_tol,
                case.rel_tol,
            );
        }

        if let Some(expected_first_occupations) = &case.expected_first_occupations {
            let actual_first_occupations =
                &configuration.occupations()[..expected_first_occupations.len()];
            assert_scalar_slice_close(
                &format!("{}.firstOccupations", case.id),
                expected_first_occupations,
                actual_first_occupations,
                case.abs_tol,
                case.rel_tol,
            );
        }

        if let Some(expected_first_valence) = &case.expected_first_valence {
            let actual_first_valence = &configuration.valence()[..expected_first_valence.len()];
            assert_scalar_slice_close(
                &format!("{}.firstValence", case.id),
                expected_first_valence,
                actual_first_valence,
                case.abs_tol,
                case.rel_tol,
            );
        }

        if let Some(expected_symbol) = &case.expected_symbol {
            let actual_symbol = element_symbol(case.atomic_number)
                .unwrap_or_else(|| panic!("{} element symbol lookup should succeed", case.id));
            assert_eq!(
                actual_symbol, expected_symbol,
                "{} symbol lookup should match",
                case.id
            );
            assert_eq!(
                atomic_number_for_symbol(expected_symbol),
                Some(case.atomic_number),
                "{} symbol round-trip should match",
                case.id
            );
        }
    }
}

#[test]
fn common_orbital_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.orbital_cases {
        let recipe = case.recipe.as_recipe();
        let shell = case.shell.as_shell();
        let shell_orbitals = shell_orbitals_for_atomic_number(case.atomic_number, recipe, shell)
            .unwrap_or_else(|| panic!("{} shell lookup should succeed", case.id));

        let actual_indices: Vec<usize> = shell_orbitals
            .iter()
            .map(|orbital| orbital.metadata.orbital_index)
            .collect();
        assert_eq!(
            actual_indices, case.expected_orbital_indices,
            "{} shell orbital indices should match",
            case.id
        );

        let actual_occupations: Vec<f64> = shell_orbitals
            .iter()
            .map(|orbital| orbital.occupation)
            .collect();
        assert_scalar_slice_close(
            &format!("{}.occupations", case.id),
            &case.expected_occupations,
            &actual_occupations,
            case.abs_tol,
            case.rel_tol,
        );

        let extraction = getorb_for_atomic_number(case.atomic_number, recipe)
            .unwrap_or_else(|| panic!("{} getorb extraction should succeed", case.id));
        for projection in case.projection_expectations {
            assert_eq!(
                extraction.projection_orbital_index_for_kappa(projection.kappa),
                projection.expected_orbital_index,
                "{} projection(kappa={}) should match",
                case.id,
                projection.kappa
            );
        }
    }
}

#[test]
fn common_edge_lifetime_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.edge_lifetime_cases {
        assert!(
            is_edge(&case.edge_spec),
            "{} edge spec should be valid",
            case.id
        );

        let hole_code = hole_code_from_edge_spec(&case.edge_spec)
            .unwrap_or_else(|| panic!("{} edge spec should map to hole code", case.id));
        assert_eq!(
            hole_code, case.expected_hole_code,
            "{} hole code should match",
            case.id
        );

        let expected_label = case.expected_edge_label.as_str();
        assert_eq!(
            edge_label_from_hole_code(hole_code),
            Some(expected_label),
            "{} edge label should match",
            case.id
        );

        let numeric_spec = case.expected_hole_code.to_string();
        assert!(
            is_edge(&numeric_spec),
            "{} numeric edge spec should be valid",
            case.id
        );

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

fn load_fixtures() -> CommonRegressionFixtures {
    let fixture_path = workspace_root().join("tasks/common-regression-fixtures.json");
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

fn assert_scalar_slice_close(
    label: &str,
    expected: &[f64],
    actual: &[f64],
    abs_tol: f64,
    rel_tol: f64,
) {
    assert_eq!(
        expected.len(),
        actual.len(),
        "{} length mismatch (expected {}, actual {})",
        label,
        expected.len(),
        actual.len()
    );

    for (index, (expected_value, actual_value)) in expected.iter().zip(actual.iter()).enumerate() {
        assert_scalar_close(
            &format!("{}[{}]", label, index),
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
