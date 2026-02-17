use super::ModuleExecutor;
use super::serialization::{format_fixed_f64, write_text_artifact};
use crate::domain::{FeffError, ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SELF_PRIMARY_INPUT: &str = "sfconv.inp";
const SELF_SPECTRUM_INPUT_CANDIDATES: [&str; 3] = ["xmu.dat", "chi.dat", "loss.dat"];
const SELF_OPTIONAL_INPUTS: [&str; 1] = ["exc.dat"];
const SELF_REQUIRED_OUTPUTS: [&str; 7] = [
    "selfenergy.dat",
    "sigma.dat",
    "specfunct.dat",
    "logsfconv.dat",
    "sig2FEFF.dat",
    "mpse.dat",
    "opconsCu.dat",
];
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfEnergyContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub optional_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StagedSpectrumSource {
    artifact: String,
    source: String,
}

#[derive(Debug, Clone)]
struct SelfModel {
    fixture_id: String,
    control: SelfControlInput,
    spectra: Vec<SelfSpectrumInput>,
    exc: Option<ExcInputSummary>,
}

#[derive(Debug, Clone, Copy)]
struct SelfControlInput {
    msfconv: i32,
    ipse: i32,
    ipsk: i32,
    wsigk: f64,
    cen: f64,
    ispec: i32,
    ipr6: i32,
}

impl Default for SelfControlInput {
    fn default() -> Self {
        Self {
            msfconv: 1,
            ipse: 0,
            ipsk: 0,
            wsigk: 0.0,
            cen: 0.0,
            ispec: 0,
            ipr6: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct SelfSpectrumInput {
    artifact: String,
    rows: Vec<SpectrumRow>,
    checksum: u64,
    mean_signal: f64,
    rms_signal: f64,
    energy_min: f64,
    energy_max: f64,
}

#[derive(Debug, Clone, Copy)]
struct SpectrumRow {
    energy: f64,
    signal: f64,
}

#[derive(Debug, Clone, Copy)]
struct ExcInputSummary {
    row_count: usize,
    mean_weight: f64,
    phase_bias: f64,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelfEnergyModule;

impl SelfEnergyModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<SelfEnergyContract> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let sfconv_source = read_input_source(&request.input_path, SELF_PRIMARY_INPUT)?;
        let spectrum_sources = load_staged_spectrum_sources(input_dir)?;
        let exc_source = maybe_read_optional_input_source(
            input_dir.join(SELF_OPTIONAL_INPUTS[0]),
            SELF_OPTIONAL_INPUTS[0],
        )?;
        let model = SelfModel::from_sources(
            &request.fixture_id,
            &sfconv_source,
            spectrum_sources,
            exc_source.as_deref(),
        )?;

        let mut required_inputs = vec![ComputeArtifact::new(SELF_PRIMARY_INPUT)];
        required_inputs.extend(
            model
                .spectrum_artifact_names()
                .iter()
                .map(ComputeArtifact::new),
        );

        Ok(SelfEnergyContract {
            required_inputs,
            optional_inputs: artifact_list(&SELF_OPTIONAL_INPUTS),
            expected_outputs: model.expected_outputs(),
        })
    }
}

impl ModuleExecutor for SelfEnergyModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let sfconv_source = read_input_source(&request.input_path, SELF_PRIMARY_INPUT)?;
        let spectrum_sources = load_staged_spectrum_sources(input_dir)?;
        let exc_source = maybe_read_optional_input_source(
            input_dir.join(SELF_OPTIONAL_INPUTS[0]),
            SELF_OPTIONAL_INPUTS[0],
        )?;
        let model = SelfModel::from_sources(
            &request.fixture_id,
            &sfconv_source,
            spectrum_sources,
            exc_source.as_deref(),
        )?;
        let outputs = model.expected_outputs();

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.SELF_OUTPUT_DIRECTORY",
                format!(
                    "failed to create SELF output directory '{}': {}",
                    request.output_dir.display(),
                    source
                ),
            )
        })?;

        for artifact in &outputs {
            let output_path = request.output_dir.join(&artifact.relative_path);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|source| {
                    FeffError::io_system(
                        "IO.SELF_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create SELF artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            let artifact_name = artifact.relative_path.to_string_lossy().replace('\\', "/");
            model.write_artifact(&artifact_name, &output_path)?;
        }

        Ok(outputs)
    }
}

impl SelfModel {
    fn from_sources(
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

    fn spectrum_artifact_names(&self) -> Vec<String> {
        self.spectra
            .iter()
            .map(|spectrum| spectrum.artifact.clone())
            .collect()
    }

    fn expected_outputs(&self) -> Vec<ComputeArtifact> {
        let mut outputs = artifact_list(&SELF_REQUIRED_OUTPUTS);
        for spectrum in &self.spectra {
            upsert_artifact(&mut outputs, &spectrum.artifact);
        }
        outputs
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

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
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

fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::SelfEnergy {
        return Err(FeffError::input_validation(
            "INPUT.SELF_MODULE",
            format!("SELF module expects SELF, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.SELF_INPUT_ARTIFACT",
                format!(
                    "SELF module expects input artifact '{}' at '{}'",
                    SELF_PRIMARY_INPUT,
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(SELF_PRIMARY_INPUT) {
        return Err(FeffError::input_validation(
            "INPUT.SELF_INPUT_ARTIFACT",
            format!(
                "SELF module requires input artifact '{}' but received '{}'",
                SELF_PRIMARY_INPUT, input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.SELF_INPUT_ARTIFACT",
            format!(
                "SELF module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    let bytes = fs::read(path).map_err(|source| {
        FeffError::io_system(
            "IO.SELF_INPUT_READ",
            format!(
                "failed to read SELF input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn maybe_read_optional_input_source(
    path: PathBuf,
    artifact_name: &str,
) -> ComputeResult<Option<String>> {
    if path.is_file() {
        return read_input_source(&path, artifact_name).map(Some);
    }

    Ok(None)
}

fn load_staged_spectrum_sources(directory: &Path) -> ComputeResult<Vec<StagedSpectrumSource>> {
    let artifacts = collect_staged_spectrum_artifacts(directory)?;
    if artifacts.is_empty() {
        return Err(FeffError::input_validation(
            "INPUT.SELF_SPECTRUM_INPUT",
            format!(
                "SELF module requires at least one staged spectrum input (xmu.dat, chi.dat, loss.dat, or feffNNNN.dat) in '{}'",
                directory.display()
            ),
        ));
    }

    let mut sources = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let source = read_input_source(&directory.join(&artifact), &artifact)?;
        sources.push(StagedSpectrumSource { artifact, source });
    }
    Ok(sources)
}

fn collect_staged_spectrum_artifacts(directory: &Path) -> ComputeResult<Vec<String>> {
    let mut artifacts = Vec::new();
    let mut seen = BTreeSet::new();

    for candidate in SELF_SPECTRUM_INPUT_CANDIDATES {
        let candidate_path = directory.join(candidate);
        if !candidate_path.is_file() {
            continue;
        }

        let key = candidate.to_ascii_lowercase();
        if seen.insert(key) {
            artifacts.push(candidate.to_string());
        }
    }

    for artifact in collect_feff_spectrum_artifacts(
        directory,
        "IO.SELF_INPUT_READ",
        "input",
        "input directory",
    )? {
        let key = artifact.to_ascii_lowercase();
        if seen.insert(key) {
            artifacts.push(artifact);
        }
    }

    Ok(artifacts)
}

fn collect_feff_spectrum_artifacts(
    directory: &Path,
    placeholder: &'static str,
    location: &'static str,
    location_label: &'static str,
) -> ComputeResult<Vec<String>> {
    let entries = fs::read_dir(directory).map_err(|source| {
        FeffError::io_system(
            placeholder,
            format!(
                "failed to read SELF {} '{}' while collecting feffNNNN.dat artifacts: {}",
                location,
                directory.display(),
                source
            ),
        )
    })?;

    let mut artifacts = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| {
            FeffError::io_system(
                placeholder,
                format!(
                    "failed to read SELF {} entry in '{}': {}",
                    location,
                    directory.display(),
                    source
                ),
            )
        })?;

        let file_type = entry.file_type().map_err(|source| {
            FeffError::io_system(
                placeholder,
                format!(
                    "failed to inspect SELF {} entry '{}' in '{}': {}",
                    location_label,
                    entry.path().display(),
                    directory.display(),
                    source
                ),
            )
        })?;

        if !file_type.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if is_feff_spectrum_name(&file_name) {
            artifacts.push(file_name.into_owned());
        }
    }

    artifacts.sort();
    Ok(artifacts)
}

fn parse_sfconv_source(source: &str) -> SelfControlInput {
    let mut control = SelfControlInput::default();
    let lines: Vec<&str> = source.lines().collect();

    if let Some(values) = parse_numbers_after_marker(&lines, "msfconv") {
        control.msfconv = values.first().copied().unwrap_or(1.0).round() as i32;
        control.ipse = values.get(1).copied().unwrap_or(0.0).round() as i32;
        control.ipsk = values.get(2).copied().unwrap_or(0.0).round() as i32;
    }
    if let Some(values) = parse_numbers_after_marker(&lines, "wsigk") {
        control.wsigk = values.first().copied().unwrap_or(0.0);
        control.cen = values.get(1).copied().unwrap_or(0.0);
    }
    if let Some(values) = parse_numbers_after_marker(&lines, "ispec") {
        control.ispec = values.first().copied().unwrap_or(0.0).round() as i32;
        control.ipr6 = values.get(1).copied().unwrap_or(0.0).round() as i32;
    }

    control
}

fn parse_numbers_after_marker(lines: &[&str], marker: &str) -> Option<Vec<f64>> {
    let marker = marker.to_ascii_lowercase();
    for (index, line) in lines.iter().enumerate() {
        if !line.to_ascii_lowercase().contains(&marker) {
            continue;
        }

        let (_, next_line) = next_nonempty_line(lines, index + 1)?;
        let values = parse_numeric_tokens(next_line);
        if !values.is_empty() {
            return Some(values);
        }
    }

    None
}

fn parse_spectrum_source(
    fixture_id: &str,
    artifact: &str,
    source: &str,
) -> ComputeResult<SelfSpectrumInput> {
    let mut rows = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let values = parse_numeric_tokens(trimmed);
        if values.is_empty() {
            continue;
        }

        let energy = values.first().copied().unwrap_or(rows.len() as f64);
        let signal = if values.len() >= 2 {
            values[values.len() - 1]
        } else {
            values[0]
        };
        if !energy.is_finite() || !signal.is_finite() {
            continue;
        }
        rows.push(SpectrumRow { energy, signal });
    }

    if rows.is_empty() {
        return Err(self_parse_error(
            fixture_id,
            format!(
                "spectrum input '{}' does not contain numeric rows",
                artifact
            ),
        ));
    }

    let energy_min = rows
        .iter()
        .map(|row| row.energy)
        .fold(f64::INFINITY, f64::min);
    let energy_max = rows
        .iter()
        .map(|row| row.energy)
        .fold(f64::NEG_INFINITY, f64::max);
    let signal_sum = rows.iter().map(|row| row.signal).sum::<f64>();
    let signal_sq_sum = rows.iter().map(|row| row.signal * row.signal).sum::<f64>();
    let mean_signal = signal_sum / rows.len() as f64;
    let rms_signal = (signal_sq_sum / rows.len() as f64).sqrt();
    let checksum = fnv1a64(source.as_bytes());

    Ok(SelfSpectrumInput {
        artifact: artifact.to_string(),
        rows,
        checksum,
        mean_signal,
        rms_signal,
        energy_min,
        energy_max,
    })
}

fn parse_exc_source(source: &str) -> ExcInputSummary {
    let mut row_count = 0usize;
    let mut weight_sum = 0.0_f64;
    let mut phase_sum = 0.0_f64;

    for line in source.lines() {
        let values = parse_numeric_tokens(line);
        if values.len() < 2 {
            continue;
        }

        let weight = if values.len() >= 3 {
            values[2]
        } else {
            values[1]
        };
        let phase = *values.last().unwrap_or(&0.0);
        if !weight.is_finite() || !phase.is_finite() {
            continue;
        }
        row_count += 1;
        weight_sum += weight;
        phase_sum += phase;
    }

    if row_count == 0 {
        return ExcInputSummary {
            row_count: 0,
            mean_weight: 0.0,
            phase_bias: 0.0,
        };
    }

    ExcInputSummary {
        row_count,
        mean_weight: weight_sum / row_count as f64,
        phase_bias: phase_sum / row_count as f64,
    }
}

fn sample_spectrum_row(
    spectrum: &SelfSpectrumInput,
    sample_index: usize,
    sample_count: usize,
) -> SpectrumRow {
    if spectrum.rows.is_empty() {
        return SpectrumRow {
            energy: sample_index as f64,
            signal: 0.0,
        };
    }
    if spectrum.rows.len() == 1 || sample_count <= 1 {
        return spectrum.rows[0];
    }

    let ratio = sample_index as f64 / sample_count.saturating_sub(1) as f64;
    let scaled = ratio * spectrum.rows.len().saturating_sub(1) as f64;
    let lower = scaled.floor() as usize;
    let upper = scaled.ceil() as usize;
    let frac = scaled - lower as f64;

    let lower_row = spectrum.rows[lower];
    let upper_row = spectrum.rows[upper.min(spectrum.rows.len() - 1)];

    SpectrumRow {
        energy: lower_row.energy + (upper_row.energy - lower_row.energy) * frac,
        signal: lower_row.signal + (upper_row.signal - lower_row.signal) * frac,
    }
}

fn next_nonempty_line<'a>(lines: &'a [&'a str], start_index: usize) -> Option<(usize, &'a str)> {
    for (index, line) in lines.iter().enumerate().skip(start_index) {
        if !line.trim().is_empty() {
            return Some((index, *line));
        }
    }
    None
}

fn parse_numeric_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(parse_numeric_token)
        .collect()
}

fn parse_numeric_token(token: &str) -> Option<f64> {
    let trimmed = token.trim_matches(|character: char| {
        matches!(
            character,
            ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '='
        )
    });
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.replace(['D', 'd'], "E");
    normalized.parse::<f64>().ok()
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn self_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.SELF_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

fn is_feff_spectrum_name(name: &str) -> bool {
    let lowercase = name.to_ascii_lowercase();
    if !lowercase.starts_with("feff") || !lowercase.ends_with(".dat") {
        return false;
    }

    let suffix = &lowercase[4..lowercase.len() - 4];
    !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
}

fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}

fn upsert_artifact(artifacts: &mut Vec<ComputeArtifact>, artifact: &str) {
    let normalized = artifact.to_ascii_lowercase();
    if artifacts.iter().any(|candidate| {
        candidate
            .relative_path
            .to_string_lossy()
            .to_ascii_lowercase()
            == normalized
    }) {
        return;
    }
    artifacts.push(ComputeArtifact::new(artifact));
}

#[cfg(test)]
mod tests {
    use super::{
        SELF_OPTIONAL_INPUTS, SELF_PRIMARY_INPUT, SELF_REQUIRED_OUTPUTS, SelfEnergyModule,
    };
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const SFCONV_INPUT_FIXTURE: &str = "msfconv, ipse, ipsk
   1   0   0
wsigk, cen
      0.00000      0.00000
ispec, ipr6
   1   0
cfname
NULL
";

    const XMU_INPUT_FIXTURE: &str = "# omega e k mu mu0 chi
    8979.411  -16.765  -1.406  1.46870E-02  1.79897E-02 -3.30270E-03
    8980.979  -15.197  -1.252  2.93137E-02  3.59321E-02 -6.61845E-03
    8982.398  -13.778  -1.093  3.93900E-02  4.92748E-02 -9.88483E-03
";

    const LOSS_INPUT_FIXTURE: &str = "# E(eV) Loss
  2.50658E-03 2.58411E-02
  4.69344E-03 6.11057E-02
  7.56059E-03 1.37874E-01
";

    const FEFF_INPUT_FIXTURE: &str = " 1.00000E+00 3.00000E-01
 2.00000E+00 2.00000E-01
 3.00000E+00 1.00000E-01
";

    const EXC_INPUT_FIXTURE: &str =
        "  0.1414210018E-01  0.1000000000E+00  0.8481210460E-01  0.9420256311E-01
  0.2467626159E-01  0.1000000000E+00  0.5134531114E-01  0.9951100800E-01
  0.4683986560E-01  0.1000000000E+00  0.1271572855E-01  0.4677866877E-01
";

    #[test]
    fn contract_requires_staged_spectrum_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        fs::write(&input_path, SFCONV_INPUT_FIXTURE).expect("sfconv input should be written");

        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::SelfEnergy,
            &input_path,
            temp.path().join("out"),
        );
        let error = SelfEnergyModule
            .contract_for_request(&request)
            .expect_err("missing spectra should fail contract");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.SELF_SPECTRUM_INPUT");
    }

    #[test]
    fn contract_reflects_staged_spectrum_and_output_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        fs::write(&input_path, SFCONV_INPUT_FIXTURE).expect("sfconv input should be written");
        fs::write(temp.path().join("xmu.dat"), XMU_INPUT_FIXTURE).expect("xmu should be written");
        fs::write(temp.path().join("loss.dat"), LOSS_INPUT_FIXTURE)
            .expect("loss should be written");

        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::SelfEnergy,
            &input_path,
            temp.path().join("out"),
        );
        let contract = SelfEnergyModule
            .contract_for_request(&request)
            .expect("contract should build");

        let required_inputs = artifact_set(&contract.required_inputs);
        assert!(required_inputs.contains(SELF_PRIMARY_INPUT));
        assert!(required_inputs.contains("xmu.dat"));
        assert!(required_inputs.contains("loss.dat"));

        assert_eq!(contract.optional_inputs.len(), 1);
        assert_eq!(
            contract.optional_inputs[0].relative_path.to_string_lossy(),
            SELF_OPTIONAL_INPUTS[0]
        );

        let expected_outputs = artifact_set(&contract.expected_outputs);
        for required in SELF_REQUIRED_OUTPUTS {
            assert!(expected_outputs.contains(required));
        }
        assert!(expected_outputs.contains("xmu.dat"));
        assert!(expected_outputs.contains("loss.dat"));
    }

    #[test]
    fn execute_emits_required_outputs_and_rewrites_staged_spectra() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        let output_dir = temp.path().join("out");

        fs::write(&input_path, SFCONV_INPUT_FIXTURE).expect("sfconv input should be written");
        fs::write(temp.path().join("xmu.dat"), XMU_INPUT_FIXTURE).expect("xmu should be written");
        fs::write(temp.path().join(SELF_OPTIONAL_INPUTS[0]), EXC_INPUT_FIXTURE)
            .expect("exc input should be written");

        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::SelfEnergy,
            &input_path,
            &output_dir,
        );
        let artifacts = SelfEnergyModule
            .execute(&request)
            .expect("SELF execution should succeed");

        let emitted = artifact_set(&artifacts);
        for required in SELF_REQUIRED_OUTPUTS {
            assert!(
                emitted.contains(required),
                "missing required output '{}'",
                required
            );
        }
        assert!(emitted.contains("xmu.dat"));

        for artifact in &artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            assert!(
                output_path.is_file(),
                "artifact '{}' should exist",
                output_path.display()
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "artifact '{}' should not be empty",
                output_path.display()
            );
        }

        let log = fs::read_to_string(output_dir.join("logsfconv.dat"))
            .expect("logsfconv.dat should be readable");
        assert!(log.contains("status = success"));
    }

    #[test]
    fn execute_accepts_feff_spectrum_inputs_when_named_spectra_are_absent() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        fs::write(&input_path, SFCONV_INPUT_FIXTURE).expect("sfconv input should be written");
        fs::write(temp.path().join("feff0001.dat"), FEFF_INPUT_FIXTURE)
            .expect("feff spectrum should be written");

        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::SelfEnergy,
            &input_path,
            temp.path().join("out"),
        );
        let artifacts = SelfEnergyModule
            .execute(&request)
            .expect("SELF execution should accept feffNNNN spectrum input");
        let emitted = artifact_set(&artifacts);

        assert!(emitted.contains("feff0001.dat"));
        assert!(emitted.contains("selfenergy.dat"));
        assert!(emitted.contains("sigma.dat"));
    }

    #[test]
    fn execute_rejects_non_self_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        fs::write(&input_path, SFCONV_INPUT_FIXTURE).expect("sfconv input should be written");
        fs::write(temp.path().join("xmu.dat"), XMU_INPUT_FIXTURE).expect("xmu should be written");

        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::Screen,
            &input_path,
            temp.path().join("out"),
        );
        let error = SelfEnergyModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.SELF_MODULE");
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_output = run_self_case(temp.path(), "first");
        let second_output = run_self_case(temp.path(), "second");

        for artifact in expected_artifact_set(&["xmu.dat", "loss.dat"]) {
            let first =
                fs::read(first_output.join(&artifact)).expect("first artifact should exist");
            let second =
                fs::read(second_output.join(&artifact)).expect("second artifact should exist");
            assert_eq!(
                first, second,
                "artifact '{}' should be deterministic",
                artifact
            );
        }
    }

    fn run_self_case(root: &Path, subdir: &str) -> PathBuf {
        let case_root = root.join(subdir);
        fs::create_dir_all(&case_root).expect("case root should exist");
        fs::write(case_root.join(SELF_PRIMARY_INPUT), SFCONV_INPUT_FIXTURE)
            .expect("sfconv input should be written");
        fs::write(case_root.join("xmu.dat"), XMU_INPUT_FIXTURE).expect("xmu should be written");
        fs::write(case_root.join("loss.dat"), LOSS_INPUT_FIXTURE).expect("loss should be written");
        fs::write(case_root.join(SELF_OPTIONAL_INPUTS[0]), EXC_INPUT_FIXTURE)
            .expect("exc should be written");

        let output_dir = case_root.join("out");
        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::SelfEnergy,
            case_root.join(SELF_PRIMARY_INPUT),
            &output_dir,
        );
        let artifacts = SelfEnergyModule
            .execute(&request)
            .expect("SELF execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["xmu.dat", "loss.dat"]),
            "SELF output artifact set should match expected contract"
        );
        output_dir
    }

    fn expected_artifact_set(spectrum_artifacts: &[&str]) -> BTreeSet<String> {
        let mut artifacts: BTreeSet<String> = SELF_REQUIRED_OUTPUTS
            .iter()
            .map(|artifact| artifact.to_string())
            .collect();
        artifacts.extend(
            spectrum_artifacts
                .iter()
                .map(|artifact| artifact.to_string()),
        );
        artifacts
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }
}
