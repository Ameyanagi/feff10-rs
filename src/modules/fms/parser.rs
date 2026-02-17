use super::FMS_REQUIRED_INPUTS;
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use crate::modules::xsph::XSPH_PHASE_BINARY_MAGIC;
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub(super) struct FmsControlInput {
    pub(super) mfms: i32,
    pub(super) idwopt: i32,
    pub(super) minv: i32,
    pub(super) rfms2: f64,
    pub(super) rdirec: f64,
    pub(super) toler1: f64,
    pub(super) toler2: f64,
    pub(super) tk: f64,
    pub(super) thetad: f64,
    pub(super) sig2g: f64,
    pub(super) lmaxph_sum: i32,
    pub(super) decomposition: i32,
}

#[derive(Debug, Clone)]
pub(super) struct GeomFmsInput {
    pub(super) nat: usize,
    pub(super) nph: usize,
    pub(super) atom_count: usize,
    pub(super) radius_mean: f64,
    pub(super) radius_rms: f64,
    pub(super) radius_max: f64,
    pub(super) ipot_mean: f64,
}

#[derive(Debug, Clone, Copy)]
struct AtomSite {
    x: f64,
    y: f64,
    z: f64,
    ipot: i32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GlobalFmsInput {
    pub(super) token_count: usize,
    pub(super) mean: f64,
    pub(super) rms: f64,
    pub(super) max_abs: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PhaseFmsInput {
    pub(super) has_xsph_magic: bool,
    pub(super) channel_count: usize,
    pub(super) spectral_points: usize,
    pub(super) energy_step: f64,
    pub(super) base_phase: f64,
    pub(super) byte_len: usize,
    pub(super) checksum: u64,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Fms {
        return Err(FeffError::input_validation(
            "INPUT.FMS_MODULE",
            format!("FMS module expects FMS, got {}", request.module),
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
                    "FMS module expects input artifact '{}' at '{}'",
                    FMS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(FMS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.FMS_INPUT_ARTIFACT",
            format!(
                "FMS module requires input artifact '{}' but received '{}'",
                FMS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.FMS_INPUT_ARTIFACT",
            format!(
                "FMS module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
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

pub(super) fn read_input_bytes(path: &Path, artifact_name: &str) -> ComputeResult<Vec<u8>> {
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

pub(super) fn parse_fms_source(fixture_id: &str, source: &str) -> ComputeResult<FmsControlInput> {
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

pub(super) fn parse_geom_source(fixture_id: &str, source: &str) -> ComputeResult<GeomFmsInput> {
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

pub(super) fn parse_global_source(fixture_id: &str, source: &str) -> ComputeResult<GlobalFmsInput> {
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

pub(super) fn parse_phase_source(fixture_id: &str, bytes: &[u8]) -> ComputeResult<PhaseFmsInput> {
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

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
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

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
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

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
