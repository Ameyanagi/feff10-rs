use super::{BAND_REQUIRED_INPUTS};
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use crate::modules::xsph::XSPH_PHASE_BINARY_MAGIC;
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub(super) struct BandControlInput {
    pub(super) mband: i32,
    pub(super) emin: f64,
    pub(super) emax: f64,
    pub(super) estep: f64,
    pub(super) nkp: i32,
    pub(super) ikpath: i32,
    pub(super) freeprop: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GeomBandInput {
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
pub(super) struct GlobalBandInput {
    pub(super) token_count: usize,
    pub(super) mean: f64,
    pub(super) rms: f64,
    pub(super) max_abs: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PhaseBandInput {
    pub(super) has_xsph_magic: bool,
    pub(super) channel_count: usize,
    pub(super) spectral_points: usize,
    pub(super) energy_start: f64,
    pub(super) energy_step: f64,
    pub(super) base_phase: f64,
    pub(super) byte_len: usize,
    pub(super) checksum: u64,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Band {
        return Err(FeffError::input_validation(
            "INPUT.BAND_MODULE",
            format!("BAND module expects BAND, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.BAND_INPUT_ARTIFACT",
                format!(
                    "BAND module expects input artifact '{}' at '{}'",
                    BAND_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(BAND_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.BAND_INPUT_ARTIFACT",
            format!(
                "BAND module requires input artifact '{}' but received '{}'",
                BAND_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.BAND_INPUT_ARTIFACT",
            format!(
                "BAND module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.BAND_INPUT_READ",
            format!(
                "failed to read BAND input '{}' ({}): {}",
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
            "IO.BAND_INPUT_READ",
            format!(
                "failed to read BAND input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

pub(super) fn parse_band_source(fixture_id: &str, source: &str) -> ComputeResult<BandControlInput> {
    let lines = source.lines().collect::<Vec<_>>();

    let mband_row = marker_following_numeric_row(&lines, "mband").ok_or_else(|| {
        band_parse_error(
            fixture_id,
            "band.inp missing mband control row after 'mband' marker",
        )
    })?;
    let energy_row = marker_following_numeric_row(&lines, "emin").ok_or_else(|| {
        band_parse_error(
            fixture_id,
            "band.inp missing energy mesh row after 'emin' marker",
        )
    })?;
    let nkp_row = marker_following_numeric_row(&lines, "nkp").ok_or_else(|| {
        band_parse_error(fixture_id, "band.inp missing nkp row after 'nkp' marker")
    })?;
    let ikpath_row = marker_following_numeric_row(&lines, "ikpath").ok_or_else(|| {
        band_parse_error(
            fixture_id,
            "band.inp missing ikpath row after 'ikpath' marker",
        )
    })?;

    if energy_row.len() < 3 {
        return Err(band_parse_error(
            fixture_id,
            "band.inp energy mesh row must contain emin, emax, and estep",
        ));
    }

    let freeprop = marker_following_bool_token(&lines, "freeprop").unwrap_or(false);

    Ok(BandControlInput {
        mband: f64_to_i32(mband_row[0], fixture_id, "mband")?,
        emin: energy_row[0],
        emax: energy_row[1],
        estep: energy_row[2].abs(),
        nkp: f64_to_i32(nkp_row[0], fixture_id, "nkp")?,
        ikpath: f64_to_i32(ikpath_row[0], fixture_id, "ikpath")?,
        freeprop,
    })
}

pub(super) fn parse_geom_source(fixture_id: &str, source: &str) -> ComputeResult<GeomBandInput> {
    let numeric_rows = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();

    if numeric_rows.is_empty() {
        return Err(band_parse_error(
            fixture_id,
            "geom.dat is missing numeric content",
        ));
    }
    if numeric_rows[0].len() < 2 {
        return Err(band_parse_error(
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
        return Err(band_parse_error(
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

    Ok(GeomBandInput {
        nat: declared_nat.max(atom_count),
        nph: declared_nph.max(1),
        atom_count,
        radius_mean,
        radius_rms,
        radius_max,
        ipot_mean,
    })
}

pub(super) fn parse_global_source(fixture_id: &str, source: &str) -> ComputeResult<GlobalBandInput> {
    let values = source
        .lines()
        .flat_map(parse_numeric_tokens)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err(band_parse_error(
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

    Ok(GlobalBandInput {
        token_count,
        mean,
        rms,
        max_abs,
    })
}

pub(super) fn parse_phase_source(fixture_id: &str, bytes: &[u8]) -> ComputeResult<PhaseBandInput> {
    if bytes.is_empty() {
        return Err(band_parse_error(fixture_id, "phase.bin must be non-empty"));
    }

    let checksum = checksum_bytes(bytes);
    let has_xsph_magic = bytes.starts_with(XSPH_PHASE_BINARY_MAGIC);
    if !has_xsph_magic {
        let normalized = checksum as f64 / u64::MAX as f64;
        let channel_count = ((checksum & 0x1f) as usize + 2).clamp(2, 32);
        let spectral_points = ((bytes.len() / 16).max(16)).clamp(16, 4096);

        return Ok(PhaseBandInput {
            has_xsph_magic: false,
            channel_count,
            spectral_points,
            energy_start: -20.0 + normalized * 10.0,
            energy_step: 0.05 + (bytes.len() % 1024) as f64 * 1.0e-5,
            base_phase: (normalized - 0.5) * PI,
            byte_len: bytes.len(),
            checksum,
        });
    }

    let channel_count = read_u32_le(bytes, 12)
        .map(|value| value.max(1) as usize)
        .ok_or_else(|| band_parse_error(fixture_id, "phase.bin header missing channel count"))?;
    let spectral_points = read_u32_le(bytes, 16)
        .map(|value| value.max(1) as usize)
        .ok_or_else(|| band_parse_error(fixture_id, "phase.bin header missing spectral points"))?;
    let energy_start = read_f64_le(bytes, 28)
        .ok_or_else(|| band_parse_error(fixture_id, "phase.bin header missing energy start"))?;
    let energy_step = read_f64_le(bytes, 36)
        .ok_or_else(|| band_parse_error(fixture_id, "phase.bin header missing energy step"))?;
    let base_phase = read_f64_le(bytes, 44)
        .ok_or_else(|| band_parse_error(fixture_id, "phase.bin header missing base phase"))?;

    Ok(PhaseBandInput {
        has_xsph_magic: true,
        channel_count: channel_count.clamp(1, 128),
        spectral_points: spectral_points.clamp(1, 8192),
        energy_start,
        energy_step: energy_step.abs().max(1.0e-4),
        base_phase,
        byte_len: bytes.len(),
        checksum,
    })
}

fn band_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.BAND_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

fn marker_following_numeric_row(lines: &[&str], marker: &str) -> Option<Vec<f64>> {
    let marker_index = lines.iter().position(|line| {
        line.to_ascii_lowercase()
            .contains(&marker.to_ascii_lowercase())
    })?;

    lines
        .iter()
        .skip(marker_index + 1)
        .map(|line| parse_numeric_tokens(line))
        .find(|row| !row.is_empty())
}

fn marker_following_bool_token(lines: &[&str], marker: &str) -> Option<bool> {
    let marker_index = lines.iter().position(|line| {
        line.to_ascii_lowercase()
            .contains(&marker.to_ascii_lowercase())
    })?;

    for line in lines.iter().skip(marker_index + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        for token in trimmed.split_whitespace() {
            let normalized = token.trim_matches(|character: char| {
                matches!(
                    character,
                    ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
                )
            });
            if normalized.eq_ignore_ascii_case("t") || normalized.eq_ignore_ascii_case("true") {
                return Some(true);
            }
            if normalized.eq_ignore_ascii_case("f") || normalized.eq_ignore_ascii_case("false") {
                return Some(false);
            }
        }

        let numeric = parse_numeric_tokens(trimmed);
        if let Some(value) = numeric.first() {
            return Some(*value != 0.0);
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

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
    if !value.is_finite() {
        return Err(band_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-8 {
        return Err(band_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }
    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(band_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }
    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(band_parse_error(
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
    let mut buffer = [0_u8; 4];
    buffer.copy_from_slice(slice);
    Some(u32::from_le_bytes(buffer))
}

fn read_f64_le(bytes: &[u8], offset: usize) -> Option<f64> {
    let slice = bytes.get(offset..offset + 8)?;
    let mut buffer = [0_u8; 8];
    buffer.copy_from_slice(slice);
    Some(f64::from_le_bytes(buffer))
}

fn checksum_bytes(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
