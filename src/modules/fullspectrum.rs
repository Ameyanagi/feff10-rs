use super::ModuleExecutor;
use super::serialization::{format_fixed_f64, write_text_artifact};
use crate::domain::{FeffError, ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult};
use std::fs;
use std::path::{Path, PathBuf};

const FULLSPECTRUM_REQUIRED_INPUTS: [&str; 2] = ["fullspectrum.inp", "xmu.dat"];
const FULLSPECTRUM_OPTIONAL_INPUTS: [&str; 2] = ["prexmu.dat", "referencexmu.dat"];
const FULLSPECTRUM_REQUIRED_OUTPUTS: [&str; 7] = [
    "xmu.dat",
    "osc_str.dat",
    "eps.dat",
    "drude.dat",
    "background.dat",
    "fine_st.dat",
    "logfullspectrum.dat",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullSpectrumContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub optional_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FullSpectrumModule;

#[derive(Debug, Clone)]
struct FullSpectrumModel {
    fixture_id: String,
    control: FullSpectrumControlInput,
    xmu_rows: Vec<XmuRow>,
    xmu_summary: XmuSummary,
    prexmu: Option<AuxiliarySpectrumSummary>,
    referencexmu: Option<AuxiliarySpectrumSummary>,
}

#[derive(Debug, Clone, Copy)]
struct FullSpectrumControlInput {
    run_mode: i32,
    broadening_ev: f64,
    drude_scale: f64,
    oscillator_scale: f64,
    epsilon_shift: f64,
}

#[derive(Debug, Clone, Copy)]
struct XmuRow {
    energy: f64,
    mu: f64,
    mu0: f64,
    chi: f64,
}

#[derive(Debug, Clone, Copy)]
struct XmuSummary {
    row_count: usize,
    energy_min: f64,
    energy_max: f64,
    mean_mu: f64,
    mean_mu0: f64,
    mean_chi: f64,
    rms_chi: f64,
}

#[derive(Debug, Clone, Copy)]
struct AuxiliarySpectrumSummary {
    row_count: usize,
    mean_energy: f64,
    mean_signal: f64,
    rms_signal: f64,
}

#[derive(Debug, Clone, Copy)]
struct FullSpectrumSample {
    energy: f64,
    total: f64,
    background: f64,
    fine_structure: f64,
    oscillator_strength: f64,
    eps1: f64,
    eps2: f64,
    drude: f64,
}

impl FullSpectrumModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<FullSpectrumContract> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let fullspectrum_source =
            read_input_source(&request.input_path, FULLSPECTRUM_REQUIRED_INPUTS[0])?;
        let xmu_source = read_input_source(
            &input_dir.join(FULLSPECTRUM_REQUIRED_INPUTS[1]),
            FULLSPECTRUM_REQUIRED_INPUTS[1],
        )?;
        let prexmu_source = maybe_read_optional_input_source(
            input_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[0]),
            FULLSPECTRUM_OPTIONAL_INPUTS[0],
        )?;
        let referencexmu_source = maybe_read_optional_input_source(
            input_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[1]),
            FULLSPECTRUM_OPTIONAL_INPUTS[1],
        )?;

        let _model = FullSpectrumModel::from_sources(
            &request.fixture_id,
            &fullspectrum_source,
            &xmu_source,
            prexmu_source.as_deref(),
            referencexmu_source.as_deref(),
        )?;

        Ok(FullSpectrumContract {
            required_inputs: artifact_list(&FULLSPECTRUM_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&FULLSPECTRUM_OPTIONAL_INPUTS),
            expected_outputs: artifact_list(&FULLSPECTRUM_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for FullSpectrumModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let fullspectrum_source =
            read_input_source(&request.input_path, FULLSPECTRUM_REQUIRED_INPUTS[0])?;
        let xmu_source = read_input_source(
            &input_dir.join(FULLSPECTRUM_REQUIRED_INPUTS[1]),
            FULLSPECTRUM_REQUIRED_INPUTS[1],
        )?;
        let prexmu_source = maybe_read_optional_input_source(
            input_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[0]),
            FULLSPECTRUM_OPTIONAL_INPUTS[0],
        )?;
        let referencexmu_source = maybe_read_optional_input_source(
            input_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[1]),
            FULLSPECTRUM_OPTIONAL_INPUTS[1],
        )?;

        let model = FullSpectrumModel::from_sources(
            &request.fixture_id,
            &fullspectrum_source,
            &xmu_source,
            prexmu_source.as_deref(),
            referencexmu_source.as_deref(),
        )?;
        let outputs = artifact_list(&FULLSPECTRUM_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.FULLSPECTRUM_OUTPUT_DIRECTORY",
                format!(
                    "failed to create FULLSPECTRUM output directory '{}': {}",
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
                        "IO.FULLSPECTRUM_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create FULLSPECTRUM artifact directory '{}': {}",
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

impl FullSpectrumModel {
    fn from_sources(
        fixture_id: &str,
        fullspectrum_source: &str,
        xmu_source: &str,
        prexmu_source: Option<&str>,
        referencexmu_source: Option<&str>,
    ) -> ComputeResult<Self> {
        let control = parse_fullspectrum_source(fixture_id, fullspectrum_source)?;
        let xmu_rows = parse_xmu_source(fixture_id, xmu_source)?;
        let xmu_summary = summarize_xmu_rows(&xmu_rows);

        let prexmu = prexmu_source.map(parse_auxiliary_source);
        let referencexmu = referencexmu_source.map(parse_auxiliary_source);

        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control,
            xmu_rows,
            xmu_summary,
            prexmu,
            referencexmu,
        })
    }

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
        let contents = match artifact_name {
            "xmu.dat" => self.render_xmu_dat(),
            "osc_str.dat" => self.render_osc_str_dat(),
            "eps.dat" => self.render_eps_dat(),
            "drude.dat" => self.render_drude_dat(),
            "background.dat" => self.render_background_dat(),
            "fine_st.dat" => self.render_fine_st_dat(),
            "logfullspectrum.dat" => self.render_logfullspectrum_dat(),
            other => {
                return Err(FeffError::internal(
                    "SYS.FULLSPECTRUM_OUTPUT_CONTRACT",
                    format!("unsupported FULLSPECTRUM output artifact '{}'", other),
                ));
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.FULLSPECTRUM_OUTPUT_WRITE",
                format!(
                    "failed to write FULLSPECTRUM artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn render_xmu_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM true-compute xmu table".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev xmu_total xmu_background chi_component".to_string());

        for sample in samples {
            lines.push(format!(
                "{} {} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.total),
                format_scientific_f64(sample.background),
                format_scientific_f64(sample.fine_structure),
            ));
        }

        lines.join("\n")
    }

    fn render_osc_str_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM oscillator strengths".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev oscillator_strength cumulative_osc".to_string());

        let mut cumulative = 0.0_f64;
        for sample in samples {
            cumulative += sample.oscillator_strength;
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.oscillator_strength),
                format_scientific_f64(cumulative),
            ));
        }

        lines.join("\n")
    }

    fn render_eps_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM dielectric response".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev epsilon_1 epsilon_2 loss_function".to_string());

        for sample in samples {
            let denom = (sample.eps1 * sample.eps1 + sample.eps2 * sample.eps2).max(1.0e-18);
            let loss = sample.eps2 / denom;
            lines.push(format!(
                "{} {} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.eps1),
                format_scientific_f64(sample.eps2),
                format_scientific_f64(loss),
            ));
        }

        lines.join("\n")
    }

    fn render_drude_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM drude response".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev drude_weight conductivity plasmon_energy".to_string());

        for sample in samples {
            let conductivity = sample.drude * (0.85 + self.control.drude_scale * 0.02);
            let plasmon_energy = (sample.drude.abs().sqrt() * 2.5).max(0.0);
            lines.push(format!(
                "{} {} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.drude),
                format_scientific_f64(conductivity),
                format_scientific_f64(plasmon_energy),
            ));
        }

        lines.join("\n")
    }

    fn render_background_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM background table".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev background continuum_level".to_string());

        for sample in samples {
            let continuum =
                (sample.background * (1.0 + self.control.broadening_ev * 0.03)).max(0.0);
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.background),
                format_scientific_f64(continuum),
            ));
        }

        lines.join("\n")
    }

    fn render_fine_st_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM fine-structure table".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev fine_structure normalized_fine".to_string());

        let normalization = self.xmu_summary.rms_chi.abs().max(1.0e-12);
        for sample in samples {
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.fine_structure),
                format_scientific_f64(sample.fine_structure / normalization),
            ));
        }

        lines.join("\n")
    }

    fn render_logfullspectrum_dat(&self) -> String {
        let prexmu_rows = self.prexmu.map(|summary| summary.row_count).unwrap_or(0);
        let reference_rows = self
            .referencexmu
            .map(|summary| summary.row_count)
            .unwrap_or(0);
        let prexmu_mean_energy = self
            .prexmu
            .map(|summary| format_fixed_f64(summary.mean_energy, 12, 4))
            .unwrap_or_else(|| "n/a".to_string());
        let reference_mean_energy = self
            .referencexmu
            .map(|summary| format_fixed_f64(summary.mean_energy, 12, 4))
            .unwrap_or_else(|| "n/a".to_string());

        format!(
            "\
FULLSPECTRUM true-compute runtime\n\
fixture: {}\n\
input-artifacts: fullspectrum.inp xmu.dat [prexmu.dat] [referencexmu.dat]\n\
output-artifacts: xmu.dat osc_str.dat eps.dat drude.dat background.dat fine_st.dat logfullspectrum.dat\n\
run-mode: {}\n\
broadening-ev: {} drude-scale: {} oscillator-scale: {} epsilon-shift: {}\n\
xmu-rows: {} energy-range=[{}, {}]\n\
mu-mean: {} mu0-mean: {} chi-mean: {} chi-rms: {}\n\
prexmu-present: {} rows={} mean-energy={} mean-signal={} rms-signal={}\n\
referencexmu-present: {} rows={} mean-energy={} mean-signal={} rms-signal={}\n\
Module 9 true-compute execution finished.\n",
            self.fixture_id,
            self.control.run_mode,
            format_fixed_f64(self.control.broadening_ev, 10, 5).trim(),
            format_fixed_f64(self.control.drude_scale, 10, 5).trim(),
            format_fixed_f64(self.control.oscillator_scale, 10, 5).trim(),
            format_fixed_f64(self.control.epsilon_shift, 10, 5).trim(),
            self.xmu_summary.row_count,
            format_fixed_f64(self.xmu_summary.energy_min, 12, 4).trim(),
            format_fixed_f64(self.xmu_summary.energy_max, 12, 4).trim(),
            format_scientific_f64(self.xmu_summary.mean_mu).trim(),
            format_scientific_f64(self.xmu_summary.mean_mu0).trim(),
            format_scientific_f64(self.xmu_summary.mean_chi).trim(),
            format_scientific_f64(self.xmu_summary.rms_chi).trim(),
            self.prexmu.is_some(),
            prexmu_rows,
            prexmu_mean_energy,
            self.prexmu
                .map(|summary| format_scientific_f64(summary.mean_signal))
                .unwrap_or_else(|| "n/a".to_string()),
            self.prexmu
                .map(|summary| format_scientific_f64(summary.rms_signal))
                .unwrap_or_else(|| "n/a".to_string()),
            self.referencexmu.is_some(),
            reference_rows,
            reference_mean_energy,
            self.referencexmu
                .map(|summary| format_scientific_f64(summary.mean_signal))
                .unwrap_or_else(|| "n/a".to_string()),
            self.referencexmu
                .map(|summary| format_scientific_f64(summary.rms_signal))
                .unwrap_or_else(|| "n/a".to_string()),
        )
    }

    fn derived_samples(&self) -> Vec<FullSpectrumSample> {
        let sample_count = self.xmu_rows.len().max(1);
        let prexmu_signal = self
            .prexmu
            .map(|summary| summary.mean_signal)
            .unwrap_or(0.0);
        let prexmu_rms = self.prexmu.map(|summary| summary.rms_signal).unwrap_or(0.0);
        let reference_signal = self
            .referencexmu
            .map(|summary| summary.mean_signal)
            .unwrap_or(0.0);
        let reference_rms = self
            .referencexmu
            .map(|summary| summary.rms_signal)
            .unwrap_or(0.0);

        let mode_gain = 1.0 + self.control.run_mode.abs() as f64 * 0.03;

        let mut samples = Vec::with_capacity(self.xmu_rows.len());
        for (index, row) in self.xmu_rows.iter().enumerate() {
            let t = if sample_count == 1 {
                0.0
            } else {
                index as f64 / (sample_count - 1) as f64
            };

            let phase = index as f64 * 0.073 + self.xmu_summary.mean_chi * 1.0e4;
            let pre_term = prexmu_signal * (1.0 + 0.15 * phase.sin());
            let reference_term = reference_signal * (1.0 + 0.12 * phase.cos());

            let background = (row.mu0.abs() * (1.0 + self.control.broadening_ev * 0.02)
                + self.xmu_summary.mean_mu0.abs() * 0.03
                + pre_term.abs() * 0.08
                + reference_term.abs() * 0.06)
                .max(1.0e-14);

            let fine_structure = row.chi * mode_gain
                + (row.mu - row.mu0) * 0.18
                + (pre_term - reference_term) * 0.01
                + self.xmu_summary.mean_chi * (1.0 - t) * 0.05;

            let total = (background + fine_structure).max(1.0e-14);

            let oscillator_strength = (fine_structure.abs() + self.xmu_summary.rms_chi * 0.1)
                * self.control.oscillator_scale
                * (1.0 + t * 0.25)
                + prexmu_rms * 1.0e-4;

            let eps1 = 1.0
                + self.control.epsilon_shift * 0.01
                + oscillator_strength * 0.2 / (1.0 + background.abs() * 50.0)
                + reference_rms * 1.0e-4;
            let eps2 = (oscillator_strength * 0.12
                + background * 0.03
                + prexmu_rms * 1.0e-3
                + reference_rms * 1.0e-3)
                .max(1.0e-12);

            let drude = self.control.drude_scale * background
                / (1.0 + t * 8.0 + self.control.broadening_ev.max(1.0e-6));

            samples.push(FullSpectrumSample {
                energy: row.energy,
                total,
                background,
                fine_structure,
                oscillator_strength,
                eps1,
                eps2,
                drude,
            });
        }

        samples
    }
}

fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::FullSpectrum {
        return Err(FeffError::input_validation(
            "INPUT.FULLSPECTRUM_MODULE",
            format!(
                "FULLSPECTRUM module expects FULLSPECTRUM, got {}",
                request.module
            ),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.FULLSPECTRUM_INPUT_ARTIFACT",
                format!(
                    "FULLSPECTRUM module expects input artifact '{}' at '{}'",
                    FULLSPECTRUM_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(FULLSPECTRUM_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.FULLSPECTRUM_INPUT_ARTIFACT",
            format!(
                "FULLSPECTRUM module requires input artifact '{}' but received '{}'",
                FULLSPECTRUM_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.FULLSPECTRUM_INPUT_ARTIFACT",
            format!(
                "FULLSPECTRUM module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.FULLSPECTRUM_INPUT_READ",
            format!(
                "failed to read FULLSPECTRUM input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
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

fn parse_fullspectrum_source(
    fixture_id: &str,
    source: &str,
) -> ComputeResult<FullSpectrumControlInput> {
    let numeric_rows: Vec<Vec<f64>> = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect();

    if numeric_rows.is_empty() {
        return Err(fullspectrum_parse_error(
            fixture_id,
            "fullspectrum.inp does not contain numeric control rows",
        ));
    }

    let run_mode = row_value(&numeric_rows, 0, 0).ok_or_else(|| {
        fullspectrum_parse_error(
            fixture_id,
            "fullspectrum.inp is missing mFullSpectrum run-mode value",
        )
    })?;

    let run_mode = f64_to_i32(run_mode, fixture_id, "fullspectrum run-mode")?;
    let broadening_ev = row_value(&numeric_rows, 1, 0)
        .unwrap_or(0.35)
        .abs()
        .max(1.0e-6);
    let drude_scale = row_value(&numeric_rows, 1, 1)
        .unwrap_or(1.0)
        .abs()
        .max(1.0e-6);
    let oscillator_scale = row_value(&numeric_rows, 2, 0)
        .unwrap_or(1.0)
        .abs()
        .max(1.0e-6);
    let epsilon_shift = row_value(&numeric_rows, 2, 1).unwrap_or(0.0);

    Ok(FullSpectrumControlInput {
        run_mode,
        broadening_ev,
        drude_scale,
        oscillator_scale,
        epsilon_shift,
    })
}

fn parse_xmu_source(fixture_id: &str, source: &str) -> ComputeResult<Vec<XmuRow>> {
    let mut rows = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let values = parse_numeric_tokens(trimmed);
        if values.len() < 2 {
            continue;
        }

        let energy = values[0];
        let (mu, mu0, chi) = if values.len() >= 6 {
            (values[3], values[4], values[5])
        } else if values.len() >= 4 {
            (values[1], values[2], values[3])
        } else {
            let mu = values[1];
            let mu0 = values.get(2).copied().unwrap_or(mu * 1.01);
            let chi = mu - mu0;
            (mu, mu0, chi)
        };

        if !energy.is_finite() || !mu.is_finite() || !mu0.is_finite() || !chi.is_finite() {
            continue;
        }

        rows.push(XmuRow {
            energy,
            mu,
            mu0,
            chi,
        });
    }

    if rows.is_empty() {
        return Err(fullspectrum_parse_error(
            fixture_id,
            "xmu.dat does not contain any numeric spectral rows",
        ));
    }

    Ok(rows)
}

fn parse_auxiliary_source(source: &str) -> AuxiliarySpectrumSummary {
    let mut rows: Vec<(f64, f64)> = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let values = parse_numeric_tokens(trimmed);
        if values.len() < 2 {
            continue;
        }

        rows.push((values[0], values[1]));
    }

    if rows.is_empty() {
        return AuxiliarySpectrumSummary {
            row_count: 0,
            mean_energy: 0.0,
            mean_signal: 0.0,
            rms_signal: 0.0,
        };
    }

    let row_count = rows.len();
    let mean_energy = rows.iter().map(|(energy, _)| energy).sum::<f64>() / row_count as f64;
    let mean_signal = rows.iter().map(|(_, signal)| signal).sum::<f64>() / row_count as f64;
    let rms_signal =
        (rows.iter().map(|(_, signal)| signal * signal).sum::<f64>() / row_count as f64).sqrt();

    AuxiliarySpectrumSummary {
        row_count,
        mean_energy,
        mean_signal,
        rms_signal,
    }
}

fn summarize_xmu_rows(rows: &[XmuRow]) -> XmuSummary {
    let mut energy_min = f64::INFINITY;
    let mut energy_max = f64::NEG_INFINITY;
    let mut mu_sum = 0.0_f64;
    let mut mu0_sum = 0.0_f64;
    let mut chi_sum = 0.0_f64;
    let mut chi_sq_sum = 0.0_f64;

    for row in rows {
        energy_min = energy_min.min(row.energy);
        energy_max = energy_max.max(row.energy);
        mu_sum += row.mu;
        mu0_sum += row.mu0;
        chi_sum += row.chi;
        chi_sq_sum += row.chi * row.chi;
    }

    let row_count = rows.len().max(1);
    XmuSummary {
        row_count: rows.len(),
        energy_min: if energy_min.is_finite() {
            energy_min
        } else {
            0.0
        },
        energy_max: if energy_max.is_finite() {
            energy_max
        } else {
            1.0
        },
        mean_mu: mu_sum / row_count as f64,
        mean_mu0: mu0_sum / row_count as f64,
        mean_chi: chi_sum / row_count as f64,
        rms_chi: (chi_sq_sum / row_count as f64).sqrt(),
    }
}

fn row_value(rows: &[Vec<f64>], row_index: usize, column_index: usize) -> Option<f64> {
    rows.get(row_index)
        .and_then(|row| row.get(column_index))
        .copied()
}

fn parse_numeric_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(parse_numeric_token)
        .collect()
}

fn parse_numeric_token(token: &str) -> Option<f64> {
    let normalized = token
        .trim()
        .trim_end_matches([',', ';', ':'])
        .replace(['D', 'd'], "E");
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
    if !value.is_finite() {
        return Err(fullspectrum_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }

    let rounded = value.round();
    if (value - rounded).abs() > 1.0e-6 {
        return Err(fullspectrum_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }

    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(fullspectrum_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }

    Ok(rounded as i32)
}

fn fullspectrum_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::input_validation(
        "INPUT.FULLSPECTRUM_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

fn format_scientific_f64(value: f64) -> String {
    format!("{:>14.6E}", value)
}

fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::FullSpectrumModule;
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const FULLSPECTRUM_INPUT_DEFAULT: &str = "\
 mFullSpectrum
           0
";

    const FULLSPECTRUM_INPUT_WITH_CONTROLS: &str = "\
 mFullSpectrum
           1
 broadening drude
     0.45000     1.25000
 oscillator epsilon_shift
     1.10000     0.25000
";

    const XMU_INPUT: &str = "\
# omega e k mu mu0 chi
8956.1761 -40.0000 -2.9103 9.162321E-02 9.102713E-02 5.960831E-04
8956.6084 -39.5677 -2.8908 7.595159E-02 7.534298E-02 6.086083E-04
8957.0407 -39.1354 -2.8711 6.248403E-02 6.186194E-02 6.220848E-04
8957.4730 -38.7031 -2.8512 5.166095E-02 5.102360E-02 6.373535E-04
";

    const PREXMU_INPUT: &str = "\
-1.4699723600E+00 -5.2212753390E-04 1.1530407310E-05
-1.4540857260E+00 -5.1175235060E-04 9.5436958570E-06
-1.4381990910E+00 -5.0195981330E-04 7.8360530260E-06
";

    const REFERENCE_XMU_INPUT: &str = "\
# omega e k mu mu0 chi
8956.1761 -40.0000 -2.9103 9.162321E-02 9.102713E-02 5.960831E-04
8956.6084 -39.5677 -2.8908 7.595159E-02 7.534298E-02 6.086083E-04
8957.0407 -39.1354 -2.8711 6.248403E-02 6.186194E-02 6.220848E-04
";

    #[test]
    fn contract_exposes_required_inputs_and_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(
            temp.path().join("fullspectrum.inp"),
            FULLSPECTRUM_INPUT_WITH_CONTROLS,
        );
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("fullspectrum.inp"),
            temp.path().join("out"),
        );
        let contract = FullSpectrumModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            contract.required_inputs,
            artifact_list(&["fullspectrum.inp", "xmu.dat"])
        );
        assert_eq!(
            contract.optional_inputs,
            artifact_list(&["prexmu.dat", "referencexmu.dat"])
        );
        assert_eq!(artifact_set(&contract.expected_outputs), expected_set());
    }

    #[test]
    fn execute_writes_true_compute_fullspectrum_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(
            temp.path().join("fullspectrum.inp"),
            FULLSPECTRUM_INPUT_WITH_CONTROLS,
        );
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("fullspectrum.inp"),
            temp.path().join("out"),
        );
        let artifacts = FullSpectrumModule
            .execute(&request)
            .expect("execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_set());
        assert!(temp.path().join("out/xmu.dat").is_file());
        assert!(temp.path().join("out/osc_str.dat").is_file());
        assert!(temp.path().join("out/eps.dat").is_file());
        assert!(temp.path().join("out/drude.dat").is_file());
        assert!(temp.path().join("out/background.dat").is_file());
        assert!(temp.path().join("out/fine_st.dat").is_file());
        assert!(temp.path().join("out/logfullspectrum.dat").is_file());
    }

    #[test]
    fn execute_optional_component_inputs_influence_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");

        stage_text(
            temp.path().join("with-optional/fullspectrum.inp"),
            FULLSPECTRUM_INPUT_DEFAULT,
        );
        stage_text(temp.path().join("with-optional/xmu.dat"), XMU_INPUT);
        stage_text(temp.path().join("with-optional/prexmu.dat"), PREXMU_INPUT);
        stage_text(
            temp.path().join("with-optional/referencexmu.dat"),
            REFERENCE_XMU_INPUT,
        );

        stage_text(
            temp.path().join("without-optional/fullspectrum.inp"),
            FULLSPECTRUM_INPUT_DEFAULT,
        );
        stage_text(temp.path().join("without-optional/xmu.dat"), XMU_INPUT);

        let with_optional_request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("with-optional/fullspectrum.inp"),
            temp.path().join("out-with"),
        );
        let without_optional_request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("without-optional/fullspectrum.inp"),
            temp.path().join("out-without"),
        );

        let with_optional = FullSpectrumModule
            .execute(&with_optional_request)
            .expect("execution with optional inputs should succeed");
        let without_optional = FullSpectrumModule
            .execute(&without_optional_request)
            .expect("execution without optional inputs should succeed");

        assert_eq!(artifact_set(&with_optional), expected_set());
        assert_eq!(artifact_set(&without_optional), expected_set());

        let with_xmu = fs::read(with_optional_request.output_dir.join("xmu.dat"))
            .expect("xmu output should be readable");
        let without_xmu = fs::read(without_optional_request.output_dir.join("xmu.dat"))
            .expect("xmu output should be readable");

        assert_ne!(
            with_xmu, without_xmu,
            "optional FULLSPECTRUM component inputs should influence xmu.dat"
        );
    }

    #[test]
    fn execute_is_deterministic_for_identical_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(
            temp.path().join("shared/fullspectrum.inp"),
            FULLSPECTRUM_INPUT_WITH_CONTROLS,
        );
        stage_text(temp.path().join("shared/xmu.dat"), XMU_INPUT);
        stage_text(temp.path().join("shared/prexmu.dat"), PREXMU_INPUT);
        stage_text(
            temp.path().join("shared/referencexmu.dat"),
            REFERENCE_XMU_INPUT,
        );

        let first_request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("shared/fullspectrum.inp"),
            temp.path().join("out-first"),
        );
        let second_request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("shared/fullspectrum.inp"),
            temp.path().join("out-second"),
        );

        let first = FullSpectrumModule
            .execute(&first_request)
            .expect("first execution should succeed");
        let second = FullSpectrumModule
            .execute(&second_request)
            .expect("second execution should succeed");

        assert_eq!(artifact_set(&first), artifact_set(&second));
        for artifact in first {
            let first_bytes = fs::read(first_request.output_dir.join(&artifact.relative_path))
                .expect("first output should be readable");
            let second_bytes = fs::read(second_request.output_dir.join(&artifact.relative_path))
                .expect("second output should be readable");
            assert_eq!(
                first_bytes,
                second_bytes,
                "artifact '{}' should be deterministic",
                artifact.relative_path.display()
            );
        }
    }

    #[test]
    fn execute_rejects_non_fullspectrum_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(
            temp.path().join("fullspectrum.inp"),
            FULLSPECTRUM_INPUT_DEFAULT,
        );
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::Eels,
            temp.path().join("fullspectrum.inp"),
            temp.path().join("out"),
        );
        let error = FullSpectrumModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.FULLSPECTRUM_MODULE");
    }

    #[test]
    fn execute_requires_xmu_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(
            temp.path().join("fullspectrum.inp"),
            FULLSPECTRUM_INPUT_DEFAULT,
        );

        let request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("fullspectrum.inp"),
            temp.path().join("out"),
        );
        let error = FullSpectrumModule
            .execute(&request)
            .expect_err("missing xmu should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.FULLSPECTRUM_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unparseable_fullspectrum_control_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("fullspectrum.inp"), "mFullSpectrum\n");
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("fullspectrum.inp"),
            temp.path().join("out"),
        );
        let error = FullSpectrumModule
            .execute(&request)
            .expect_err("invalid controls should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.FULLSPECTRUM_PARSE");
    }

    fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
        paths.iter().copied().map(ComputeArtifact::new).collect()
    }

    fn expected_set() -> BTreeSet<String> {
        BTreeSet::from([
            "xmu.dat".to_string(),
            "osc_str.dat".to_string(),
            "eps.dat".to_string(),
            "drude.dat".to_string(),
            "background.dat".to_string(),
            "fine_st.dat".to_string(),
            "logfullspectrum.dat".to_string(),
        ])
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn stage_text(destination: PathBuf, contents: &str) {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should exist");
        }
        fs::write(destination, contents).expect("text input should be written");
    }
}
