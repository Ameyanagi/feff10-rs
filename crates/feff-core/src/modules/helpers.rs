use crate::domain::{ComputeModule, ComputeRequest, InputCard, InputDeck};
use crate::numerics::{deterministic_argsort, distance3, stable_weighted_mean};
use crate::support::kspace::cgcrac::{CgcracInput, cgcrac};
use crate::support::kspace::factorial_table;
use crate::support::kspace::strconfra::strconfra;
use crate::support::kspace::strfunqjl::strfunqjl;
use crate::support::mkgtr::mkgtr::{MkgtrConfig, MkgtrMode, mkgtr_coupling};
use crate::support::opconsat::opconsat::{OpconsatComponent, opconsat, sample_dielectric};

#[derive(Debug, Clone, PartialEq)]
pub struct DistanceShell {
    pub site_index: usize,
    pub radius: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreModuleHelper {
    module: ComputeModule,
}

impl CoreModuleHelper {
    pub fn new(module: ComputeModule) -> Option<Self> {
        if is_core_module(module) {
            Some(Self { module })
        } else {
            None
        }
    }

    pub fn module(&self) -> ComputeModule {
        self.module
    }

    pub fn sorted_neighbor_shells(
        &self,
        origin: [f64; 3],
        neighbors: &[[f64; 3]],
    ) -> Vec<DistanceShell> {
        let radii: Vec<f64> = neighbors
            .iter()
            .map(|neighbor| distance3(origin, *neighbor))
            .collect();
        let order = deterministic_argsort(&radii);
        order
            .into_iter()
            .map(|site_index| DistanceShell {
                site_index,
                radius: radii[site_index],
            })
            .collect()
    }

    pub fn weighted_channel_average(
        &self,
        channel_values: &[f64],
        channel_weights: &[f64],
    ) -> Option<f64> {
        stable_weighted_mean(channel_values, channel_weights)
    }
}

pub fn is_core_module(module: ComputeModule) -> bool {
    matches!(
        module,
        ComputeModule::Rdinp
            | ComputeModule::Pot
            | ComputeModule::Path
            | ComputeModule::Fms
            | ComputeModule::Xsph
    )
}

pub fn cards_for_compute_request<'a>(
    deck: &'a InputDeck,
    request: &ComputeRequest,
) -> Vec<&'a InputCard> {
    deck.cards_for_module(request.module)
}

pub(crate) fn kspace_workflow_coupling(
    ikpath: i32,
    channel_count: usize,
    nph: usize,
    freeprop: bool,
) -> f64 {
    let factorials = factorial_table(100);
    let l = channel_count.clamp(1, 12);
    let j = nph.min(l);

    let harmonic_prefactor = strfunqjl(&factorials, j, l);
    let clebsch = cgcrac(
        &factorials,
        CgcracInput {
            j1: l as f64,
            j2: l as f64,
            j3: j as f64,
            m1: 0.0,
            m2: 0.0,
            m3: 0.0,
        },
    )
    .abs();

    let aa = ikpath.abs().max(1) as f64 + if freeprop { 0.5 } else { 1.5 };
    let x = (channel_count.max(1) as f64 * 0.35 + nph.max(1) as f64 * 0.15 + 1.0).max(1.0e-6);
    let continued_fraction = strconfra(aa, x).abs();
    let continued_fraction = if continued_fraction.is_finite() {
        continued_fraction.min(2.0)
    } else {
        0.0
    };

    let freeprop_factor = if freeprop { 1.08 } else { 0.96 };
    ((0.85 + harmonic_prefactor * 0.35 + clebsch * 0.4 + continued_fraction * 0.2)
        * freeprop_factor)
        .clamp(0.25, 2.5)
}

pub(crate) fn mkgtr_workflow_coupling(maxl: i32, ner: i32, nei: i32, use_nrixs: bool) -> f64 {
    let mode = if use_nrixs {
        MkgtrMode::Nrixs
    } else {
        MkgtrMode::Standard
    };

    let config = MkgtrConfig {
        mode,
        nsp: if maxl.abs() > 2 { 2 } else { 1 },
        lx: maxl.abs().max(1) as usize,
        channel_count: ((ner.unsigned_abs() as usize / 6) + (nei.unsigned_abs() as usize / 4))
            .clamp(2, 12),
        q_weight: 1.0 + (ner.abs() + nei.abs()) as f64 * 1.0e-3,
        elpty: if use_nrixs { 1.0 } else { -1.0 },
    };

    mkgtr_coupling(&config).unwrap_or(1.0)
}

pub(crate) fn opconsat_workflow_spectrum(
    atomic_numbers: &[usize],
    number_densities: &[f64],
    energies_ev: &[f64],
) -> Option<Vec<(f64, f64, f64)>> {
    if atomic_numbers.is_empty() || atomic_numbers.len() != number_densities.len() {
        return None;
    }

    let components = atomic_numbers
        .iter()
        .zip(number_densities.iter())
        .map(|(&atomic_number, &number_density)| OpconsatComponent {
            atomic_number,
            number_density,
        })
        .collect::<Vec<_>>();

    let mut energy_grid = energies_ev
        .iter()
        .map(|energy| energy.abs().max(1.0e-6))
        .collect::<Vec<_>>();
    if energy_grid.is_empty() {
        return Some(Vec::new());
    }
    energy_grid.sort_by(f64::total_cmp);
    energy_grid.dedup_by(|lhs, rhs| (*lhs - *rhs).abs() <= 1.0e-9);

    let opcons = opconsat(&components, &energy_grid).ok()?;
    energies_ev
        .iter()
        .map(|energy| sample_dielectric(&opcons, energy.abs().max(1.0e-6)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        CoreModuleHelper, cards_for_compute_request, kspace_workflow_coupling,
        mkgtr_workflow_coupling, opconsat_workflow_spectrum,
    };
    use crate::domain::{ComputeModule, ComputeRequest, InputCard, InputCardKind, InputDeck};

    #[test]
    fn module_helpers_consume_typed_input_cards() {
        let request = ComputeRequest::new("FX-001", ComputeModule::Compton, "feff.inp", "out");
        let deck = InputDeck {
            cards: vec![
                InputCard::new("TITLE", InputCardKind::Title, vec!["Cu".to_string()], 1),
                InputCard::new("COMPTON", InputCardKind::Compton, Vec::new(), 2),
                InputCard::new("RIXS", InputCardKind::Rixs, Vec::new(), 3),
            ],
        };

        let cards = cards_for_compute_request(&deck, &request);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].kind, InputCardKind::Title);
        assert_eq!(cards[1].kind, InputCardKind::Compton);
    }

    #[test]
    fn core_module_helper_is_restricted_to_core_modules() {
        assert!(CoreModuleHelper::new(ComputeModule::Pot).is_some());
        assert!(CoreModuleHelper::new(ComputeModule::Compton).is_none());
    }

    #[test]
    fn core_module_helper_uses_numerics_for_deterministic_shell_order() {
        let scaffold = CoreModuleHelper::new(ComputeModule::Path).expect("core scaffold");
        let shells = scaffold.sorted_neighbor_shells(
            [0.0, 0.0, 0.0],
            &[[0.0, 0.0, 2.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
        );

        assert_eq!(shells.len(), 3);
        assert_eq!(shells[0].site_index, 1);
        assert_eq!(shells[1].site_index, 2);
        assert_eq!(shells[2].site_index, 0);
        assert!((shells[0].radius - 1.0).abs() < 1.0e-12);
        assert!((shells[1].radius - 1.0).abs() < 1.0e-12);
        assert!((shells[2].radius - 2.0).abs() < 1.0e-12);
    }

    #[test]
    fn core_module_helper_uses_numerics_for_weighted_channel_average() {
        let scaffold = CoreModuleHelper::new(ComputeModule::Fms).expect("core scaffold");
        let average = scaffold
            .weighted_channel_average(&[2.0, 8.0], &[1.0, 3.0])
            .expect("weighted average");

        assert!((average - 6.5).abs() < 1.0e-12);
        assert_eq!(scaffold.weighted_channel_average(&[1.0], &[0.0]), None);
    }

    #[test]
    fn kspace_workflow_coupling_is_finite_and_bounded() {
        let coupling = kspace_workflow_coupling(2, 8, 4, false);
        assert!(coupling.is_finite());
        assert!((0.25..=2.5).contains(&coupling));
    }

    #[test]
    fn kspace_workflow_coupling_is_deterministic() {
        let first = kspace_workflow_coupling(3, 10, 5, true);
        let second = kspace_workflow_coupling(3, 10, 5, true);
        assert!((first - second).abs() <= 1.0e-14);
    }

    #[test]
    fn kspace_workflow_coupling_respects_freeprop_toggle() {
        let disabled = kspace_workflow_coupling(1, 6, 3, false);
        let enabled = kspace_workflow_coupling(1, 6, 3, true);
        assert!(enabled > disabled);
    }

    #[test]
    fn mkgtr_workflow_coupling_is_deterministic_and_bounded() {
        let first = mkgtr_workflow_coupling(3, 48, 12, true);
        let second = mkgtr_workflow_coupling(3, 48, 12, true);

        assert!((0.10..=4.0).contains(&first));
        assert_eq!(first.to_bits(), second.to_bits());
    }

    #[test]
    fn opconsat_workflow_spectrum_samples_requested_energies() {
        let samples = opconsat_workflow_spectrum(&[29, 8], &[0.8, 0.2], &[1.0, 2.5, 10.0, 50.0])
            .expect("opconsat helper should produce spectrum");

        assert_eq!(samples.len(), 4);
        for (eps1, eps2, loss) in samples {
            assert!(eps1.is_finite());
            assert!(eps2.is_finite());
            assert!(loss.is_finite());
            assert!(eps2 >= 0.0);
            assert!(loss >= 0.0);
        }
    }
}
