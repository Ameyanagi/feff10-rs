use super::{LDOS_REQUIRED_INPUTS, POT_BINARY_MAGIC, POT_CONTROL_F64_COUNT, POT_CONTROL_I32_COUNT};
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct LdosControlInput {
    pub(super) mldos_enabled: bool,
    pub(super) neldos: usize,
    pub(super) rfms2: f64,
    pub(super) emin: f64,
    pub(super) emax: f64,
    pub(super) eimag: f64,
    pub(super) rgrd: f64,
    pub(super) rdirec: f64,
    pub(super) toler1: f64,
    pub(super) toler2: f64,
    pub(super) lmaxph: Vec<i32>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GeomLdosInput {
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
pub(super) struct PotLdosInput {
    pub(super) nat: usize,
    pub(super) nph: usize,
    pub(super) npot: usize,
    pub(super) rfms: f64,
    pub(super) radius_mean: f64,
    pub(super) radius_rms: f64,
    pub(super) radius_max: f64,
    pub(super) charge_scale: f64,
    pub(super) checksum: u64,
    pub(super) has_true_compute_magic: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ReciprocalLdosInput {
    pub(super) ispace: i32,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Ldos {
        return Err(FeffError::input_validation(
            "INPUT.LDOS_MODULE",
            format!("LDOS module expects LDOS, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.LDOS_INPUT_ARTIFACT",
                format!(
                    "LDOS module expects input artifact '{}' at '{}'",
                    LDOS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(LDOS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.LDOS_INPUT_ARTIFACT",
            format!(
                "LDOS module requires input artifact '{}' but received '{}'",
                LDOS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.LDOS_INPUT_ARTIFACT",
            format!(
                "LDOS module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.LDOS_INPUT_READ",
            format!(
                "failed to read LDOS input '{}' ({}): {}",
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
            "IO.LDOS_INPUT_READ",
            format!(
                "failed to read LDOS input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

pub(super) fn parse_ldos_source(fixture_id: &str, source: &str) -> ComputeResult<LdosControlInput> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut mldos_enabled: Option<bool> = None;
    let mut neldos: Option<usize> = None;
    let mut rfms2: Option<f64> = None;
    let mut emin: Option<f64> = None;
    let mut emax: Option<f64> = None;
    let mut eimag: Option<f64> = None;
    let mut rgrd: Option<f64> = None;
    let mut rdirec: Option<f64> = None;
    let mut toler1: Option<f64> = None;
    let mut toler2: Option<f64> = None;
    let mut lmaxph: Option<Vec<i32>> = None;

    for index in 0..lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("mldos") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 5 {
                    mldos_enabled = Some(f64_to_i32(values[0], fixture_id, "ldos.inp mldos")? != 0);
                    neldos = Some(if values.len() >= 6 {
                        f64_to_usize(values[5], fixture_id, "ldos.inp neldos")?
                    } else {
                        101
                    });
                }
            }
            continue;
        }

        if lower.starts_with("rfms2") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 5 {
                    rfms2 = Some(values[0]);
                    emin = Some(values[1]);
                    emax = Some(values[2]);
                    eimag = Some(values[3]);
                    rgrd = Some(values[4].abs());
                }
            }
            continue;
        }

        if lower.starts_with("rdirec") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 3 {
                    rdirec = Some(values[0].abs());
                    toler1 = Some(values[1].abs());
                    toler2 = Some(values[2].abs());
                }
            }
            continue;
        }

        if lower.contains("lmaxph")
            && let Some((_, values_line)) = next_nonempty_line(&lines, index + 1)
        {
            let values = parse_numeric_tokens(values_line);
            if !values.is_empty() {
                let mut parsed = Vec::with_capacity(values.len());
                for value in values {
                    parsed.push(f64_to_i32(value, fixture_id, "ldos.inp lmaxph")?.max(0));
                }
                lmaxph = Some(parsed);
            }
        }
    }

    let neldos = neldos.ok_or_else(|| {
        ldos_parse_error(
            fixture_id,
            "ldos.inp missing neldos in mldos/lfms2 control block",
        )
    })?;

    Ok(LdosControlInput {
        mldos_enabled: mldos_enabled.unwrap_or(true),
        neldos: neldos.max(1),
        rfms2: rfms2.ok_or_else(|| {
            ldos_parse_error(
                fixture_id,
                "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
            )
        })?,
        emin: emin.ok_or_else(|| {
            ldos_parse_error(
                fixture_id,
                "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
            )
        })?,
        emax: emax.ok_or_else(|| {
            ldos_parse_error(
                fixture_id,
                "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
            )
        })?,
        eimag: eimag.ok_or_else(|| {
            ldos_parse_error(
                fixture_id,
                "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
            )
        })?,
        rgrd: rgrd
            .ok_or_else(|| {
                ldos_parse_error(
                    fixture_id,
                    "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
                )
            })?
            .max(1.0e-6),
        rdirec: rdirec.unwrap_or(12.0),
        toler1: toler1.unwrap_or(1.0e-3),
        toler2: toler2.unwrap_or(1.0e-3),
        lmaxph: lmaxph.unwrap_or_else(|| vec![3]),
    })
}

pub(super) fn parse_geom_source(fixture_id: &str, source: &str) -> ComputeResult<GeomLdosInput> {
    let numeric_rows = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();

    if numeric_rows.is_empty() {
        return Err(ldos_parse_error(
            fixture_id,
            "geom.dat is missing numeric content",
        ));
    }
    if numeric_rows[0].len() < 2 {
        return Err(ldos_parse_error(
            fixture_id,
            "geom.dat header must provide nat and nph values",
        ));
    }

    let declared_nat = f64_to_usize(numeric_rows[0][0], fixture_id, "geom.dat nat")?;
    let declared_nph = f64_to_usize(numeric_rows[0][1], fixture_id, "geom.dat nph")?;

    let mut atoms = Vec::new();
    for row in numeric_rows {
        if row.len() < 6 {
            continue;
        }
        atoms.push(AtomSite {
            x: row[1],
            y: row[2],
            z: row[3],
            ipot: f64_to_i32(row[4], fixture_id, "geom.dat ipot")?,
        });
    }
    if atoms.is_empty() {
        return Err(ldos_parse_error(
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

    Ok(GeomLdosInput {
        nat: declared_nat.max(atom_count),
        nph: declared_nph.max(1),
        atom_count,
        radius_mean,
        radius_rms,
        radius_max,
        ipot_mean,
    })
}

pub(super) fn parse_pot_source(fixture_id: &str, bytes: &[u8]) -> ComputeResult<PotLdosInput> {
    if bytes.is_empty() {
        return Err(ldos_parse_error(fixture_id, "pot.bin is empty"));
    }

    if bytes.starts_with(POT_BINARY_MAGIC) {
        return parse_true_compute_pot_binary(fixture_id, bytes);
    }

    let checksum = checksum_bytes(bytes);
    let byte_len = bytes.len();
    let radius_mean = ((byte_len % 7_500) as f64 / 1_500.0).max(1.0);
    let radius_rms = radius_mean * 1.1;
    let radius_max = radius_mean * 1.4;
    let charge_scale = ((checksum % 900) as f64 / 180.0 + 1.0).max(1.0e-6);

    Ok(PotLdosInput {
        nat: (byte_len / 2_048).max(1),
        nph: ((checksum % 4) as usize).max(1),
        npot: (byte_len / 16_384).max(1),
        rfms: ((byte_len % 6_000) as f64 / 800.0 + 2.0).clamp(2.0, 12.0),
        radius_mean,
        radius_rms,
        radius_max,
        charge_scale,
        checksum,
        has_true_compute_magic: false,
    })
}

fn parse_true_compute_pot_binary(fixture_id: &str, bytes: &[u8]) -> ComputeResult<PotLdosInput> {
    let mut offset = POT_BINARY_MAGIC.len();

    for _ in 0..POT_CONTROL_I32_COUNT {
        let _ = take_i32(bytes, &mut offset).ok_or_else(|| {
            ldos_parse_error(fixture_id, "pot.bin missing POT control i32 values")
        })?;
    }

    let mut control_f64 = [0.0_f64; POT_CONTROL_F64_COUNT];
    for value in &mut control_f64 {
        *value = take_f64(bytes, &mut offset).ok_or_else(|| {
            ldos_parse_error(fixture_id, "pot.bin missing POT control f64 values")
        })?;
    }

    let nat = take_u32(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing nat metadata"))?
        as usize;
    let nph = take_u32(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing nph metadata"))?
        as usize;
    let npot = take_u32(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing npot metadata"))?
        as usize;
    let radius_mean = take_f64(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing radius_mean metadata"))?;
    let radius_rms = take_f64(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing radius_rms metadata"))?;
    let radius_max = take_f64(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing radius_max metadata"))?;

    let mut zeff_sum = 0.0_f64;
    let potential_count = npot.max(1);
    for _ in 0..potential_count {
        let _ = take_u32(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential index"))?;
        let _ = take_i32(bytes, &mut offset).ok_or_else(|| {
            ldos_parse_error(fixture_id, "pot.bin missing potential atomic number")
        })?;
        let _ = take_i32(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential lmaxsc"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential xnatph"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential xion"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential folp"))?;
        let zeff = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential zeff"))?;
        let _ = take_f64(bytes, &mut offset).ok_or_else(|| {
            ldos_parse_error(fixture_id, "pot.bin missing potential local_density")
        })?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential vmt0"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential vxc"))?;
        zeff_sum += zeff.abs();
    }

    Ok(PotLdosInput {
        nat: nat.max(1),
        nph: nph.max(1),
        npot: npot.max(1),
        rfms: control_f64[5].abs().max(0.1),
        radius_mean: radius_mean.abs().max(1.0e-6),
        radius_rms: radius_rms.abs().max(1.0e-6),
        radius_max: radius_max.abs().max(1.0e-6),
        charge_scale: (zeff_sum / npot.max(1) as f64).max(1.0e-6),
        checksum: checksum_bytes(bytes),
        has_true_compute_magic: true,
    })
}

pub(super) fn parse_reciprocal_source(
    fixture_id: &str,
    source: &str,
) -> ComputeResult<ReciprocalLdosInput> {
    let values = source
        .lines()
        .flat_map(parse_numeric_tokens)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err(ldos_parse_error(
            fixture_id,
            "reciprocal.inp does not contain numeric values",
        ));
    }

    Ok(ReciprocalLdosInput {
        ispace: f64_to_i32(values[0], fixture_id, "reciprocal.inp ispace")?,
    })
}

pub(super) fn parse_ldos_channel_name(file_name: &str) -> Option<usize> {
    let normalized = file_name.to_ascii_lowercase();
    if !normalized.starts_with("ldos") || !normalized.ends_with(".dat") {
        return None;
    }

    let digits = &normalized[4..normalized.len().saturating_sub(4)];
    if digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    digits.parse::<usize>().ok()
}

pub(super) fn expected_output_artifacts(channel_count: usize) -> Vec<ComputeArtifact> {
    let mut outputs = (0..channel_count)
        .map(|channel| ComputeArtifact::new(format!("ldos{channel:02}.dat")))
        .collect::<Vec<_>>();
    outputs.push(ComputeArtifact::new(super::LDOS_LOG_OUTPUT));
    outputs
}

fn ldos_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.LDOS_INPUT_PARSE",
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
        return Err(ldos_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-6 {
        return Err(ldos_parse_error(
            fixture_id,
            format!("{} must be an integer value", field),
        ));
    }
    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(ldos_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }
    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(ldos_parse_error(
            fixture_id,
            format!("{} must be non-negative", field),
        ));
    }
    Ok(integer as usize)
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

fn checksum_bytes(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0_u64, |accumulator, byte| {
        accumulator.wrapping_mul(131).wrapping_add(u64::from(*byte))
    })
}

fn distance(left: AtomSite, right: AtomSite) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    let dz = left.z - right.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
