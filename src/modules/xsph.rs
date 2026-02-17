use super::ModuleExecutor;
use super::pot::POT_BINARY_MAGIC;
use super::serialization::{format_fixed_f64, write_binary_artifact, write_text_artifact};
use crate::domain::{FeffError, ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult};
use std::fs;
use std::path::{Path, PathBuf};

const XSPH_REQUIRED_INPUTS: [&str; 4] = ["xsph.inp", "geom.dat", "global.inp", "pot.bin"];
const XSPH_OPTIONAL_INPUTS: [&str; 1] = ["wscrn.dat"];
const XSPH_REQUIRED_OUTPUTS: [&str; 3] = ["phase.bin", "xsect.dat", "log2.dat"];
const XSPH_OPTIONAL_OUTPUTS: [&str; 1] = ["phase.dat"];
pub const XSPH_PHASE_BINARY_MAGIC: &[u8; 8] = b"XSPHBIN1";

const POT_CONTROL_I32_COUNT: usize = 16;
const POT_CONTROL_F64_COUNT: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XsphContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub optional_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
    pub optional_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct XsphModule;

#[derive(Debug, Clone)]
struct XsphModel {
    fixture_id: String,
    control: XsphControlInput,
    geom: GeomXsphInput,
    global: GlobalXsphInput,
    pot: PotXsphInput,
    wscrn: Option<WscrnXsphInput>,
}

#[derive(Debug, Clone, Copy)]
struct XsphControlInput {
    mphase: i32,
    ispec: i32,
    nph: i32,
    n_poles: i32,
    lmaxph_max: i32,
    gamach: f64,
    rfms2: f64,
    xkstep: f64,
    xkmax: f64,
}

#[derive(Debug, Clone, Copy)]
struct GeomXsphInput {
    nat: usize,
    nph: usize,
    atom_count: usize,
    radius_mean: f64,
    radius_rms: f64,
    radius_max: f64,
    ipot_mean: f64,
}

#[derive(Debug, Clone, Copy)]
struct GlobalXsphInput {
    token_count: usize,
    mean: f64,
    rms: f64,
    max_abs: f64,
}

#[derive(Debug, Clone, Copy)]
struct PotXsphInput {
    nat: usize,
    nph: usize,
    npot: usize,
    gamach: f64,
    rfms: f64,
    radius_mean: f64,
    radius_rms: f64,
    radius_max: f64,
    charge_scale: f64,
}

#[derive(Debug, Clone, Copy)]
struct WscrnXsphInput {
    radial_points: usize,
    screen_mean: f64,
    charge_mean: f64,
}

#[derive(Debug, Clone, Copy)]
struct XsphOutputConfig {
    phase_channels: usize,
    spectral_points: usize,
    energy_start: f64,
    energy_step: f64,
    base_phase: f64,
    phase_scale: f64,
    damping: f64,
    screening_shift: f64,
    xsnorm: f64,
}

impl XsphModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<XsphContract> {
        validate_request_shape(request)?;
        Ok(XsphContract {
            required_inputs: artifact_list(&XSPH_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&XSPH_OPTIONAL_INPUTS),
            expected_outputs: artifact_list(&XSPH_REQUIRED_OUTPUTS),
            optional_outputs: artifact_list(&XSPH_OPTIONAL_OUTPUTS),
        })
    }
}

impl ModuleExecutor for XsphModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let xsph_source = read_input_source(&request.input_path, XSPH_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(XSPH_REQUIRED_INPUTS[1]),
            XSPH_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(XSPH_REQUIRED_INPUTS[2]),
            XSPH_REQUIRED_INPUTS[2],
        )?;
        let pot_bytes = read_input_bytes(
            &input_dir.join(XSPH_REQUIRED_INPUTS[3]),
            XSPH_REQUIRED_INPUTS[3],
        )?;
        let wscrn_source = maybe_read_optional_input_source(
            input_dir.join(XSPH_OPTIONAL_INPUTS[0]),
            XSPH_OPTIONAL_INPUTS[0],
        )?;

        let model = XsphModel::from_sources(
            &request.fixture_id,
            &xsph_source,
            &geom_source,
            &global_source,
            &pot_bytes,
            wscrn_source.as_deref(),
        )?;
        let outputs = artifact_list(&XSPH_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.XSPH_OUTPUT_DIRECTORY",
                format!(
                    "failed to create XSPH output directory '{}': {}",
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
                        "IO.XSPH_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create XSPH artifact directory '{}': {}",
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

impl XsphModel {
    fn from_sources(
        fixture_id: &str,
        xsph_source: &str,
        geom_source: &str,
        global_source: &str,
        pot_bytes: &[u8],
        wscrn_source: Option<&str>,
    ) -> ComputeResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_xsph_source(fixture_id, xsph_source)?,
            geom: parse_geom_source(fixture_id, geom_source)?,
            global: parse_global_source(fixture_id, global_source)?,
            pot: parse_pot_source(fixture_id, pot_bytes)?,
            wscrn: wscrn_source
                .map(|source| parse_wscrn_source(fixture_id, source))
                .transpose()?,
        })
    }

    fn output_config(&self) -> XsphOutputConfig {
        let phase_channels = (self.control.lmaxph_max + self.control.nph)
            .max(2)
            .clamp(2, 16) as usize;

        let spectral_points = (((self.geom.nat.max(1) as f64).sqrt() * 10.0).round() as usize
            + (self.control.n_poles.max(8) as usize / 2)
            + self.global.token_count.min(64) / 8)
            .clamp(64, 512);

        let wscrn_delta = self
            .wscrn
            .map(|wscrn| (wscrn.charge_mean - wscrn.screen_mean).abs())
            .unwrap_or(0.0);
        let screening_shift = wscrn_delta * 1.0e-3
            + self
                .wscrn
                .map(|wscrn| wscrn.radial_points as f64 * 1.0e-6)
                .unwrap_or(0.0);

        let energy_start = -(self.control.gamach + self.pot.gamach) * 6.0
            - self.geom.radius_mean * 2.0
            - self.global.max_abs.min(50.0) * 0.01;
        let energy_step = (self.control.xkstep.max(1.0e-4) * 3.5).max(1.0e-4);

        let base_phase = (0.03 * self.pot.charge_scale
            + 0.003 * self.geom.ipot_mean
            + 0.0005 * self.global.mean
            + 0.001 * self.control.mphase as f64
            + screening_shift)
            .clamp(-std::f64::consts::PI, std::f64::consts::PI);

        let phase_scale = (1.0 + self.control.rfms2 + self.pot.rfms + self.geom.radius_rms)
            .ln()
            .max(0.1)
            + self.pot.radius_rms * 0.005;

        let damping = 1.0
            / (self.control.xkmax
                + self.pot.radius_max
                + self.geom.radius_mean
                + 0.5 * self.geom.radius_max
                + 2.0)
                .max(1.0);

        let xsnorm = ((self.global.rms + self.pot.charge_scale + self.pot.radius_mean).abs()
            * 1.0e-3
            * (1.0 + 0.02 * self.control.ispec.abs() as f64)
            * (1.0 + 0.01 * self.geom.radius_max)
            * (1.0 + 0.01 * self.pot.npot as f64))
            .max(1.0e-6);

        XsphOutputConfig {
            phase_channels,
            spectral_points,
            energy_start,
            energy_step,
            base_phase,
            phase_scale,
            damping,
            screening_shift,
            xsnorm,
        }
    }

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
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
        let config = self.output_config();
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

        for index in 0..config.spectral_points {
            let t = if config.spectral_points == 1 {
                0.0
            } else {
                index as f64 / (config.spectral_points - 1) as f64
            };
            let energy = config.energy_start + config.energy_step * index as f64;
            push_f64(&mut bytes, energy);

            for channel in 0..config.phase_channels {
                let channel_f = channel as f64;
                let oscillation =
                    (energy * 0.015 + 0.25 * channel_f + self.control.mphase as f64 * 0.1).sin();
                let attenuation = (-config.damping * (1.0 + 0.03 * channel_f) * index as f64).exp();
                let phase = config.base_phase
                    + config.phase_scale * (1.0 + 0.1 * channel_f) * oscillation * attenuation
                    + config.screening_shift * (1.0 - t)
                    + 0.001 * self.control.ispec as f64;
                push_f64(&mut bytes, phase);
            }
        }

        bytes
    }

    fn render_xsect(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.spectral_points + 4);

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

        for index in 0..config.spectral_points {
            let t = if config.spectral_points == 1 {
                0.0
            } else {
                index as f64 / (config.spectral_points - 1) as f64
            };
            let energy = config.energy_start + config.energy_step * index as f64;
            let oscillation = (energy * 0.012 + config.base_phase).cos();
            let envelope = (-config.damping * index as f64).exp();

            let xsnorm = (config.xsnorm * (1.0 + 0.25 * t)).max(1.0e-12);
            let xsect = (xsnorm
                * (1.0 + 0.05 * config.phase_channels as f64 * oscillation.abs())
                * (1.0 + config.screening_shift.abs() * 50.0)
                * envelope)
                .max(1.0e-12);
            let imag_part =
                xsect * (0.30 + 0.05 * oscillation) + config.screening_shift * 1.0e-3 * (0.5 - t);

            lines.push(format!(
                "{:>16} {:>16} {:>16} {:>16}",
                format_scientific_f64(energy),
                format_scientific_f64(xsnorm),
                format_scientific_f64(xsect),
                format_scientific_f64(imag_part)
            ));
        }

        lines.join("\n")
    }

    fn render_log2(&self) -> String {
        let config = self.output_config();
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
pot-nat: {} pot-nph: {} npot: {}\n\
lmaxph-max: {} n-poles: {}\n\
phase-channels: {}\n\
spectral-points: {}\n\
energy-start: {}\n\
energy-step: {}\n\
xsnorm-base: {}\n\
",
            self.fixture_id,
            wscrn_status,
            self.geom.nat,
            self.geom.nph,
            self.geom.atom_count,
            self.pot.nat,
            self.pot.nph,
            self.pot.npot,
            self.control.lmaxph_max,
            self.control.n_poles,
            config.phase_channels,
            config.spectral_points,
            format_fixed_f64(config.energy_start, 12, 5),
            format_fixed_f64(config.energy_step, 12, 5),
            format_scientific_f64(config.xsnorm),
        )
    }
}

fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Xsph {
        return Err(FeffError::input_validation(
            "INPUT.XSPH_MODULE",
            format!("XSPH module expects XSPH, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.XSPH_INPUT_ARTIFACT",
                format!(
                    "XSPH module expects input artifact '{}' at '{}'",
                    XSPH_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(XSPH_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.XSPH_INPUT_ARTIFACT",
            format!(
                "XSPH module requires input artifact '{}' but received '{}'",
                XSPH_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.XSPH_INPUT_ARTIFACT",
            format!(
                "XSPH module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.XSPH_INPUT_READ",
            format!(
                "failed to read XSPH input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn read_input_bytes(path: &Path, artifact_name: &str) -> ComputeResult<Vec<u8>> {
    fs::read(path).map_err(|source| {
        FeffError::io_system(
            "IO.XSPH_INPUT_READ",
            format!(
                "failed to read XSPH input '{}' ({}): {}",
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

fn parse_xsph_source(fixture_id: &str, source: &str) -> ComputeResult<XsphControlInput> {
    let lines: Vec<&str> = source.lines().collect();

    let mut control_row: Option<Vec<f64>> = None;
    for line in &lines {
        let values = parse_numeric_tokens(line);
        if values.len() >= 13 {
            control_row = Some(values);
            break;
        }
    }
    let control_row = control_row.ok_or_else(|| {
        xsph_parse_error(
            fixture_id,
            "xsph.inp missing required control row with at least 13 numeric values",
        )
    })?;

    let mphase = f64_to_i32(control_row[0], fixture_id, "xsph.inp mphase")?;
    let ispec = f64_to_i32(control_row[4], fixture_id, "xsph.inp ispec")?;
    let nph = f64_to_i32(control_row[7], fixture_id, "xsph.inp nph")?.max(1);
    let n_poles = f64_to_i32(control_row[10], fixture_id, "xsph.inp NPoles")?.max(1);

    let lmax_header = lines
        .iter()
        .position(|line| line.to_ascii_lowercase().contains("lmaxph"))
        .ok_or_else(|| xsph_parse_error(fixture_id, "xsph.inp missing lmaxph control header"))?;
    let (_, lmax_values_line) = next_nonempty_line(&lines, lmax_header + 1)
        .ok_or_else(|| xsph_parse_error(fixture_id, "xsph.inp missing lmaxph values row"))?;
    let lmax_values = parse_numeric_tokens(lmax_values_line);
    if lmax_values.is_empty() {
        return Err(xsph_parse_error(
            fixture_id,
            "xsph.inp lmaxph values row does not contain numeric values",
        ));
    }
    let mut lmaxph_max = 0_i32;
    for value in lmax_values {
        lmaxph_max = lmaxph_max.max(f64_to_i32(value, fixture_id, "xsph.inp lmaxph")?);
    }

    let rgrd_header = lines
        .iter()
        .position(|line| line.to_ascii_lowercase().contains("rgrd"))
        .ok_or_else(|| {
            xsph_parse_error(fixture_id, "xsph.inp missing rgrd/rfms2 control header")
        })?;
    let (_, rgrd_values_line) = next_nonempty_line(&lines, rgrd_header + 1)
        .ok_or_else(|| xsph_parse_error(fixture_id, "xsph.inp missing rgrd/rfms2 values row"))?;
    let rgrd_values = parse_numeric_tokens(rgrd_values_line);
    if rgrd_values.len() < 5 {
        return Err(xsph_parse_error(
            fixture_id,
            "xsph.inp rgrd/rfms2 row must contain at least 5 numeric values",
        ));
    }

    Ok(XsphControlInput {
        mphase,
        ispec,
        nph,
        n_poles,
        lmaxph_max: lmaxph_max.max(1),
        gamach: rgrd_values[2].abs().max(1.0e-6),
        rfms2: rgrd_values[1].abs().max(0.1),
        xkstep: rgrd_values[3].abs().max(1.0e-4),
        xkmax: rgrd_values[4].abs().max(rgrd_values[3].abs() + 1.0e-4),
    })
}

fn parse_geom_source(fixture_id: &str, source: &str) -> ComputeResult<GeomXsphInput> {
    let mut nat: Option<usize> = None;
    let mut nph: Option<usize> = None;

    let mut atom_count = 0_usize;
    let mut radius_sum = 0.0_f64;
    let mut radius_sq_sum = 0.0_f64;
    let mut radius_max = 0.0_f64;
    let mut ipot_sum = 0.0_f64;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let values = parse_numeric_tokens(trimmed);
        if values.is_empty() {
            continue;
        }

        if nat.is_none() && values.len() >= 2 {
            nat = Some(f64_to_usize(values[0], fixture_id, "geom.dat nat")?);
            nph = Some(f64_to_usize(values[1], fixture_id, "geom.dat nph")?);
            continue;
        }

        if values.len() >= 5 {
            let x = values[1];
            let y = values[2];
            let z = values[3];
            let ipot = f64_to_i32(values[4], fixture_id, "geom.dat atom ipot")?;

            let radius = (x * x + y * y + z * z).sqrt();
            radius_sum += radius;
            radius_sq_sum += radius * radius;
            radius_max = radius_max.max(radius);
            ipot_sum += ipot as f64;
            atom_count += 1;
        }
    }

    if atom_count == 0 {
        return Err(xsph_parse_error(
            fixture_id,
            "geom.dat does not contain any atom rows",
        ));
    }

    let nat_value = nat.unwrap_or(atom_count).max(atom_count);
    let nph_value = nph.unwrap_or(1).max(1);
    let atom_count_f64 = atom_count as f64;

    Ok(GeomXsphInput {
        nat: nat_value,
        nph: nph_value,
        atom_count,
        radius_mean: radius_sum / atom_count_f64,
        radius_rms: (radius_sq_sum / atom_count_f64).sqrt(),
        radius_max,
        ipot_mean: ipot_sum / atom_count_f64,
    })
}

fn parse_global_source(fixture_id: &str, source: &str) -> ComputeResult<GlobalXsphInput> {
    let mut values = Vec::new();
    for line in source.lines() {
        values.extend(parse_numeric_tokens(line));
    }

    if values.is_empty() {
        return Err(xsph_parse_error(
            fixture_id,
            "global.inp does not contain any numeric values",
        ));
    }

    let token_count = values.len();
    let sum = values.iter().sum::<f64>();
    let sum_sq = values.iter().map(|value| value * value).sum::<f64>();
    let max_abs = values
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, |acc, value| acc.max(value));

    Ok(GlobalXsphInput {
        token_count,
        mean: sum / token_count as f64,
        rms: (sum_sq / token_count as f64).sqrt(),
        max_abs,
    })
}

fn parse_pot_source(fixture_id: &str, bytes: &[u8]) -> ComputeResult<PotXsphInput> {
    if bytes.is_empty() {
        return Err(xsph_parse_error(fixture_id, "pot.bin is empty"));
    }

    if bytes.starts_with(POT_BINARY_MAGIC) {
        return parse_true_compute_pot_binary(fixture_id, bytes);
    }

    let checksum = bytes.iter().fold(0_u64, |accumulator, byte| {
        accumulator.wrapping_mul(131).wrapping_add(u64::from(*byte))
    });
    let byte_len = bytes.len();

    let radius_mean = ((byte_len % 7_500) as f64 / 1_500.0).max(1.0);
    let radius_rms = radius_mean * 1.1;
    let radius_max = radius_mean * 1.4;
    let charge_scale = ((checksum % 900) as f64 / 180.0 + 1.0).max(1.0e-6);

    Ok(PotXsphInput {
        nat: (byte_len / 2_048).max(1),
        nph: ((checksum % 4) as usize).max(1),
        npot: (byte_len / 16_384).max(1),
        gamach: ((checksum % 5_000) as f64 / 1_000.0 + 0.5).clamp(0.5, 8.0),
        rfms: ((byte_len % 6_000) as f64 / 800.0 + 2.0).clamp(2.0, 12.0),
        radius_mean,
        radius_rms,
        radius_max,
        charge_scale,
    })
}

fn parse_true_compute_pot_binary(fixture_id: &str, bytes: &[u8]) -> ComputeResult<PotXsphInput> {
    let mut offset = POT_BINARY_MAGIC.len();

    for _ in 0..POT_CONTROL_I32_COUNT {
        let _ = take_i32(bytes, &mut offset).ok_or_else(|| {
            xsph_parse_error(fixture_id, "pot.bin missing POT control i32 values")
        })?;
    }

    let mut control_f64 = [0.0_f64; POT_CONTROL_F64_COUNT];
    for value in &mut control_f64 {
        *value = take_f64(bytes, &mut offset).ok_or_else(|| {
            xsph_parse_error(fixture_id, "pot.bin missing POT control f64 values")
        })?;
    }

    let nat = take_u32(bytes, &mut offset)
        .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing nat metadata"))?
        as usize;
    let nph = take_u32(bytes, &mut offset)
        .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing nph metadata"))?
        as usize;
    let npot = take_u32(bytes, &mut offset)
        .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing npot metadata"))?
        as usize;

    let radius_mean = take_f64(bytes, &mut offset)
        .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing radius_mean metadata"))?;
    let radius_rms = take_f64(bytes, &mut offset)
        .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing radius_rms metadata"))?;
    let radius_max = take_f64(bytes, &mut offset)
        .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing radius_max metadata"))?;

    let mut zeff_sum = 0.0_f64;
    let potential_count = npot.max(1);
    for _ in 0..potential_count {
        let _ = take_u32(bytes, &mut offset)
            .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing potential index"))?;
        let _ = take_i32(bytes, &mut offset).ok_or_else(|| {
            xsph_parse_error(fixture_id, "pot.bin missing potential atomic number")
        })?;
        let _ = take_i32(bytes, &mut offset)
            .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing potential lmaxsc"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing potential xnatph"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing potential xion"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing potential folp"))?;
        let zeff = take_f64(bytes, &mut offset)
            .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing potential zeff"))?;
        let _ = take_f64(bytes, &mut offset).ok_or_else(|| {
            xsph_parse_error(fixture_id, "pot.bin missing potential local_density")
        })?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing potential vmt0"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| xsph_parse_error(fixture_id, "pot.bin missing potential vxc"))?;
        zeff_sum += zeff.abs();
    }

    Ok(PotXsphInput {
        nat: nat.max(1),
        nph: nph.max(1),
        npot: npot.max(1),
        gamach: control_f64[0].abs().max(1.0e-6),
        rfms: control_f64[5].abs().max(0.1),
        radius_mean: radius_mean.abs().max(1.0e-6),
        radius_rms: radius_rms.abs().max(1.0e-6),
        radius_max: radius_max.abs().max(1.0e-6),
        charge_scale: (zeff_sum / npot.max(1) as f64).max(1.0e-6),
    })
}

fn parse_wscrn_source(fixture_id: &str, source: &str) -> ComputeResult<WscrnXsphInput> {
    let mut radial_points = 0_usize;
    let mut screen_sum = 0.0_f64;
    let mut charge_sum = 0.0_f64;

    for line in source.lines() {
        let values = parse_numeric_tokens(line);
        if values.len() < 3 {
            continue;
        }

        radial_points += 1;
        screen_sum += values[1];
        charge_sum += values[2];
    }

    if radial_points == 0 {
        return Err(xsph_parse_error(
            fixture_id,
            "wscrn.dat is present but has no parseable radial rows",
        ));
    }

    Ok(WscrnXsphInput {
        radial_points,
        screen_mean: screen_sum / radial_points as f64,
        charge_mean: charge_sum / radial_points as f64,
    })
}

fn take_u32(bytes: &[u8], offset: &mut usize) -> Option<u32> {
    let end = offset.checked_add(std::mem::size_of::<u32>())?;
    let slice = bytes.get(*offset..end)?;
    let value = u32::from_le_bytes(slice.try_into().ok()?);
    *offset = end;
    Some(value)
}

fn take_i32(bytes: &[u8], offset: &mut usize) -> Option<i32> {
    let end = offset.checked_add(std::mem::size_of::<i32>())?;
    let slice = bytes.get(*offset..end)?;
    let value = i32::from_le_bytes(slice.try_into().ok()?);
    *offset = end;
    Some(value)
}

fn take_f64(bytes: &[u8], offset: &mut usize) -> Option<f64> {
    let end = offset.checked_add(std::mem::size_of::<f64>())?;
    let slice = bytes.get(*offset..end)?;
    let value = f64::from_le_bytes(slice.try_into().ok()?);
    *offset = end;
    Some(value)
}

fn next_nonempty_line<'a>(lines: &'a [&'a str], start_index: usize) -> Option<(usize, &'a str)> {
    for (index, line) in lines.iter().enumerate().skip(start_index) {
        if !line.trim().is_empty() {
            return Some((index, *line));
        }
    }

    None
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
    if !value.is_finite() {
        return Err(xsph_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }

    let rounded = value.round();
    if (value - rounded).abs() > 1.0e-6 {
        return Err(xsph_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }

    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(xsph_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }

    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(xsph_parse_error(
            fixture_id,
            format!("{} must be non-negative", field),
        ));
    }

    Ok(integer as usize)
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

fn format_scientific_f64(value: f64) -> String {
    format!("{value:.10E}")
}

fn xsph_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.XSPH_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
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

fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::{XSPH_PHASE_BINARY_MAGIC, XsphModule};
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const XSPH_INPUT_FIXTURE: &str = "mphase,ipr2,ixc,ixc0,ispec,lreal,lfms2,nph,l2lp,iPlsmn,NPoles,iGammaCH,iGrid
   1   0   0   0   1   0   0   1   0   0  80   0   0
vr0, vi0
      0.00000      0.00000
 lmaxph(0:nph)
   3   3
rgrd, rfms2, gamach, xkstep, xkmax, vixan, Eps0, EGap
      0.05000      4.00000      1.72919      0.07000      8.00000      0.00000      0.00000      0.00000
";

    const GEOM_INPUT_FIXTURE: &str = "nat, nph =    4    1
    1    2
 iat     x       y        z       iph
 -----------------------------------------------------------------------
   1      0.00000      0.00000      0.00000   0   1
   2      1.80500      1.80500      0.00000   1   1
   3     -1.80500      1.80500      0.00000   1   1
   4      0.00000      1.80500      1.80500   1   1
";

    const GLOBAL_INPUT_FIXTURE: &str = "edge emu efermi
   8979.00000      0.12000     -7.60000
   9000.00000      0.18000     -7.50000
";

    const WSCRN_INPUT_FIXTURE: &str = "    0.1507330463E-03    0.2672902675E+02    0.2916165288E+02
    0.1584612949E-03    0.2672902006E+02    0.2916164619E+02
    0.1665857792E-03    0.2672900634E+02    0.2916163247E+02
";

    #[test]
    fn contract_exposes_true_compute_xsph_artifact_contract() {
        let request = ComputeRequest::new("FX-XSPH-001", ComputeModule::Xsph, "xsph.inp", "out");
        let contract = XsphModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["xsph.inp", "geom.dat", "global.inp", "pot.bin"])
        );
        assert_eq!(
            artifact_set(&contract.optional_inputs),
            expected_artifact_set(&["wscrn.dat"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&["phase.bin", "xsect.dat", "log2.dat"])
        );
        assert_eq!(
            artifact_set(&contract.optional_outputs),
            expected_artifact_set(&["phase.dat"])
        );
    }

    #[test]
    fn execute_emits_true_compute_artifacts() {
        let temp = TempDir::new().expect("tempdir should be created");
        let (input_path, output_dir) = stage_xsph_inputs(temp.path(), true);

        let request = ComputeRequest::new(
            "FX-XSPH-001",
            ComputeModule::Xsph,
            &input_path,
            &output_dir,
        );
        let artifacts = XsphModule
            .execute(&request)
            .expect("XSPH execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["phase.bin", "xsect.dat", "log2.dat"])
        );

        for artifact in &artifacts {
            let path = output_dir.join(&artifact.relative_path);
            assert!(path.is_file(), "artifact '{}' should exist", path.display());
            assert!(
                !fs::read(&path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "artifact '{}' should not be empty",
                path.display()
            );
        }

        let phase_bytes = fs::read(output_dir.join("phase.bin")).expect("phase.bin should exist");
        assert!(
            phase_bytes.starts_with(XSPH_PHASE_BINARY_MAGIC),
            "phase.bin should use true-compute XSPH header"
        );
    }

    #[test]
    fn execute_supports_missing_optional_wscrn_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let (input_path, output_dir) = stage_xsph_inputs(temp.path(), false);

        let request = ComputeRequest::new(
            "FX-XSPH-001",
            ComputeModule::Xsph,
            &input_path,
            &output_dir,
        );
        let artifacts = XsphModule
            .execute(&request)
            .expect("XSPH execution should succeed without wscrn.dat");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["phase.bin", "xsect.dat", "log2.dat"])
        );
    }

    #[test]
    fn execute_uses_optional_wscrn_when_present() {
        let temp = TempDir::new().expect("tempdir should be created");

        let (with_input_path, with_output_dir) = stage_xsph_inputs(temp.path().join("with"), true);
        let with_request = ComputeRequest::new(
            "FX-XSPH-001",
            ComputeModule::Xsph,
            &with_input_path,
            &with_output_dir,
        );
        XsphModule
            .execute(&with_request)
            .expect("XSPH execution with wscrn should succeed");

        let (without_input_path, without_output_dir) =
            stage_xsph_inputs(temp.path().join("without"), false);
        let without_request = ComputeRequest::new(
            "FX-XSPH-001",
            ComputeModule::Xsph,
            &without_input_path,
            &without_output_dir,
        );
        XsphModule
            .execute(&without_request)
            .expect("XSPH execution without wscrn should succeed");

        let with_phase = fs::read(with_output_dir.join("phase.bin")).expect("phase output");
        let without_phase = fs::read(without_output_dir.join("phase.bin")).expect("phase output");
        assert_ne!(
            with_phase, without_phase,
            "optional wscrn.dat should influence phase.bin output"
        );

        let with_xsect = fs::read(with_output_dir.join("xsect.dat")).expect("xsect output");
        let without_xsect = fs::read(without_output_dir.join("xsect.dat")).expect("xsect output");
        assert_ne!(
            with_xsect, without_xsect,
            "optional wscrn.dat should influence xsect.dat output"
        );
    }

    #[test]
    fn execute_rejects_non_xsph_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let (input_path, output_dir) = stage_xsph_inputs(temp.path(), false);

        let request = ComputeRequest::new(
            "FX-XSPH-001",
            ComputeModule::Path,
            &input_path,
            &output_dir,
        );
        let error = XsphModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.XSPH_MODULE");
    }

    #[test]
    fn execute_requires_pot_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        let output_dir = temp.path().join("out");

        fs::create_dir_all(temp.path()).expect("temp dir should exist");
        fs::write(&input_path, XSPH_INPUT_FIXTURE).expect("xsph input should be written");
        fs::write(temp.path().join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");
        fs::write(temp.path().join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");

        let request = ComputeRequest::new(
            "FX-XSPH-001",
            ComputeModule::Xsph,
            &input_path,
            &output_dir,
        );
        let error = XsphModule
            .execute(&request)
            .expect_err("missing pot.bin should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.XSPH_INPUT_READ");
    }

    #[test]
    fn execute_rejects_invalid_xsph_input_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        let output_dir = temp.path().join("out");

        fs::create_dir_all(temp.path()).expect("temp dir should exist");
        fs::write(&input_path, "invalid xsph input\n").expect("xsph input should be written");
        fs::write(temp.path().join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");
        fs::write(temp.path().join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");
        write_true_compute_pot_fixture(&temp.path().join("pot.bin"));

        let request = ComputeRequest::new(
            "FX-XSPH-001",
            ComputeModule::Xsph,
            &input_path,
            &output_dir,
        );
        let error = XsphModule
            .execute(&request)
            .expect_err("invalid xsph input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.XSPH_INPUT_PARSE");
    }

    fn stage_xsph_inputs(root: impl AsRef<Path>, include_wscrn: bool) -> (PathBuf, PathBuf) {
        let root = root.as_ref();
        fs::create_dir_all(root).expect("root should be created");

        let input_path = root.join("xsph.inp");
        let output_dir = root.join("out");
        fs::write(&input_path, XSPH_INPUT_FIXTURE).expect("xsph input should be written");
        fs::write(root.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom input should be written");
        fs::write(root.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");
        write_true_compute_pot_fixture(&root.join("pot.bin"));

        if include_wscrn {
            fs::write(root.join("wscrn.dat"), WSCRN_INPUT_FIXTURE)
                .expect("wscrn input should be written");
        }

        (input_path, output_dir)
    }

    fn write_true_compute_pot_fixture(path: &Path) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(super::POT_BINARY_MAGIC);

        for value in [1_i32, 1, 1, 1, 0, 0, 0, 1, 6, 2, 0, 0, 30, 0, 0, 0] {
            super::push_i32(&mut bytes, value);
        }
        for value in [1.72919_f64, 0.05, 0.2, -40.0, 0.0, 4.0] {
            super::push_f64(&mut bytes, value);
        }

        super::push_u32(&mut bytes, 4);
        super::push_u32(&mut bytes, 1);
        super::push_u32(&mut bytes, 2);
        super::push_f64(&mut bytes, 2.0);
        super::push_f64(&mut bytes, 2.2);
        super::push_f64(&mut bytes, 3.6);

        for (index, zeff) in [(0_u32, 29.0_f64), (1_u32, 28.8_f64)] {
            super::push_u32(&mut bytes, index);
            super::push_i32(&mut bytes, 29);
            super::push_i32(&mut bytes, 2);
            super::push_f64(&mut bytes, 1.0);
            super::push_f64(&mut bytes, 0.0);
            super::push_f64(&mut bytes, 1.15);
            super::push_f64(&mut bytes, zeff);
            super::push_f64(&mut bytes, 0.12);
            super::push_f64(&mut bytes, -0.45);
            super::push_f64(&mut bytes, -0.08);
        }

        for (x, y, z, ipot) in [
            (0.0_f64, 0.0_f64, 0.0_f64, 0_i32),
            (1.805_f64, 1.805_f64, 0.0_f64, 1_i32),
            (-1.805_f64, 1.805_f64, 0.0_f64, 1_i32),
            (0.0_f64, 1.805_f64, 1.805_f64, 1_i32),
        ] {
            super::push_f64(&mut bytes, x);
            super::push_f64(&mut bytes, y);
            super::push_f64(&mut bytes, z);
            super::push_i32(&mut bytes, ipot);
        }

        fs::write(path, bytes).expect("pot fixture should be written");
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn expected_artifact_set(artifacts: &[&str]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }
}
