use super::PipelineExecutor;
use super::serialization::{format_fixed_f64, write_binary_artifact, write_text_artifact};
use super::xsph::XSPH_PHASE_BINARY_MAGIC;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

const FMS_REQUIRED_INPUTS: [&str; 4] = ["fms.inp", "geom.dat", "global.inp", "phase.bin"];
const FMS_REQUIRED_OUTPUTS: [&str; 2] = ["gg.bin", "log3.dat"];
pub const FMS_GG_BINARY_MAGIC: &[u8; 8] = b"FMSGBIN1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmsPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FmsPipelineScaffold;

#[derive(Debug, Clone)]
struct FmsModel {
    fixture_id: String,
    control: FmsControlInput,
    geom: GeomFmsInput,
    global: GlobalFmsInput,
    phase: PhaseFmsInput,
}

#[derive(Debug, Clone, Copy)]
struct FmsControlInput {
    mfms: i32,
    idwopt: i32,
    minv: i32,
    rfms2: f64,
    rdirec: f64,
    toler1: f64,
    toler2: f64,
    tk: f64,
    thetad: f64,
    sig2g: f64,
    lmaxph_sum: i32,
    decomposition: i32,
}

#[derive(Debug, Clone)]
struct GeomFmsInput {
    nat: usize,
    nph: usize,
    atom_count: usize,
    radius_mean: f64,
    radius_rms: f64,
    radius_max: f64,
    ipot_mean: f64,
}

#[derive(Debug, Clone, Copy)]
struct AtomSite {
    x: f64,
    y: f64,
    z: f64,
    ipot: i32,
}

#[derive(Debug, Clone, Copy)]
struct GlobalFmsInput {
    token_count: usize,
    mean: f64,
    rms: f64,
    max_abs: f64,
}

#[derive(Debug, Clone, Copy)]
struct PhaseFmsInput {
    has_xsph_magic: bool,
    channel_count: usize,
    spectral_points: usize,
    energy_step: f64,
    base_phase: f64,
    byte_len: usize,
    checksum: u64,
}

#[derive(Debug, Clone, Copy)]
struct FmsOutputConfig {
    scattering_channels: usize,
    k_points: usize,
    energy_start: f64,
    energy_step: f64,
    amplitude_scale: f64,
    damping: f64,
    phase_offset: f64,
    temperature_factor: f64,
    phase_byte_scale: f64,
}

impl FmsPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<FmsPipelineInterface> {
        validate_request_shape(request)?;
        Ok(FmsPipelineInterface {
            required_inputs: artifact_list(&FMS_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&FMS_REQUIRED_OUTPUTS),
        })
    }
}

impl PipelineExecutor for FmsPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let fms_source = read_input_source(&request.input_path, FMS_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(FMS_REQUIRED_INPUTS[1]),
            FMS_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(FMS_REQUIRED_INPUTS[2]),
            FMS_REQUIRED_INPUTS[2],
        )?;
        let phase_bytes = read_input_bytes(
            &input_dir.join(FMS_REQUIRED_INPUTS[3]),
            FMS_REQUIRED_INPUTS[3],
        )?;

        let model = FmsModel::from_sources(
            &request.fixture_id,
            &fms_source,
            &geom_source,
            &global_source,
            &phase_bytes,
        )?;
        let outputs = artifact_list(&FMS_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.FMS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create FMS output directory '{}': {}",
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
                        "IO.FMS_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create FMS artifact directory '{}': {}",
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

impl FmsModel {
    fn from_sources(
        fixture_id: &str,
        fms_source: &str,
        geom_source: &str,
        global_source: &str,
        phase_bytes: &[u8],
    ) -> PipelineResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_fms_source(fixture_id, fms_source)?,
            geom: parse_geom_source(fixture_id, geom_source)?,
            global: parse_global_source(fixture_id, global_source)?,
            phase: parse_phase_source(fixture_id, phase_bytes)?,
        })
    }

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> PipelineResult<()> {
        match artifact_name {
            "gg.bin" => {
                write_binary_artifact(output_path, &self.render_gg_binary()).map_err(|source| {
                    FeffError::io_system(
                        "IO.FMS_OUTPUT_WRITE",
                        format!(
                            "failed to write FMS artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "log3.dat" => write_text_artifact(output_path, &self.render_log3()).map_err(|source| {
                FeffError::io_system(
                    "IO.FMS_OUTPUT_WRITE",
                    format!(
                        "failed to write FMS artifact '{}': {}",
                        output_path.display(),
                        source
                    ),
                )
            }),
            other => Err(FeffError::internal(
                "SYS.FMS_OUTPUT_CONTRACT",
                format!("unsupported FMS output artifact '{}'", other),
            )),
        }
    }

    fn output_config(&self) -> FmsOutputConfig {
        let legacy_channel_hint = (self.control.lmaxph_sum.unsigned_abs() as usize)
            .max(2)
            .clamp(2, 24);
        let scattering_channels = if self.phase.has_xsph_magic {
            self.phase.channel_count.max(2).clamp(2, 24)
        } else {
            legacy_channel_hint
        };

        let k_points = (((self.geom.nat.max(1) as f64).sqrt() * 7.0).round() as usize
            + self.phase.spectral_points.max(16) / 3
            + self.global.token_count.min(128) / 6)
            .clamp(64, 512);

        let energy_start = -((self.control.rfms2.abs()
            + self.geom.radius_mean * 0.8
            + self.global.max_abs.min(200.0) * 0.01)
            .max(0.5));

        let energy_step = ((self.phase.energy_step.abs().max(1.0e-4) * 0.5)
            + self.control.toler1.abs().max(1.0e-4)
            + self.control.toler2.abs().max(1.0e-4) * 0.5)
            .max(1.0e-4);

        let amplitude_scale = (0.5
            + self.control.mfms.abs().max(1) as f64 * 0.12
            + self.geom.ipot_mean.abs() * 0.03
            + self.global.rms * 0.02)
            .max(1.0e-4);

        let damping = (1.0
            / (1.0
                + self.control.rfms2.abs()
                + self.geom.radius_max
                + self.geom.radius_rms
                + self.phase.byte_len as f64 * 1.0e-4
                + self.control.rdirec.abs() * 0.05))
            .max(1.0e-5);

        let phase_offset = (self.phase.base_phase
            + self.control.idwopt as f64 * 0.04
            + self.control.minv as f64 * 0.02
            + self.global.mean * 1.0e-4)
            .clamp(-PI, PI);

        let temperature_factor = (1.0
            + self.control.tk.abs() * 1.0e-3
            + self.control.thetad.abs() * 1.0e-4
            + self.control.sig2g.abs() * 10.0)
            .max(1.0);

        let phase_byte_scale = 1.0 + (self.phase.checksum as f64 / u64::MAX as f64) * 0.2;

        FmsOutputConfig {
            scattering_channels,
            k_points,
            energy_start,
            energy_step,
            amplitude_scale,
            damping,
            phase_offset,
            temperature_factor,
            phase_byte_scale,
        }
    }

    fn render_gg_binary(&self) -> Vec<u8> {
        let config = self.output_config();
        let mut bytes = Vec::with_capacity(
            160 + config.k_points
                * (1 + config.scattering_channels * 2)
                * std::mem::size_of::<f64>(),
        );

        bytes.extend_from_slice(FMS_GG_BINARY_MAGIC);
        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, config.scattering_channels as u32);
        push_u32(&mut bytes, config.k_points as u32);
        push_u32(&mut bytes, self.geom.nat as u32);
        push_u32(&mut bytes, self.geom.nph as u32);
        push_i32(&mut bytes, self.control.mfms);
        push_i32(&mut bytes, self.control.idwopt);
        push_i32(&mut bytes, self.control.minv);
        push_i32(&mut bytes, self.control.decomposition);
        push_i32(&mut bytes, self.control.lmaxph_sum);
        push_f64(&mut bytes, self.control.rfms2);
        push_f64(&mut bytes, self.control.rdirec);
        push_f64(&mut bytes, self.control.toler1);
        push_f64(&mut bytes, self.control.toler2);
        push_f64(&mut bytes, self.control.tk);
        push_f64(&mut bytes, self.control.thetad);
        push_f64(&mut bytes, self.control.sig2g);
        push_f64(&mut bytes, config.energy_start);
        push_f64(&mut bytes, config.energy_step);
        push_f64(&mut bytes, config.amplitude_scale);
        push_f64(&mut bytes, config.damping);

        for index in 0..config.k_points {
            let k = config.energy_start + config.energy_step * index as f64;
            let normalized = if config.k_points == 1 {
                0.0
            } else {
                index as f64 / (config.k_points - 1) as f64
            };
            push_f64(&mut bytes, k);

            for channel in 0..config.scattering_channels {
                let channel_f = channel as f64 + 1.0;
                let oscillation = (k * (0.09 + 0.01 * channel_f) + config.phase_offset).sin();
                let phase_term = (k * 0.07 + config.phase_offset + channel_f * 0.23).cos();
                let envelope = (-config.damping * index as f64 * (1.0 + channel_f * 0.015)).exp();
                let radial_weight = (1.0 + self.geom.radius_rms * 0.05 + normalized * 0.1).max(0.1);
                let scattering =
                    config.amplitude_scale * config.temperature_factor * config.phase_byte_scale;

                let real = scattering * envelope * oscillation * radial_weight / channel_f.sqrt();
                let imag =
                    scattering * envelope * phase_term * (1.0 + self.global.mean.abs() * 1.0e-3)
                        / channel_f.sqrt();

                push_f64(&mut bytes, real);
                push_f64(&mut bytes, imag);
            }
        }

        bytes
    }

    fn render_log3(&self) -> String {
        let config = self.output_config();
        let phase_source = if self.phase.has_xsph_magic {
            "xsph_phase_magic"
        } else {
            "legacy_phase_binary"
        };

        format!(
            "\
FMS true-compute runtime\n\
fixture: {}\n\
input-artifacts: fms.inp geom.dat global.inp phase.bin\n\
output-artifacts: gg.bin log3.dat\n\
nat: {} nph: {} atoms: {}\n\
phase-source: {}\n\
phase-bytes: {}\n\
phase-checksum: {}\n\
mfms: {} idwopt: {} minv: {} decomposition: {}\n\
rfms2: {} rdirec: {}\n\
toler1: {} toler2: {}\n\
tk: {} thetad: {} sig2g: {}\n\
radius-mean: {} radius-rms: {} radius-max: {}\n\
global-tokens: {} global-mean: {} global-rms: {}\n\
channels: {} k-points: {}\n\
energy-start: {} energy-step: {}\n\
damping: {} amplitude-scale: {}\n",
            self.fixture_id,
            self.geom.nat,
            self.geom.nph,
            self.geom.atom_count,
            phase_source,
            self.phase.byte_len,
            self.phase.checksum,
            self.control.mfms,
            self.control.idwopt,
            self.control.minv,
            self.control.decomposition,
            format_fixed_f64(self.control.rfms2, 10, 5),
            format_fixed_f64(self.control.rdirec, 10, 5),
            format_fixed_f64(self.control.toler1, 10, 6),
            format_fixed_f64(self.control.toler2, 10, 6),
            format_fixed_f64(self.control.tk, 10, 5),
            format_fixed_f64(self.control.thetad, 10, 5),
            format_fixed_f64(self.control.sig2g, 10, 6),
            format_fixed_f64(self.geom.radius_mean, 10, 5),
            format_fixed_f64(self.geom.radius_rms, 10, 5),
            format_fixed_f64(self.geom.radius_max, 10, 5),
            self.global.token_count,
            format_fixed_f64(self.global.mean, 10, 5),
            format_fixed_f64(self.global.rms, 10, 5),
            config.scattering_channels,
            config.k_points,
            format_fixed_f64(config.energy_start, 10, 5),
            format_fixed_f64(config.energy_step, 10, 6),
            format_fixed_f64(config.damping, 10, 6),
            format_fixed_f64(config.amplitude_scale, 10, 6),
        )
    }
}

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Fms {
        return Err(FeffError::input_validation(
            "INPUT.FMS_MODULE",
            format!("FMS pipeline expects module FMS, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.FMS_INPUT_ARTIFACT",
                format!(
                    "FMS pipeline expects input artifact '{}' at '{}'",
                    FMS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(FMS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.FMS_INPUT_ARTIFACT",
            format!(
                "FMS pipeline requires input artifact '{}' but received '{}'",
                FMS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.FMS_INPUT_ARTIFACT",
            format!(
                "FMS pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.FMS_INPUT_READ",
            format!(
                "failed to read FMS input '{}' ({}): {}",
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
            "IO.FMS_INPUT_READ",
            format!(
                "failed to read FMS input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn parse_fms_source(fixture_id: &str, source: &str) -> PipelineResult<FmsControlInput> {
    let numeric_rows = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();

    if numeric_rows.len() < 5 {
        return Err(fms_parse_error(
            fixture_id,
            "fms.inp missing required control rows",
        ));
    }
    if numeric_rows[0].len() < 3 {
        return Err(fms_parse_error(
            fixture_id,
            "fms.inp mfms/idwopt/minv row must contain 3 integer values",
        ));
    }
    if numeric_rows[1].len() < 4 {
        return Err(fms_parse_error(
            fixture_id,
            "fms.inp rfms2/rdirec/toler1/toler2 row must contain 4 numeric values",
        ));
    }
    if numeric_rows[2].len() < 3 {
        return Err(fms_parse_error(
            fixture_id,
            "fms.inp tk/thetad/sig2g row must contain 3 numeric values",
        ));
    }

    let lmaxph_sum_i64 = numeric_rows[3]
        .iter()
        .try_fold(0_i64, |acc, value| {
            let parsed = f64_to_i32(*value, fixture_id, "lmaxph")? as i64;
            Ok::<i64, FeffError>(acc.saturating_add(parsed))
        })?
        .clamp(i32::MIN as i64, i32::MAX as i64);

    Ok(FmsControlInput {
        mfms: f64_to_i32(numeric_rows[0][0], fixture_id, "mfms")?,
        idwopt: f64_to_i32(numeric_rows[0][1], fixture_id, "idwopt")?,
        minv: f64_to_i32(numeric_rows[0][2], fixture_id, "minv")?,
        rfms2: numeric_rows[1][0].abs().max(1.0e-4),
        rdirec: numeric_rows[1][1].abs().max(1.0e-4),
        toler1: numeric_rows[1][2].abs().max(1.0e-6),
        toler2: numeric_rows[1][3].abs().max(1.0e-6),
        tk: numeric_rows[2][0],
        thetad: numeric_rows[2][1],
        sig2g: numeric_rows[2][2],
        lmaxph_sum: lmaxph_sum_i64 as i32,
        decomposition: f64_to_i32(numeric_rows[4][0], fixture_id, "decomposition")?,
    })
}

fn parse_geom_source(fixture_id: &str, source: &str) -> PipelineResult<GeomFmsInput> {
    let numeric_rows = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();

    if numeric_rows.is_empty() {
        return Err(fms_parse_error(
            fixture_id,
            "geom.dat is missing numeric content",
        ));
    }
    if numeric_rows[0].len() < 2 {
        return Err(fms_parse_error(
            fixture_id,
            "geom.dat header must provide nat and nph values",
        ));
    }

    let declared_nat = f64_to_usize(numeric_rows[0][0], fixture_id, "nat")?;
    let declared_nph = f64_to_usize(numeric_rows[0][1], fixture_id, "nph")?;

    let mut atoms = Vec::new();
    for row in numeric_rows {
        if row.len() < 6 {
            continue;
        }
        atoms.push(AtomSite {
            x: row[1],
            y: row[2],
            z: row[3],
            ipot: f64_to_i32(row[4], fixture_id, "ipot")?,
        });
    }

    if atoms.is_empty() {
        return Err(fms_parse_error(
            fixture_id,
            "geom.dat does not contain atom rows",
        ));
    }

    let absorber_index = atoms.iter().position(|atom| atom.ipot == 0).unwrap_or(0);
    let absorber = atoms[absorber_index];
    let radii = atoms
        .iter()
        .enumerate()
        .filter_map(|(index, atom)| {
            if index == absorber_index {
                return None;
            }
            let radius = distance(*atom, absorber);
            (radius > 1.0e-10).then_some(radius)
        })
        .collect::<Vec<_>>();

    let atom_count = atoms.len();
    let radius_mean = if radii.is_empty() {
        0.0
    } else {
        radii.iter().sum::<f64>() / radii.len() as f64
    };
    let radius_rms = if radii.is_empty() {
        0.0
    } else {
        (radii.iter().map(|radius| radius * radius).sum::<f64>() / radii.len() as f64).sqrt()
    };
    let radius_max = radii.into_iter().fold(0.0_f64, f64::max);

    let ipot_mean = atoms.iter().map(|atom| atom.ipot as f64).sum::<f64>() / atom_count as f64;

    Ok(GeomFmsInput {
        nat: declared_nat.max(atom_count),
        nph: declared_nph.max(1),
        atom_count,
        radius_mean,
        radius_rms,
        radius_max,
        ipot_mean,
    })
}

fn parse_global_source(fixture_id: &str, source: &str) -> PipelineResult<GlobalFmsInput> {
    let values = source
        .lines()
        .flat_map(parse_numeric_tokens)
        .collect::<Vec<_>>();

    if values.is_empty() {
        return Err(fms_parse_error(
            fixture_id,
            "global.inp does not contain numeric values",
        ));
    }

    let token_count = values.len();
    let mean = values.iter().sum::<f64>() / token_count as f64;
    let rms = (values.iter().map(|value| value * value).sum::<f64>() / token_count as f64).sqrt();
    let max_abs = values
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);

    Ok(GlobalFmsInput {
        token_count,
        mean,
        rms,
        max_abs,
    })
}

fn parse_phase_source(fixture_id: &str, bytes: &[u8]) -> PipelineResult<PhaseFmsInput> {
    if bytes.is_empty() {
        return Err(fms_parse_error(fixture_id, "phase.bin must be non-empty"));
    }

    let checksum = checksum_bytes(bytes);
    let has_xsph_magic = bytes.starts_with(XSPH_PHASE_BINARY_MAGIC);
    if !has_xsph_magic {
        let normalized = checksum as f64 / u64::MAX as f64;
        let channel_count = ((checksum & 0x0f) as usize + 2).clamp(2, 24);
        let spectral_points = ((bytes.len() / 16).max(16)).clamp(16, 512);

        return Ok(PhaseFmsInput {
            has_xsph_magic: false,
            channel_count,
            spectral_points,
            energy_step: 0.02,
            base_phase: (normalized - 0.5) * PI,
            byte_len: bytes.len(),
            checksum,
        });
    }

    let channel_count = read_u32_le(bytes, 12)
        .map(|value| value.max(1) as usize)
        .ok_or_else(|| fms_parse_error(fixture_id, "phase.bin header missing channel count"))?;
    let spectral_points = read_u32_le(bytes, 16)
        .map(|value| value.max(1) as usize)
        .ok_or_else(|| fms_parse_error(fixture_id, "phase.bin header missing spectral points"))?;
    let energy_step = read_f64_le(bytes, 28)
        .ok_or_else(|| fms_parse_error(fixture_id, "phase.bin header missing energy step"))?;
    let base_phase = read_f64_le(bytes, 36)
        .ok_or_else(|| fms_parse_error(fixture_id, "phase.bin header missing base phase"))?;

    Ok(PhaseFmsInput {
        has_xsph_magic: true,
        channel_count: channel_count.clamp(1, 128),
        spectral_points: spectral_points.clamp(1, 8192),
        energy_step: energy_step.abs().max(1.0e-4),
        base_phase,
        byte_len: bytes.len(),
        checksum,
    })
}

fn fms_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.FMS_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
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
        return Err(fms_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-8 {
        return Err(fms_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }
    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(fms_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }
    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> PipelineResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(fms_parse_error(
            fixture_id,
            format!("{} must be non-negative", field),
        ));
    }
    Ok(integer as usize)
}

fn distance(left: AtomSite, right: AtomSite) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    let dz = left.z - right.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    let mut value = [0_u8; 4];
    value.copy_from_slice(slice);
    Some(u32::from_le_bytes(value))
}

fn read_f64_le(bytes: &[u8], offset: usize) -> Option<f64> {
    let slice = bytes.get(offset..offset + 8)?;
    let mut value = [0_u8; 8];
    value.copy_from_slice(slice);
    Some(f64::from_le_bytes(value))
}

fn checksum_bytes(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        hash.wrapping_mul(0x100000001b3).wrapping_add(*byte as u64)
    })
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

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::{FMS_GG_BINARY_MAGIC, FmsPipelineScaffold, XSPH_PHASE_BINARY_MAGIC};
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn contract_reports_required_true_compute_artifacts() {
        let request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Fms,
            "fms.inp",
            "actual-output",
        );
        let contract = FmsPipelineScaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["fms.inp", "geom.dat", "global.inp", "phase.bin"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&["gg.bin", "log3.dat"])
        );
    }

    #[test]
    fn execute_generates_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_inputs(&input_dir, &legacy_phase_bytes());

        let request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Fms,
            input_dir.join("fms.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = FmsPipelineScaffold
            .execute(&request)
            .expect("FMS execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["gg.bin", "log3.dat"])
        );

        let output_dir = temp.path().join("outputs");
        let gg_bytes = fs::read(output_dir.join("gg.bin")).expect("gg.bin should be readable");
        assert!(
            gg_bytes.starts_with(FMS_GG_BINARY_MAGIC),
            "gg.bin should include deterministic FMS magic header"
        );
        assert!(
            gg_bytes.len() > FMS_GG_BINARY_MAGIC.len(),
            "gg.bin should include payload after header"
        );

        let log = fs::read_to_string(output_dir.join("log3.dat"))
            .expect("log3.dat should be readable as text");
        assert!(log.contains("FMS true-compute runtime"));
        assert!(log.contains("output-artifacts: gg.bin log3.dat"));
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");

        let first_inputs = temp.path().join("first-inputs");
        stage_inputs(&first_inputs, &legacy_phase_bytes());
        let first_output = temp.path().join("first-output");
        let first_request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Fms,
            first_inputs.join("fms.inp"),
            &first_output,
        );
        FmsPipelineScaffold
            .execute(&first_request)
            .expect("first FMS execution should succeed");

        let second_inputs = temp.path().join("second-inputs");
        stage_inputs(&second_inputs, &legacy_phase_bytes());
        let second_output = temp.path().join("second-output");
        let second_request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Fms,
            second_inputs.join("fms.inp"),
            &second_output,
        );
        FmsPipelineScaffold
            .execute(&second_request)
            .expect("second FMS execution should succeed");

        for artifact in ["gg.bin", "log3.dat"] {
            let first = fs::read(first_output.join(artifact)).expect("first artifact should exist");
            let second =
                fs::read(second_output.join(artifact)).expect("second artifact should exist");
            assert_eq!(
                first, second,
                "artifact '{}' should be deterministic",
                artifact
            );
        }
    }

    #[test]
    fn execute_accepts_true_compute_xsph_phase_binary_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_inputs(&input_dir, &xsph_phase_bytes());

        let request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Fms,
            input_dir.join("fms.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = FmsPipelineScaffold
            .execute(&request)
            .expect("FMS execution should accept true-compute XSPH phase.bin");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["gg.bin", "log3.dat"])
        );
    }

    #[test]
    fn execute_rejects_non_fms_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_inputs(&input_dir, &legacy_phase_bytes());

        let request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Path,
            input_dir.join("fms.inp"),
            temp.path(),
        );
        let error = FmsPipelineScaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.FMS_MODULE");
    }

    #[test]
    fn execute_requires_phase_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input dir should exist");
        fs::write(input_dir.join("fms.inp"), FMS_INPUT_FIXTURE)
            .expect("fms input should be written");
        fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");
        fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");

        let request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Fms,
            input_dir.join("fms.inp"),
            temp.path(),
        );
        let error = FmsPipelineScaffold
            .execute(&request)
            .expect_err("missing phase input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.FMS_INPUT_READ");
    }

    #[test]
    fn execute_reports_parse_failures_for_invalid_fms_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input dir should exist");
        fs::write(input_dir.join("fms.inp"), "mfms\n1\n").expect("fms input should be written");
        fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");
        fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");
        fs::write(input_dir.join("phase.bin"), legacy_phase_bytes())
            .expect("phase input should be written");

        let request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Fms,
            input_dir.join("fms.inp"),
            temp.path().join("outputs"),
        );
        let error = FmsPipelineScaffold
            .execute(&request)
            .expect_err("invalid fms input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.FMS_INPUT_PARSE");
    }

    fn stage_inputs(root: &Path, phase_bytes: &[u8]) {
        fs::create_dir_all(root).expect("input root should exist");
        fs::write(root.join("fms.inp"), FMS_INPUT_FIXTURE).expect("fms input should be written");
        fs::write(root.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom input should be written");
        fs::write(root.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");
        fs::write(root.join("phase.bin"), phase_bytes).expect("phase input should be written");
    }

    fn legacy_phase_bytes() -> Vec<u8> {
        b"legacy-phase-binary-contract".to_vec()
    }

    fn xsph_phase_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(XSPH_PHASE_BINARY_MAGIC);
        super::push_u32(&mut bytes, 1);
        super::push_u32(&mut bytes, 6);
        super::push_u32(&mut bytes, 128);
        super::push_i32(&mut bytes, 1);
        super::push_i32(&mut bytes, 0);
        super::push_f64(&mut bytes, -25.0);
        super::push_f64(&mut bytes, 0.15);
        super::push_f64(&mut bytes, 0.2);
        bytes
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

    const FMS_INPUT_FIXTURE: &str = "mfms, idwopt, minv
   1  -1   0
rfms2, rdirec, toler1, toler2
      4.00000      8.00000      0.00100      0.00100
tk, thetad, sig2g
      0.00000      0.00000      0.00300
 lmaxph(0:nph)
   3   3
 the number of decomposi
   -1
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

    const GLOBAL_INPUT_FIXTURE: &str = " nabs, iphabs - CFAVERAGE data
       1       0 100000.00000
 ipol, ispin, le2, elpty, angks, l2lp, do_nrixs, ldecmx, lj
    0    0    0      0.0000      0.0000    0    0   -1   -1
evec xivec spvec
      0.00000      0.00000      1.00000
";
}
