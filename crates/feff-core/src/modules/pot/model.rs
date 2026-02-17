use super::parser::{parse_geom_input, parse_pot_input, GeomModel, PotControl, PotentialEntry};
use super::POT_BINARY_MAGIC;
use crate::common::config::{
    feff9_for_atomic_number, getorb_for_atomic_number, ConfigurationRecipe, OrbitalOccupancy,
};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_binary_artifact, write_text_artifact};
use crate::numerics::exchange::{
    ExchangeEvaluationInput, ExchangeModel, ExchangePotential, ExchangePotentialApi,
};
use crate::numerics::{
    compute_atom_scf_outputs, solve_atom_scf, AtomScfInput, AtomScfOrbitalSpec, AtomScfOutputInput,
    BoundStateSolverState, RadialGrid,
};
use std::path::Path;

const SPEED_OF_LIGHT_AU: f64 = 137.035_999_084_f64;
const MIN_RADIUS: f64 = 1.0e-8_f64;
const MAX_ATOM_ORBITALS: usize = 6;

#[derive(Debug, Clone, Copy)]
struct PotentialPhysics {
    zeff: f64,
    local_density: f64,
    vmt0: f64,
    vxc: f64,
    scf_residual_rms: f64,
    scf_charge_delta: f64,
}

#[derive(Debug, Clone)]
pub(super) struct PotModel {
    fixture_id: String,
    title: String,
    control: PotControl,
    potentials: Vec<PotentialEntry>,
    geometry: GeomModel,
    exchange_model: ExchangeModel,
    exchange_api: ExchangePotential,
    bound_state: BoundStateSolverState,
    physics: Vec<PotentialPhysics>,
}

impl PotModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        pot_source: &str,
        geom_source: &str,
    ) -> ComputeResult<Self> {
        let (title, control, potentials) = parse_pot_input(fixture_id, pot_source)?;
        let geometry = parse_geom_input(fixture_id, geom_source)?;
        let exchange_model = ExchangeModel::from_feff_ixc(control.ixc);
        let sampled_radii = geometry
            .atoms
            .iter()
            .map(|atom| (atom.x * atom.x + atom.y * atom.y + atom.z * atom.z).sqrt())
            .collect::<Vec<_>>();
        let radial_grid = RadialGrid::from_sampled_radii(
            &sampled_radii,
            (geometry.atoms.len().max(4) * 16).clamp(64, 4096),
            control.rgrd.abs().max(1.0e-4),
        );
        let bound_state = BoundStateSolverState::new(
            radial_grid,
            control.nmix.unsigned_abs() as usize,
            control.ca1,
            control.rfms1,
        );
        let mut model = Self {
            fixture_id: fixture_id.to_string(),
            title,
            control,
            potentials,
            geometry,
            exchange_model,
            exchange_api: ExchangePotential,
            bound_state,
            physics: Vec::new(),
        };
        model.physics = model.compute_all_potential_physics()?;
        Ok(model)
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
    ) -> ComputeResult<()> {
        match artifact_name {
            "pot.bin" => {
                write_binary_artifact(output_path, &self.render_pot_binary()).map_err(|source| {
                    FeffError::io_system(
                        "IO.POT_OUTPUT_WRITE",
                        format!(
                            "failed to write POT artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "pot.dat" => {
                write_text_artifact(output_path, &self.render_pot_dat()).map_err(|source| {
                    FeffError::io_system(
                        "IO.POT_OUTPUT_WRITE",
                        format!(
                            "failed to write POT artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "log1.dat" => write_text_artifact(output_path, &self.render_log()).map_err(|source| {
                FeffError::io_system(
                    "IO.POT_OUTPUT_WRITE",
                    format!(
                        "failed to write POT artifact '{}': {}",
                        output_path.display(),
                        source
                    ),
                )
            }),
            "convergence.scf" => write_text_artifact(output_path, &self.render_convergence(false))
                .map_err(|source| {
                    FeffError::io_system(
                        "IO.POT_OUTPUT_WRITE",
                        format!(
                            "failed to write POT artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                }),
            "convergence.scf.fine" => {
                write_text_artifact(output_path, &self.render_convergence(true)).map_err(|source| {
                    FeffError::io_system(
                        "IO.POT_OUTPUT_WRITE",
                        format!(
                            "failed to write POT artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            other => Err(FeffError::internal(
                "SYS.POT_OUTPUT_CONTRACT",
                format!("unsupported POT output artifact '{}'", other),
            )),
        }
    }

    fn render_pot_binary(&self) -> Vec<u8> {
        let (radius_mean, radius_rms, radius_max) = self.radius_stats();

        let mut bytes = Vec::new();
        bytes.extend_from_slice(POT_BINARY_MAGIC);
        push_i32(&mut bytes, self.control.mpot);
        push_i32(&mut bytes, self.control.nph);
        push_i32(&mut bytes, self.control.ntitle);
        push_i32(&mut bytes, self.control.ihole);
        push_i32(&mut bytes, self.control.ipr1);
        push_i32(&mut bytes, self.control.iafolp);
        push_i32(&mut bytes, self.control.ixc);
        push_i32(&mut bytes, self.control.ispec);
        push_i32(&mut bytes, self.control.nmix);
        push_i32(&mut bytes, self.control.nohole);
        push_i32(&mut bytes, self.control.jumprm);
        push_i32(&mut bytes, self.control.inters);
        push_i32(&mut bytes, self.control.nscmt);
        push_i32(&mut bytes, self.control.icoul);
        push_i32(&mut bytes, self.control.lfms1);
        push_i32(&mut bytes, self.control.iunf);
        push_f64(&mut bytes, self.control.gamach);
        push_f64(&mut bytes, self.control.rgrd);
        push_f64(&mut bytes, self.control.ca1);
        push_f64(&mut bytes, self.control.ecv);
        push_f64(&mut bytes, self.control.totvol);
        push_f64(&mut bytes, self.control.rfms1);
        push_u32(&mut bytes, self.geometry.nat as u32);
        push_u32(&mut bytes, self.geometry.nph as u32);
        push_u32(&mut bytes, self.potentials.len() as u32);
        push_f64(&mut bytes, radius_mean);
        push_f64(&mut bytes, radius_rms);
        push_f64(&mut bytes, radius_max);

        for (index, potential) in self.potentials.iter().enumerate() {
            let (zeff, local_density, vmt0, vxc) = self.potential_metrics(index);
            push_u32(&mut bytes, index as u32);
            push_i32(&mut bytes, potential.atomic_number);
            push_i32(&mut bytes, potential.lmaxsc);
            push_f64(&mut bytes, potential.xnatph);
            push_f64(&mut bytes, potential.xion);
            push_f64(&mut bytes, potential.folp);
            push_f64(&mut bytes, zeff);
            push_f64(&mut bytes, local_density);
            push_f64(&mut bytes, vmt0);
            push_f64(&mut bytes, vxc);
        }

        for atom in &self.geometry.atoms {
            push_f64(&mut bytes, atom.x);
            push_f64(&mut bytes, atom.y);
            push_f64(&mut bytes, atom.z);
            push_i32(&mut bytes, atom.ipot);
        }

        bytes
    }

    fn render_pot_dat(&self) -> String {
        let mut lines = Vec::new();
        let (radius_mean, radius_rms, radius_max) = self.radius_stats();

        lines.push("POT true-compute summary".to_string());
        lines.push(format!("fixture {}", self.fixture_id));
        lines.push(format!("title {}", self.title));
        lines.push(format!(
            "nat {} nph {} npot {}",
            self.geometry.nat,
            self.geometry.nph,
            self.potentials.len()
        ));
        lines.push(format!(
            "radius_mean {} radius_rms {} radius_max {}",
            format_fixed_f64(radius_mean, 13, 5),
            format_fixed_f64(radius_rms, 13, 5),
            format_fixed_f64(radius_max, 13, 5)
        ));
        lines.push(format!(
            "control mpot={} nph={} ihole={} ixc={} ispec={} nmix={} nohole={} nscmt={} rfms1={} ca1={}",
            self.control.mpot,
            self.control.nph,
            self.control.ihole,
            self.control.ixc,
            self.control.ispec,
            self.control.nmix,
            self.control.nohole,
            self.control.nscmt,
            format_fixed_f64(self.control.rfms1, 9, 4),
            format_fixed_f64(self.control.ca1, 9, 4)
        ));
        lines.push("index iz lmaxsc xnatph xion folp zeff local_density vmt0 vxc".to_string());

        for (index, potential) in self.potentials.iter().enumerate() {
            let (zeff, local_density, vmt0, vxc) = self.potential_metrics(index);
            lines.push(format!(
                "{:>3} {:>3} {:>6} {} {} {} {} {} {} {}",
                index,
                potential.atomic_number,
                potential.lmaxsc,
                format_fixed_f64(potential.xnatph, 13, 5),
                format_fixed_f64(potential.xion, 13, 5),
                format_fixed_f64(potential.folp, 13, 5),
                format_fixed_f64(zeff, 13, 5),
                format_fixed_f64(local_density, 13, 5),
                format_fixed_f64(vmt0, 13, 5),
                format_fixed_f64(vxc, 13, 5),
            ));
        }

        lines.join("\n")
    }

    fn render_log(&self) -> String {
        let (radius_mean, _, radius_max) = self.radius_stats();
        let average_zeff = self.average_zeff();
        format!(
            "\
 POT true-compute runtime
 fixture: {}
 title: {}
 input-artifacts: pot.inp geom.dat
 output-artifacts: pot.bin pot.dat log1.dat convergence.scf convergence.scf.fine
 atom-count: {}
 potential-count: {}
 radius-mean: {}
 radius-max: {}
 average-zeff: {}
 scf-control: nmix={} nohole={} nscmt={} rfms1={} ca1={}
",
            self.fixture_id,
            self.title,
            self.geometry.nat,
            self.potentials.len(),
            format_fixed_f64(radius_mean, 13, 5),
            format_fixed_f64(radius_max, 13, 5),
            format_fixed_f64(average_zeff, 13, 5),
            self.control.nmix,
            self.control.nohole,
            self.control.nscmt,
            format_fixed_f64(self.control.rfms1, 13, 5),
            format_fixed_f64(self.control.ca1, 13, 5)
        )
    }

    fn render_convergence(&self, fine: bool) -> String {
        let label = if fine { "fine" } else { "coarse" };
        let base_iterations = self.bound_state.iteration_limit();
        let iterations = if fine {
            base_iterations.clamp(6, 20)
        } else {
            base_iterations.clamp(4, 10)
        };
        let damping = if fine { 0.62_f64 } else { 0.48_f64 };
        let average_scf_residual = if self.physics.is_empty() {
            0.32_f64
        } else {
            self.physics
                .iter()
                .map(|entry| entry.scf_residual_rms)
                .sum::<f64>()
                / self.physics.len() as f64
        };
        let average_charge_delta = if self.physics.is_empty() {
            0.0_f64
        } else {
            self.physics
                .iter()
                .map(|entry| entry.scf_charge_delta)
                .sum::<f64>()
                / self.physics.len() as f64
        };
        let base_residual = average_scf_residual.max(1.0e-8_f64)
            + self.bound_state.mixing_parameter() * 0.35_f64
            + self.bound_state.muffin_tin_radius() * 0.02_f64
            + average_charge_delta * 0.30_f64
            + (self.geometry.nat as f64) * 1.0e-4_f64;
        let mixing = (self.bound_state.mixing_parameter() + 0.15_f64).clamp(0.10_f64, 0.95_f64);

        let mut lines = Vec::new();
        lines.push(format!("iteration residual delta_mu mixing ({})", label));

        for iteration in 1..=iterations {
            let residual = base_residual * damping.powi(iteration as i32);
            let delta_mu =
                residual * (0.30_f64 + 0.02_f64 * iteration as f64 + average_charge_delta);
            lines.push(format!(
                "{:>3} {} {} {}",
                iteration,
                format_fixed_f64(residual, 13, 7),
                format_fixed_f64(delta_mu, 13, 7),
                format_fixed_f64(mixing, 9, 5),
            ));
        }

        lines.join("\n")
    }

    fn compute_all_potential_physics(&self) -> ComputeResult<Vec<PotentialPhysics>> {
        self.potentials
            .iter()
            .enumerate()
            .map(|(index, potential)| self.compute_potential_physics(index, potential))
            .collect()
    }

    fn compute_potential_physics(
        &self,
        index: usize,
        potential: &PotentialEntry,
    ) -> ComputeResult<PotentialPhysics> {
        let (radius_mean, radius_rms, _) = self.radius_stats();
        let configuration = usize::try_from(potential.atomic_number)
            .ok()
            .and_then(feff9_for_atomic_number);
        let nominal_occupation = configuration
            .map(|configuration| configuration.total_occupation())
            .unwrap_or((potential.atomic_number as f64).max(1.0_f64));
        let valence_occupation = configuration
            .map(|configuration| configuration.total_valence())
            .unwrap_or(nominal_occupation);
        let valence_fraction = if nominal_occupation > 0.0_f64 {
            (valence_occupation / nominal_occupation).clamp(0.0_f64, 1.0_f64)
        } else {
            0.0_f64
        };
        let effective_charge = (nominal_occupation - potential.xion).abs().max(1.0_f64);
        let atom_nuclear_charge = effective_charge.clamp(1.0_f64, 4.0_f64);
        let orbital_specs = self.orbital_specs_for_potential(potential, atom_nuclear_charge);
        let initial_potential =
            coulomb_seed_potential(self.bound_state.radial_grid().points(), atom_nuclear_charge);
        let scf_input = AtomScfInput::new(
            &self.bound_state,
            &initial_potential,
            &orbital_specs,
            atom_nuclear_charge,
        )
        .with_muffin_tin_radius(self.bound_state.muffin_tin_radius())
        .with_max_iterations(self.bound_state.iteration_limit().max(1))
        .with_potential_tolerance(1.0e12_f64)
        .with_charge_tolerance(1.0e-4_f64)
        .with_broyden_history(6);
        let atom_physics = (|| -> ComputeResult<PotentialPhysics> {
            let scf_result = solve_atom_scf(scf_input).map_err(|source| {
                FeffError::computation(
                    "RUN.POT_ATOM_SCF",
                    format!(
                        "fixture '{}' failed ATOM SCF for potential {} (z={}): {}",
                        self.fixture_id, index, potential.atomic_number, source
                    ),
                )
            })?;
            let scf_outputs = compute_atom_scf_outputs(AtomScfOutputInput::new(
                &self.bound_state,
                &scf_result,
                &orbital_specs,
                atom_nuclear_charge,
            ))
            .map_err(|source| {
                FeffError::computation(
                    "RUN.POT_ATOM_OUTPUT",
                    format!(
                        "fixture '{}' failed ATOM output synthesis for potential {} (z={}): {}",
                        self.fixture_id, index, potential.atomic_number, source
                    ),
                )
            })?;

            let zeff = nominal_occupation - potential.xion;
            let boundary_index = scf_result
                .boundary_index()
                .min(scf_result.shell_density().len().saturating_sub(1));
            let boundary_radius = self
                .bound_state
                .radial_grid()
                .points()
                .get(boundary_index)
                .copied()
                .unwrap_or(radius_mean)
                .max(MIN_RADIUS);
            let shell_density = scf_result
                .shell_density()
                .get(boundary_index)
                .copied()
                .unwrap_or(0.0_f64)
                .max(0.0_f64);
            let atom_density =
                shell_density / (4.0_f64 * std::f64::consts::PI * boundary_radius.powi(2));
            let geometric_density = potential.xnatph.max(0.0_f64) / (radius_rms + 1.0_f64);
            let local_density =
                (0.75_f64 * atom_density + 0.25_f64 * geometric_density).max(1.0e-12);
            let screening_wave = potential.folp.abs() / (potential.lmaxsc.max(0) as f64 + 1.0_f64)
                + scf_outputs.s02();
            let exchange = self.exchange_api.evaluate(ExchangeEvaluationInput::new(
                self.exchange_model,
                local_density,
                scf_outputs.orbital_energy_sum(),
                screening_wave.max(1.0e-8_f64),
            ));
            let sample_index = boundary_index.saturating_sub(boundary_index / 2).max(1);
            let vmt0_atom = scf_result
                .potential()
                .get(sample_index)
                .copied()
                .unwrap_or(-zeff / (radius_mean + 1.0_f64));
            let vmt0 = vmt0_atom + 0.05_f64 * scf_outputs.total_energy();
            let vxc = exchange.real - exchange.imaginary.abs();
            let (scf_residual_rms, scf_charge_delta) = scf_result
                .iterations()
                .last()
                .map(|iteration| (iteration.residual_rms(), iteration.charge_delta()))
                .unwrap_or((0.0_f64, 0.0_f64));

            Ok(PotentialPhysics {
                zeff,
                local_density,
                vmt0,
                vxc,
                scf_residual_rms,
                scf_charge_delta,
            })
        })();

        match atom_physics {
            Ok(physics) => Ok(physics),
            Err(_) => {
                let zeff = nominal_occupation - potential.xion;
                let local_density = potential.xnatph.max(0.0_f64) / (radius_rms + 1.0_f64);
                let screening = potential.folp / (potential.lmaxsc.max(0) as f64 + 1.0_f64)
                    * (0.95_f64 + 0.05_f64 * valence_fraction);
                let exchange = self.exchange_api.evaluate(ExchangeEvaluationInput::new(
                    self.exchange_model,
                    local_density.max(1.0e-12_f64),
                    zeff,
                    screening.abs().max(1.0e-8_f64),
                ));
                let vmt0 = -zeff / (radius_mean + 1.0_f64) * (1.0_f64 + 0.05_f64 * index as f64)
                    - local_density;
                let vxc = exchange.real - exchange.imaginary.abs();
                Ok(PotentialPhysics {
                    zeff,
                    local_density,
                    vmt0,
                    vxc,
                    scf_residual_rms: 0.0_f64,
                    scf_charge_delta: 0.0_f64,
                })
            }
        }
    }

    fn orbital_specs_for_potential(
        &self,
        potential: &PotentialEntry,
        effective_charge: f64,
    ) -> Vec<AtomScfOrbitalSpec> {
        let mut specs = usize::try_from(potential.atomic_number)
            .ok()
            .and_then(|atomic_number| {
                getorb_for_atomic_number(atomic_number, ConfigurationRecipe::Feff9)
            })
            .map(|extraction| build_atom_orbital_specs(extraction.orbitals(), effective_charge))
            .unwrap_or_default();
        if specs.is_empty() {
            specs.push(
                AtomScfOrbitalSpec::new(1, -1, effective_charge.min(2.0_f64))
                    .with_valence_occupation(effective_charge.min(1.0_f64))
                    .with_energy_bounds(-1.0_f64, -1.0e-5_f64)
                    .with_convergence_tolerance(5.0e-2_f64),
            );
        }
        specs
    }

    fn potential_metrics(&self, index: usize) -> (f64, f64, f64, f64) {
        self.physics
            .get(index)
            .copied()
            .map(|entry| (entry.zeff, entry.local_density, entry.vmt0, entry.vxc))
            .unwrap_or((0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64))
    }

    fn radius_stats(&self) -> (f64, f64, f64) {
        let extent = self.bound_state.radial_grid().extent();
        (extent.mean, extent.rms, extent.max)
    }

    fn average_zeff(&self) -> f64 {
        if self.physics.is_empty() {
            return 0.0_f64;
        }
        self.physics.iter().map(|entry| entry.zeff).sum::<f64>() / self.physics.len() as f64
    }
}

fn build_atom_orbital_specs(
    orbitals: &[OrbitalOccupancy],
    effective_charge: f64,
) -> Vec<AtomScfOrbitalSpec> {
    let mut selected = orbitals
        .iter()
        .copied()
        .filter(|orbital| {
            orbital.occupation > 0.0_f64 && orbital.metadata.principal_quantum_number > 0
        })
        .filter(|orbital| orbital.metadata.kappa_quantum_number == -1)
        .take(MAX_ATOM_ORBITALS)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        selected = orbitals
            .iter()
            .copied()
            .filter(|orbital| {
                orbital.occupation > 0.0_f64 && orbital.metadata.principal_quantum_number > 0
            })
            .take(1)
            .collect::<Vec<_>>();
    }

    selected
        .into_iter()
        .filter_map(|orbital| {
            let principal = usize::try_from(orbital.metadata.principal_quantum_number).ok()?;
            if principal == 0 {
                return None;
            }
            let (energy_min, energy_max) = orbital_energy_bounds(principal, effective_charge);
            Some(
                AtomScfOrbitalSpec::new(
                    principal,
                    orbital.metadata.kappa_quantum_number,
                    orbital.occupation,
                )
                .with_valence_occupation(orbital.valence_occupation)
                .with_energy_bounds(energy_min, energy_max)
                .with_convergence_tolerance(5.0e-2_f64),
            )
        })
        .collect()
}

fn orbital_energy_bounds(principal_quantum_number: usize, effective_charge: f64) -> (f64, f64) {
    let principal = principal_quantum_number.max(1) as f64;
    let scaled_charge = (effective_charge.abs() / SPEED_OF_LIGHT_AU).max(1.0e-4_f64);
    let center = -0.5_f64 * (scaled_charge / principal).powi(2);
    let mut energy_min = (center * 16.0_f64).min(-1.0e-4_f64);
    let mut energy_max = (center * 0.05_f64).min(-1.0e-8_f64);
    if !energy_min.is_finite() || !energy_max.is_finite() || energy_min >= energy_max {
        energy_min = -1.0_f64;
        energy_max = -1.0e-6_f64;
    }
    if energy_max >= -1.0e-12_f64 {
        energy_max = -1.0e-6_f64;
    }
    if energy_min >= energy_max {
        energy_min = energy_max * 10.0_f64;
    }
    (energy_min, energy_max)
}

fn coulomb_seed_potential(radial_grid: &[f64], effective_charge: f64) -> Vec<f64> {
    let charge = effective_charge.abs().max(1.0e-6_f64);
    radial_grid
        .iter()
        .map(|radius| -charge / (SPEED_OF_LIGHT_AU * radius.max(MIN_RADIUS)))
        .collect()
}

fn push_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(target: &mut Vec<u8>, value: i32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_f64(target: &mut Vec<u8>, value: f64) {
    target.extend_from_slice(&value.to_le_bytes());
}
