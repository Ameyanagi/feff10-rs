use feff_core::numerics::{evaluate_exchange_potential, ExchangeEvaluationInput, ExchangeModel};
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
struct ExchangeRegressionFixtures {
    exchange_cases: Vec<ExchangeCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExchangeCase {
    id: String,
    model: ExchangeFixtureModel,
    electron_density: f64,
    energy: f64,
    wave_number: f64,
    expected: ExchangeValue,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExchangeFixtureModel {
    HedinLundqvist,
    DiracHara,
    VonBarthHedin,
    PerdewZunger,
}

impl ExchangeFixtureModel {
    fn as_exchange_model(self) -> ExchangeModel {
        match self {
            Self::HedinLundqvist => ExchangeModel::HedinLundqvist,
            Self::DiracHara => ExchangeModel::DiracHara,
            Self::VonBarthHedin => ExchangeModel::VonBarthHedin,
            Self::PerdewZunger => ExchangeModel::PerdewZunger,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ExchangeValue {
    real: f64,
    imaginary: f64,
}

#[test]
fn exchange_regression_fixtures_match_reference_outputs() {
    let fixtures = load_fixtures();

    for case in fixtures.exchange_cases {
        let input = ExchangeEvaluationInput::new(
            case.model.as_exchange_model(),
            case.electron_density,
            case.energy,
            case.wave_number,
        );

        let actual = evaluate_exchange_potential(input);
        let repeated = evaluate_exchange_potential(input);
        assert_eq!(
            actual, repeated,
            "{} should be deterministic for fixed inputs",
            case.id
        );

        assert_scalar_close(
            &format!("{}.real", case.id),
            case.expected.real,
            actual.real,
            case.abs_tol,
            case.rel_tol,
        );
        assert_scalar_close(
            &format!("{}.imaginary", case.id),
            case.expected.imaginary,
            actual.imaginary,
            case.abs_tol,
            case.rel_tol,
        );
    }
}

fn load_fixtures() -> ExchangeRegressionFixtures {
    let fixture_path = workspace_root().join("tasks/exchange-regression-fixtures.json");
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
