use feff_core::numerics::{
    atom_s02_from_overlap, atom_total_energy_from_terms, AtomS02Input, AtomTotalEnergyTerms,
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
struct AtomEnergyS02Fixtures {
    energy_cases: Vec<EnergyCaseFixture>,
    s02_cases: Vec<S02CaseFixture>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnergyCaseFixture {
    id: String,
    occupations: Vec<f64>,
    orbital_energies: Vec<f64>,
    terms: EnergyTermsFixture,
    expected_total_energy: f64,
    abs_tol: f64,
    rel_tol: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnergyTermsFixture {
    direct_coulomb: f64,
    exchange_coulomb: f64,
    magnetic: f64,
    retardation: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct S02CaseFixture {
    id: String,
    kappa: Vec<i32>,
    core_occupations: Vec<f64>,
    core_hole_orbital: Option<usize>,
    overlap_matrix: Vec<f64>,
    expected_s02: f64,
    abs_tol: f64,
    rel_tol: f64,
}

#[test]
fn atom_total_energy_matches_feff_etotal_reference_vectors() {
    let fixtures = load_fixtures();
    for case in &fixtures.energy_cases {
        let terms = AtomTotalEnergyTerms::new(
            case.terms.direct_coulomb,
            case.terms.exchange_coulomb,
            case.terms.magnetic,
            case.terms.retardation,
        );

        let total_energy =
            atom_total_energy_from_terms(&case.orbital_energies, &case.occupations, terms)
                .unwrap_or_else(|error| panic!("{} should compute: {}", case.id, error));

        assert_scalar_close(
            &format!("{}.totalEnergy", case.id),
            case.expected_total_energy,
            total_energy,
            case.abs_tol,
            case.rel_tol,
        );
    }
}

#[test]
fn atom_s02_matches_feff_s02at_reference_vectors() {
    let fixtures = load_fixtures();
    for case in &fixtures.s02_cases {
        let mut input =
            AtomS02Input::new(&case.kappa, &case.core_occupations, &case.overlap_matrix);
        if let Some(core_hole_orbital) = case.core_hole_orbital {
            input = input.with_core_hole_orbital(core_hole_orbital);
        }

        let s02 = atom_s02_from_overlap(input)
            .unwrap_or_else(|error| panic!("{} should compute: {}", case.id, error));

        assert_scalar_close(
            &format!("{}.s02", case.id),
            case.expected_s02,
            s02,
            case.abs_tol,
            case.rel_tol,
        );
    }
}

fn load_fixtures() -> AtomEnergyS02Fixtures {
    let fixture_path = workspace_root().join("tasks/atom-energy-s02-fixtures.json");
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
        rel_tol,
    );
}
