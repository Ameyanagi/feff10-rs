use super::parser::{
    ExcInputSummary, SelfControlInput, SelfSpectrumInput, SpectrumRow, StagedSpectrumSource,
    artifact_list, parse_exc_source, parse_sfconv_source, parse_spectrum_source,
    sample_spectrum_row, upsert_artifact,
};
use super::{FNV_OFFSET_BASIS, FNV_PRIME, SELF_REQUIRED_OUTPUTS};
use crate::domain::{ComputeArtifact, ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use crate::numerics::{SfconvConvolutionInput, SfconvError, convolve_sfconv_point};
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct SelfModel {
    fixture_id: String,
    control: SelfControlInput,
    spectra: Vec<SelfSpectrumInput>,
    exc: Option<ExcInputSummary>,
}

#[derive(Debug, Clone, Copy)]
struct SelfOutputConfig {
    sample_count: usize,
    energy_min: f64,
    energy_step: f64,
    self_scale: f64,
    broadening: f64,
    spectrum_weight: f64,
    rewrite_gain: f64,
    checksum_mix: u64,
}

#[derive(Debug, Clone)]
pub(super) struct SelfKernelState {
    config: SelfOutputConfig,
    energies: Vec<f64>,
    aggregate_signal: Vec<f64>,
    sfconv_real: Vec<f64>,
    sfconv_imag: Vec<f64>,
    sfconv_magnitude: Vec<f64>,
    sfconv_phase: Vec<f64>,
    sfconv_normalization: Vec<f64>,
    sfconv_quasiparticle_weight: Vec<f64>,
    spectral_source_artifact: String,
    spectral_point_count: usize,
    sfconv_weights: [f64; 8],
    use_asymmetric_phase: bool,
    apply_energy_cutoff: bool,
    plasma_frequency: f64,
}

impl SelfModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        sfconv_source: &str,
        spectrum_sources: Vec<StagedSpectrumSource>,
        exc_source: Option<&str>,
    ) -> ComputeResult<Self> {
        let control = parse_sfconv_source(sfconv_source);
        let mut spectra = Vec::with_capacity(spectrum_sources.len());
        for spectrum in spectrum_sources {
            spectra.push(parse_spectrum_source(
                fixture_id,
                &spectrum.artifact,
                &spectrum.source,
            )?);
        }

        if spectra.is_empty() {
            return Err(FeffError::input_validation(
                "INPUT.SELF_SPECTRUM_INPUT",
                format!(
                    "SELF module requires at least one staged spectrum input for fixture '{}'",
                    fixture_id
                ),
            ));
        }

        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control,
            spectra,
            exc: exc_source.map(parse_exc_source),
        })
    }

    pub(super) fn spectrum_artifact_names(&self) -> Vec<String> {
        self.spectra
            .iter()
            .map(|spectrum| spectrum.artifact.clone())
            .collect()
    }

    pub(super) fn expected_outputs(&self) -> Vec<ComputeArtifact> {
        let mut outputs = artifact_list(&SELF_REQUIRED_OUTPUTS);
        for spectrum in &self.spectra {
            upsert_artifact(&mut outputs, &spectrum.artifact);
        }
        outputs
    }

    pub(super) fn compute_state(&self) -> ComputeResult<SelfKernelState> {
        let config = self.output_config();
        let energies = (0..config.sample_count)
            .map(|index| config.energy_min + index as f64 * config.energy_step)
            .collect::<Vec<_>>();
        let aggregate_rows = (0..config.sample_count)
            .map(|index| self.aggregate_spectrum_row(index, config.sample_count))
            .collect::<Vec<_>>();
        let aggregate_signal = aggregate_rows
            .iter()
            .map(|row| row.signal)
            .collect::<Vec<_>>();

        let signal_energies = energies.clone();
        let signal_values = aggregate_rows
            .iter()
            .map(|row| row.signal)
            .collect::<Vec<_>>();

        let spectral_source = self.preferred_spectral_source();
        let (mut spectral_energies, mut spectral_values) = Self::sanitize_spectrum_rows(
            &spectral_source.rows,
            config.energy_step.abs().max(1.0e-6),
        );
        if spectral_energies.len() < 2 {
            spectral_energies = vec![
                config.energy_min,
                config.energy_min + config.energy_step.abs().max(1.0e-6),
            ];
            let fallback_signal = signal_values.first().copied().unwrap_or(0.0);
            spectral_values = vec![fallback_signal, fallback_signal];
        }

        let use_asymmetric_phase = self.control.ipse != 0 || self.control.ipsk != 0;
        let apply_energy_cutoff = self.control.msfconv != 0;
        let sfconv_weights = self.build_sfconv_weights(config.spectrum_weight);
        let plasma_frequency = self.estimate_plasma_frequency(&spectral_energies);

        let mut sfconv_real = Vec::with_capacity(config.sample_count);
        let mut sfconv_imag = Vec::with_capacity(config.sample_count);
        let mut sfconv_magnitude = Vec::with_capacity(config.sample_count);
        let mut sfconv_phase = Vec::with_capacity(config.sample_count);
        let mut sfconv_normalization = Vec::with_capacity(config.sample_count);
        let mut sfconv_quasiparticle_weight = Vec::with_capacity(config.sample_count);

        for &photoelectron_energy in &energies {
            let result = convolve_sfconv_point(SfconvConvolutionInput::new(
                photoelectron_energy,
                self.control.cen,
                config.broadening,
                &signal_energies,
                &signal_values,
                &spectral_energies,
                &spectral_values,
                sfconv_weights,
                use_asymmetric_phase,
                apply_energy_cutoff,
                plasma_frequency,
            ))
            .map_err(|source| {
                self.sfconv_error(
                    "RUN.SELF_SFCONV_CONVOLVE",
                    "failed to convolve SELF spectrum with SFCONV kernel",
                    source,
                )
            })?;

            sfconv_real.push(result.real);
            sfconv_imag.push(result.imaginary);
            sfconv_magnitude.push(result.magnitude);
            sfconv_phase.push(result.phase);
            sfconv_normalization.push(result.normalization);
            sfconv_quasiparticle_weight.push(result.quasiparticle_weight);
        }

        Ok(SelfKernelState {
            config,
            energies,
            aggregate_signal,
            sfconv_real,
            sfconv_imag,
            sfconv_magnitude,
            sfconv_phase,
            sfconv_normalization,
            sfconv_quasiparticle_weight,
            spectral_source_artifact: spectral_source.artifact.clone(),
            spectral_point_count: spectral_energies.len(),
            sfconv_weights,
            use_asymmetric_phase,
            apply_energy_cutoff,
            plasma_frequency,
        })
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
        state: &SelfKernelState,
    ) -> ComputeResult<()> {
        let contents = match artifact_name {
            "selfenergy.dat" => self.render_selfenergy(state),
            "sigma.dat" => self.render_sigma(state),
            "specfunct.dat" => self.render_specfunct(state),
            "logsfconv.dat" => self.render_logsfconv(state),
            "sig2FEFF.dat" => self.render_sig2feff(state),
            "mpse.dat" => self.render_mpse(state),
            "opconsCu.dat" => self.render_opcons_cu(state),
            other => {
                if self.find_spectrum(other).is_some() {
                    self.render_rewritten_spectrum(other, state)
                } else {
                    return Err(FeffError::internal(
                        "SYS.SELF_OUTPUT_CONTRACT",
                        format!("unsupported SELF output artifact '{}'", other),
                    ));
                }
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.SELF_OUTPUT_WRITE",
                format!(
                    "failed to write SELF artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn output_config(&self) -> SelfOutputConfig {
        let sample_count = self
            .spectra
            .iter()
            .map(|spectrum| spectrum.rows.len())
            .max()
            .unwrap_or(64)
            .clamp(64, 1024);

        let energy_min = self
            .spectra
            .iter()
            .map(|spectrum| spectrum.energy_min)
            .fold(f64::INFINITY, f64::min);
        let mut energy_max = self
            .spectra
            .iter()
            .map(|spectrum| spectrum.energy_max)
            .fold(f64::NEG_INFINITY, f64::max);
        let energy_min = if energy_min.is_finite() {
            energy_min
        } else {
            0.0
        };
        if !energy_max.is_finite() || energy_max <= energy_min {
            energy_max = energy_min + 1.0;
        }
        let energy_step = (energy_max - energy_min) / sample_count.saturating_sub(1).max(1) as f64;

        let spectrum_count = self.spectra.len().max(1);
        let mean_signal = self
            .spectra
            .iter()
            .map(|spectrum| spectrum.mean_signal)
            .sum::<f64>()
            / spectrum_count as f64;
        let rms_signal = self
            .spectra
            .iter()
            .map(|spectrum| spectrum.rms_signal)
            .sum::<f64>()
            / spectrum_count as f64;
        let spectrum_weight = mean_signal + 0.5 * rms_signal;
        let exc_weight = self.exc.map(|exc| exc.mean_weight).unwrap_or(0.0);
        let exc_phase = self.exc.map(|exc| exc.phase_bias).unwrap_or(0.0);

        let self_scale = (0.12
            + mean_signal.abs() * 0.20
            + rms_signal * 0.12
            + self.control.msfconv.abs() as f64 * 0.03
            + self.control.ispec.abs() as f64 * 0.02)
            .max(0.03);
        let broadening = (0.01
            + self.control.wsigk.abs() * 0.25
            + self.control.cen.abs() * 0.0008
            + exc_weight.abs() * 0.05
            + self.control.ipse.abs() as f64 * 0.007
            + self.control.ipr6.abs() as f64 * 0.004)
            .clamp(0.001, 2.0);
        let rewrite_gain = (1.0
            + self.control.ipse as f64 * 0.04
            + self.control.ipsk as f64 * 0.03
            + exc_phase * 0.06)
            .clamp(0.2, 3.0);

        let mut checksum_mix = FNV_OFFSET_BASIS;
        for spectrum in &self.spectra {
            checksum_mix ^= spectrum.checksum;
            checksum_mix = checksum_mix.wrapping_mul(FNV_PRIME);
        }
        if let Some(exc) = self.exc {
            checksum_mix ^= exc.row_count as u64;
            checksum_mix = checksum_mix.wrapping_mul(FNV_PRIME);
        }

        SelfOutputConfig {
            sample_count,
            energy_min,
            energy_step,
            self_scale,
            broadening,
            spectrum_weight,
            rewrite_gain,
            checksum_mix,
        }
    }

    fn render_selfenergy(&self, state: &SelfKernelState) -> String {
        let mut lines = Vec::with_capacity(state.config.sample_count + 5);
        lines.push("# SELF sfconv-backed self-energy".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index energy re_self im_self sigma_abs".to_string());

        for index in 0..state.config.sample_count {
            let energy = state.energies[index];
            let re_self = state.config.self_scale * state.sfconv_real[index];
            let im_self = state.config.self_scale * state.sfconv_imag[index]
                - state.config.broadening * (0.35 + 0.15 * state.sfconv_magnitude[index].abs());
            let sigma_abs = (re_self * re_self + im_self * im_self).sqrt();

            lines.push(format!(
                "{:5} {} {} {} {}",
                index + 1,
                format_fixed_f64(energy, 13, 6),
                format_fixed_f64(re_self, 13, 7),
                format_fixed_f64(im_self, 13, 7),
                format_fixed_f64(sigma_abs, 13, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_sigma(&self, state: &SelfKernelState) -> String {
        let mut lines = Vec::with_capacity(state.config.sample_count + 5);
        lines.push("# SELF sfconv-backed sigma table".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index energy sigma_real sigma_imag sigma_abs".to_string());

        for index in 0..state.config.sample_count {
            let energy = state.energies[index];
            let sigma_real = state.config.self_scale
                * state.sfconv_real[index]
                * (0.75 + 0.25 * state.sfconv_phase[index].cos().abs());
            let sigma_imag =
                state.config.self_scale * state.sfconv_imag[index] - state.config.broadening;
            let sigma_abs = (sigma_real * sigma_real + sigma_imag * sigma_imag).sqrt();

            lines.push(format!(
                "{:5} {} {} {} {}",
                index + 1,
                format_fixed_f64(energy, 13, 6),
                format_fixed_f64(sigma_real, 13, 7),
                format_fixed_f64(sigma_imag, 13, 7),
                format_fixed_f64(sigma_abs, 13, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_specfunct(&self, state: &SelfKernelState) -> String {
        let mut lines = Vec::with_capacity(state.config.sample_count + 6);
        lines.push("# SELF sfconv-backed spectral function".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index energy aomega quasiparticle_weight damping".to_string());

        for index in 0..state.config.sample_count {
            let energy = state.energies[index];
            let re_self = state.config.self_scale * state.sfconv_real[index];
            let gamma =
                (state.config.broadening * (1.0 + state.sfconv_magnitude[index].abs())).max(1.0e-9);
            let denominator = (energy - self.control.cen - re_self).powi(2) + gamma.powi(2);
            let aomega = gamma / denominator.max(1.0e-9);
            let qp_weight = state.sfconv_quasiparticle_weight[index].clamp(0.0, 1.0);
            let damping = (gamma / state.sfconv_normalization[index].abs().max(1.0e-9)).max(0.0);

            lines.push(format!(
                "{:5} {} {} {} {}",
                index + 1,
                format_fixed_f64(energy, 13, 6),
                format_fixed_f64(aomega, 14, 8),
                format_fixed_f64(qp_weight, 12, 7),
                format_fixed_f64(damping, 12, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_sig2feff(&self, state: &SelfKernelState) -> String {
        let rows = state.config.sample_count.clamp(24, 96);
        let mut lines = Vec::with_capacity(rows + 4);
        lines.push("# SELF sfconv-backed sigma^2 summary".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));

        let spectrum_count = self.spectra.len().max(1);
        for index in 0..rows {
            let sample_index = index * state.config.sample_count / rows.max(1);
            let shell_index = 1 + (index % spectrum_count);
            let path_index = 3 + (index / spectrum_count);
            let sigma2 = state.config.broadening * (1.0 + 0.02 * index as f64)
                + state.config.self_scale * state.sfconv_magnitude[sample_index].powi(2) * 0.05;
            let k_weight = (state.config.self_scale
                * (1.0 + state.sfconv_quasiparticle_weight[sample_index]))
                / (1.0 + state.energies[sample_index].abs() * 0.01);

            lines.push(format!(
                "{:10}{:11} {} {}",
                shell_index,
                path_index,
                format_fixed_f64(sigma2, 15, 10),
                format_fixed_f64(k_weight, 15, 8),
            ));
        }

        lines.join("\n")
    }

    fn render_mpse(&self, state: &SelfKernelState) -> String {
        let rows = state.config.sample_count.clamp(32, 128);
        let mut lines = Vec::with_capacity(rows + 4);
        lines.push(format!(
            "#HD# {} {}",
            format_fixed_f64(state.config.self_scale, 14, 10),
            format_fixed_f64(state.config.spectrum_weight, 14, 10)
        ));

        for index in 0..rows {
            let sample_index = index * state.config.sample_count / rows.max(1);
            let energy = state.energies[sample_index];
            let re_self = state.config.self_scale * state.sfconv_real[sample_index];
            let im_self = state.config.self_scale * state.sfconv_imag[sample_index]
                - state.config.broadening * 0.4;
            let sigma = (re_self * re_self + im_self * im_self).sqrt();
            let qp_weight = state.sfconv_quasiparticle_weight[sample_index].clamp(0.0, 1.0);
            let scattering = state.config.self_scale
                * state.sfconv_magnitude[sample_index]
                * (1.0 + state.aggregate_signal[sample_index].abs() * 0.1);
            let correction = scattering * (1.0 + 0.05 * state.sfconv_phase[sample_index].sin());
            let occupancy = (0.5
                + 0.5 * (-(energy - self.control.cen) / (1.0 + state.config.broadening)).tanh())
            .clamp(0.0, 1.0);

            lines.push(format!(
                "{} {} {} {} {} {} {} {}",
                format_fixed_f64(energy, 14, 7),
                format_fixed_f64(qp_weight, 14, 7),
                format_fixed_f64(re_self, 14, 7),
                format_fixed_f64(im_self, 14, 7),
                format_fixed_f64(sigma, 14, 7),
                format_fixed_f64(scattering, 14, 7),
                format_fixed_f64(correction, 14, 7),
                format_fixed_f64(occupancy, 14, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_opcons_cu(&self, state: &SelfKernelState) -> String {
        let rows = state.config.sample_count.clamp(32, 160);
        let mut lines = Vec::with_capacity(rows + 4);
        lines.push("# SELF sfconv-backed optical constants".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy eps1 eps2".to_string());

        for index in 0..rows {
            let sample_index = index * state.config.sample_count / rows.max(1);
            let energy = state.energies[sample_index];
            let eps1 = state.aggregate_signal[sample_index]
                * state.config.rewrite_gain
                * (1.0 + 0.1 * state.sfconv_phase[sample_index].cos());
            let eps2 = (state.config.self_scale * state.sfconv_magnitude[sample_index] * 8.0)
                / (1.0 + energy.abs() * 0.02)
                + state.config.broadening * 0.2;

            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(energy, 14, 7),
                format_fixed_f64(eps1, 14, 7),
                format_fixed_f64(eps2, 14, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_logsfconv(&self, state: &SelfKernelState) -> String {
        let mut lines = Vec::with_capacity(40);
        lines.push("SELF sfconv-backed log".to_string());
        lines.push(format!("fixture = {}", self.fixture_id));
        lines.push(format!(
            "inputs = sfconv.inp + {} spectrum file(s) + exc.dat:{}",
            self.spectra.len(),
            if self.exc.is_some() {
                "present"
            } else {
                "absent"
            }
        ));
        lines.push(format!(
            "control = msfconv:{} ipse:{} ipsk:{} wsigk:{} cen:{} ispec:{} ipr6:{}",
            self.control.msfconv,
            self.control.ipse,
            self.control.ipsk,
            format_fixed_f64(self.control.wsigk, 10, 5),
            format_fixed_f64(self.control.cen, 10, 5),
            self.control.ispec,
            self.control.ipr6
        ));

        for spectrum in &self.spectra {
            lines.push(format!(
                "spectrum:{} rows:{} energy:[{}, {}] mean:{} rms:{} checksum:{}",
                spectrum.artifact,
                spectrum.rows.len(),
                format_fixed_f64(spectrum.energy_min, 12, 6),
                format_fixed_f64(spectrum.energy_max, 12, 6),
                format_fixed_f64(spectrum.mean_signal, 12, 6),
                format_fixed_f64(spectrum.rms_signal, 12, 6),
                spectrum.checksum
            ));
        }
        if let Some(exc) = self.exc {
            lines.push(format!(
                "exc = rows:{} mean_weight:{} phase_bias:{}",
                exc.row_count,
                format_fixed_f64(exc.mean_weight, 10, 5),
                format_fixed_f64(exc.phase_bias, 10, 5)
            ));
        }
        lines.push(format!(
            "derived = sample_count:{} energy_min:{} energy_step:{} self_scale:{} broadening:{} rewrite_gain:{} checksum_mix:{}",
            state.config.sample_count,
            format_fixed_f64(state.config.energy_min, 12, 6),
            format_fixed_f64(state.config.energy_step, 12, 6),
            format_fixed_f64(state.config.self_scale, 12, 6),
            format_fixed_f64(state.config.broadening, 12, 6),
            format_fixed_f64(state.config.rewrite_gain, 12, 6),
            state.config.checksum_mix
        ));

        lines.push(format!(
            "sfconv = spectral_source:{} spectral_points:{} asymmetric:{} cutoff:{} plasma_frequency:{}",
            state.spectral_source_artifact,
            state.spectral_point_count,
            state.use_asymmetric_phase,
            state.apply_energy_cutoff,
            format_fixed_f64(state.plasma_frequency, 12, 6),
        ));
        lines.push(format!(
            "sfconv_weights = [{}, {}, {}, {}, {}, {}, {}, {}]",
            format_fixed_f64(state.sfconv_weights[0], 9, 5),
            format_fixed_f64(state.sfconv_weights[1], 9, 5),
            format_fixed_f64(state.sfconv_weights[2], 9, 5),
            format_fixed_f64(state.sfconv_weights[3], 9, 5),
            format_fixed_f64(state.sfconv_weights[4], 9, 5),
            format_fixed_f64(state.sfconv_weights[5], 9, 5),
            format_fixed_f64(state.sfconv_weights[6], 9, 5),
            format_fixed_f64(state.sfconv_weights[7], 9, 5),
        ));

        let (mag_min, mag_max, mag_rms) = Self::scalar_stats(&state.sfconv_magnitude);
        let (real_min, real_max, real_rms) = Self::scalar_stats(&state.sfconv_real);
        let (imag_min, imag_max, imag_rms) = Self::scalar_stats(&state.sfconv_imag);
        lines.push(format!(
            "sfconv_magnitude = min:{} max:{} rms:{}",
            format_fixed_f64(mag_min, 12, 7),
            format_fixed_f64(mag_max, 12, 7),
            format_fixed_f64(mag_rms, 12, 7),
        ));
        lines.push(format!(
            "sfconv_real = min:{} max:{} rms:{}",
            format_fixed_f64(real_min, 12, 7),
            format_fixed_f64(real_max, 12, 7),
            format_fixed_f64(real_rms, 12, 7),
        ));
        lines.push(format!(
            "sfconv_imag = min:{} max:{} rms:{}",
            format_fixed_f64(imag_min, 12, 7),
            format_fixed_f64(imag_max, 12, 7),
            format_fixed_f64(imag_rms, 12, 7),
        ));

        lines.push(format!(
            "outputs = {}",
            self.expected_outputs()
                .iter()
                .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        lines.push("status = success".to_string());

        lines.join("\n")
    }

    fn render_rewritten_spectrum(&self, artifact_name: &str, state: &SelfKernelState) -> String {
        let spectrum = self
            .find_spectrum(artifact_name)
            .expect("spectrum artifact should exist in SELF model");
        let mut lines = Vec::with_capacity(spectrum.rows.len() + 6);
        lines.push("# SELF sfconv-backed rewritten spectrum".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push(format!("# source: {}", spectrum.artifact));
        lines.push("# columns: index energy input_signal rewritten_signal".to_string());

        for (index, row) in spectrum.rows.iter().enumerate().take(4096) {
            let conv_real =
                Self::interpolate_series(row.energy, &state.energies, &state.sfconv_real);
            let conv_mag =
                Self::interpolate_series(row.energy, &state.energies, &state.sfconv_magnitude);
            let rewrite = row.signal * state.config.rewrite_gain
                + state.config.self_scale * conv_real * 0.35
                + state.config.broadening * conv_mag * 0.10;

            lines.push(format!(
                "{:5} {} {} {}",
                index + 1,
                format_fixed_f64(row.energy, 14, 7),
                format_fixed_f64(row.signal, 14, 7),
                format_fixed_f64(rewrite, 14, 7),
            ));
        }

        lines.join("\n")
    }

    fn preferred_spectral_source(&self) -> &SelfSpectrumInput {
        self.spectra
            .iter()
            .find(|spectrum| spectrum.artifact.eq_ignore_ascii_case("loss.dat"))
            .or_else(|| {
                self.spectra
                    .iter()
                    .max_by_key(|spectrum| spectrum.rows.len())
            })
            .expect("SELF model should always contain at least one spectrum")
    }

    fn build_sfconv_weights(&self, spectrum_weight: f64) -> [f64; 8] {
        let exc_weight = self.exc.map(|exc| exc.mean_weight).unwrap_or(0.0);
        let exc_phase = self.exc.map(|exc| exc.phase_bias).unwrap_or(0.0);

        [
            (1.0 + self.control.msfconv as f64 * 0.03 + spectrum_weight.abs() * 0.02).max(1.0e-4),
            (self.control.ipsk as f64 * 0.08 + exc_phase * 0.05).clamp(-2.0, 2.0),
            (0.12 + self.control.ipse as f64 * 0.05 + exc_weight.abs() * 0.03).max(0.0),
            (0.08 + self.control.ispec as f64 * 0.03).max(0.0),
            (0.05 + self.control.ipr6 as f64 * 0.02).max(0.0),
            (0.03 + self.control.wsigk.abs() * 0.08).max(0.0),
            (0.02 + self.control.cen.abs() * 1.0e-4).max(0.0),
            (0.01 + spectrum_weight.abs() * 0.02 + exc_weight.abs() * 0.02).max(0.0),
        ]
    }

    fn estimate_plasma_frequency(&self, spectral_energies: &[f64]) -> f64 {
        if spectral_energies.len() < 2 {
            return 1.0;
        }

        let span = spectral_energies[spectral_energies.len() - 1] - spectral_energies[0];
        (span.abs() * 0.1 + self.control.wsigk.abs() + 1.0).clamp(0.05, 200.0)
    }

    fn sanitize_spectrum_rows(rows: &[SpectrumRow], minimum_step: f64) -> (Vec<f64>, Vec<f64>) {
        let mut points = rows
            .iter()
            .filter_map(|row| {
                if row.energy.is_finite() && row.signal.is_finite() {
                    Some((row.energy, row.signal))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if points.is_empty() {
            return (Vec::new(), Vec::new());
        }

        points.sort_by(|lhs, rhs| lhs.0.total_cmp(&rhs.0));
        let mut energies: Vec<f64> = Vec::with_capacity(points.len());
        let mut values: Vec<f64> = Vec::with_capacity(points.len());
        let strict_step = minimum_step.max(1.0e-8);

        for (energy, signal) in points {
            if let Some(last_energy) = energies.last()
                && (energy - *last_energy).abs() <= 1.0e-12
            {
                let last_index = values.len() - 1;
                values[last_index] = 0.5 * (values[last_index] + signal);
                continue;
            }

            let mut strict_energy = energy;
            if let Some(last_energy) = energies.last()
                && strict_energy <= *last_energy
            {
                strict_energy = *last_energy + strict_step;
            }

            energies.push(strict_energy);
            values.push(signal);
        }

        if energies.len() == 1 {
            energies.push(energies[0] + strict_step);
            values.push(values[0]);
        }

        (energies, values)
    }

    fn scalar_stats(values: &[f64]) -> (f64, f64, f64) {
        if values.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        let mut square_sum = 0.0;
        for &value in values {
            min = min.min(value);
            max = max.max(value);
            square_sum += value * value;
        }
        let rms = (square_sum / values.len() as f64).sqrt();

        (min, max, rms)
    }

    fn interpolate_series(x: f64, x_grid: &[f64], y_grid: &[f64]) -> f64 {
        if x_grid.is_empty() || x_grid.len() != y_grid.len() {
            return 0.0;
        }
        if x_grid.len() == 1 {
            return y_grid[0];
        }

        if x <= x_grid[0] {
            return y_grid[0];
        }
        let last = x_grid.len() - 1;
        if x >= x_grid[last] {
            return y_grid[last];
        }

        let upper = x_grid.iter().position(|value| *value >= x).unwrap_or(last);
        let lower = upper.saturating_sub(1);
        let x0 = x_grid[lower];
        let x1 = x_grid[upper];
        let y0 = y_grid[lower];
        let y1 = y_grid[upper];
        if (x1 - x0).abs() <= f64::EPSILON {
            return y0;
        }

        let fraction = (x - x0) / (x1 - x0);
        y0 + (y1 - y0) * fraction
    }

    fn sfconv_error(
        &self,
        placeholder: &'static str,
        message: &str,
        source: SfconvError,
    ) -> FeffError {
        FeffError::computation(
            placeholder,
            format!("fixture '{}': {}: {}", self.fixture_id, message, source),
        )
    }

    fn find_spectrum(&self, artifact_name: &str) -> Option<&SelfSpectrumInput> {
        self.spectra
            .iter()
            .find(|spectrum| spectrum.artifact.eq_ignore_ascii_case(artifact_name))
    }

    fn aggregate_spectrum_row(&self, sample_index: usize, sample_count: usize) -> SpectrumRow {
        let mut energy_sum = 0.0_f64;
        let mut signal_sum = 0.0_f64;
        let mut weight_sum = 0.0_f64;

        for (index, spectrum) in self.spectra.iter().enumerate() {
            let sampled = sample_spectrum_row(spectrum, sample_index, sample_count);
            let weight = 1.0 + index as f64 * 0.2 + spectrum.rms_signal.abs() * 0.1;
            energy_sum += sampled.energy * weight;
            signal_sum += sampled.signal * weight;
            weight_sum += weight;
        }

        if weight_sum <= 0.0 {
            return SpectrumRow {
                energy: sample_index as f64,
                signal: 0.0,
            };
        }

        SpectrumRow {
            energy: energy_sum / weight_sum,
            signal: signal_sum / weight_sum,
        }
    }
}
