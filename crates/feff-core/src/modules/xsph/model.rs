use super::parser::{
    format_scientific_f64, parse_geom_source, parse_global_source, parse_pot_source,
    parse_wscrn_source, parse_xsph_source, push_f64, push_i32, push_u32, GeomXsphInput,
    GlobalXsphInput, PotXsphInput, WscrnXsphInput, XsphControlInput,
};
use super::XSPH_PHASE_BINARY_MAGIC;
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_binary_artifact, write_text_artifact};
use crate::numerics::{
    solve_complex_energy_dirac, ComplexEnergySolverState, ComplexExchangeCoupling,
    ComplexRadialDiracInput, ExchangeModel, RadialExtent, RadialGrid,
};
use num_complex::Complex64;
use std::f64::consts::PI;
use std::path::Path;

const HARTREE_TO_EV: f64 = 27.211_386_245_988_f64;
const MIN_SPECTRAL_POINTS: usize = 48;
const MAX_SPECTRAL_POINTS: usize = 256;
const MIN_SOLVE_POINTS: usize = 8;
const MAX_SOLVE_POINTS: usize = 24;
const MIN_PHASE_CHANNELS: usize = 2;
const MAX_PHASE_CHANNELS: usize = 12;
const MIN_RADIUS: f64 = 1.0e-6;
const MIN_XSECT_VALUE: f64 = 1.0e-12;

#[derive(Debug, Clone)]
pub(super) struct XsphModel {
    fixture_id: String,
    control: XsphControlInput,
    geom: GeomXsphInput,
    global: GlobalXsphInput,
    pot: PotXsphInput,
    wscrn: Option<WscrnXsphInput>,
    output: XsphComputedOutput,
}

#[derive(Debug, Clone, Copy)]
struct XsphOutputConfig {
    phase_channels: usize,
    spectral_points: usize,
    solve_points: usize,
    energy_start: f64,
    energy_step: f64,
    base_phase: f64,
    phase_scale: f64,
    damping: f64,
    screening_shift: f64,
    xsnorm: f64,
    interstitial_index: usize,
    interstitial_radius: f64,
}

#[derive(Debug, Clone)]
struct XsphNodeSample {
    energy: f64,
    phases: Vec<f64>,
    xsnorm: f64,
    xsect: f64,
    imag_part: f64,
    continuity_residual: f64,
    exchange_shift: f64,
}

#[derive(Debug, Clone)]
struct XsphSpectralSample {
    energy: f64,
    phases: Vec<f64>,
    xsnorm: f64,
    xsect: f64,
    imag_part: f64,
}

#[derive(Debug, Clone)]
struct XsphComputedOutput {
    config: XsphOutputConfig,
    spectral_samples: Vec<XsphSpectralSample>,
    average_continuity_residual: f64,
    max_continuity_residual: f64,
    max_exchange_shift: f64,
}

impl XsphModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        xsph_source: &str,
        geom_source: &str,
        global_source: &str,
        pot_bytes: &[u8],
        wscrn_source: Option<&str>,
    ) -> ComputeResult<Self> {
        let control = parse_xsph_source(fixture_id, xsph_source)?;
        let exchange_model = ExchangeModel::from_feff_ixc(control.ixc);
        let geom = parse_geom_source(fixture_id, geom_source)?;
        let global = parse_global_source(fixture_id, global_source)?;
        let pot = parse_pot_source(fixture_id, pot_bytes)?;
        let wscrn = wscrn_source
            .map(|source| parse_wscrn_source(fixture_id, source))
            .transpose()?;
        let radial_point_count = wscrn
            .map(|input| input.radial_points)
            .unwrap_or((geom.atom_count.max(4) * 16).clamp(64, 4096))
            .clamp(64, 4096);
        let radial_grid = RadialGrid::from_extent(
            RadialExtent::new(geom.radius_mean, geom.radius_rms, geom.radius_max),
            radial_point_count,
            control.xkstep.abs().max(1.0e-4),
        );
        let reference_k = control.xkstep.abs().max(1.0e-4);
        let reference_energy_au = 0.5 * reference_k * reference_k;
        let complex_state = ComplexEnergySolverState::new(
            radial_grid,
            Complex64::new(
                reference_energy_au,
                (control.gamach + pot.gamach).abs().max(1.0e-4),
            ),
            control.xkmax.abs().max(reference_k),
            (control.lmaxph_max + control.nph).max(1) as usize,
        );

        let output = Self::compute_fovrg_output(
            fixture_id,
            &control,
            &geom,
            &global,
            &pot,
            wscrn,
            exchange_model,
            &complex_state,
        )?;

        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control,
            geom,
            global,
            pot,
            wscrn,
            output,
        })
    }

    fn compute_fovrg_output(
        fixture_id: &str,
        control: &XsphControlInput,
        geom: &GeomXsphInput,
        global: &GlobalXsphInput,
        pot: &PotXsphInput,
        wscrn: Option<WscrnXsphInput>,
        exchange_model: ExchangeModel,
        complex_state: &ComplexEnergySolverState,
    ) -> ComputeResult<XsphComputedOutput> {
        let mut config = Self::output_config(control, geom, global, pot, wscrn, complex_state);
        let base_potential =
            Self::build_complex_potential(config, control, geom, global, pot, complex_state);
        let exchange_density =
            Self::estimate_exchange_density(geom, pot, config.interstitial_radius);
        let exchange_coupling = ComplexExchangeCoupling::new(exchange_model, exchange_density)
            .with_cycles(2)
            .with_mixing(0.35);

        let node_energies =
            Self::linear_grid(config.energy_start, config.energy_step, config.solve_points);
        let mut node_samples = Vec::with_capacity(node_energies.len());
        let mut successful_solves = 0_usize;
        let mut last_solver_error: Option<String> = None;

        for energy in node_energies {
            let energy_au = (energy / HARTREE_TO_EV).max(1.0e-6);
            let wave_number = (2.0 * energy_au).sqrt().max(1.0e-4);
            let solver_state = ComplexEnergySolverState::new(
                complex_state.radial_grid().clone(),
                Complex64::new(energy_au, config.damping.max(1.0e-6)),
                wave_number,
                config.phase_channels,
            );

            let mut phases = Vec::with_capacity(config.phase_channels);
            let mut continuity_sum = 0.0_f64;
            let mut exchange_sum = 0.0_f64;
            let mut solved_channels = 0_usize;

            for channel in 0..config.phase_channels {
                let kappa = channel_to_kappa(channel);
                match Self::solve_channel_phase(
                    &solver_state,
                    &base_potential,
                    config.interstitial_index,
                    kappa,
                    exchange_coupling,
                ) {
                    Ok((phase, continuity_residual, exchange_shift)) => {
                        phases.push(phase);
                        continuity_sum += continuity_residual;
                        exchange_sum += exchange_shift;
                        solved_channels += 1;
                        successful_solves += 1;
                    }
                    Err(error) => {
                        last_solver_error = Some(error);
                        let fallback_phase = node_samples
                            .last()
                            .and_then(|sample: &XsphNodeSample| sample.phases.get(channel).copied())
                            .unwrap_or(0.0);
                        phases.push(fallback_phase);
                        continuity_sum += 1.0;
                    }
                }
            }

            let solved_weight = solved_channels.max(1) as f64;
            let continuity_residual = continuity_sum / solved_weight;
            let exchange_shift = exchange_sum / solved_weight;
            let (xsnorm, xsect, imag_part) = Self::cross_section_from_phases(
                energy_au,
                &phases,
                continuity_residual,
                config.xsnorm,
                config.screening_shift,
            );

            node_samples.push(XsphNodeSample {
                energy,
                phases,
                xsnorm,
                xsect,
                imag_part,
                continuity_residual,
                exchange_shift,
            });
        }

        if successful_solves == 0 {
            let detail = last_solver_error.unwrap_or_else(|| "no converged channels".to_string());
            return Err(FeffError::computation(
                "RUN.XSPH_FOVRG_SOLVE",
                format!(
                    "fixture '{}' failed to converge any FOVRG channels: {}",
                    fixture_id, detail
                ),
            ));
        }

        let spectral_energies = Self::linear_grid(
            config.energy_start,
            config.energy_step,
            config.spectral_points,
        );
        let mut spectral_samples = Vec::with_capacity(spectral_energies.len());
        for energy in spectral_energies {
            spectral_samples.push(Self::interpolate_node_sample(
                &node_samples,
                energy,
                config.phase_channels,
            ));
        }

        let average_continuity_residual = node_samples
            .iter()
            .map(|sample| sample.continuity_residual)
            .sum::<f64>()
            / node_samples.len().max(1) as f64;
        let max_continuity_residual = node_samples
            .iter()
            .map(|sample| sample.continuity_residual)
            .fold(0.0_f64, f64::max);
        let max_exchange_shift = node_samples
            .iter()
            .map(|sample| sample.exchange_shift)
            .fold(0.0_f64, f64::max);

        let base_phase = spectral_samples
            .first()
            .and_then(|sample| sample.phases.first())
            .copied()
            .unwrap_or(0.0);
        let mut phase_sq_sum = 0.0_f64;
        let mut phase_count = 0_usize;
        for sample in &spectral_samples {
            for phase in &sample.phases {
                let delta = normalize_phase(*phase - base_phase);
                phase_sq_sum += delta * delta;
                phase_count += 1;
            }
        }
        config.base_phase = normalize_phase(base_phase);
        config.phase_scale = (phase_sq_sum / phase_count.max(1) as f64).sqrt().max(0.05);
        config.xsnorm = spectral_samples
            .iter()
            .map(|sample| sample.xsnorm)
            .sum::<f64>()
            / spectral_samples.len().max(1) as f64;

        Ok(XsphComputedOutput {
            config,
            spectral_samples,
            average_continuity_residual,
            max_continuity_residual,
            max_exchange_shift,
        })
    }

    fn output_config(
        control: &XsphControlInput,
        geom: &GeomXsphInput,
        global: &GlobalXsphInput,
        pot: &PotXsphInput,
        wscrn: Option<WscrnXsphInput>,
        complex_state: &ComplexEnergySolverState,
    ) -> XsphOutputConfig {
        let radial_grid = complex_state.radial_grid();
        let radial_points = radial_grid.points();
        let radial_extent = radial_grid.extent();

        let phase_channels = (control.lmaxph_max.max(1) as usize + control.nph.max(1) as usize / 2)
            .clamp(MIN_PHASE_CHANNELS, MAX_PHASE_CHANNELS);

        let step = control.xkstep.abs().max(1.0e-4);
        let max_k = control.xkmax.abs().max(step);
        let base_spectral_points =
            ((max_k / step).ceil() as usize + 1).clamp(MIN_SPECTRAL_POINTS, MAX_SPECTRAL_POINTS);
        let spectral_points = (base_spectral_points
            + control.n_poles.max(4) as usize / 6
            + global.token_count.min(64) / 16)
            .clamp(MIN_SPECTRAL_POINTS, MAX_SPECTRAL_POINTS);
        let solve_points = spectral_points.clamp(MIN_SOLVE_POINTS, MAX_SOLVE_POINTS);

        let k_start = step.max(0.03);
        let k_end = max_k.max(k_start + 1.0e-3);
        let energy_start = 0.5 * k_start * k_start * HARTREE_TO_EV;
        let energy_end = 0.5 * k_end * k_end * HARTREE_TO_EV;
        let energy_step = if spectral_points == 1 {
            1.0e-4
        } else {
            ((energy_end - energy_start) / (spectral_points - 1) as f64).max(1.0e-4)
        };

        let wscrn_delta = wscrn
            .map(|input| (input.charge_mean - input.screen_mean).abs())
            .unwrap_or(0.0);
        let screening_shift = wscrn_delta * 5.0e-4
            + wscrn
                .map(|input| input.radial_points as f64 * 5.0e-7)
                .unwrap_or(0.0);

        let interstitial_target = pot
            .radius_mean
            .max(0.75 * pot.radius_max)
            .max(geom.radius_mean);
        let mut interstitial_index = radial_points
            .iter()
            .position(|radius| *radius >= interstitial_target)
            .unwrap_or(radial_points.len() * 3 / 4);
        let min_interstitial = 4;
        let safety_cap = (radial_points.len() * 4 / 5).max(min_interstitial);
        let max_interstitial = radial_points.len().saturating_sub(5).min(safety_cap);
        if max_interstitial >= min_interstitial {
            interstitial_index = interstitial_index.clamp(min_interstitial, max_interstitial);
        } else {
            interstitial_index = radial_points.len().saturating_sub(1);
        }
        let interstitial_radius = radial_points
            .get(interstitial_index)
            .copied()
            .unwrap_or(radial_extent.mean.max(MIN_RADIUS));

        let base_phase = normalize_phase(
            0.02 * pot.charge_scale
                + 0.002 * geom.ipot_mean
                + 0.0005 * control.mphase as f64
                + screening_shift,
        );
        let phase_scale = (1.0 + pot.rfms + control.rfms2 + radial_extent.rms)
            .ln()
            .max(0.05);
        let damping = ((control.gamach + pot.gamach).abs() * 0.02
            + 1.0 / (max_k + radial_extent.max + pot.radius_max + 2.0)
            + global.max_abs.min(50_000.0) * 1.0e-6)
            .max(1.0e-5);
        let xsnorm = ((global.rms + pot.charge_scale + radial_extent.mean).abs()
            * 1.0e-3
            * (1.0 + 0.01 * control.ispec.abs() as f64)
            * (1.0 + 0.02 * pot.npot as f64)
            * (1.0 + 0.005 * pot.radius_max)
            * (1.0 + 1.0e-4 * global.max_abs.min(10_000.0)))
        .max(1.0e-6);

        XsphOutputConfig {
            phase_channels,
            spectral_points,
            solve_points,
            energy_start,
            energy_step,
            base_phase,
            phase_scale,
            damping,
            screening_shift,
            xsnorm,
            interstitial_index,
            interstitial_radius,
        }
    }

    fn build_complex_potential(
        config: XsphOutputConfig,
        control: &XsphControlInput,
        geom: &GeomXsphInput,
        global: &GlobalXsphInput,
        pot: &PotXsphInput,
        complex_state: &ComplexEnergySolverState,
    ) -> Vec<Complex64> {
        let radial_points = complex_state.radial_grid().points();
        let screening_radius = pot
            .radius_rms
            .max(geom.radius_mean * 0.3)
            .max(0.25 * pot.radius_max)
            .max(config.interstitial_radius * 0.2)
            .max(MIN_RADIUS);
        let muffin_tin_radius = config.interstitial_radius.max(MIN_RADIUS);
        let core_strength =
            (pot.charge_scale * (1.0 + 0.02 * geom.ipot_mean.abs())).clamp(0.5, 20.0);
        let static_offset = -(pot.rfms + control.rfms2).abs() * 0.015
            - 0.002 * global.mean.tanh()
            - 1.0e-4 * global.max_abs.min(10_000.0);
        let imag_strength =
            (control.gamach + pot.gamach).abs() * 0.02 + config.screening_shift.abs() * 5.0e-2;

        let mut potential = Vec::with_capacity(radial_points.len());
        for &radius in radial_points {
            let radius = radius.max(MIN_RADIUS);
            let radial_fraction = (radius / muffin_tin_radius).clamp(0.0, 4.0);
            let screening = (-radius / screening_radius).exp();
            let real_part = static_offset
                - core_strength * screening / (radius + 0.2 * screening_radius)
                + config.screening_shift * (1.0 - radial_fraction.min(1.0));
            let imag_part = -imag_strength * screening * (1.0 - radial_fraction.min(1.0)).powi(2);
            potential.push(Complex64::new(
                finite_or_zero(real_part),
                finite_or_zero(imag_part),
            ));
        }

        let tail = potential
            .get(config.interstitial_index)
            .copied()
            .unwrap_or(Complex64::new(0.0, 0.0));
        for value in potential
            .iter_mut()
            .skip(config.interstitial_index.saturating_add(1))
        {
            *value = tail;
        }

        potential
    }

    fn estimate_exchange_density(
        geom: &GeomXsphInput,
        pot: &PotXsphInput,
        boundary_radius: f64,
    ) -> f64 {
        let radius = boundary_radius.max(pot.radius_mean).max(MIN_RADIUS);
        let volume = 4.0 * PI * radius * radius * radius / 3.0;
        let electron_count =
            (pot.charge_scale * pot.npot.max(1) as f64 / geom.nat.max(1) as f64).max(1.0e-6);
        (electron_count / volume).max(1.0e-6)
    }

    fn solve_channel_phase(
        state: &ComplexEnergySolverState,
        potential: &[Complex64],
        interstitial_index: usize,
        kappa: i32,
        exchange_coupling: ComplexExchangeCoupling,
    ) -> Result<(f64, f64, f64), String> {
        let coupled_input = ComplexRadialDiracInput::new(state, potential, kappa)
            .with_interstitial_index(interstitial_index)
            .with_exchange_coupling(exchange_coupling);
        let uncoupled_input = ComplexRadialDiracInput::new(state, potential, kappa)
            .with_interstitial_index(interstitial_index);

        let solution = match solve_complex_energy_dirac(coupled_input) {
            Ok(solution) => solution,
            Err(coupled_error) => {
                solve_complex_energy_dirac(uncoupled_input).map_err(|uncoupled_error| {
                    format!(
                        "kappa {}: coupled={}, uncoupled={}",
                        kappa, coupled_error, uncoupled_error
                    )
                })?
            }
        };

        let phase = normalize_phase(
            solution.boundary_phase_shift().re + 0.05 * solution.boundary_phase_shift().im,
        );
        let continuity_residual = finite_or_zero(solution.boundary_continuity_residual()).max(0.0);
        let exchange_shift = finite_or_zero(solution.exchange_shift().norm()).max(0.0);
        Ok((phase, continuity_residual, exchange_shift))
    }

    fn cross_section_from_phases(
        energy_au: f64,
        phases: &[f64],
        continuity_residual: f64,
        xsnorm_base: f64,
        screening_shift: f64,
    ) -> (f64, f64, f64) {
        let wave_number = (2.0 * energy_au).sqrt().max(1.0e-4);
        let mut partial_sum = 0.0_f64;
        let mut imag_sum = 0.0_f64;

        for (channel, phase) in phases.iter().enumerate() {
            let kappa = channel_to_kappa(channel);
            let orbital_order = orbital_order_from_kappa(kappa) as f64;
            let weight = 2.0 * orbital_order + 1.0;
            let sin_phase = phase.sin();
            let attenuation = (-(continuity_residual * 0.5)
                * (1.0 + channel as f64 / phases.len().max(1) as f64))
                .exp();
            partial_sum += weight * sin_phase * sin_phase * attenuation;
            imag_sum += weight * phase.sin() * phase.cos();
        }

        let continuity_factor = 1.0 / (1.0 + continuity_residual.max(0.0));
        let xsnorm =
            (xsnorm_base * (1.0 + 0.03 * wave_number) * (1.0 + 40.0 * screening_shift.abs()))
                .max(MIN_XSECT_VALUE);
        let xsect = (xsnorm * partial_sum * continuity_factor / (wave_number * wave_number + 1.0))
            .max(MIN_XSECT_VALUE);
        let imag_part = xsect * (0.25 + 0.05 * imag_sum.tanh()) + screening_shift * 1.0e-3;

        (
            finite_or_zero(xsnorm).max(MIN_XSECT_VALUE),
            finite_or_zero(xsect).max(MIN_XSECT_VALUE),
            finite_or_zero(imag_part),
        )
    }

    fn interpolate_node_sample(
        nodes: &[XsphNodeSample],
        energy: f64,
        phase_channels: usize,
    ) -> XsphSpectralSample {
        if nodes.len() == 1 {
            return Self::spectral_from_node(&nodes[0], energy);
        }

        if energy <= nodes[0].energy {
            return Self::spectral_from_node(&nodes[0], energy);
        }
        if energy >= nodes[nodes.len() - 1].energy {
            return Self::spectral_from_node(&nodes[nodes.len() - 1], energy);
        }

        for pair in nodes.windows(2) {
            let left = &pair[0];
            let right = &pair[1];
            if energy > right.energy {
                continue;
            }
            let span = (right.energy - left.energy).abs().max(1.0e-12);
            let t = ((energy - left.energy) / span).clamp(0.0, 1.0);
            let mut phases = Vec::with_capacity(phase_channels);
            for channel in 0..phase_channels {
                let left_phase = left.phases.get(channel).copied().unwrap_or(0.0);
                let right_phase = right.phases.get(channel).copied().unwrap_or(left_phase);
                phases.push(interpolate_phase(left_phase, right_phase, t));
            }

            return XsphSpectralSample {
                energy,
                phases,
                xsnorm: lerp(left.xsnorm, right.xsnorm, t),
                xsect: lerp(left.xsect, right.xsect, t).max(MIN_XSECT_VALUE),
                imag_part: lerp(left.imag_part, right.imag_part, t),
            };
        }

        Self::spectral_from_node(&nodes[nodes.len() - 1], energy)
    }

    fn spectral_from_node(node: &XsphNodeSample, energy: f64) -> XsphSpectralSample {
        XsphSpectralSample {
            energy,
            phases: node.phases.clone(),
            xsnorm: node.xsnorm,
            xsect: node.xsect,
            imag_part: node.imag_part,
        }
    }

    fn linear_grid(start: f64, step: f64, count: usize) -> Vec<f64> {
        let mut values = Vec::with_capacity(count.max(1));
        for index in 0..count.max(1) {
            values.push(start + step * index as f64);
        }
        values
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
    ) -> ComputeResult<()> {
        match artifact_name {
            "phase.bin" => {
                write_binary_artifact(output_path, &self.render_phase_binary()).map_err(|source| {
                    FeffError::io_system(
                        "IO.XSPH_OUTPUT_WRITE",
                        format!(
                            "failed to write XSPH artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "xsect.dat" => {
                write_text_artifact(output_path, &self.render_xsect()).map_err(|source| {
                    FeffError::io_system(
                        "IO.XSPH_OUTPUT_WRITE",
                        format!(
                            "failed to write XSPH artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "log2.dat" => write_text_artifact(output_path, &self.render_log2()).map_err(|source| {
                FeffError::io_system(
                    "IO.XSPH_OUTPUT_WRITE",
                    format!(
                        "failed to write XSPH artifact '{}': {}",
                        output_path.display(),
                        source
                    ),
                )
            }),
            other => Err(FeffError::internal(
                "SYS.XSPH_OUTPUT_CONTRACT",
                format!("unsupported XSPH output artifact '{}'", other),
            )),
        }
    }

    fn render_phase_binary(&self) -> Vec<u8> {
        let config = self.output.config;
        let mut bytes = Vec::with_capacity(
            96 + config.spectral_points * (config.phase_channels + 1) * std::mem::size_of::<f64>(),
        );

        bytes.extend_from_slice(XSPH_PHASE_BINARY_MAGIC);
        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, config.phase_channels as u32);
        push_u32(&mut bytes, config.spectral_points as u32);
        push_i32(&mut bytes, self.control.mphase);
        push_i32(&mut bytes, self.control.ispec);
        push_f64(&mut bytes, config.energy_start);
        push_f64(&mut bytes, config.energy_step);
        push_f64(&mut bytes, config.base_phase);
        push_f64(&mut bytes, config.phase_scale);
        push_f64(&mut bytes, config.damping);
        push_f64(&mut bytes, config.screening_shift);

        for sample in &self.output.spectral_samples {
            push_f64(&mut bytes, sample.energy);
            for phase in &sample.phases {
                push_f64(&mut bytes, *phase);
            }
        }

        bytes
    }

    fn render_xsect(&self) -> String {
        let mut lines = Vec::with_capacity(self.output.spectral_samples.len() + 4);
        lines.push("# XSPH true-compute cross section".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push(format!(
            "# optional_wscrn: {}",
            if self.wscrn.is_some() {
                "present"
            } else {
                "absent"
            }
        ));
        lines.push("# energy(eV) xsnorm xsect imag_part".to_string());

        for sample in &self.output.spectral_samples {
            lines.push(format!(
                "{:>16} {:>16} {:>16} {:>16}",
                format_scientific_f64(sample.energy),
                format_scientific_f64(sample.xsnorm),
                format_scientific_f64(sample.xsect),
                format_scientific_f64(sample.imag_part),
            ));
        }

        lines.join("\n")
    }

    fn render_log2(&self) -> String {
        let config = self.output.config;
        let wscrn_status = if self.wscrn.is_some() {
            "present"
        } else {
            "absent"
        };

        format!(
            "\
XSPH true-compute runtime\n\
fixture: {}\n\
input-artifacts: xsph.inp geom.dat global.inp pot.bin\n\
optional-input-wscrn: {}\n\
output-artifacts: phase.bin xsect.dat log2.dat\n\
nat: {} nph: {} atoms: {}\n\
global-token-count: {} global-mean: {} global-rms: {} global-max-abs: {}\n\
pot-nat: {} pot-nph: {} npot: {}\n\
pot-radius-max: {}\n\
lmaxph-max: {} n-poles: {}\n\
phase-channels: {}\n\
spectral-points: {}\n\
fovrg-solve-points: {}\n\
interstitial-index: {}\n\
interstitial-radius: {}\n\
energy-start: {}\n\
energy-step: {}\n\
xsnorm-base: {}\n\
boundary-residual-mean: {}\n\
boundary-residual-max: {}\n\
exchange-shift-max: {}\n\
",
            self.fixture_id,
            wscrn_status,
            self.geom.nat,
            self.geom.nph,
            self.geom.atom_count,
            self.global.token_count,
            format_scientific_f64(self.global.mean),
            format_scientific_f64(self.global.rms),
            format_scientific_f64(self.global.max_abs),
            self.pot.nat,
            self.pot.nph,
            self.pot.npot,
            format_scientific_f64(self.pot.radius_max),
            self.control.lmaxph_max,
            self.control.n_poles,
            config.phase_channels,
            config.spectral_points,
            config.solve_points,
            config.interstitial_index,
            format_fixed_f64(config.interstitial_radius, 12, 5),
            format_fixed_f64(config.energy_start, 12, 5),
            format_fixed_f64(config.energy_step, 12, 5),
            format_scientific_f64(config.xsnorm),
            format_scientific_f64(self.output.average_continuity_residual),
            format_scientific_f64(self.output.max_continuity_residual),
            format_scientific_f64(self.output.max_exchange_shift),
        )
    }
}

fn channel_to_kappa(channel: usize) -> i32 {
    let shell = (channel / 2 + 1) as i32;
    if channel % 2 == 0 {
        -shell
    } else {
        shell
    }
}

fn orbital_order_from_kappa(kappa: i32) -> usize {
    if kappa < 0 {
        (-kappa - 1) as usize
    } else {
        kappa as usize
    }
}

fn normalize_phase(phase: f64) -> f64 {
    if !phase.is_finite() {
        return 0.0;
    }
    let mut normalized = phase;
    while normalized > PI {
        normalized -= 2.0 * PI;
    }
    while normalized < -PI {
        normalized += 2.0 * PI;
    }
    normalized
}

fn interpolate_phase(left: f64, right: f64, t: f64) -> f64 {
    let mut delta = right - left;
    while delta > PI {
        delta -= 2.0 * PI;
    }
    while delta < -PI {
        delta += 2.0 * PI;
    }
    normalize_phase(left + delta * t.clamp(0.0, 1.0))
}

fn lerp(left: f64, right: f64, t: f64) -> f64 {
    left + (right - left) * t.clamp(0.0, 1.0)
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}
