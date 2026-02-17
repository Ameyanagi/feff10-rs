use feff_core::numerics::{
    update_atom_charge_density, update_muffin_tin_potential, AtomRadialOrbitalInput,
    BoundStateSolverState, RadialExtent, RadialGrid,
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
struct AtomScfFixtures {
    atom_scf_cases: Vec<AtomScfCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AtomScfCase {
    id: String,
    grid: GridFixture,
    orbitals: Vec<OrbitalFixture>,
    previous_density_mix: Option<PreviousDensityMixFixture>,
    nuclear_charge: f64,
    muffin_tin_radius: f64,
    expected: AtomScfExpected,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GridFixture {
    mean: f64,
    rms: f64,
    max: f64,
    point_count: usize,
    log_step: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrbitalFixture {
    kind: OrbitalProfileKind,
    effective_charge: f64,
    occupation: f64,
    valence_occupation: Option<f64>,
    small_component_scale: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
enum OrbitalProfileKind {
    #[serde(rename = "hydrogenic_1s")]
    Hydrogenic1s,
    #[serde(rename = "hydrogenic_2p")]
    Hydrogenic2p,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviousDensityMixFixture {
    weight: f64,
    scale: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AtomScfExpected {
    target_charge: f64,
    target_valence_charge: f64,
    charge_abs_tol: f64,
    charge_rel_tol: f64,
    boundary_potential: f64,
    boundary_abs_tol: f64,
    max_outer_potential_magnitude: f64,
    minimum_core_potential: f64,
    min_boundary_enclosed_charge: f64,
    max_boundary_enclosed_charge: f64,
}

#[test]
fn atom_radial_density_and_muffin_tin_updates_match_regression_constraints() {
    let fixtures = load_fixtures();
    for case in fixtures.atom_scf_cases {
        let state = BoundStateSolverState::new(
            RadialGrid::from_extent(
                RadialExtent::new(case.grid.mean, case.grid.rms, case.grid.max),
                case.grid.point_count,
                case.grid.log_step,
            ),
            32,
            0.5,
            case.muffin_tin_radius,
        );
        let radial_points = state.radial_grid().points();

        let mut large_components = Vec::with_capacity(case.orbitals.len());
        let mut small_components = Vec::with_capacity(case.orbitals.len());
        for orbital in &case.orbitals {
            let (large, small) = build_orbital_profile(
                orbital.kind,
                orbital.effective_charge,
                orbital.small_component_scale,
                radial_points,
            );
            large_components.push(large);
            small_components.push(small);
        }

        let orbital_inputs = case
            .orbitals
            .iter()
            .enumerate()
            .map(|(index, orbital)| {
                let base = AtomRadialOrbitalInput::new(
                    orbital.occupation,
                    &large_components[index],
                    &small_components[index],
                );
                match orbital.valence_occupation {
                    Some(valence_occupation) => base.with_valence_occupation(valence_occupation),
                    None => base,
                }
            })
            .collect::<Vec<_>>();

        let previous_density = case.previous_density_mix.as_ref().map(|mix| {
            radial_points
                .iter()
                .enumerate()
                .map(|(index, radius)| {
                    let modulation = 1.0 + 0.04 * (0.125 * index as f64).sin();
                    mix.scale * modulation / (1.0 + radius * radius)
                })
                .collect::<Vec<_>>()
        });
        let mixing = case
            .previous_density_mix
            .as_ref()
            .map(|mix| mix.weight)
            .unwrap_or(1.0);

        let charge_update = update_atom_charge_density(
            &state,
            &orbital_inputs,
            previous_density.as_deref(),
            mixing,
        )
        .unwrap_or_else(|error| {
            panic!(
                "{} charge-density update should succeed: {}",
                case.id, error
            )
        });

        assert_scalar_close(
            &format!("{}.totalCharge", case.id),
            case.expected.target_charge,
            charge_update.total_charge(),
            case.expected.charge_abs_tol,
            case.expected.charge_rel_tol,
        );
        assert_scalar_close(
            &format!("{}.valenceCharge", case.id),
            case.expected.target_valence_charge,
            charge_update.valence_charge(),
            case.expected.charge_abs_tol,
            case.expected.charge_rel_tol,
        );

        let integrated_charge =
            integrate_shell_density(radial_points, charge_update.shell_density());
        assert_scalar_close(
            &format!("{}.integratedCharge", case.id),
            case.expected.target_charge,
            integrated_charge,
            case.expected.charge_abs_tol,
            case.expected.charge_rel_tol,
        );
        assert!(
            charge_update
                .density_4pi()
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0),
            "{} density profile should remain finite and non-negative",
            case.id
        );

        let potential_update = update_muffin_tin_potential(
            &state,
            charge_update.shell_density(),
            case.nuclear_charge,
            Some(case.muffin_tin_radius),
        )
        .unwrap_or_else(|error| panic!("{} muffin-tin update should succeed: {}", case.id, error));

        assert_scalar_close(
            &format!("{}.boundaryPotential", case.id),
            case.expected.boundary_potential,
            potential_update.boundary_value(),
            case.expected.boundary_abs_tol,
            case.expected.boundary_abs_tol,
        );
        let max_outer_potential = potential_update.muffin_tin_potential()
            [potential_update.boundary_index()..]
            .iter()
            .fold(0.0_f64, |current, value| current.max(value.abs()));
        assert!(
            max_outer_potential <= case.expected.max_outer_potential_magnitude,
            "{} outer muffin-tin potential exceeded limit: |V_outer|max={:.15e} limit={:.15e}",
            case.id,
            max_outer_potential,
            case.expected.max_outer_potential_magnitude
        );

        let minimum_core_potential = potential_update.muffin_tin_potential()
            [..=potential_update.boundary_index()]
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        assert!(
            minimum_core_potential <= case.expected.minimum_core_potential,
            "{} core-region potential should remain below {:.15e}; observed {:.15e}",
            case.id,
            case.expected.minimum_core_potential,
            minimum_core_potential
        );

        let boundary_enclosed_charge =
            potential_update.enclosed_charge()[potential_update.boundary_index()];
        assert!(
            boundary_enclosed_charge >= case.expected.min_boundary_enclosed_charge
                && boundary_enclosed_charge <= case.expected.max_boundary_enclosed_charge,
            "{} enclosed charge at muffin-tin boundary out of range: {:.15e} not in [{:.15e}, {:.15e}]",
            case.id,
            boundary_enclosed_charge,
            case.expected.min_boundary_enclosed_charge,
            case.expected.max_boundary_enclosed_charge
        );

        assert!(
            potential_update
                .muffin_tin_potential()
                .iter()
                .all(|value| value.is_finite()),
            "{} muffin-tin potential should remain finite",
            case.id
        );
    }
}

fn build_orbital_profile(
    kind: OrbitalProfileKind,
    effective_charge: f64,
    small_component_scale: f64,
    radial_points: &[f64],
) -> (Vec<f64>, Vec<f64>) {
    let z = effective_charge.abs().max(1.0e-6);
    let mut large = Vec::with_capacity(radial_points.len());
    let mut small = Vec::with_capacity(radial_points.len());

    for radius in radial_points.iter().copied() {
        let radius = radius.max(1.0e-8);
        let profile = match kind {
            OrbitalProfileKind::Hydrogenic1s => 2.0 * z.powf(1.5) * radius * (-z * radius).exp(),
            OrbitalProfileKind::Hydrogenic2p => {
                z.powf(2.5) * radius * radius * (-0.5 * z * radius).exp()
            }
        };
        large.push(profile);
        small.push(profile * small_component_scale);
    }

    (large, small)
}

fn integrate_shell_density(radial_grid: &[f64], shell_density: &[f64]) -> f64 {
    let mut integral = 0.0_f64;
    for index in 1..radial_grid.len() {
        let step = (radial_grid[index] - radial_grid[index - 1]).max(0.0);
        integral += 0.5 * (shell_density[index] + shell_density[index - 1]) * step;
    }
    integral
}

fn load_fixtures() -> AtomScfFixtures {
    let fixture_path = workspace_root().join("tasks/atom-radial-update-fixtures.json");
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
