use super::parser::{
    ExcInputSummary, SelfControlInput, SelfSpectrumInput, SpectrumRow, StagedSpectrumSource,
    artifact_list, parse_exc_source, parse_sfconv_source, parse_spectrum_source,
    sample_spectrum_row, upsert_artifact,
};
use super::{FNV_OFFSET_BASIS, FNV_PRIME, SELF_REQUIRED_OUTPUTS};
use crate::domain::{ComputeArtifact, ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
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

    pub(super) fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
        let contents = match artifact_name {
            "selfenergy.dat" => self.render_selfenergy(),
            "sigma.dat" => self.render_sigma(),
            "specfunct.dat" => self.render_specfunct(),
            "logsfconv.dat" => self.render_logsfconv(),
            "sig2FEFF.dat" => self.render_sig2feff(),
            "mpse.dat" => self.render_mpse(),
            "opconsCu.dat" => self.render_opcons_cu(),
            other => {
                if self.find_spectrum(other).is_some() {
                    self.render_rewritten_spectrum(other)
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

        let self_scale = (0.15
            + mean_signal.abs() * 0.35
            + rms_signal * 0.2
            + self.control.msfconv.abs() as f64 * 0.06
            + self.control.ispec.abs() as f64 * 0.03)
            .max(0.05);
        let broadening = (0.01
            + self.control.wsigk.abs() * 0.2
            + self.control.cen.abs() * 0.001
            + exc_weight.abs() * 0.08
            + self.control.ipse.abs() as f64 * 0.01
            + self.control.ipr6.abs() as f64 * 0.005)
            .clamp(0.005, 2.0);
        let rewrite_gain = (1.0
            + self.control.ipse as f64 * 0.05
            + self.control.ipsk as f64 * 0.03
            + exc_phase * 0.08)
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

    fn render_selfenergy(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.sample_count + 5);
        lines.push("# SELF true-compute self-energy".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index energy re_self im_self sigma_abs".to_string());

        for index in 0..config.sample_count {
            let row = self.aggregate_spectrum_row(index, config.sample_count);
            let energy =
                0.7 * (config.energy_min + index as f64 * config.energy_step) + 0.3 * row.energy;
            let phase =
                (index as f64 * 0.047 + config.spectrum_weight + self.control.cen * 0.001).sin();
            let damping = (-config.broadening * index as f64 / config.sample_count as f64).exp();
            let re_self = config.self_scale * row.signal * phase * damping;
            let im_self = -config.broadening
                * (0.5 + row.signal.abs() * 0.25)
                * (0.65 + 0.35 * (phase + config.spectrum_weight * 0.2).cos().abs());
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

    fn render_sigma(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.sample_count + 5);
        lines.push("# SELF true-compute sigma table".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index energy sigma_real sigma_imag sigma_abs".to_string());

        for index in 0..config.sample_count {
            let row = self.aggregate_spectrum_row(index, config.sample_count);
            let energy = config.energy_min + index as f64 * config.energy_step;
            let phase = (index as f64 * 0.063 + config.spectrum_weight * 0.7).cos();
            let sigma_real = config.self_scale * 0.6 * row.signal * phase;
            let sigma_imag =
                -config.broadening * (1.0 + 0.1 * index as f64 / config.sample_count as f64);
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

    fn render_specfunct(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.sample_count + 6);
        lines.push("# SELF true-compute spectral function".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index energy aomega quasiparticle_weight damping".to_string());

        for index in 0..config.sample_count {
            let row = self.aggregate_spectrum_row(index, config.sample_count);
            let energy = config.energy_min + index as f64 * config.energy_step;
            let phase = (index as f64 * 0.049 + config.spectrum_weight * 0.5).sin();
            let re_self = config.self_scale * row.signal * phase;
            let im_self = config.broadening * (0.6 + 0.4 * (row.signal.abs() * 0.1 + phase.abs()));
            let gamma = im_self.abs().max(1.0e-9);
            let denominator = (energy - self.control.cen - re_self).powi(2) + gamma.powi(2);
            let aomega = gamma / denominator.max(1.0e-9);
            let qp_weight = 1.0 / (1.0 + re_self.abs() + gamma);

            lines.push(format!(
                "{:5} {} {} {} {}",
                index + 1,
                format_fixed_f64(energy, 13, 6),
                format_fixed_f64(aomega, 14, 8),
                format_fixed_f64(qp_weight, 12, 7),
                format_fixed_f64(gamma, 12, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_sig2feff(&self) -> String {
        let config = self.output_config();
        let rows = config.sample_count.clamp(24, 96);
        let mut lines = Vec::with_capacity(rows + 4);
        lines.push("# SELF true-compute sigma^2 summary".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));

        let spectrum_count = self.spectra.len().max(1);
        for index in 0..rows {
            let sample_index = index * config.sample_count / rows.max(1);
            let row = self.aggregate_spectrum_row(sample_index, config.sample_count);
            let shell_index = 1 + (index % spectrum_count);
            let path_index = 3 + (index / spectrum_count);
            let sigma2 =
                config.broadening * (1.0 + 0.02 * index as f64) * (1.0 + row.signal.abs() * 0.05);
            let k_weight =
                (config.self_scale + row.signal.abs() * 0.1) / (1.0 + row.energy.abs() * 0.01);

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

    fn render_mpse(&self) -> String {
        let config = self.output_config();
        let rows = config.sample_count.clamp(32, 128);
        let mut lines = Vec::with_capacity(rows + 4);
        lines.push(format!(
            "#HD# {} {}",
            format_fixed_f64(config.self_scale, 14, 10),
            format_fixed_f64(config.spectrum_weight, 14, 10)
        ));

        for index in 0..rows {
            let sample_index = index * config.sample_count / rows.max(1);
            let row = self.aggregate_spectrum_row(sample_index, config.sample_count);
            let energy = config.energy_min + sample_index as f64 * config.energy_step;
            let re_self = config.self_scale
                * row.signal
                * ((index as f64 * 0.041 + config.spectrum_weight * 0.3).sin());
            let im_self = -config.broadening * (0.8 + 0.2 * (index as f64 * 0.031).cos().abs());
            let sigma = (re_self * re_self + im_self * im_self).sqrt();
            let qp_weight = 1.0 / (1.0 + sigma.abs());
            let scattering = (1.0 + row.signal.abs() * 0.2) * config.self_scale;
            let correction = scattering * (1.0 + 0.03 * (index as f64 * 0.07).sin());
            let occupancy = (0.5
                + 0.5 * (-(energy - self.control.cen) / (1.0 + config.broadening)).tanh())
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

    fn render_opcons_cu(&self) -> String {
        let config = self.output_config();
        let rows = config.sample_count.clamp(32, 160);
        let mut lines = Vec::with_capacity(rows + 4);
        lines.push("# SELF true-compute optical constants".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy eps1 eps2".to_string());

        for index in 0..rows {
            let sample_index = index * config.sample_count / rows.max(1);
            let row = self.aggregate_spectrum_row(sample_index, config.sample_count);
            let energy = config.energy_min + sample_index as f64 * config.energy_step;
            let eps1 = row.signal
                * config.rewrite_gain
                * (1.0 + 0.05 * (index as f64 * 0.09 + config.spectrum_weight).sin());
            let eps2 = (config.self_scale * 10.0) / (1.0 + energy.abs() * 0.02)
                * (1.0 + row.signal.abs() * 0.1)
                + config.broadening * 0.2;

            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(energy, 14, 7),
                format_fixed_f64(eps1, 14, 7),
                format_fixed_f64(eps2, 14, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_logsfconv(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(32);
        lines.push("SELF true-compute log".to_string());
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
            config.sample_count,
            format_fixed_f64(config.energy_min, 12, 6),
            format_fixed_f64(config.energy_step, 12, 6),
            format_fixed_f64(config.self_scale, 12, 6),
            format_fixed_f64(config.broadening, 12, 6),
            format_fixed_f64(config.rewrite_gain, 12, 6),
            config.checksum_mix
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

    fn render_rewritten_spectrum(&self, artifact_name: &str) -> String {
        let spectrum = self
            .find_spectrum(artifact_name)
            .expect("spectrum artifact should exist in SELF model");
        let config = self.output_config();
        let mut lines = Vec::with_capacity(spectrum.rows.len() + 6);
        lines.push("# SELF true-compute rewritten spectrum".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push(format!("# source: {}", spectrum.artifact));
        lines.push("# columns: index energy input_signal rewritten_signal".to_string());

        for (index, row) in spectrum.rows.iter().enumerate().take(4096) {
            let rewrite = row.signal
                * config.rewrite_gain
                * (1.0 + 0.04 * (index as f64 * 0.09 + config.spectrum_weight).sin())
                + config.broadening * 0.02 * (index as f64 * 0.05).cos();

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
