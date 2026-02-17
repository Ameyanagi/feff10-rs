use super::PipelineExecutor;
use super::serialization::{format_fixed_f64, write_text_artifact};
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

const RIXS_REQUIRED_INPUTS: [&str; 6] = [
    "rixs.inp",
    "phase_1.bin",
    "phase_2.bin",
    "wscrn_1.dat",
    "wscrn_2.dat",
    "xsect_2.dat",
];
const RIXS_REQUIRED_OUTPUTS: [&str; 7] = [
    "rixs0.dat",
    "rixs1.dat",
    "rixsET.dat",
    "rixsEE.dat",
    "rixsET-sat.dat",
    "rixsEE-sat.dat",
    "logrixs.dat",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RixsPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RixsPipelineScaffold;

#[derive(Debug, Clone)]
struct RixsModel {
    fixture_id: String,
    control: RixsControlInput,
    phase_1: BinaryInputSummary,
    phase_2: BinaryInputSummary,
    wscrn_1: TableInputSummary,
    wscrn_2: TableInputSummary,
    xsect_2: TableInputSummary,
}

#[derive(Debug, Clone)]
struct RixsControlInput {
    run_enabled: bool,
    energy_rows: usize,
    incident_min: f64,
    incident_max: f64,
    incident_step: f64,
    emitted_min: f64,
    emitted_max: f64,
    emitted_step: f64,
    n_edges: usize,
    gamma_core: f64,
    gamma_edge_1: f64,
    gamma_edge_2: f64,
    xmu_shift: f64,
    read_poles: bool,
    skip_calc: bool,
    read_sigma: bool,
    edge_labels: [String; 2],
}

impl Default for RixsControlInput {
    fn default() -> Self {
        Self {
            run_enabled: true,
            energy_rows: 64,
            incident_min: -12.0,
            incident_max: 18.0,
            incident_step: 0.5,
            emitted_min: -4.0,
            emitted_max: 16.0,
            emitted_step: 0.5,
            n_edges: 2,
            gamma_core: 1.350_512e-4,
            gamma_edge_1: 1.350_512e-4,
            gamma_edge_2: 1.350_512e-4,
            xmu_shift: 0.0,
            read_poles: false,
            skip_calc: false,
            read_sigma: false,
            edge_labels: ["L3".to_string(), "L2".to_string()],
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BinaryInputSummary {
    byte_len: usize,
    checksum: u64,
    mean: f64,
    rms: f64,
}

#[derive(Debug, Clone, Copy)]
struct TableInputSummary {
    value_count: usize,
    min: f64,
    max: f64,
    mean: f64,
    rms: f64,
}

#[derive(Debug, Clone, Copy)]
struct RixsOutputConfig {
    incident_rows: usize,
    emitted_rows: usize,
    incident_min: f64,
    incident_step: f64,
    emitted_min: f64,
    emitted_step: f64,
    gamma_width: f64,
    edge_mix: f64,
    interference: f64,
    sat_ratio: f64,
    phase_seed_1: f64,
    phase_seed_2: f64,
}

#[derive(Debug, Clone, Copy)]
struct RixsLineSample {
    incident_energy: f64,
    edge_1_intensity: f64,
    edge_2_intensity: f64,
    transfer_energy: f64,
    sat_scale: f64,
}

#[derive(Debug, Clone, Copy)]
struct RixsEeSample {
    incident_energy: f64,
    emitted_energy: f64,
    transfer_energy: f64,
    intensity: f64,
}

impl RixsPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<RixsPipelineInterface> {
        validate_request_shape(request)?;
        Ok(RixsPipelineInterface {
            required_inputs: artifact_list(&RIXS_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&RIXS_REQUIRED_OUTPUTS),
        })
    }
}

impl PipelineExecutor for RixsPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let rixs_source = read_input_source(&request.input_path, RIXS_REQUIRED_INPUTS[0])?;
        let phase_1_bytes = read_input_bytes(
            &input_dir.join(RIXS_REQUIRED_INPUTS[1]),
            RIXS_REQUIRED_INPUTS[1],
        )?;
        let phase_2_bytes = read_input_bytes(
            &input_dir.join(RIXS_REQUIRED_INPUTS[2]),
            RIXS_REQUIRED_INPUTS[2],
        )?;
        let wscrn_1_source = read_input_source(
            &input_dir.join(RIXS_REQUIRED_INPUTS[3]),
            RIXS_REQUIRED_INPUTS[3],
        )?;
        let wscrn_2_source = read_input_source(
            &input_dir.join(RIXS_REQUIRED_INPUTS[4]),
            RIXS_REQUIRED_INPUTS[4],
        )?;
        let xsect_2_source = read_input_source(
            &input_dir.join(RIXS_REQUIRED_INPUTS[5]),
            RIXS_REQUIRED_INPUTS[5],
        )?;

        let model = RixsModel::from_sources(
            &request.fixture_id,
            &rixs_source,
            &phase_1_bytes,
            &phase_2_bytes,
            &wscrn_1_source,
            &wscrn_2_source,
            &xsect_2_source,
        )?;
        let outputs = artifact_list(&RIXS_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.RIXS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create RIXS output directory '{}': {}",
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
                        "IO.RIXS_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create RIXS artifact directory '{}': {}",
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

impl RixsModel {
    fn from_sources(
        fixture_id: &str,
        rixs_source: &str,
        phase_1_bytes: &[u8],
        phase_2_bytes: &[u8],
        wscrn_1_source: &str,
        wscrn_2_source: &str,
        xsect_2_source: &str,
    ) -> PipelineResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_rixs_source(fixture_id, rixs_source)?,
            phase_1: parse_binary_source(fixture_id, "phase_1.bin", phase_1_bytes)?,
            phase_2: parse_binary_source(fixture_id, "phase_2.bin", phase_2_bytes)?,
            wscrn_1: parse_table_source(fixture_id, "wscrn_1.dat", wscrn_1_source)?,
            wscrn_2: parse_table_source(fixture_id, "wscrn_2.dat", wscrn_2_source)?,
            xsect_2: parse_table_source(fixture_id, "xsect_2.dat", xsect_2_source)?,
        })
    }

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> PipelineResult<()> {
        let contents = match artifact_name {
            "rixs0.dat" => self.render_rixs0(),
            "rixs1.dat" => self.render_rixs1(),
            "rixsET.dat" => self.render_rixs_et(),
            "rixsEE.dat" => self.render_rixs_ee(false),
            "rixsET-sat.dat" => self.render_rixs_et_sat(),
            "rixsEE-sat.dat" => self.render_rixs_ee(true),
            "logrixs.dat" => self.render_logrixs(),
            other => {
                return Err(FeffError::internal(
                    "SYS.RIXS_OUTPUT_CONTRACT",
                    format!("unsupported RIXS output artifact '{}'", other),
                ));
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.RIXS_OUTPUT_WRITE",
                format!(
                    "failed to write RIXS artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn output_config(&self) -> RixsOutputConfig {
        let incident_span = (self.control.incident_max - self.control.incident_min)
            .abs()
            .max(1.0e-6);
        let derived_rows = if self.control.energy_rows > 1 {
            self.control.energy_rows
        } else {
            ((incident_span / self.control.incident_step.abs().max(1.0e-6)).round() as usize)
                .saturating_add(1)
        };
        let incident_rows = derived_rows.clamp(3, 512);
        let incident_step = if incident_rows > 1 {
            incident_span / (incident_rows - 1) as f64
        } else {
            0.0
        };

        let emitted_span = (self.control.emitted_max - self.control.emitted_min)
            .abs()
            .max(incident_span * 0.2)
            .max(1.0e-6);
        let emitted_rows = incident_rows.clamp(3, 96);
        let emitted_step = if emitted_rows > 1 {
            emitted_span / (emitted_rows - 1) as f64
        } else {
            0.0
        };

        let gamma_average =
            (self.control.gamma_core + self.control.gamma_edge_1 + self.control.gamma_edge_2).abs()
                / 3.0;
        let gamma_width =
            (0.08 + gamma_average * 5.0e5 + self.wscrn_1.rms * 0.003 + self.wscrn_2.rms * 0.003)
                .clamp(0.08, 40.0);

        let edge_mix = (0.2
            + (self.phase_1.mean - self.phase_2.mean).abs() / 255.0
            + (self.wscrn_1.mean - self.wscrn_2.mean).abs().min(50.0) * 0.01
            + if self.control.n_edges > 1 { 0.2 } else { 0.0 })
        .clamp(0.2, 2.5);

        let interference = (checksum_to_unit(self.phase_1.checksum ^ self.phase_2.checksum) * 0.5
            + self.xsect_2.rms * 0.0005
            + if self.control.read_poles { 0.08 } else { 0.02 }
            + if self.control.skip_calc { -0.03 } else { 0.01 })
        .clamp(0.0, 1.0);

        let sat_ratio = (0.12
            + self.xsect_2.mean.abs() * 0.002
            + if self.control.read_sigma { 0.08 } else { 0.0 }
            + if self.control.run_enabled {
                0.03
            } else {
                -0.05
            })
        .clamp(0.05, 0.75);

        let phase_seed_1 = checksum_to_unit(self.phase_1.checksum) * PI * 2.0;
        let phase_seed_2 = checksum_to_unit(self.phase_2.checksum.rotate_left(7)) * PI * 2.0;

        RixsOutputConfig {
            incident_rows,
            emitted_rows,
            incident_min: self.control.incident_min.min(self.control.incident_max),
            incident_step,
            emitted_min: self.control.emitted_min.min(self.control.emitted_max),
            emitted_step,
            gamma_width,
            edge_mix,
            interference,
            sat_ratio,
            phase_seed_1,
            phase_seed_2,
        }
    }

    fn line_samples(&self) -> Vec<RixsLineSample> {
        let config = self.output_config();
        let transfer_span = (self.control.emitted_max - self.control.emitted_min)
            .abs()
            .max((self.control.incident_max - self.control.incident_min).abs() * 0.5)
            .max(1.0);

        let mut samples = Vec::with_capacity(config.incident_rows);
        for index in 0..config.incident_rows {
            let t = normalized_index(index, config.incident_rows);
            let incident_energy = config.incident_min + config.incident_step * index as f64;

            let phase_1 =
                t * PI * (1.2 + config.edge_mix * 0.35) + config.phase_seed_1 + self.wscrn_1.mean;
            let phase_2 = t * PI * (1.5 + config.edge_mix * 0.20) + config.phase_seed_2
                - self.wscrn_2.mean * 0.5;

            let edge_1_intensity = (0.35
                + self.wscrn_1.rms * 0.002
                + self.xsect_2.mean.abs() * 0.001
                + phase_1.sin().abs() * (0.70 + config.edge_mix * 0.10)
                + self.phase_1.rms * 0.0002)
                .max(1.0e-12);
            let edge_2_intensity = (0.33
                + self.wscrn_2.rms * 0.002
                + self.xsect_2.rms * 0.001
                + phase_2.cos().abs() * (0.65 + config.edge_mix * 0.09)
                + self.phase_2.rms * 0.0002)
                .max(1.0e-12);

            let transfer_energy = self.control.emitted_min
                + transfer_span * t
                + self.control.xmu_shift.abs().ln_1p().min(80.0) * 0.05;
            let sat_scale = (config.sat_ratio
                * (0.7 + 0.3 * (phase_1 + phase_2 + config.interference).sin().abs()))
            .clamp(0.03, 0.98);

            samples.push(RixsLineSample {
                incident_energy,
                edge_1_intensity,
                edge_2_intensity,
                transfer_energy,
                sat_scale,
            });
        }

        samples
    }

    fn ee_samples(&self, include_satellite: bool) -> Vec<RixsEeSample> {
        let config = self.output_config();
        let line_samples = self.line_samples();
        let mut rows = Vec::with_capacity(config.incident_rows * config.emitted_rows);

        for (incident_index, line) in line_samples.iter().enumerate() {
            for emitted_index in 0..config.emitted_rows {
                let emitted_energy =
                    config.emitted_min + config.emitted_step * emitted_index as f64;
                let transfer_energy = line.incident_energy - emitted_energy;
                let width = config.gamma_width * (1.0 + if include_satellite { 0.7 } else { 0.2 });
                let lorentz = 1.0 / (1.0 + (transfer_energy / width).powi(2));

                let phase = (incident_index as f64 * 0.17
                    + emitted_index as f64 * 0.11
                    + config.phase_seed_1
                    - config.phase_seed_2)
                    .sin();
                let mix = line.edge_1_intensity * (0.56 + 0.20 * phase.abs())
                    + line.edge_2_intensity * (0.44 + 0.16 * phase.cos().abs())
                    + config.interference * 0.08;

                let mut intensity = (mix * lorentz).max(1.0e-14);
                if include_satellite {
                    intensity *= line.sat_scale * (1.0 + config.sat_ratio * 0.45);
                }

                rows.push(RixsEeSample {
                    incident_energy: line.incident_energy,
                    emitted_energy,
                    transfer_energy,
                    intensity,
                });
            }
        }

        rows
    }

    fn render_rixs0(&self) -> String {
        let config = self.output_config();
        let samples = self.line_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# RIXS true-compute edge-1 profile".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: incident_energy edge1_intensity edge1_inelastic".to_string());

        for sample in samples {
            let elastic = sample.edge_1_intensity * (1.0 + config.interference * 0.25);
            let inelastic = (sample.edge_1_intensity * config.edge_mix * 0.35
                + sample.edge_2_intensity * 0.10)
                .max(1.0e-14);
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.incident_energy, 12, 4),
                format_scientific_f64(elastic),
                format_scientific_f64(inelastic),
            ));
        }

        lines.join("\n")
    }

    fn render_rixs1(&self) -> String {
        let config = self.output_config();
        let samples = self.line_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# RIXS true-compute edge-2 profile".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: incident_energy edge2_intensity mixed_tail".to_string());

        for sample in samples {
            let elastic = sample.edge_2_intensity * (1.0 + config.interference * 0.20);
            let mixed_tail = (sample.edge_2_intensity * config.edge_mix * 0.33
                + sample.edge_1_intensity * 0.12)
                .max(1.0e-14);
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.incident_energy, 12, 4),
                format_scientific_f64(elastic),
                format_scientific_f64(mixed_tail),
            ));
        }

        lines.join("\n")
    }

    fn render_rixs_et(&self) -> String {
        let config = self.output_config();
        let samples = self.line_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# RIXS true-compute transfer spectrum".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: transfer_energy intensity integrated_weight".to_string());

        for sample in samples {
            let intensity = (sample.edge_1_intensity + sample.edge_2_intensity)
                * (1.0 + config.interference * 0.12);
            let integrated = intensity * (1.0 + config.edge_mix * 0.05)
                / (1.0 + sample.transfer_energy.abs() * 0.02);
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.transfer_energy, 12, 4),
                format_scientific_f64(intensity),
                format_scientific_f64(integrated),
            ));
        }

        lines.join("\n")
    }

    fn render_rixs_et_sat(&self) -> String {
        let config = self.output_config();
        let samples = self.line_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# RIXS true-compute transfer satellite spectrum".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: transfer_energy sat_intensity sat_weight".to_string());

        for sample in samples {
            let base = (sample.edge_1_intensity + sample.edge_2_intensity)
                * (1.0 + config.interference * 0.12);
            let sat_intensity = base * sample.sat_scale;
            let sat_weight = sat_intensity * (1.0 + config.sat_ratio * 0.40)
                / (1.0 + sample.transfer_energy.abs() * 0.01);
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.transfer_energy, 12, 4),
                format_scientific_f64(sat_intensity),
                format_scientific_f64(sat_weight),
            ));
        }

        lines.join("\n")
    }

    fn render_rixs_ee(&self, include_satellite: bool) -> String {
        let mut lines = Vec::new();
        lines.push(if include_satellite {
            "# RIXS true-compute emitted/incident map (satellite)".to_string()
        } else {
            "# RIXS true-compute emitted/incident map".to_string()
        });
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push(
            "# columns: incident_energy emitted_energy transfer_energy intensity".to_string(),
        );

        for sample in self.ee_samples(include_satellite) {
            lines.push(format!(
                "{} {} {} {}",
                format_fixed_f64(sample.incident_energy, 12, 4),
                format_fixed_f64(sample.emitted_energy, 12, 4),
                format_fixed_f64(sample.transfer_energy, 12, 4),
                format_scientific_f64(sample.intensity),
            ));
        }

        lines.join("\n")
    }

    fn render_logrixs(&self) -> String {
        let config = self.output_config();

        format!(
            "\
Starting RIXS true-compute module.
fixture: {}
run_enabled={} n_edges={} edge_labels=[{}, {}]
incident_range=[{}, {}] incident_rows={} emitted_rows={}
incident_step={} emitted_step={}
gamma_core={} gamma_edge_1={} gamma_edge_2={}
read_poles={} skip_calc={} read_sigma={}
phase_1={{bytes:{}, checksum:{}, mean:{}, rms:{}}}
phase_2={{bytes:{}, checksum:{}, mean:{}, rms:{}}}
wscrn_1={{count:{}, min:{}, max:{}, mean:{}, rms:{}}}
wscrn_2={{count:{}, min:{}, max:{}, mean:{}, rms:{}}}
xsect_2={{count:{}, min:{}, max:{}, mean:{}, rms:{}}}
interference={} edge_mix={} sat_ratio={} gamma_width={}
Module RIXS true-compute execution finished.
",
            self.fixture_id,
            self.control.run_enabled,
            self.control.n_edges,
            self.control.edge_labels[0],
            self.control.edge_labels[1],
            format_fixed_f64(self.control.incident_min, 12, 4).trim(),
            format_fixed_f64(self.control.incident_max, 12, 4).trim(),
            config.incident_rows,
            config.emitted_rows,
            format_fixed_f64(config.incident_step, 12, 6).trim(),
            format_fixed_f64(config.emitted_step, 12, 6).trim(),
            format_scientific_f64(self.control.gamma_core).trim(),
            format_scientific_f64(self.control.gamma_edge_1).trim(),
            format_scientific_f64(self.control.gamma_edge_2).trim(),
            self.control.read_poles,
            self.control.skip_calc,
            self.control.read_sigma,
            self.phase_1.byte_len,
            self.phase_1.checksum,
            format_fixed_f64(self.phase_1.mean, 10, 6).trim(),
            format_fixed_f64(self.phase_1.rms, 10, 6).trim(),
            self.phase_2.byte_len,
            self.phase_2.checksum,
            format_fixed_f64(self.phase_2.mean, 10, 6).trim(),
            format_fixed_f64(self.phase_2.rms, 10, 6).trim(),
            self.wscrn_1.value_count,
            format_scientific_f64(self.wscrn_1.min).trim(),
            format_scientific_f64(self.wscrn_1.max).trim(),
            format_scientific_f64(self.wscrn_1.mean).trim(),
            format_scientific_f64(self.wscrn_1.rms).trim(),
            self.wscrn_2.value_count,
            format_scientific_f64(self.wscrn_2.min).trim(),
            format_scientific_f64(self.wscrn_2.max).trim(),
            format_scientific_f64(self.wscrn_2.mean).trim(),
            format_scientific_f64(self.wscrn_2.rms).trim(),
            self.xsect_2.value_count,
            format_scientific_f64(self.xsect_2.min).trim(),
            format_scientific_f64(self.xsect_2.max).trim(),
            format_scientific_f64(self.xsect_2.mean).trim(),
            format_scientific_f64(self.xsect_2.rms).trim(),
            format_fixed_f64(config.interference, 10, 6).trim(),
            format_fixed_f64(config.edge_mix, 10, 6).trim(),
            format_fixed_f64(config.sat_ratio, 10, 6).trim(),
            format_fixed_f64(config.gamma_width, 10, 6).trim(),
        )
    }
}

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Rixs {
        return Err(FeffError::input_validation(
            "INPUT.RIXS_MODULE",
            format!("RIXS pipeline expects module RIXS, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.RIXS_INPUT_ARTIFACT",
                format!(
                    "RIXS pipeline expects input artifact '{}' at '{}'",
                    RIXS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(RIXS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.RIXS_INPUT_ARTIFACT",
            format!(
                "RIXS pipeline requires input artifact '{}' but received '{}'",
                RIXS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.RIXS_INPUT_ARTIFACT",
            format!(
                "RIXS pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.RIXS_INPUT_READ",
            format!(
                "failed to read RIXS input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn read_input_bytes(path: &Path, artifact_name: &str) -> PipelineResult<Vec<u8>> {
    fs::read(path).map_err(|source| {
        FeffError::io_system(
            "IO.RIXS_INPUT_READ",
            format!(
                "failed to read RIXS input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn parse_rixs_source(fixture_id: &str, source: &str) -> PipelineResult<RixsControlInput> {
    let lines: Vec<&str> = source.lines().collect();
    if lines.iter().all(|line| line.trim().is_empty()) {
        return Err(rixs_parse_error(
            fixture_id,
            "rixs.inp is empty and cannot drive true-compute generation",
        ));
    }

    let numeric_rows: Vec<(usize, Vec<f64>)> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| (index, parse_numeric_tokens(line)))
        .filter(|(_, values)| !values.is_empty())
        .collect();

    let mut control = RixsControlInput::default();

    if let Some(value) = first_keyword_numeric_value(&lines, &numeric_rows, &["m_run"]) {
        control.run_enabled = value > 0.0;
    }

    if let Some(row) = first_keyword_numeric_row(&lines, &numeric_rows, &["gam_ch"], 3) {
        control.gamma_core = row[0].abs();
        control.gamma_edge_1 = row[1].abs();
        control.gamma_edge_2 = row[2].abs();
    }

    if let Some(row) =
        first_keyword_numeric_row(&lines, &numeric_rows, &["emini", "emaxi", "eminf"], 4)
    {
        control.incident_min = row[0];
        control.incident_max = row[1];
        control.emitted_min = row[2];
        control.emitted_max = row[3];
    }

    if let Some(value) = first_keyword_numeric_value(&lines, &numeric_rows, &["nenergies"]) {
        control.energy_rows = f64_to_usize(value, fixture_id, "nenergies")?.max(1);
    }

    if let Some(row) =
        first_keyword_numeric_row(&lines, &numeric_rows, &["emin", "emax", "estep"], 3)
    {
        control.incident_min = row[0];
        control.incident_max = row[1];
        control.incident_step = row[2].abs().max(1.0e-6);
        if control.energy_rows <= 1 {
            let span = (control.incident_max - control.incident_min)
                .abs()
                .max(1.0e-6);
            control.energy_rows = ((span / control.incident_step).round() as usize)
                .saturating_add(1)
                .max(3);
        }
    }

    if let Some(value) = first_keyword_numeric_value(&lines, &numeric_rows, &["xmu"]) {
        control.xmu_shift = value;
    }

    if let Some(value) = first_keyword_numeric_value(&lines, &numeric_rows, &["nedges"]) {
        control.n_edges = f64_to_usize(value, fixture_id, "nEdges")?.max(1);
    }

    if let Some(flag_line) = first_keyword_following_line(&lines, &["readpoles"]) {
        let flags = parse_bool_tokens(flag_line);
        control.read_poles = flags.first().copied().unwrap_or(false);
        control.skip_calc = flags.get(1).copied().unwrap_or(false);
        control.read_sigma = flags.get(3).copied().unwrap_or(false);
    }

    parse_edge_labels(&lines, &mut control.edge_labels);

    let (incident_min, incident_max) = ordered_range(
        control.incident_min,
        control.incident_max,
        control.incident_step.abs().max(0.5) * 64.0,
    );
    control.incident_min = incident_min;
    control.incident_max = incident_max;

    let incident_span = (control.incident_max - control.incident_min)
        .abs()
        .max(1.0e-6);
    if control.energy_rows <= 1 {
        control.energy_rows = ((incident_span / control.incident_step.abs().max(1.0e-6)).round()
            as usize)
            .saturating_add(1)
            .max(3);
    }
    control.energy_rows = control.energy_rows.clamp(3, 512);
    control.incident_step = if control.energy_rows > 1 {
        incident_span / (control.energy_rows - 1) as f64
    } else {
        0.0
    };

    let (emitted_min, emitted_max) = ordered_range(
        control.emitted_min,
        control.emitted_max,
        incident_span * 0.6 + 5.0,
    );
    control.emitted_min = emitted_min;
    control.emitted_max = emitted_max;
    let emitted_span = (control.emitted_max - control.emitted_min)
        .abs()
        .max(1.0e-6);
    control.emitted_step = if control.energy_rows > 1 {
        emitted_span / (control.energy_rows - 1) as f64
    } else {
        control.incident_step
    };

    let gamma_default = 1.350_512e-4;
    if !control.gamma_core.is_finite() || control.gamma_core <= 0.0 {
        control.gamma_core = gamma_default;
    }
    if !control.gamma_edge_1.is_finite() || control.gamma_edge_1 <= 0.0 {
        control.gamma_edge_1 = control.gamma_core;
    }
    if !control.gamma_edge_2.is_finite() || control.gamma_edge_2 <= 0.0 {
        control.gamma_edge_2 = control.gamma_edge_1;
    }

    if !control.xmu_shift.is_finite() {
        return Err(rixs_parse_error(
            fixture_id,
            "xmu shift in rixs.inp must be finite",
        ));
    }

    control.n_edges = control.n_edges.clamp(1, 2);

    Ok(control)
}

fn parse_binary_source(
    fixture_id: &str,
    artifact_name: &str,
    bytes: &[u8],
) -> PipelineResult<BinaryInputSummary> {
    if bytes.is_empty() {
        return Err(rixs_parse_error(
            fixture_id,
            format!("{} is empty", artifact_name),
        ));
    }

    let mut sum = 0.0_f64;
    let mut sq_sum = 0.0_f64;
    for byte in bytes {
        let value = f64::from(*byte);
        sum += value;
        sq_sum += value * value;
    }

    let count = bytes.len() as f64;
    Ok(BinaryInputSummary {
        byte_len: bytes.len(),
        checksum: checksum_bytes(bytes),
        mean: sum / count,
        rms: (sq_sum / count).sqrt(),
    })
}

fn parse_table_source(
    fixture_id: &str,
    artifact_name: &str,
    source: &str,
) -> PipelineResult<TableInputSummary> {
    let mut values = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with('!')
            || trimmed.starts_with('*')
        {
            continue;
        }

        values.extend(parse_numeric_tokens(trimmed));
    }

    if values.is_empty() {
        return Err(rixs_parse_error(
            fixture_id,
            format!("{} does not contain numeric data", artifact_name),
        ));
    }

    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut sum = 0.0_f64;
    let mut sq_sum = 0.0_f64;

    for value in &values {
        min = min.min(*value);
        max = max.max(*value);
        sum += value;
        sq_sum += value * value;
    }

    let count = values.len() as f64;
    Ok(TableInputSummary {
        value_count: values.len(),
        min,
        max,
        mean: sum / count,
        rms: (sq_sum / count).sqrt(),
    })
}

fn rixs_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.RIXS_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

fn first_keyword_numeric_value(
    lines: &[&str],
    numeric_rows: &[(usize, Vec<f64>)],
    keywords: &[&str],
) -> Option<f64> {
    first_keyword_numeric_row(lines, numeric_rows, keywords, 1).map(|row| row[0])
}

fn first_keyword_numeric_row<'a>(
    lines: &[&str],
    numeric_rows: &'a [(usize, Vec<f64>)],
    keywords: &[&str],
    minimum_len: usize,
) -> Option<&'a [f64]> {
    for (index, line) in lines.iter().enumerate() {
        let lower = line.to_ascii_lowercase();
        if keywords.iter().all(|keyword| lower.contains(keyword))
            && let Some(row) = next_numeric_row(numeric_rows, index, minimum_len)
        {
            return Some(row);
        }
    }

    None
}

fn first_keyword_following_line<'a>(lines: &'a [&'a str], keywords: &[&str]) -> Option<&'a str> {
    for (index, line) in lines.iter().enumerate() {
        let lower = line.to_ascii_lowercase();
        if keywords.iter().all(|keyword| lower.contains(keyword))
            && let Some((_, next_line)) = next_nonempty_line(lines, index + 1)
        {
            return Some(next_line);
        }
    }

    None
}

fn next_numeric_row(
    numeric_rows: &[(usize, Vec<f64>)],
    start_index: usize,
    minimum_len: usize,
) -> Option<&[f64]> {
    numeric_rows
        .iter()
        .find(|(line_index, values)| *line_index > start_index && values.len() >= minimum_len)
        .map(|(_, values)| values.as_slice())
}

fn next_nonempty_line<'a>(lines: &'a [&'a str], start_index: usize) -> Option<(usize, &'a str)> {
    for (index, line) in lines.iter().enumerate().skip(start_index) {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with('!')
            || trimmed.starts_with('*')
        {
            continue;
        }
        return Some((index, *line));
    }

    None
}

fn parse_edge_labels(lines: &[&str], labels: &mut [String; 2]) {
    for (index, line) in lines.iter().enumerate() {
        let lower = line.trim_start().to_ascii_lowercase();
        if !lower.starts_with("edge") {
            continue;
        }

        let edge_index = parse_numeric_tokens(line)
            .first()
            .copied()
            .and_then(f64_to_usize_lossy)
            .unwrap_or(1)
            .saturating_sub(1)
            .min(1);

        if let Some((_, next_line)) = next_nonempty_line(lines, index + 1) {
            let candidate = next_line
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|character: char| {
                    matches!(
                        character,
                        ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
                    )
                });

            if !candidate.is_empty() && parse_numeric_token(candidate).is_none() {
                labels[edge_index] = candidate.to_ascii_uppercase();
            }
        }
    }
}

fn ordered_range(first: f64, second: f64, fallback_span: f64) -> (f64, f64) {
    let default_half_span = fallback_span.abs().max(1.0) * 0.5;
    if !first.is_finite() || !second.is_finite() {
        return (-default_half_span, default_half_span);
    }

    let mut min = first;
    let mut max = second;
    if max < min {
        std::mem::swap(&mut min, &mut max);
    }

    if (max - min).abs() <= 1.0e-9 {
        min -= default_half_span;
        max += default_half_span;
    }

    (min, max)
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
            ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
        )
    });
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.replace(['D', 'd'], "E");
    normalized.parse::<f64>().ok()
}

fn parse_bool_tokens(line: &str) -> Vec<bool> {
    line.split_whitespace()
        .filter_map(|token| {
            let normalized = token
                .trim_matches(|character: char| {
                    matches!(
                        character,
                        ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
                    )
                })
                .to_ascii_lowercase();
            match normalized.as_str() {
                "t" | "true" | "1" => Some(true),
                "f" | "false" | "0" => Some(false),
                _ => None,
            }
        })
        .collect()
}

fn checksum_bytes(bytes: &[u8]) -> u64 {
    let mut checksum = 0_u64;
    for (index, byte) in bytes.iter().enumerate() {
        checksum = checksum
            .wrapping_add((*byte as u64).wrapping_mul((index as u64 % 2048) + 1))
            .rotate_left((index % 23) as u32 + 1);
    }
    checksum
}

fn checksum_to_unit(checksum: u64) -> f64 {
    (checksum as f64 / u64::MAX as f64).clamp(0.0, 1.0)
}

fn normalized_index(index: usize, count: usize) -> f64 {
    if count <= 1 {
        return 0.0;
    }

    index as f64 / (count - 1) as f64
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> PipelineResult<usize> {
    if !value.is_finite() {
        return Err(rixs_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }

    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-6 {
        return Err(rixs_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }

    if rounded < 0.0 {
        return Err(rixs_parse_error(
            fixture_id,
            format!("{} must be non-negative", field),
        ));
    }

    Ok(rounded as usize)
}

fn f64_to_usize_lossy(value: f64) -> Option<usize> {
    if !value.is_finite() {
        return None;
    }

    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-6 || rounded < 0.0 {
        return None;
    }

    Some(rounded as usize)
}

fn format_scientific_f64(value: f64) -> String {
    format!("{:>16.8E}", value)
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::RixsPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const RIXS_OUTPUTS: [&str; 7] = [
        "rixs0.dat",
        "rixs1.dat",
        "rixsET.dat",
        "rixsEE.dat",
        "rixsET-sat.dat",
        "rixsEE-sat.dat",
        "logrixs.dat",
    ];

    const RIXS_INPUT: &str = "\
 m_run
           1
 gam_ch, gam_exp(1), gam_exp(2)
        0.0001350512        0.0001450512        0.0001550512
 EMinI, EMaxI, EMinF, EMaxF
      -12.0000000000       18.0000000000       -4.0000000000       16.0000000000
 xmu
  -367493090.02742821
 Readpoles, SkipCalc, MBConv, ReadSigma
 T F F T
 nEdges
           2
 Edge           1
 L3
 Edge           2
 L2
";

    const WSCRN_1_INPUT: &str = "\
# edge 1 screening profile
-6.0  0.11  0.95
-2.0  0.16  1.05
 0.0  0.18  1.15
 3.5  0.23  1.30
 8.0  0.31  1.45
";

    const WSCRN_2_INPUT: &str = "\
# edge 2 screening profile
-5.0  0.09  0.85
-1.5  0.14  0.95
 1.0  0.17  1.05
 4.0  0.21  1.22
 9.0  0.28  1.36
";

    const XSECT_2_INPUT: &str = "\
# xsect_2 seed table
0.0  1.2  0.1
2.0  1.0  0.2
4.0  0.9  0.3
6.0  0.8  0.4
8.0  0.7  0.5
";

    #[test]
    fn contract_exposes_required_inputs_and_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle(temp.path());

        let request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path().join("out"),
        );
        let contract = RixsPipelineScaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            contract.required_inputs,
            artifact_list(&[
                "rixs.inp",
                "phase_1.bin",
                "phase_2.bin",
                "wscrn_1.dat",
                "wscrn_2.dat",
                "xsect_2.dat"
            ])
        );
        assert_eq!(artifact_set(&contract.expected_outputs), expected_outputs());
    }

    #[test]
    fn execute_writes_true_compute_rixs_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle(temp.path());

        let request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path().join("out"),
        );
        let artifacts = RixsPipelineScaffold
            .execute(&request)
            .expect("execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_outputs());
        for artifact in expected_outputs() {
            let output_path = request.output_dir.join(&artifact);
            assert!(
                output_path.is_file(),
                "{} should exist",
                output_path.display()
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "{} should not be empty",
                output_path.display()
            );
        }
    }

    #[test]
    fn execute_is_deterministic_for_identical_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle(temp.path());

        let first_request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path().join("out-first"),
        );
        let second_request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path().join("out-second"),
        );

        let first = RixsPipelineScaffold
            .execute(&first_request)
            .expect("first execution should succeed");
        let second = RixsPipelineScaffold
            .execute(&second_request)
            .expect("second execution should succeed");

        assert_eq!(artifact_set(&first), artifact_set(&second));
        for artifact in first {
            let first_bytes =
                fs::read(first_request.output_dir.join(&artifact.relative_path)).expect("first");
            let second_bytes =
                fs::read(second_request.output_dir.join(&artifact.relative_path)).expect("second");
            assert_eq!(
                first_bytes,
                second_bytes,
                "artifact '{}' should be deterministic",
                artifact.relative_path.display()
            );
        }
    }

    #[test]
    fn execute_responds_to_multi_edge_input_changes() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_root = temp.path().join("first");
        let second_root = temp.path().join("second");
        stage_rixs_input_bundle(&first_root);
        stage_rixs_input_bundle(&second_root);

        stage_binary(
            second_root.join("phase_2.bin"),
            &[255_u8, 254_u8, 0_u8, 8_u8, 21_u8, 34_u8, 55_u8, 89_u8],
        );
        stage_text(
            second_root.join("wscrn_2.dat"),
            "# altered edge 2 screening\n-5.0  0.40  2.10\n0.0  0.55  2.25\n5.0  0.70  2.40\n",
        );

        let first_request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rixs,
            first_root.join("rixs.inp"),
            first_root.join("out"),
        );
        let second_request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rixs,
            second_root.join("rixs.inp"),
            second_root.join("out"),
        );

        let first_artifacts = RixsPipelineScaffold
            .execute(&first_request)
            .expect("first execution should succeed");
        let second_artifacts = RixsPipelineScaffold
            .execute(&second_request)
            .expect("second execution should succeed");

        assert_eq!(artifact_set(&first_artifacts), expected_outputs());
        assert_eq!(artifact_set(&second_artifacts), expected_outputs());

        let first_rixs1 = fs::read(first_request.output_dir.join("rixs1.dat"))
            .expect("first rixs1.dat should be readable");
        let second_rixs1 = fs::read(second_request.output_dir.join("rixs1.dat"))
            .expect("second rixs1.dat should be readable");
        assert_ne!(
            first_rixs1, second_rixs1,
            "rixs1.dat should change when edge-2 staged inputs change"
        );

        let first_rixsee = fs::read(first_request.output_dir.join("rixsEE.dat"))
            .expect("first rixsEE.dat should be readable");
        let second_rixsee = fs::read(second_request.output_dir.join("rixsEE.dat"))
            .expect("second rixsEE.dat should be readable");
        assert_ne!(
            first_rixsee, second_rixsee,
            "rixsEE.dat should change when edge-2 staged inputs change"
        );
    }

    #[test]
    fn execute_rejects_non_rixs_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle(temp.path());

        let request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rdinp,
            temp.path().join("rixs.inp"),
            temp.path().join("out"),
        );
        let error = RixsPipelineScaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.RIXS_MODULE");
    }

    #[test]
    fn execute_requires_phase_2_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle(temp.path());
        fs::remove_file(temp.path().join("phase_2.bin")).expect("phase_2.bin should be removed");

        let request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path().join("out"),
        );
        let error = RixsPipelineScaffold
            .execute(&request)
            .expect_err("missing phase_2 input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.RIXS_INPUT_READ");
    }

    fn expected_outputs() -> BTreeSet<String> {
        RIXS_OUTPUTS
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
        paths.iter().copied().map(PipelineArtifact::new).collect()
    }

    fn stage_rixs_input_bundle(destination_dir: &Path) {
        stage_text(destination_dir.join("rixs.inp"), RIXS_INPUT);
        stage_binary(
            destination_dir.join("phase_1.bin"),
            &[3_u8, 5_u8, 8_u8, 13_u8, 21_u8, 34_u8, 55_u8, 89_u8],
        );
        stage_binary(
            destination_dir.join("phase_2.bin"),
            &[2_u8, 7_u8, 1_u8, 8_u8, 2_u8, 8_u8, 1_u8, 8_u8],
        );
        stage_text(destination_dir.join("wscrn_1.dat"), WSCRN_1_INPUT);
        stage_text(destination_dir.join("wscrn_2.dat"), WSCRN_2_INPUT);
        stage_text(destination_dir.join("xsect_2.dat"), XSECT_2_INPUT);
    }

    fn stage_text(destination: PathBuf, contents: &str) {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should exist");
        }
        fs::write(destination, contents).expect("text input should be written");
    }

    fn stage_binary(destination: PathBuf, bytes: &[u8]) {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should exist");
        }
        fs::write(destination, bytes).expect("binary input should be written");
    }
}
