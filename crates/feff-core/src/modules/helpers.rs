use crate::domain::{ComputeModule, ComputeRequest, InputCard, InputDeck};
use crate::numerics::{deterministic_argsort, distance3, stable_weighted_mean};
use crate::support::eelsmdff::mdff_angularmesh::{AngularMeshConfig, mdff_angularmesh};
use crate::support::eelsmdff::mdff_eels::{
    EnergyQMesh, MdffEelsConfig, mdff_eels, normalize_wave_amplitudes,
    scale_sigma_rows_with_wavelength,
};
use crate::support::eelsmdff::mdff_qmesh::{MdffQMeshConfig, MdffQMeshPoint, mdff_qmesh};
use crate::support::eelsmdff::mdff_readsp::{MdffInputKind, MdffReadspConfig, mdff_readsp};
use crate::support::eelsmdff::mdff_wavelength::{
    DEFAULT_H_ON_SQRT_TWO_ME_AU, DEFAULT_ME_C2_EV, mdff_wavelength,
};
use crate::support::kspace::cgcrac::{CgcracInput, cgcrac};
use crate::support::kspace::factorial_table;
use crate::support::kspace::strconfra::strconfra;
use crate::support::kspace::strfunqjl::strfunqjl;
use crate::support::mkgtr::mkgtr::{MkgtrConfig, MkgtrMode, mkgtr_coupling};
use crate::support::opconsat::opconsat::{OpconsatComponent, opconsat, sample_dielectric};
use num_complex::Complex64;
use std::collections::BTreeMap;

const HBARC_ATOMIC_EV_A0: f64 = 3727.3794066;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EelsMdffCouplingSummary {
    pub spectrum_norm: f64,
    pub mean_q_length: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EelsMdffWorkflowConfig {
    pub beam_energy_ev: f64,
    pub beam_direction: [f64; 3],
    pub relativistic_q: bool,
    pub qmesh_radial: usize,
    pub qmesh_angular: usize,
    pub average: bool,
    pub cross_terms: bool,
}

pub(crate) fn eelsmdff_workflow_coupling(
    config: EelsMdffWorkflowConfig,
    xmu_rows: &[(f64, f64, f64, f64)],
) -> Option<EelsMdffCouplingSummary> {
    if xmu_rows.is_empty() {
        return None;
    }

    let nqr = config.qmesh_radial.clamp(1, 3);
    let nqf = config.qmesh_angular.clamp(1, 2);
    let npos = nqr * nqr * nqf;
    let mesh = mdff_angularmesh(&AngularMeshConfig {
        theta_x_center: 0.0,
        theta_y_center: 0.0,
        npos,
        nqr,
        nqf,
        qmodus: 'U',
        th0: 1.0e-3,
        thpart: 0.0012 / nqr as f64,
        acoll: 0.0024,
        aconv: 0.0,
        legacy_manual_hack: false,
    })
    .ok()?;

    let detector_points = mesh
        .theta_x
        .iter()
        .zip(mesh.theta_y.iter())
        .take(4)
        .map(|(&theta_x, &theta_y)| MdffQMeshPoint { theta_x, theta_y })
        .collect::<Vec<_>>();
    if detector_points.is_empty() {
        return None;
    }

    let mut sources_by_ip = BTreeMap::new();
    for ip in 1..=9 {
        let mut source = String::from("# omega e k mu mu0 chi\n");
        for &(energy, mu, mu0, chi) in xmu_rows {
            let value = match ip {
                1 => mu,
                2 => (mu - mu0) * 0.5,
                3 => chi,
                4 => (mu0 - mu) * 0.5,
                5 => mu0,
                6 => chi * 0.25,
                7 => chi * 0.10,
                8 => chi * 0.20,
                9 => (mu + mu0).abs() * 0.5,
                _ => 0.0,
            };
            source.push_str(&format!(
                "{energy:.8} 0.0 0.0 {value:.12E} {mu0:.12E} {chi:.12E}\n"
            ));
        }
        sources_by_ip.insert(ip, source);
    }
    let borrowed_sources = sources_by_ip
        .iter()
        .map(|(&ip, source)| (ip, source.as_str()))
        .collect::<BTreeMap<_, _>>();

    let mut sigma_rows = mdff_readsp(
        &borrowed_sources,
        MdffReadspConfig {
            ipmin: 1,
            ipmax: 9,
            ipstep: if config.cross_terms { 1 } else { 4 },
            average: config.average,
            cross_terms: config.cross_terms,
            spcol: 4,
            input_kind: MdffInputKind::Xmu,
        },
    )
    .ok()?;

    scale_sigma_rows_with_wavelength(
        &mut sigma_rows,
        config.beam_energy_ev.max(1.0),
        HBARC_ATOMIC_EV_A0,
        DEFAULT_ME_C2_EV,
        |energy| mdff_wavelength(energy).unwrap_or(f64::NAN),
    )
    .ok()?;

    let mut q_mesh_rows = Vec::with_capacity(sigma_rows.len());
    for sigma_row in &sigma_rows {
        let scattered_energy_ev = (config.beam_energy_ev - sigma_row.energy_loss_ev).max(1.0);
        let q_mesh = mdff_qmesh(
            &detector_points,
            MdffQMeshConfig {
                beam_energy_ev: config.beam_energy_ev.max(1.0),
                scattered_energy_ev,
                beam_direction: config.beam_direction,
                relativistic_q: config.relativistic_q,
                h_on_sqrt_two_me: DEFAULT_H_ON_SQRT_TWO_ME_AU,
                me_c2_ev: DEFAULT_ME_C2_EV,
            },
        )
        .ok()?;

        q_mesh_rows.push(EnergyQMesh {
            q_vectors: q_mesh.rows.iter().map(|row| row.q_vector).collect(),
            q_lengths_classical: q_mesh
                .rows
                .iter()
                .map(|row| row.q_length_classical)
                .collect(),
        });
    }

    let mut amplitudes = (0..detector_points.len())
        .map(|index| Complex64::new(1.0 - index as f64 * 0.08, index as f64 * 0.05))
        .collect::<Vec<_>>();
    normalize_wave_amplitudes(&mut amplitudes);

    let spectrum = mdff_eels(
        &sigma_rows,
        &q_mesh_rows,
        &amplitudes,
        MdffEelsConfig {
            relativistic_q: config.relativistic_q,
            hbarc_ev: HBARC_ATOMIC_EV_A0,
        },
    )
    .ok()?;

    let spectrum_norm =
        spectrum.x.iter().map(|row| row[0].norm()).sum::<f64>() / spectrum.ne as f64;
    let mut q_length_sum = 0.0_f64;
    let mut q_length_count = 0_usize;
    for row in &q_mesh_rows {
        for &q_length in &row.q_lengths_classical {
            q_length_sum += q_length;
            q_length_count += 1;
        }
    }

    if !spectrum_norm.is_finite() || q_length_count == 0 {
        return None;
    }

    let mean_q_length = q_length_sum / q_length_count as f64;
    if !mean_q_length.is_finite() {
        return None;
    }

    Some(EelsMdffCouplingSummary {
        spectrum_norm,
        mean_q_length,
    })
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
        CoreModuleHelper, EelsMdffWorkflowConfig, cards_for_compute_request,
        eelsmdff_workflow_coupling, kspace_workflow_coupling, mkgtr_workflow_coupling,
        opconsat_workflow_spectrum,
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

    #[test]
    fn eelsmdff_workflow_coupling_is_finite_and_deterministic() {
        let xmu_rows = vec![
            (8979.411, 5.56205e-6, 6.25832e-6, -6.96262e-7),
            (8980.979, 6.61771e-6, 7.52318e-6, -9.05473e-7),
            (8982.398, 7.99662e-6, 9.19560e-6, -1.19897e-6),
            (8983.667, 9.85468e-6, 1.14689e-5, -1.61419e-6),
        ];

        let first = eelsmdff_workflow_coupling(
            EelsMdffWorkflowConfig {
                beam_energy_ev: 300_000.0,
                beam_direction: [0.0, 1.0, 0.0],
                relativistic_q: true,
                qmesh_radial: 5,
                qmesh_angular: 3,
                average: false,
                cross_terms: true,
            },
            &xmu_rows,
        )
        .expect("workflow coupling should be available");
        let second = eelsmdff_workflow_coupling(
            EelsMdffWorkflowConfig {
                beam_energy_ev: 300_000.0,
                beam_direction: [0.0, 1.0, 0.0],
                relativistic_q: true,
                qmesh_radial: 5,
                qmesh_angular: 3,
                average: false,
                cross_terms: true,
            },
            &xmu_rows,
        )
        .expect("workflow coupling should be deterministic");

        assert!(first.spectrum_norm.is_finite());
        assert!(first.mean_q_length.is_finite());
        assert!(first.spectrum_norm > 0.0);
        assert!(first.mean_q_length > 0.0);
        assert_eq!(
            first.spectrum_norm.to_bits(),
            second.spectrum_norm.to_bits()
        );
        assert_eq!(
            first.mean_q_length.to_bits(),
            second.mean_q_length.to_bits()
        );
    }
}
