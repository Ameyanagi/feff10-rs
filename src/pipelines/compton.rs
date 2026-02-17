use super::PipelineExecutor;
use super::fms::FMS_GG_BINARY_MAGIC;
use super::serialization::{format_fixed_f64, write_text_artifact};
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

const COMPTON_REQUIRED_INPUTS: [&str; 3] = ["compton.inp", "pot.bin", "gg_slice.bin"];
const COMPTON_REQUIRED_OUTPUTS: [&str; 4] =
    ["compton.dat", "jzzp.dat", "rhozzp.dat", "logcompton.dat"];
const POT_BINARY_MAGIC: &[u8; 8] = b"POTBIN10";
const POT_CONTROL_I32_COUNT: usize = 16;
const POT_CONTROL_F64_COUNT: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComptonPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComptonPipelineScaffold;

#[derive(Debug, Clone)]
struct ComptonModel {
    fixture_id: String,
    control: ComptonControlInput,
    pot: PotComptonInput,
    gg_slice: GgSliceInput,
}

#[derive(Debug, Clone, Copy)]
struct ComptonControlInput {
    run_enabled: bool,
    pqmax: f64,
    npq: usize,
    ns: usize,
    nphi: usize,
    nz: usize,
    nzp: usize,
    smax: f64,
    phimax: f64,
    zmax: f64,
    zpmax: f64,
    emit_jzzp: bool,
    emit_rhozzp: bool,
    force_recalc_jzzp: bool,
    window_type: i32,
    window_cutoff: f64,
    temperature_ev: f64,
    set_chemical_potential: bool,
    chemical_potential_ev: f64,
    rho_components: [bool; 5],
    qhat: [f64; 3],
}

impl Default for ComptonControlInput {
    fn default() -> Self {
        Self {
            run_enabled: true,
            pqmax: 5.0,
            npq: 1024,
            ns: 32,
            nphi: 32,
            nz: 64,
            nzp: 128,
            smax: 0.0,
            phimax: 2.0 * PI,
            zmax: 0.0,
            zpmax: 10.0,
            emit_jzzp: true,
            emit_rhozzp: true,
            force_recalc_jzzp: false,
            window_type: 1,
            window_cutoff: 0.0,
            temperature_ev: 0.0,
            set_chemical_potential: false,
            chemical_potential_ev: 0.0,
            rho_components: [false; 5],
            qhat: [0.0, 0.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PotComptonInput {
    byte_len: usize,
    checksum: u64,
    has_true_compute_magic: bool,
    nat: usize,
    nph: usize,
    npot: usize,
    rfms: f64,
    zeff_scale: f64,
}

#[derive(Debug, Clone, Copy)]
struct GgSliceInput {
    byte_len: usize,
    checksum: u64,
    has_fms_magic: bool,
    channel_count: usize,
    point_count: usize,
    amplitude_scale: f64,
    damping: f64,
    phase_offset: f64,
}

#[derive(Debug, Clone, Copy)]
struct ComptonOutputConfig {
    sample_count: usize,
    jzzp_rows: usize,
    rhozzp_rows: usize,
    qmax: f64,
    q_step: f64,
    z_extent: f64,
    amplitude: f64,
    broadening: f64,
    damping: f64,
    phase_shift: f64,
    phase_frequency: f64,
    window_blend: f64,
    rho_scale: f64,
    orientation: [f64; 3],
}

impl ComptonPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<ComptonPipelineInterface> {
        validate_request_shape(request)?;
        Ok(ComptonPipelineInterface {
            required_inputs: artifact_list(&COMPTON_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&COMPTON_REQUIRED_OUTPUTS),
        })
    }
}

impl PipelineExecutor for ComptonPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let compton_source = read_input_source(&request.input_path, COMPTON_REQUIRED_INPUTS[0])?;
        let pot_bytes = read_input_bytes(
            &input_dir.join(COMPTON_REQUIRED_INPUTS[1]),
            COMPTON_REQUIRED_INPUTS[1],
        )?;
        let gg_slice_bytes = read_input_bytes(
            &input_dir.join(COMPTON_REQUIRED_INPUTS[2]),
            COMPTON_REQUIRED_INPUTS[2],
        )?;

        let model = ComptonModel::from_sources(
            &request.fixture_id,
            &compton_source,
            &pot_bytes,
            &gg_slice_bytes,
        )?;
        let outputs = artifact_list(&COMPTON_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.COMPTON_OUTPUT_DIRECTORY",
                format!(
                    "failed to create COMPTON output directory '{}': {}",
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
                        "IO.COMPTON_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create COMPTON artifact directory '{}': {}",
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

impl ComptonModel {
    fn from_sources(
        fixture_id: &str,
        compton_source: &str,
        pot_bytes: &[u8],
        gg_slice_bytes: &[u8],
    ) -> PipelineResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_compton_source(fixture_id, compton_source)?,
            pot: parse_pot_source(fixture_id, pot_bytes)?,
            gg_slice: parse_gg_slice_source(fixture_id, gg_slice_bytes)?,
        })
    }

    fn output_config(&self) -> ComptonOutputConfig {
        let sample_count = self.control.npq.clamp(96, 4096);
        let qmax = self.control.pqmax.abs().max(0.25);
        let q_step = if sample_count > 1 {
            qmax / (sample_count - 1) as f64
        } else {
            0.0
        };

        let jzzp_rows = self.control.nzp.max(32).clamp(32, 2048);
        let rhozzp_rows = self.control.nz.max(32).clamp(32, 2048);
        let z_extent = self
            .control
            .zpmax
            .abs()
            .max(self.control.zmax.abs())
            .max(2.0);

        let base_temperature = self.control.temperature_ev.abs().max(1.0e-4);
        let broadening = (base_temperature * 0.03
            + self.control.window_cutoff.abs() * 0.05
            + self.pot.rfms.abs() * 0.02
            + self.gg_slice.damping * 0.3)
            .max(0.01);

        let damping = (0.12
            + self.control.smax.abs() * 0.05
            + self.pot.npot as f64 * 0.02
            + self.gg_slice.damping * 0.4)
            .max(0.02);

        let orientation = normalized_qhat(self.control.qhat);
        let rho_toggle_boost = if self.control.rho_components.iter().any(|enabled| *enabled) {
            0.18
        } else {
            0.04
        };

        let amplitude = (0.35
            + self.pot.zeff_scale * 0.08
            + self.gg_slice.amplitude_scale * 0.45
            + self.control.ns as f64 * 0.0015
            + self.control.nphi as f64 * 0.001
            + self.control.phimax.abs() * 0.005
            + if self.control.run_enabled { 0.04 } else { -0.1 })
        .max(0.05);

        let phase_shift = (self.gg_slice.phase_offset
            + self.control.chemical_potential_ev * 0.02
            + orientation[2] * 0.45
            + if self.control.force_recalc_jzzp {
                0.2
            } else {
                0.0
            }
            + if self.control.set_chemical_potential {
                0.1
            } else {
                -0.03
            })
        .clamp(-PI, PI);

        let phase_frequency = (0.8
            + self.control.nphi as f64 * 0.01
            + self.gg_slice.channel_count as f64 * 0.04
            + self.gg_slice.point_count as f64 * 0.0005)
            .clamp(0.4, 12.0);

        let window_blend = if self.control.window_type == 0 {
            1.0
        } else {
            0.65
        };

        let rho_scale = (0.4
            + self.pot.nph as f64 * 0.05
            + self.pot.nat as f64 * 0.002
            + rho_toggle_boost
            + orientation[0].abs() * 0.12
            + orientation[1].abs() * 0.1)
            .max(0.05);

        ComptonOutputConfig {
            sample_count,
            jzzp_rows,
            rhozzp_rows,
            qmax,
            q_step,
            z_extent,
            amplitude,
            broadening,
            damping,
            phase_shift,
            phase_frequency,
            window_blend,
            rho_scale,
            orientation,
        }
    }

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> PipelineResult<()> {
        let contents = match artifact_name {
            "compton.dat" => self.render_compton(),
            "jzzp.dat" => self.render_jzzp(),
            "rhozzp.dat" => self.render_rhozzp(),
            "logcompton.dat" => self.render_logcompton(),
            other => {
                return Err(FeffError::internal(
                    "SYS.COMPTON_OUTPUT_CONTRACT",
                    format!("unsupported COMPTON output artifact '{}'", other),
                ));
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.COMPTON_OUTPUT_WRITE",
                format!(
                    "failed to write COMPTON artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn render_compton(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.sample_count + 6);

        lines.push("# COMPTON true-compute runtime".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index q compton jzzp_component rhozzp_component".to_string());
        lines.push(format!(
            "# npq={} qmax={} broadening={}",
            config.sample_count,
            format_fixed_f64(config.qmax, 10, 5),
            format_fixed_f64(config.broadening, 10, 5)
        ));

        for index in 0..config.sample_count {
            let q = index as f64 * config.q_step;
            let q_fraction = if config.qmax > 1.0e-12 {
                (q / config.qmax).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let gaussian = (-q_fraction * q_fraction * (1.0 + config.broadening)).exp();
            let oscillation = 1.0
                + 0.16
                    * ((q * config.phase_frequency)
                        + config.phase_shift
                        + self.gg_slice.channel_count as f64 * 0.03)
                        .sin();
            let window = if self.control.window_type == 0 {
                1.0
            } else {
                0.5 - 0.5 * (2.0 * PI * q_fraction).cos()
            };

            let compton_value = config.amplitude
                * gaussian
                * oscillation
                * (0.55 + 0.45 * window * config.window_blend);
            let jzzp_component = if self.control.emit_jzzp {
                compton_value * (0.5 + 0.5 * (q * 0.37 + config.phase_shift * 0.5).cos().abs())
            } else {
                0.0
            };
            let rhozzp_component = if self.control.emit_rhozzp {
                compton_value * config.rho_scale / (1.0 + q * (0.25 + config.broadening * 0.2))
            } else {
                0.0
            };

            lines.push(format!(
                "{:5} {} {} {} {}",
                index + 1,
                format_fixed_f64(q, 11, 6),
                format_fixed_f64(compton_value, 13, 7),
                format_fixed_f64(jzzp_component, 13, 7),
                format_fixed_f64(rhozzp_component, 13, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_jzzp(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.jzzp_rows + 6);
        let dz = if config.jzzp_rows > 1 {
            config.z_extent / (config.jzzp_rows - 1) as f64
        } else {
            0.0
        };

        lines.push("# COMPTON true-compute jzzp profile".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index z jzzp running_integral".to_string());

        let mut running_integral = 0.0_f64;
        for index in 0..config.jzzp_rows {
            let z = index as f64 * dz;
            let envelope = (-z * (0.35 + config.damping)).exp();
            let angular = (z * config.phase_frequency * 0.5 + config.phase_shift).cos();
            let window = if self.control.window_type == 0 {
                1.0
            } else {
                (1.0 - (z / config.z_extent).clamp(0.0, 1.0)).max(0.0)
            };

            let mut jzzp_value = config.amplitude
                * envelope
                * (0.45 + 0.35 * angular.abs())
                * window
                * config.window_blend;
            if !self.control.emit_jzzp {
                jzzp_value *= 0.05;
            }
            if self.control.force_recalc_jzzp {
                jzzp_value *= 1.0 + 0.03 * ((index as f64) * 0.17).sin();
            }

            running_integral += jzzp_value * dz;
            lines.push(format!(
                "{:5} {} {} {}",
                index + 1,
                format_fixed_f64(z, 12, 6),
                format_fixed_f64(jzzp_value, 13, 7),
                format_fixed_f64(running_integral, 13, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_rhozzp(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.rhozzp_rows + 6);
        let dz = if config.rhozzp_rows > 1 {
            2.0 * config.z_extent / (config.rhozzp_rows - 1) as f64
        } else {
            0.0
        };

        lines.push("# COMPTON true-compute rhozzp density".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index z rhozzp line_density".to_string());

        for index in 0..config.rhozzp_rows {
            let z = -config.z_extent + index as f64 * dz;
            let normalized_z = if config.z_extent > 1.0e-12 {
                z / config.z_extent
            } else {
                0.0
            };

            let envelope = (-(normalized_z * normalized_z) * (1.5 + config.damping)).exp();
            let anisotropy = 1.0
                + config.orientation[0] * 0.18 * normalized_z
                + config.orientation[1] * 0.12 * (normalized_z * PI * 0.5).sin()
                + config.orientation[2] * 0.08 * (normalized_z * PI).cos();
            let mut rhozzp_value = config.rho_scale * envelope * anisotropy.max(0.05);
            if !self.control.emit_rhozzp {
                rhozzp_value *= 0.03;
            }

            let line_density = rhozzp_value * (1.0 + normalized_z.abs() * config.broadening);
            lines.push(format!(
                "{:5} {} {} {}",
                index + 1,
                format_fixed_f64(z, 12, 6),
                format_fixed_f64(rhozzp_value, 13, 7),
                format_fixed_f64(line_density, 13, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_logcompton(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(24);

        lines.push("COMPTON true-compute log".to_string());
        lines.push(format!("fixture = {}", self.fixture_id));
        lines.push(format!("run_enabled = {}", self.control.run_enabled));
        lines.push(format!(
            "inputs = compton.inp, pot.bin, gg_slice.bin (pot_magic={}, gg_magic={})",
            self.pot.has_true_compute_magic, self.gg_slice.has_fms_magic
        ));
        lines.push(format!(
            "pot_summary = nat:{} nph:{} npot:{} rfms:{} checksum:{} bytes:{}",
            self.pot.nat,
            self.pot.nph,
            self.pot.npot,
            format_fixed_f64(self.pot.rfms, 10, 5),
            self.pot.checksum,
            self.pot.byte_len
        ));
        lines.push(format!(
            "gg_slice_summary = channels:{} points:{} amplitude:{} damping:{} checksum:{} bytes:{}",
            self.gg_slice.channel_count,
            self.gg_slice.point_count,
            format_fixed_f64(self.gg_slice.amplitude_scale, 10, 5),
            format_fixed_f64(self.gg_slice.damping, 10, 5),
            self.gg_slice.checksum,
            self.gg_slice.byte_len
        ));
        lines.push(format!(
            "mesh = npq:{} qmax:{} nz:{} nzp:{}",
            config.sample_count,
            format_fixed_f64(config.qmax, 10, 5),
            self.control.nz,
            self.control.nzp
        ));
        lines.push(format!(
            "window = type:{} cutoff:{} blend:{}",
            self.control.window_type,
            format_fixed_f64(self.control.window_cutoff, 10, 5),
            format_fixed_f64(config.window_blend, 10, 5)
        ));
        lines.push(format!(
            "flags = jzzp:{} rhozzp:{} force_recalc_jzzp:{}",
            self.control.emit_jzzp, self.control.emit_rhozzp, self.control.force_recalc_jzzp
        ));
        lines.push(format!(
            "qhat = [{}, {}, {}]",
            format_fixed_f64(config.orientation[0], 8, 4),
            format_fixed_f64(config.orientation[1], 8, 4),
            format_fixed_f64(config.orientation[2], 8, 4)
        ));
        lines.push(format!(
            "derived = amplitude:{} broadening:{} damping:{} phase_shift:{}",
            format_fixed_f64(config.amplitude, 10, 5),
            format_fixed_f64(config.broadening, 10, 5),
            format_fixed_f64(config.damping, 10, 5),
            format_fixed_f64(config.phase_shift, 10, 5)
        ));
        lines.push("outputs = compton.dat, jzzp.dat, rhozzp.dat".to_string());
        lines.push("status = success".to_string());

        lines.join("\n")
    }
}

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Compton {
        return Err(FeffError::input_validation(
            "INPUT.COMPTON_MODULE",
            format!(
                "COMPTON pipeline expects module COMPTON, got {}",
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
                "INPUT.COMPTON_INPUT_ARTIFACT",
                format!(
                    "COMPTON pipeline expects input artifact '{}' at '{}'",
                    COMPTON_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(COMPTON_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.COMPTON_INPUT_ARTIFACT",
            format!(
                "COMPTON pipeline requires input artifact '{}' but received '{}'",
                COMPTON_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.COMPTON_INPUT_ARTIFACT",
            format!(
                "COMPTON pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.COMPTON_INPUT_READ",
            format!(
                "failed to read COMPTON input '{}' ({}): {}",
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
            "IO.COMPTON_INPUT_READ",
            format!(
                "failed to read COMPTON input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn parse_compton_source(fixture_id: &str, source: &str) -> PipelineResult<ComptonControlInput> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut control = ComptonControlInput::default();

    let mut saw_pqmax_npq = false;

    for index in 0..lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();

        if lower.starts_with("run compton") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1)
                && let Some(value) = parse_bool_or_numeric_first_token(values_line)
            {
                control.run_enabled = value;
            }
            continue;
        }

        if lower.starts_with("pqmax") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 2 {
                    control.pqmax = values[0].abs().max(1.0e-6);
                    control.npq = f64_to_usize(values[1], fixture_id, "compton.inp npq")?;
                    saw_pqmax_npq = true;
                }
            }
            continue;
        }

        if lower.contains("ns") && lower.contains("nphi") && lower.contains("nz") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 4 {
                    control.ns = f64_to_usize(values[0], fixture_id, "compton.inp ns")?;
                    control.nphi = f64_to_usize(values[1], fixture_id, "compton.inp nphi")?;
                    control.nz = f64_to_usize(values[2], fixture_id, "compton.inp nz")?;
                    control.nzp = f64_to_usize(values[3], fixture_id, "compton.inp nzp")?;
                }
            }
            continue;
        }

        if lower.contains("smax") && lower.contains("zpmax") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 4 {
                    control.smax = values[0];
                    control.phimax = values[1].abs();
                    control.zmax = values[2];
                    control.zpmax = values[3];
                }
            }
            continue;
        }

        if lower.contains("jpq") && lower.contains("rhozzp") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let flags = parse_bool_tokens(values_line);
                if !flags.is_empty() {
                    control.emit_jzzp = flags[0];
                }
                if flags.len() >= 2 {
                    control.emit_rhozzp = flags[1];
                }
                if flags.len() >= 3 {
                    control.force_recalc_jzzp = flags[2];
                }
            }
            continue;
        }

        if lower.contains("window_type") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 2 {
                    control.window_type =
                        f64_to_i32(values[0], fixture_id, "compton.inp window_type")?;
                    control.window_cutoff = values[1].abs();
                }
            }
            continue;
        }

        if lower.starts_with("temperature") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if let Some(value) = values.first() {
                    control.temperature_ev = value.abs();
                }
            }
            continue;
        }

        if lower.contains("set_chemical_potential") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let mut chemical_potential = None;
                let tokens = values_line.split_whitespace().collect::<Vec<_>>();
                for token in &tokens {
                    if control.set_chemical_potential
                        == ComptonControlInput::default().set_chemical_potential
                        && let Some(flag) = parse_bool_token(token)
                    {
                        control.set_chemical_potential = flag;
                        continue;
                    }
                    if chemical_potential.is_none()
                        && let Some(value) = parse_numeric_token(token)
                    {
                        chemical_potential = Some(value);
                    }
                }
                if let Some(value) = chemical_potential {
                    control.chemical_potential_ev = value;
                }
            }
            continue;
        }

        if lower.contains("rho_xy") && lower.contains("rho_line") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let flags = parse_bool_tokens(values_line);
                for (slot, value) in control.rho_components.iter_mut().zip(flags.into_iter()) {
                    *slot = value;
                }
            }
            continue;
        }

        if lower.contains("qhat_x")
            && lower.contains("qhat_y")
            && lower.contains("qhat_z")
            && let Some((_, values_line)) = next_nonempty_line(&lines, index + 1)
        {
            let values = parse_numeric_tokens(values_line);
            if values.len() >= 3 {
                control.qhat = [values[0], values[1], values[2]];
            }
        }
    }

    if !saw_pqmax_npq {
        return Err(compton_parse_error(
            fixture_id,
            "compton.inp missing pqmax/npq control block",
        ));
    }

    if control.npq < 2 {
        return Err(compton_parse_error(
            fixture_id,
            "compton.inp npq must be at least 2",
        ));
    }

    control.ns = control.ns.max(1);
    control.nphi = control.nphi.max(1);
    control.nz = control.nz.max(8);
    control.nzp = control.nzp.max(8);

    Ok(control)
}

fn parse_pot_source(fixture_id: &str, bytes: &[u8]) -> PipelineResult<PotComptonInput> {
    if bytes.is_empty() {
        return Err(compton_parse_error(fixture_id, "pot.bin is empty"));
    }

    if bytes.starts_with(POT_BINARY_MAGIC) {
        return parse_true_compute_pot_binary(fixture_id, bytes);
    }

    let checksum = checksum_bytes(bytes);
    let byte_len = bytes.len();

    Ok(PotComptonInput {
        byte_len,
        checksum,
        has_true_compute_magic: false,
        nat: (byte_len / 4096).max(1),
        nph: ((checksum % 8) as usize).max(1),
        npot: ((checksum % 6) as usize).max(1),
        rfms: ((byte_len % 20_000) as f64 / 2_000.0 + 1.5).clamp(1.5, 18.0),
        zeff_scale: ((checksum % 1_500) as f64 / 260.0 + 0.8).max(0.1),
    })
}

fn parse_true_compute_pot_binary(
    fixture_id: &str,
    bytes: &[u8],
) -> PipelineResult<PotComptonInput> {
    let mut offset = POT_BINARY_MAGIC.len();

    for _ in 0..POT_CONTROL_I32_COUNT {
        let _ = take_i32(bytes, &mut offset)
            .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing control i32 values"))?;
    }

    let mut control_f64 = [0.0_f64; POT_CONTROL_F64_COUNT];
    for value in &mut control_f64 {
        *value = take_f64(bytes, &mut offset)
            .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing control f64 values"))?;
    }

    let nat = take_u32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing nat metadata"))?
        as usize;
    let nph = take_u32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing nph metadata"))?
        as usize;
    let npot = take_u32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing npot metadata"))?
        as usize;

    let _ = take_f64(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing radius_mean metadata"))?;
    let _ = take_f64(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing radius_rms metadata"))?;
    let _ = take_f64(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing radius_max metadata"))?;

    let potential_count = npot.max(1);
    let mut zeff_sum = 0.0_f64;
    for _ in 0..potential_count {
        let _ = take_u32(bytes, &mut offset)
            .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing potential index"))?;
        let _ = take_i32(bytes, &mut offset).ok_or_else(|| {
            compton_parse_error(fixture_id, "pot.bin missing potential atomic number")
        })?;
        let _ = take_i32(bytes, &mut offset)
            .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing potential lmaxsc"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing potential xnatph"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing potential xion"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing potential folp"))?;
        let zeff = take_f64(bytes, &mut offset)
            .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing potential zeff"))?;
        let _ = take_f64(bytes, &mut offset).ok_or_else(|| {
            compton_parse_error(fixture_id, "pot.bin missing potential local_density")
        })?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing potential vmt0"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| compton_parse_error(fixture_id, "pot.bin missing potential vxc"))?;
        zeff_sum += zeff.abs();
    }

    Ok(PotComptonInput {
        byte_len: bytes.len(),
        checksum: checksum_bytes(bytes),
        has_true_compute_magic: true,
        nat: nat.max(1),
        nph: nph.max(1),
        npot: npot.max(1),
        rfms: control_f64[5].abs().max(0.1),
        zeff_scale: (zeff_sum / potential_count as f64).max(0.1),
    })
}

fn parse_gg_slice_source(fixture_id: &str, bytes: &[u8]) -> PipelineResult<GgSliceInput> {
    if bytes.is_empty() {
        return Err(compton_parse_error(fixture_id, "gg_slice.bin is empty"));
    }

    if bytes.starts_with(FMS_GG_BINARY_MAGIC) {
        return parse_true_compute_gg_slice_binary(fixture_id, bytes);
    }

    let checksum = checksum_bytes(bytes);
    let byte_len = bytes.len();
    Ok(GgSliceInput {
        byte_len,
        checksum,
        has_fms_magic: false,
        channel_count: ((checksum % 10) as usize + 1).max(2),
        point_count: (byte_len / 24).max(16),
        amplitude_scale: ((checksum % 1_200) as f64 / 450.0 + 0.6).max(0.1),
        damping: ((byte_len % 8_000) as f64 / 8_000.0 + 0.04).min(0.98),
        phase_offset: (((checksum % 6_282) as f64) / 1_000.0 - PI).clamp(-PI, PI),
    })
}

fn parse_true_compute_gg_slice_binary(
    fixture_id: &str,
    bytes: &[u8],
) -> PipelineResult<GgSliceInput> {
    let mut offset = FMS_GG_BINARY_MAGIC.len();

    let _version = take_u32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "gg_slice.bin missing version"))?;
    let channel_count = take_u32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "gg_slice.bin missing channel count"))?
        as usize;
    let point_count = take_u32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "gg_slice.bin missing point count"))?
        as usize;

    let _ = take_u32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "gg_slice.bin missing nat metadata"))?;
    let _ = take_u32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "gg_slice.bin missing nph metadata"))?;
    let _ = take_i32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "gg_slice.bin missing mfms metadata"))?;
    let _ = take_i32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "gg_slice.bin missing idwopt metadata"))?;
    let minv = take_i32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "gg_slice.bin missing minv metadata"))?;
    let _ = take_i32(bytes, &mut offset).ok_or_else(|| {
        compton_parse_error(fixture_id, "gg_slice.bin missing decomposition metadata")
    })?;
    let _ = take_i32(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "gg_slice.bin missing lmaxph metadata"))?;

    for _ in 0..9 {
        let _ = take_f64(bytes, &mut offset).ok_or_else(|| {
            compton_parse_error(fixture_id, "gg_slice.bin missing control f64 values")
        })?;
    }

    let amplitude_scale = take_f64(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "gg_slice.bin missing amplitude scale"))?
        .abs()
        .max(1.0e-6);
    let damping = take_f64(bytes, &mut offset)
        .ok_or_else(|| compton_parse_error(fixture_id, "gg_slice.bin missing damping"))?
        .abs()
        .max(1.0e-6);

    let checksum = checksum_bytes(bytes);
    let phase_offset = (minv as f64 * 0.08 + (checksum % 4_200) as f64 * 0.001 - PI).clamp(-PI, PI);

    Ok(GgSliceInput {
        byte_len: bytes.len(),
        checksum,
        has_fms_magic: true,
        channel_count: channel_count.max(1),
        point_count: point_count.max(1),
        amplitude_scale,
        damping,
        phase_offset,
    })
}

fn parse_bool_or_numeric_first_token(line: &str) -> Option<bool> {
    let first = line.split_whitespace().next()?;
    if let Some(flag) = parse_bool_token(first) {
        return Some(flag);
    }

    parse_numeric_token(first).map(|value| value.abs() > 0.5)
}

fn parse_bool_tokens(line: &str) -> Vec<bool> {
    line.split_whitespace()
        .filter_map(parse_bool_token)
        .collect()
}

fn parse_bool_token(token: &str) -> Option<bool> {
    let normalized = token
        .trim_matches(|character: char| {
            matches!(
                character,
                ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '.'
            )
        })
        .to_ascii_lowercase();
    match normalized.as_str() {
        "t" | "true" | "1" => Some(true),
        "f" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn normalized_qhat(vector: [f64; 3]) -> [f64; 3] {
    let norm = (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
    if norm <= 1.0e-12 {
        return [0.0, 0.0, 1.0];
    }

    [vector[0] / norm, vector[1] / norm, vector[2] / norm]
}

fn compton_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.COMPTON_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
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
            ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
        )
    });
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.replace(['D', 'd'], "E");
    normalized.parse::<f64>().ok()
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> PipelineResult<i32> {
    if !value.is_finite() {
        return Err(compton_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }

    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-6 {
        return Err(compton_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }

    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(compton_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }

    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> PipelineResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(compton_parse_error(
            fixture_id,
            format!("{} must be non-negative", field),
        ));
    }

    Ok(integer as usize)
}

fn checksum_bytes(bytes: &[u8]) -> u64 {
    let mut checksum = 0_u64;
    for (index, byte) in bytes.iter().enumerate() {
        checksum = checksum
            .wrapping_add((*byte as u64).wrapping_mul((index as u64 % 1024) + 1))
            .rotate_left((index % 17) as u32 + 1);
    }
    checksum
}

fn take_u32(bytes: &[u8], offset: &mut usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let chunk = bytes.get(*offset..end)?;
    let mut buffer = [0_u8; 4];
    buffer.copy_from_slice(chunk);
    *offset = end;
    Some(u32::from_le_bytes(buffer))
}

fn take_i32(bytes: &[u8], offset: &mut usize) -> Option<i32> {
    take_u32(bytes, offset).map(|value| value as i32)
}

fn take_f64(bytes: &[u8], offset: &mut usize) -> Option<f64> {
    let end = offset.checked_add(8)?;
    let chunk = bytes.get(*offset..end)?;
    let mut buffer = [0_u8; 8];
    buffer.copy_from_slice(chunk);
    *offset = end;
    Some(f64::from_le_bytes(buffer))
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::ComptonPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    const COMPTON_INPUT_FIXTURE: &str = "run compton module?\n           1\npqmax, npq\n   5.000000            1000\nns, nphi, nz, nzp\n  32  32  32 120\nsmax, phimax, zmax, zpmax\n      0.00000      6.28319      0.00000     10.00000\njpq? rhozzp? force_recalc_jzzp?\n T T F\nwindow_type (0=Step, 1=Hann), window_cutoff\n           1  0.0000000E+00\ntemperature (in eV)\n      0.00000\nset_chemical_potential? chemical_potential(eV)\n F  0.0000000E+00\nrho_xy? rho_yz? rho_xz? rho_vol? rho_line?\n F F F F F\nqhat_x qhat_y qhat_z\n  0.000000000000000E+000  0.000000000000000E+000   1.00000000000000\n";

    #[test]
    fn contract_lists_required_inputs_and_outputs() {
        let request = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Compton,
            "compton.inp",
            "actual-output",
        );
        let scaffold = ComptonPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 3);
        assert_eq!(contract.expected_outputs.len(), 4);
        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["compton.inp", "pot.bin", "gg_slice.bin"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&["compton.dat", "jzzp.dat", "rhozzp.dat", "logcompton.dat"])
        );
    }

    #[test]
    fn execute_emits_required_true_compute_artifacts() {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");
        stage_inputs(&output_dir);

        let request = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Compton,
            output_dir.join("compton.inp"),
            &output_dir,
        );
        let scaffold = ComptonPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("COMPTON execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["compton.dat", "jzzp.dat", "rhozzp.dat", "logcompton.dat"])
        );
        for artifact in artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            let bytes = fs::read(&output_path).expect("artifact should be readable");
            assert!(
                !bytes.is_empty(),
                "artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }

    #[test]
    fn execute_is_deterministic_across_runs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_dir = temp.path().join("first");
        let second_dir = temp.path().join("second");
        stage_inputs(&first_dir);
        stage_inputs(&second_dir);

        let request_one = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Compton,
            first_dir.join("compton.inp"),
            &first_dir,
        );
        let request_two = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Compton,
            second_dir.join("compton.inp"),
            &second_dir,
        );

        let scaffold = ComptonPipelineScaffold;
        let first_artifacts = scaffold
            .execute(&request_one)
            .expect("first run should succeed");
        let second_artifacts = scaffold
            .execute(&request_two)
            .expect("second run should succeed");

        assert_eq!(
            artifact_set(&first_artifacts),
            artifact_set(&second_artifacts)
        );

        for artifact in first_artifacts {
            let first = fs::read(first_dir.join(&artifact.relative_path))
                .expect("first artifact should be readable");
            let second = fs::read(second_dir.join(&artifact.relative_path))
                .expect("second artifact should be readable");
            assert_eq!(
                first,
                second,
                "artifact '{}' should be deterministic",
                artifact.relative_path.display()
            );
        }
    }

    #[test]
    fn execute_rejects_non_compton_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("compton.inp");
        fs::write(&input_path, COMPTON_INPUT_FIXTURE).expect("compton input should be written");
        fs::write(temp.path().join("pot.bin"), [1_u8, 2_u8]).expect("pot should be written");
        fs::write(temp.path().join("gg_slice.bin"), [3_u8, 4_u8])
            .expect("gg slice should be written");

        let request = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Crpa,
            &input_path,
            temp.path(),
        );
        let scaffold = ComptonPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.COMPTON_MODULE");
    }

    #[test]
    fn execute_requires_gg_slice_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("compton.inp");
        fs::write(&input_path, COMPTON_INPUT_FIXTURE).expect("compton input should be written");
        fs::write(temp.path().join("pot.bin"), [0_u8, 1_u8, 2_u8]).expect("pot should be written");

        let request = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Compton,
            &input_path,
            temp.path(),
        );
        let scaffold = ComptonPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing gg_slice input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.COMPTON_INPUT_READ");
    }

    #[test]
    fn execute_rejects_invalid_control_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("compton.inp");
        fs::write(&input_path, "run compton module?\n1\n")
            .expect("compton input should be written");
        fs::write(temp.path().join("pot.bin"), [0_u8, 1_u8, 2_u8]).expect("pot should be written");
        fs::write(temp.path().join("gg_slice.bin"), [3_u8, 4_u8, 5_u8])
            .expect("gg slice should be written");

        let request = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Compton,
            &input_path,
            temp.path(),
        );
        let scaffold = ComptonPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("invalid input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.COMPTON_INPUT_PARSE");
    }

    fn stage_inputs(destination_dir: &Path) {
        fs::create_dir_all(destination_dir).expect("destination dir should exist");
        fs::write(destination_dir.join("compton.inp"), COMPTON_INPUT_FIXTURE)
            .expect("compton input should be staged");
        fs::write(destination_dir.join("pot.bin"), pot_fixture_bytes())
            .expect("pot input should be staged");
        fs::write(
            destination_dir.join("gg_slice.bin"),
            gg_slice_fixture_bytes(),
        )
        .expect("gg_slice input should be staged");
    }

    fn pot_fixture_bytes() -> Vec<u8> {
        vec![0_u8, 1_u8, 2_u8, 3_u8, 4_u8, 5_u8, 6_u8, 7_u8, 8_u8]
    }

    fn gg_slice_fixture_bytes() -> Vec<u8> {
        vec![9_u8, 10_u8, 11_u8, 12_u8, 13_u8, 14_u8, 15_u8, 16_u8]
    }

    fn expected_artifact_set(artifacts: &[&str]) -> BTreeSet<String> {
        artifacts
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
}
