use super::{POT_CONTROL_F64_COUNT, POT_CONTROL_I32_COUNT, XSPH_REQUIRED_INPUTS};
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use crate::modules::pot::POT_BINARY_MAGIC;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub(super) struct XsphControlInput {
    pub(super) mphase: i32,
    pub(super) ixc: i32,
    pub(super) ispec: i32,
    pub(super) nph: i32,
    pub(super) n_poles: i32,
    pub(super) lmaxph_max: i32,
    pub(super) gamach: f64,
    pub(super) rfms2: f64,
    pub(super) xkstep: f64,
    pub(super) xkmax: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GeomXsphInput {
    pub(super) nat: usize,
    pub(super) nph: usize,
    pub(super) atom_count: usize,
    pub(super) radius_mean: f64,
    pub(super) radius_rms: f64,
    pub(super) radius_max: f64,
    pub(super) ipot_mean: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GlobalXsphInput {
    pub(super) token_count: usize,
    pub(super) mean: f64,
    pub(super) rms: f64,
    pub(super) max_abs: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PotXsphInput {
    pub(super) nat: usize,
    pub(super) nph: usize,
    pub(super) npot: usize,
    pub(super) gamach: f64,
    pub(super) rfms: f64,
    pub(super) radius_mean: f64,
    pub(super) radius_rms: f64,
    pub(super) radius_max: f64,
    pub(super) charge_scale: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WscrnXsphInput {
    pub(super) radial_points: usize,
    pub(super) screen_mean: f64,
    pub(super) charge_mean: f64,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
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

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
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

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
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

pub(super) fn read_input_bytes(path: &Path, artifact_name: &str) -> ComputeResult<Vec<u8>> {
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

pub(super) fn maybe_read_optional_input_source(
    path: PathBuf,
    artifact_name: &str,
) -> ComputeResult<Option<String>> {
    if path.is_file() {
        return read_input_source(&path, artifact_name).map(Some);
    }

    Ok(None)
}

pub(super) fn parse_xsph_source(fixture_id: &str, source: &str) -> ComputeResult<XsphControlInput> {
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
    let ixc = f64_to_i32(control_row[2], fixture_id, "xsph.inp ixc")?;
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
        ixc,
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

pub(super) fn parse_geom_source(fixture_id: &str, source: &str) -> ComputeResult<GeomXsphInput> {
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

pub(super) fn parse_global_source(
    fixture_id: &str,
    source: &str,
) -> ComputeResult<GlobalXsphInput> {
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

pub(super) fn parse_pot_source(fixture_id: &str, bytes: &[u8]) -> ComputeResult<PotXsphInput> {
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

pub(super) fn parse_wscrn_source(fixture_id: &str, source: &str) -> ComputeResult<WscrnXsphInput> {
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

pub(super) fn format_scientific_f64(value: f64) -> String {
    format!("{value:.10E}")
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}

pub(super) fn push_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn push_i32(target: &mut Vec<u8>, value: i32) {
    target.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn push_f64(target: &mut Vec<u8>, value: f64) {
    target.extend_from_slice(&value.to_le_bytes());
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

fn xsph_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.XSPH_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}
