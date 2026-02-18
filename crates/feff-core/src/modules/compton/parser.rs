use super::{
    COMPTON_REQUIRED_INPUTS, POT_BINARY_MAGIC, POT_CONTROL_F64_COUNT, POT_CONTROL_I32_COUNT,
};
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use crate::modules::fms::FMS_GG_BINARY_MAGIC;
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub(super) struct ComptonControlInput {
    pub(super) run_enabled: bool,
    pub(super) pqmax: f64,
    pub(super) npq: usize,
    pub(super) ns: usize,
    pub(super) nphi: usize,
    pub(super) nz: usize,
    pub(super) nzp: usize,
    pub(super) smax: f64,
    pub(super) phimax: f64,
    pub(super) zmax: f64,
    pub(super) zpmax: f64,
    pub(super) emit_jzzp: bool,
    pub(super) emit_rhozzp: bool,
    pub(super) force_recalc_jzzp: bool,
    pub(super) window_type: i32,
    pub(super) window_cutoff: f64,
    pub(super) temperature_ev: f64,
    pub(super) set_chemical_potential: bool,
    pub(super) chemical_potential_ev: f64,
    pub(super) rho_components: [bool; 5],
    pub(super) qhat: [f64; 3],
}

impl Default for ComptonControlInput {
    fn default() -> Self {
        Self {
            run_enabled: true,
            pqmax: 5.0,
            npq: 256,
            ns: 32,
            nphi: 32,
            nz: 32,
            nzp: 128,
            smax: 0.0,
            phimax: 2.0 * PI,
            zmax: 0.0,
            zpmax: 10.0,
            emit_jzzp: true,
            emit_rhozzp: true,
            force_recalc_jzzp: false,
            window_type: 0,
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
pub(super) struct PotComptonInput {
    pub(super) byte_len: usize,
    pub(super) checksum: u64,
    pub(super) has_true_compute_magic: bool,
    pub(super) nat: usize,
    pub(super) nph: usize,
    pub(super) npot: usize,
    pub(super) rfms: f64,
    pub(super) zeff_scale: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GgSliceInput {
    pub(super) byte_len: usize,
    pub(super) checksum: u64,
    pub(super) has_fms_magic: bool,
    pub(super) channel_count: usize,
    pub(super) point_count: usize,
    pub(super) amplitude_scale: f64,
    pub(super) damping: f64,
    pub(super) phase_offset: f64,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Compton {
        return Err(FeffError::input_validation(
            "INPUT.COMPTON_MODULE",
            format!("COMPTON module expects COMPTON, got {}", request.module),
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
                    "COMPTON module expects input artifact '{}' at '{}'",
                    COMPTON_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(COMPTON_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.COMPTON_INPUT_ARTIFACT",
            format!(
                "COMPTON module requires input artifact '{}' but received '{}'",
                COMPTON_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.COMPTON_INPUT_ARTIFACT",
            format!(
                "COMPTON module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
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

pub(super) fn read_input_bytes(path: &Path, artifact_name: &str) -> ComputeResult<Vec<u8>> {
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

pub(super) fn parse_compton_source(
    fixture_id: &str,
    source: &str,
) -> ComputeResult<ComptonControlInput> {
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

pub(super) fn parse_pot_source(fixture_id: &str, bytes: &[u8]) -> ComputeResult<PotComptonInput> {
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

fn parse_true_compute_pot_binary(fixture_id: &str, bytes: &[u8]) -> ComputeResult<PotComptonInput> {
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

pub(super) fn parse_gg_slice_source(fixture_id: &str, bytes: &[u8]) -> ComputeResult<GgSliceInput> {
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
) -> ComputeResult<GgSliceInput> {
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

pub(super) fn normalized_qhat(vector: [f64; 3]) -> [f64; 3] {
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

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
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

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
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

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
