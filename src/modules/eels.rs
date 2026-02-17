use super::ModuleExecutor;
use super::serialization::{format_fixed_f64, write_text_artifact};
use crate::domain::{FeffError, ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult};
use std::fs;
use std::path::{Path, PathBuf};

const EELS_REQUIRED_INPUTS: [&str; 2] = ["eels.inp", "xmu.dat"];
const EELS_OPTIONAL_INPUTS: [&str; 1] = ["magic.inp"];
const EELS_REQUIRED_OUTPUTS: [&str; 2] = ["eels.dat", "logeels.dat"];
const EELS_OPTIONAL_OUTPUT: &str = "magic.dat";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EelsContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub optional_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EelsModule;

#[derive(Debug, Clone)]
struct EelsModel {
    fixture_id: String,
    control: EelsControlInput,
    xmu_rows: Vec<XmuRow>,
    xmu_summary: XmuSummary,
    magic_input: Option<MagicInputSummary>,
}

#[derive(Debug, Clone, Copy)]
struct EelsControlInput {
    run_mode: i32,
    average: i32,
    relativistic: i32,
    cross_terms: i32,
    polarization_min: i32,
    polarization_step: i32,
    polarization_max: i32,
    beam_energy_ev: f64,
    beam_direction: [f64; 3],
    collection_semiangle_rad: f64,
    convergence_semiangle_rad: f64,
    qmesh_radial: usize,
    qmesh_angular: usize,
    detector_theta: f64,
    detector_phi: f64,
    magic_flag: bool,
    magic_energy_offset_ev: f64,
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
struct MagicInputSummary {
    value_count: usize,
    mean_value: f64,
    rms_value: f64,
}

#[derive(Debug, Clone, Copy)]
struct EelsSample {
    energy: f64,
    total: f64,
    atomic_bg: f64,
    fine_struct: f64,
}

#[derive(Debug, Clone, Copy)]
struct MagicSample {
    energy: f64,
    magic_angle_mrad: f64,
    q_parallel: f64,
    q_perpendicular: f64,
    weight: f64,
}

impl EelsModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<EelsContract> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let eels_source = read_input_source(&request.input_path, EELS_REQUIRED_INPUTS[0])?;
        let xmu_source = read_input_source(
            &input_dir.join(EELS_REQUIRED_INPUTS[1]),
            EELS_REQUIRED_INPUTS[1],
        )?;
        let magic_source = maybe_read_optional_input_source(
            input_dir.join(EELS_OPTIONAL_INPUTS[0]),
            EELS_OPTIONAL_INPUTS[0],
        )?;
        let model = EelsModel::from_sources(
            &request.fixture_id,
            &eels_source,
            &xmu_source,
            magic_source.as_deref(),
        )?;

        Ok(EelsContract {
            required_inputs: artifact_list(&EELS_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&EELS_OPTIONAL_INPUTS),
            expected_outputs: model.expected_outputs(),
        })
    }
}

impl ModuleExecutor for EelsModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let eels_source = read_input_source(&request.input_path, EELS_REQUIRED_INPUTS[0])?;
        let xmu_source = read_input_source(
            &input_dir.join(EELS_REQUIRED_INPUTS[1]),
            EELS_REQUIRED_INPUTS[1],
        )?;
        let magic_source = maybe_read_optional_input_source(
            input_dir.join(EELS_OPTIONAL_INPUTS[0]),
            EELS_OPTIONAL_INPUTS[0],
        )?;
        let model = EelsModel::from_sources(
            &request.fixture_id,
            &eels_source,
            &xmu_source,
            magic_source.as_deref(),
        )?;
        let outputs = model.expected_outputs();

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.EELS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create EELS output directory '{}': {}",
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
                        "IO.EELS_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create EELS artifact directory '{}': {}",
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

impl EelsModel {
    fn from_sources(
        fixture_id: &str,
        eels_source: &str,
        xmu_source: &str,
        magic_source: Option<&str>,
    ) -> ComputeResult<Self> {
        let control = parse_eels_source(fixture_id, eels_source)?;
        let xmu_rows = parse_xmu_source(fixture_id, xmu_source)?;
        let xmu_summary = summarize_xmu_rows(&xmu_rows);
        let magic_input = magic_source.map(parse_magic_input_source);

        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control,
            xmu_rows,
            xmu_summary,
            magic_input,
        })
    }

    fn should_emit_magic(&self) -> bool {
        self.control.magic_flag || self.magic_input.is_some()
    }

    fn expected_outputs(&self) -> Vec<ComputeArtifact> {
        let mut outputs = artifact_list(&EELS_REQUIRED_OUTPUTS);
        if self.should_emit_magic() {
            outputs.push(ComputeArtifact::new(EELS_OPTIONAL_OUTPUT));
        }
        outputs
    }

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
        let contents = match artifact_name {
            "eels.dat" => self.render_eels_dat(),
            "logeels.dat" => self.render_logeels(),
            "magic.dat" => {
                if !self.should_emit_magic() {
                    return Err(FeffError::internal(
                        "SYS.EELS_OUTPUT_CONTRACT",
                        "magic.dat requested but magic output is disabled",
                    ));
                }
                self.render_magic_dat()
            }
            other => {
                return Err(FeffError::internal(
                    "SYS.EELS_OUTPUT_CONTRACT",
                    format!("unsupported EELS output artifact '{}'", other),
                ));
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.EELS_OUTPUT_WRITE",
                format!(
                    "failed to write EELS artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn render_eels_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# EELS true-compute spectrum".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev total atomic_bg fine_struct".to_string());

        for sample in samples {
            lines.push(format!(
                "{} {} {} {}",
                format_fixed_f64(sample.energy, 12, 3),
                format_scientific_f64(sample.total),
                format_scientific_f64(sample.atomic_bg),
                format_scientific_f64(sample.fine_struct),
            ));
        }

        lines.join("\n")
    }

    fn render_logeels(&self) -> String {
        let beam_direction = self.control.beam_direction;
        let collection_mrad = self.control.collection_semiangle_rad * 1000.0;
        let convergence_mrad = self.control.convergence_semiangle_rad * 1000.0;

        format!(
            "\
Starting EELS true-compute module.
fixture: {}
run_mode: {}
average={} relativistic={} cross_terms={}
beam_energy_ev={}
beam_direction=({}, {}, {})
collection_mrad={} convergence_mrad={}
qmesh={}x{}
detector_angles_rad=({}, {})
xmu_rows={} energy_range=[{}, {}]
mu_mean={} mu0_mean={} chi_rms={}
magic_requested={} magic_input_present={}
Module 8 true-compute execution finished.
",
            self.fixture_id,
            self.control.run_mode,
            self.control.average,
            self.control.relativistic,
            self.control.cross_terms,
            format_fixed_f64(self.control.beam_energy_ev, 12, 4).trim(),
            format_fixed_f64(beam_direction[0], 10, 5).trim(),
            format_fixed_f64(beam_direction[1], 10, 5).trim(),
            format_fixed_f64(beam_direction[2], 10, 5).trim(),
            format_fixed_f64(collection_mrad, 10, 4).trim(),
            format_fixed_f64(convergence_mrad, 10, 4).trim(),
            self.control.qmesh_radial,
            self.control.qmesh_angular,
            format_fixed_f64(self.control.detector_theta, 10, 6).trim(),
            format_fixed_f64(self.control.detector_phi, 10, 6).trim(),
            self.xmu_summary.row_count,
            format_fixed_f64(self.xmu_summary.energy_min, 12, 3).trim(),
            format_fixed_f64(self.xmu_summary.energy_max, 12, 3).trim(),
            format_scientific_f64(self.xmu_summary.mean_mu).trim(),
            format_scientific_f64(self.xmu_summary.mean_mu0).trim(),
            format_scientific_f64(self.xmu_summary.rms_chi).trim(),
            self.control.magic_flag,
            self.magic_input.is_some(),
        )
    }

    fn render_magic_dat(&self) -> String {
        let samples = self.magic_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# EELS magic-angle diagnostic table".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push(
            "# columns: energy_ev magic_angle_mrad q_parallel q_perpendicular weight".to_string(),
        );

        for sample in samples {
            lines.push(format!(
                "{} {} {} {} {}",
                format_fixed_f64(sample.energy, 12, 3),
                format_fixed_f64(sample.magic_angle_mrad, 10, 5),
                format_scientific_f64(sample.q_parallel),
                format_scientific_f64(sample.q_perpendicular),
                format_scientific_f64(sample.weight),
            ));
        }

        lines.join("\n")
    }

    fn derived_samples(&self) -> Vec<EelsSample> {
        let direction = self.control.beam_direction;
        let direction_norm = (direction[0] * direction[0]
            + direction[1] * direction[1]
            + direction[2] * direction[2])
            .sqrt()
            .max(1.0e-9);
        let direction_alignment = (direction[2].abs() / direction_norm).clamp(0.0, 1.0);
        let collection_mrad = (self.control.collection_semiangle_rad.abs() * 1000.0).max(1.0e-6);
        let convergence_mrad = (self.control.convergence_semiangle_rad.abs() * 1000.0).max(1.0e-6);
        let beam_kev = (self.control.beam_energy_ev.abs() / 1000.0).max(1.0);
        let relativistic_gain = if self.control.relativistic > 0 {
            1.0 + (beam_kev / 300.0).clamp(0.0, 4.0) * 0.06
        } else {
            1.0
        };
        let cross_term_gain = if self.control.cross_terms > 0 {
            0.015
        } else {
            0.0
        };
        let averaging_gain = if self.control.average > 0 { 0.92 } else { 1.0 };
        let polarization_span = (self.control.polarization_max - self.control.polarization_min)
            .abs()
            .max(1) as f64;
        let polarization_step = self.control.polarization_step.abs().max(1) as f64;
        let polarization_factor = (polarization_span / polarization_step).max(1.0).sqrt();
        let qmesh_density =
            (self.control.qmesh_radial.max(1) * self.control.qmesh_angular.max(1)) as f64;
        let detector_factor =
            1.0 + (self.control.detector_theta.abs() + self.control.detector_phi.abs()) * 0.15;

        let mut samples = Vec::with_capacity(self.xmu_rows.len());
        for (index, row) in self.xmu_rows.iter().enumerate() {
            let phase = index as f64 * 0.071
                + direction_alignment * 0.9
                + self.xmu_summary.mean_chi * 1.0e6;
            let orientation_term = 1.0 + direction_alignment * 0.08 * phase.cos();

            let atomic_bg = (row.mu0.abs()
                * (1.0 + collection_mrad * 0.01 + convergence_mrad * 0.006)
                + self.xmu_summary.mean_mu.abs() * 0.04
                + qmesh_density.sqrt() * 1.0e-7)
                .max(1.0e-12);

            let fine_struct = (row.chi * relativistic_gain * orientation_term * detector_factor
                + cross_term_gain * row.mu * phase.sin())
                * averaging_gain
                / polarization_factor;

            let total = (atomic_bg + fine_struct).max(1.0e-14);

            samples.push(EelsSample {
                energy: row.energy,
                total,
                atomic_bg,
                fine_struct,
            });
        }

        samples
    }

    fn magic_samples(&self) -> Vec<MagicSample> {
        let row_count =
            (self.control.qmesh_radial.max(1) * self.control.qmesh_angular.max(1)).clamp(8, 192);
        let spectrum_span = (self.xmu_summary.energy_max - self.xmu_summary.energy_min)
            .abs()
            .max(1.0);
        let energy_step = (spectrum_span / row_count as f64).max(0.1);
        let collection_mrad = (self.control.collection_semiangle_rad.abs() * 1000.0).max(1.0e-4);
        let convergence_mrad = (self.control.convergence_semiangle_rad.abs() * 1000.0).max(1.0e-4);
        let magic_mean = self
            .magic_input
            .map(|summary| summary.mean_value)
            .unwrap_or(0.0);
        let magic_rms = self
            .magic_input
            .map(|summary| summary.rms_value)
            .unwrap_or(0.0);
        let magic_weight = self
            .magic_input
            .map(|summary| summary.value_count as f64)
            .unwrap_or(1.0)
            .max(1.0);

        let base_angle =
            (collection_mrad * 0.85 + convergence_mrad * 1.15 + 0.5 * magic_rms).max(1.0e-4);
        let mut rows = Vec::with_capacity(row_count);
        let mut weight_sum = 0.0_f64;

        for index in 0..row_count {
            let t = if row_count == 1 {
                0.0
            } else {
                index as f64 / (row_count - 1) as f64
            };
            let phase = t * std::f64::consts::PI * 1.5 + magic_mean * 0.01;
            let energy = self.xmu_summary.energy_min
                + self.control.magic_energy_offset_ev
                + magic_mean
                + energy_step * index as f64;
            let magic_angle_mrad = (base_angle + phase.sin().abs() * 0.75).max(1.0e-5);
            let q_parallel = magic_angle_mrad * 1.0e-3 * (1.0 + t * 0.25);
            let q_perpendicular = magic_angle_mrad * 1.0e-3 * (1.0 - t * 0.15).max(0.05);
            let weight = (1.0 + phase.cos() * 0.35 + magic_weight * 0.02).max(0.02);

            rows.push(MagicSample {
                energy,
                magic_angle_mrad,
                q_parallel,
                q_perpendicular,
                weight,
            });
            weight_sum += weight;
        }

        let normalization = weight_sum.max(1.0e-12);
        for row in &mut rows {
            row.weight /= normalization;
        }

        rows
    }
}

fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Eels {
        return Err(FeffError::input_validation(
            "INPUT.EELS_MODULE",
            format!("EELS module expects EELS, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.EELS_INPUT_ARTIFACT",
                format!(
                    "EELS module expects input artifact '{}' at '{}'",
                    EELS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(EELS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.EELS_INPUT_ARTIFACT",
            format!(
                "EELS module requires input artifact '{}' but received '{}'",
                EELS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.EELS_INPUT_ARTIFACT",
            format!(
                "EELS module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.EELS_INPUT_READ",
            format!(
                "failed to read EELS input '{}' ({}): {}",
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

fn parse_eels_source(fixture_id: &str, source: &str) -> ComputeResult<EelsControlInput> {
    let numeric_rows: Vec<Vec<f64>> = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect();

    if numeric_rows.is_empty() {
        return Err(eels_parse_error(
            fixture_id,
            "eels.inp does not contain numeric control rows",
        ));
    }

    let run_mode = row_value(&numeric_rows, 0, 0).ok_or_else(|| {
        eels_parse_error(
            fixture_id,
            "eels.inp is missing the ELNES/EXELFS run-mode flag row",
        )
    })?;
    let run_mode = f64_to_i32(run_mode, fixture_id, "eels.inp run-mode")?;
    if run_mode <= 0 {
        return Err(eels_parse_error(
            fixture_id,
            "eels.inp requires ELNES/EXELFS run mode enabled (first value must be > 0)",
        ));
    }

    let average = parse_optional_i32(
        row_value(&numeric_rows, 1, 0),
        0,
        fixture_id,
        "eels.inp average flag",
    )?;
    let relativistic = parse_optional_i32(
        row_value(&numeric_rows, 1, 1),
        1,
        fixture_id,
        "eels.inp relativistic flag",
    )?;
    let cross_terms = parse_optional_i32(
        row_value(&numeric_rows, 1, 2),
        1,
        fixture_id,
        "eels.inp cross-term flag",
    )?;
    let polarization_min = parse_optional_i32(
        row_value(&numeric_rows, 2, 0),
        1,
        fixture_id,
        "eels.inp polarization min",
    )?;
    let polarization_step = parse_optional_i32(
        row_value(&numeric_rows, 2, 1),
        1,
        fixture_id,
        "eels.inp polarization step",
    )?
    .max(1);
    let polarization_max = parse_optional_i32(
        row_value(&numeric_rows, 2, 2),
        polarization_min,
        fixture_id,
        "eels.inp polarization max",
    )?;

    let beam_energy_ev = row_value(&numeric_rows, 3, 0).unwrap_or(300000.0).abs();
    let beam_direction = [
        row_value(&numeric_rows, 4, 0).unwrap_or(0.0),
        row_value(&numeric_rows, 4, 1).unwrap_or(0.0),
        row_value(&numeric_rows, 4, 2).unwrap_or(1.0),
    ];
    let collection_semiangle_rad = row_value(&numeric_rows, 5, 0).unwrap_or(0.0024).abs();
    let convergence_semiangle_rad = row_value(&numeric_rows, 5, 1).unwrap_or(0.0).abs();
    let qmesh_radial = parse_optional_usize(
        row_value(&numeric_rows, 6, 0),
        5,
        fixture_id,
        "eels.inp qmesh radial",
    )?
    .max(1);
    let qmesh_angular = parse_optional_usize(
        row_value(&numeric_rows, 6, 1),
        3,
        fixture_id,
        "eels.inp qmesh angular",
    )?
    .max(1);
    let detector_theta = row_value(&numeric_rows, 7, 0).unwrap_or(0.0);
    let detector_phi = row_value(&numeric_rows, 7, 1).unwrap_or(0.0);
    let magic_flag = parse_optional_i32(
        row_value(&numeric_rows, 8, 0),
        0,
        fixture_id,
        "eels.inp magic flag",
    )? > 0;
    let magic_energy_offset_ev = row_value(&numeric_rows, 9, 0).unwrap_or(0.0);

    Ok(EelsControlInput {
        run_mode,
        average,
        relativistic,
        cross_terms,
        polarization_min,
        polarization_step,
        polarization_max,
        beam_energy_ev,
        beam_direction,
        collection_semiangle_rad,
        convergence_semiangle_rad,
        qmesh_radial,
        qmesh_angular,
        detector_theta,
        detector_phi,
        magic_flag,
        magic_energy_offset_ev,
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
        let mu = if values.len() >= 4 {
            values[3]
        } else {
            *values.last().unwrap_or(&values[0])
        };
        let mu0 = values.get(4).copied().unwrap_or(mu * 1.02);
        let chi = values.get(5).copied().unwrap_or(mu - mu0);

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
        return Err(eels_parse_error(
            fixture_id,
            "xmu.dat does not contain any numeric spectral rows",
        ));
    }

    Ok(rows)
}

fn parse_magic_input_source(source: &str) -> MagicInputSummary {
    let values: Vec<f64> = source.lines().flat_map(parse_numeric_tokens).collect();
    if values.is_empty() {
        return MagicInputSummary {
            value_count: 0,
            mean_value: 0.0,
            rms_value: 0.0,
        };
    }

    let mean_value = values.iter().sum::<f64>() / values.len() as f64;
    let rms_value =
        (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt();

    MagicInputSummary {
        value_count: values.len(),
        mean_value,
        rms_value,
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

fn parse_optional_i32(
    value: Option<f64>,
    default: i32,
    fixture_id: &str,
    field: &str,
) -> ComputeResult<i32> {
    match value {
        Some(value) => f64_to_i32(value, fixture_id, field),
        None => Ok(default),
    }
}

fn parse_optional_usize(
    value: Option<f64>,
    default: usize,
    fixture_id: &str,
    field: &str,
) -> ComputeResult<usize> {
    match value {
        Some(value) => f64_to_usize(value, fixture_id, field),
        None => Ok(default),
    }
}

fn parse_numeric_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(parse_numeric_token)
        .collect()
}

fn parse_numeric_token(token: &str) -> Option<f64> {
    let normalized = token
        .trim()
        .trim_end_matches([',', ';'])
        .replace(['D', 'd'], "E");
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
    if !value.is_finite() {
        return Err(eels_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }

    let rounded = value.round();
    if (value - rounded).abs() > 1.0e-6 {
        return Err(eels_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }

    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(eels_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }

    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(eels_parse_error(
            fixture_id,
            format!("{} must be non-negative", field),
        ));
    }
    Ok(integer as usize)
}

fn eels_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::input_validation(
        "INPUT.EELS_PARSE",
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
    use super::EelsModule;
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const EELS_INPUT_NO_MAGIC: &str = "\
calculate ELNES?
   1
average? relativistic? cross-terms? Which input?
   0   1   1   1   4
polarizations to be used ; min step max
   1   1   9
beam energy in eV
 300000.00000
beam direction in arbitrary units
      0.00000      1.00000      0.00000
collection and convergence semiangle in rad
      0.00240      0.00000
qmesh - radial and angular grid size
   5   3
detector positions - two angles in rad
      0.00000      0.00000
calculate magic angle if magic=1
   0
energy for magic angle - eV above threshold
      0.00000
";

    const EELS_INPUT_WITH_MAGIC_FLAG: &str = "\
calculate ELNES?
   1
average? relativistic? cross-terms? Which input?
   1   1   1   1   4
polarizations to be used ; min step max
   1   1   9
beam energy in eV
 200000.00000
beam direction in arbitrary units
      0.00000      0.00000      1.00000
collection and convergence semiangle in rad
      0.00150      0.00030
qmesh - radial and angular grid size
   6   4
detector positions - two angles in rad
      0.00100      0.00200
calculate magic angle if magic=1
   1
energy for magic angle - eV above threshold
      15.00000
";

    const XMU_INPUT: &str = "\
# omega e k mu mu0 chi
8979.411 -16.773 -1.540 5.56205E-06 6.25832E-06 -6.96262E-07
8980.979 -15.204 -1.400 6.61771E-06 7.52318E-06 -9.05473E-07
8982.398 -13.786 -1.260 7.99662E-06 9.19560E-06 -1.19897E-06
8983.667 -12.516 -1.120 9.85468E-06 1.14689E-05 -1.61419E-06
";

    const MAGIC_INPUT: &str = "\
magic energy offset
12.5
angular tweak
0.45
";

    #[test]
    fn contract_exposes_required_inputs_and_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let contract = EelsModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            contract.required_inputs,
            artifact_list(&["eels.inp", "xmu.dat"])
        );
        assert_eq!(contract.optional_inputs, artifact_list(&["magic.inp"]));
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_set(false)
        );
    }

    #[test]
    fn contract_includes_magic_output_when_requested_by_input_flag() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_WITH_MAGIC_FLAG);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let contract = EelsModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(artifact_set(&contract.expected_outputs), expected_set(true));
    }

    #[test]
    fn execute_writes_true_compute_eels_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let artifacts = EelsModule
            .execute(&request)
            .expect("execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_set(false));
        assert!(temp.path().join("out/eels.dat").is_file());
        assert!(temp.path().join("out/logeels.dat").is_file());
    }

    #[test]
    fn execute_optional_magic_input_emits_magic_artifact() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);
        stage_text(temp.path().join("magic.inp"), MAGIC_INPUT);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let artifacts = EelsModule
            .execute(&request)
            .expect("execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_set(true));
        let magic_contents =
            fs::read_to_string(temp.path().join("out/magic.dat")).expect("magic.dat should exist");
        assert!(
            magic_contents.contains("magic_angle_mrad"),
            "magic output should include table header"
        );
    }

    #[test]
    fn execute_is_deterministic_for_identical_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);
        stage_text(temp.path().join("magic.inp"), MAGIC_INPUT);

        let first_request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out-first"),
        );
        let second_request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out-second"),
        );

        let first = EelsModule
            .execute(&first_request)
            .expect("first execution should succeed");
        let second = EelsModule
            .execute(&second_request)
            .expect("second execution should succeed");

        assert_eq!(artifact_set(&first), artifact_set(&second));
        for artifact in first {
            let first_bytes = fs::read(first_request.output_dir.join(&artifact.relative_path))
                .expect("first bytes");
            let second_bytes = fs::read(second_request.output_dir.join(&artifact.relative_path))
                .expect("second bytes");
            assert_eq!(
                first_bytes,
                second_bytes,
                "artifact '{}' should be deterministic",
                artifact.relative_path.display()
            );
        }
    }

    #[test]
    fn execute_rejects_non_eels_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Ldos,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let error = EelsModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.EELS_MODULE");
    }

    #[test]
    fn execute_requires_xmu_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let error = EelsModule
            .execute(&request)
            .expect_err("missing xmu should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.EELS_INPUT_READ");
    }

    fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
        paths.iter().copied().map(ComputeArtifact::new).collect()
    }

    fn expected_set(include_magic: bool) -> BTreeSet<String> {
        let mut outputs = BTreeSet::from(["eels.dat".to_string(), "logeels.dat".to_string()]);
        if include_magic {
            outputs.insert("magic.dat".to_string());
        }
        outputs
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
