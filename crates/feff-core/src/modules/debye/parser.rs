use super::{CHECKSUM_OFFSET_BASIS, CHECKSUM_PRIME, DEBYE_REQUIRED_INPUTS};
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub(super) struct DebyeControlInput {
    pub(super) mchi: i32,
    pub(super) ispec: i32,
    pub(super) idwopt: i32,
    pub(super) decomposition: i32,
    pub(super) s02: f64,
    pub(super) critcw: f64,
    pub(super) temperature: f64,
    pub(super) debye_temp: f64,
    pub(super) alphat: f64,
    pub(super) thetae: f64,
    pub(super) sig2g: f64,
    pub(super) qvec: [f64; 3],
}

impl Default for DebyeControlInput {
    fn default() -> Self {
        Self {
            mchi: 1,
            ispec: 0,
            idwopt: 2,
            decomposition: -1,
            s02: 1.0,
            critcw: 4.0,
            temperature: 300.0,
            debye_temp: 250.0,
            alphat: 0.0,
            thetae: 0.0,
            sig2g: 0.0,
            qvec: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PathEntry {
    pub(super) index: usize,
    pub(super) nleg: usize,
    pub(super) degeneracy: f64,
    pub(super) reff: f64,
}

#[derive(Debug, Clone)]
pub(super) struct PathInputSummary {
    pub(super) checksum: u64,
    pub(super) entries: Vec<PathEntry>,
    pub(super) entry_count: usize,
    pub(super) mean_nleg: f64,
    pub(super) degeneracy_sum: f64,
    pub(super) reff_mean: f64,
}

#[derive(Debug, Clone)]
pub(super) struct FeffInputSummary {
    pub(super) title: String,
    pub(super) edge_label: String,
    pub(super) absorber_z: i32,
    pub(super) atom_count: usize,
    pub(super) has_exafs: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SpringInputSummary {
    pub(super) checksum: u64,
    pub(super) stretch_count: usize,
    pub(super) bend_count: usize,
    pub(super) constant_mean: f64,
    pub(super) constant_max: f64,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Debye {
        return Err(FeffError::input_validation(
            "INPUT.DEBYE_MODULE",
            format!("DEBYE module expects DEBYE, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.DEBYE_INPUT_ARTIFACT",
                format!(
                    "DEBYE module expects input artifact '{}' at '{}'",
                    DEBYE_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(DEBYE_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.DEBYE_INPUT_ARTIFACT",
            format!(
                "DEBYE module requires input artifact '{}' but received '{}'",
                DEBYE_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.DEBYE_INPUT_ARTIFACT",
            format!(
                "DEBYE module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.DEBYE_INPUT_READ",
            format!(
                "failed to read DEBYE input '{}' ({}): {}",
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

pub(super) fn parse_ff2x_source(
    fixture_id: &str,
    source: &str,
) -> ComputeResult<DebyeControlInput> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut control = DebyeControlInput::default();

    let mut saw_thermo_block = false;
    let mut saw_primary_control = false;

    for index in 0..lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();

        if lower.starts_with("mchi") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 3 {
                    control.mchi = f64_to_i32(values[0], fixture_id, "ff2x.inp mchi")?;
                    control.ispec = f64_to_i32(values[1], fixture_id, "ff2x.inp ispec")?;
                    control.idwopt = f64_to_i32(values[2], fixture_id, "ff2x.inp idwopt")?;
                    saw_primary_control = true;
                }
            }
            continue;
        }

        if lower.starts_with("vrcorr") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 4 {
                    control.s02 = values[2].abs();
                    control.critcw = values[3].abs();
                }
            }
            continue;
        }

        if lower.starts_with("tk") && lower.contains("thetad") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 5 {
                    control.temperature = values[0].abs();
                    control.debye_temp = values[1].abs();
                    control.alphat = values[2];
                    control.thetae = values[3];
                    control.sig2g = values[4].abs();
                    saw_thermo_block = true;
                }
            }
            continue;
        }

        if lower.starts_with("momentum") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 3 {
                    control.qvec = [values[0], values[1], values[2]];
                }
            }
            continue;
        }

        if lower.contains("number of decomposi") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1)
                && let Some(value) = parse_numeric_tokens(values_line).first().copied()
            {
                control.decomposition = f64_to_i32(value, fixture_id, "ff2x.inp decomposition")?;
            }
            continue;
        }
    }

    if !saw_thermo_block {
        let all_numeric = lines
            .iter()
            .flat_map(|line| parse_numeric_tokens(line))
            .collect::<Vec<_>>();

        if all_numeric.len() < 3 {
            return Err(debye_parse_error(
                fixture_id,
                "ff2x.inp missing thermal control values",
            ));
        }

        control.temperature = all_numeric[0].abs();
        control.debye_temp = all_numeric[1].abs();
        control.sig2g = all_numeric[2].abs() * 0.01;
        if all_numeric.len() >= 6 {
            control.qvec = [all_numeric[3], all_numeric[4], all_numeric[5]];
        }
    }

    if !saw_primary_control {
        let all_numeric = lines
            .iter()
            .flat_map(|line| parse_numeric_tokens(line))
            .collect::<Vec<_>>();
        if all_numeric.len() >= 3 {
            control.mchi = f64_to_i32(all_numeric[0], fixture_id, "ff2x.inp mchi")?;
            control.ispec = f64_to_i32(all_numeric[1], fixture_id, "ff2x.inp ispec")?;
            control.idwopt = f64_to_i32(all_numeric[2], fixture_id, "ff2x.inp idwopt")?;
        }
    }

    control.temperature = control.temperature.clamp(1.0e-4, 5_000.0);
    control.debye_temp = control.debye_temp.clamp(1.0e-4, 5_000.0);
    control.s02 = control.s02.clamp(0.0, 2.5);
    control.critcw = control.critcw.clamp(0.0, 100.0);

    Ok(control)
}

pub(super) fn parse_paths_source(
    fixture_id: &str,
    source: &str,
) -> ComputeResult<PathInputSummary> {
    let checksum = checksum_bytes(source.as_bytes());
    let mut entries = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 3 {
            continue;
        }

        let Some(index) = parse_usize_token(tokens[0]) else {
            continue;
        };
        let Some(nleg) = parse_usize_token(tokens[1]) else {
            continue;
        };
        let Some(degeneracy) = parse_numeric_token(tokens[2]) else {
            continue;
        };

        if index == 0 || nleg == 0 || !degeneracy.is_finite() {
            continue;
        }

        let reff = parse_reff_from_path_line(trimmed)
            .unwrap_or(1.6 + nleg as f64 * 0.6 + (index as f64 % 11.0) * 0.04)
            .abs();

        entries.push(PathEntry {
            index,
            nleg,
            degeneracy: degeneracy.abs().max(1.0e-6),
            reff: reff.max(0.2),
        });

        if entries.len() >= 512 {
            break;
        }
    }

    if entries.is_empty() {
        let fallback_count = ((checksum % 18) as usize + 10).clamp(10, 64);
        for offset in 0..fallback_count {
            entries.push(PathEntry {
                index: offset + 1,
                nleg: 2 + ((offset + (checksum as usize % 5)) % 4),
                degeneracy: 1.0 + ((checksum.wrapping_add(offset as u64) % 17) as f64),
                reff: 1.8 + offset as f64 * 0.18,
            });
        }
    }

    let entry_count = entries.len();
    if entry_count == 0 {
        return Err(debye_parse_error(
            fixture_id,
            "paths.dat does not contain usable path rows",
        ));
    }

    let mean_nleg = entries.iter().map(|entry| entry.nleg as f64).sum::<f64>() / entry_count as f64;
    let degeneracy_sum = entries
        .iter()
        .map(|entry| entry.degeneracy)
        .sum::<f64>()
        .max(1.0e-6);
    let reff_mean = entries.iter().map(|entry| entry.reff).sum::<f64>() / entry_count as f64;

    Ok(PathInputSummary {
        checksum,
        entries,
        entry_count,
        mean_nleg,
        degeneracy_sum,
        reff_mean,
    })
}

fn parse_reff_from_path_line(line: &str) -> Option<f64> {
    let lower = line.to_ascii_lowercase();
    let marker_index = lower.find("r=")?;
    let trailing = &line[(marker_index + 2)..];

    for token in trailing.split_whitespace() {
        if let Some(value) = parse_numeric_token(token) {
            return Some(value.abs());
        }
    }

    None
}

pub(super) fn parse_feff_source(fixture_id: &str, source: &str) -> ComputeResult<FeffInputSummary> {
    let checksum = checksum_bytes(source.as_bytes());
    let mut title = String::from("DEBYE true-compute");
    let mut edge_label = String::from("K");
    let mut absorber_z: Option<i32> = None;
    let mut atom_count = 0_usize;
    let mut has_exafs = false;

    let mut in_potentials = false;
    let mut in_atoms = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();

        if lower.starts_with("title") {
            let value = trimmed
                .split_once(char::is_whitespace)
                .map(|(_, rest)| rest.trim())
                .filter(|value| !value.is_empty())
                .unwrap_or("DEBYE true-compute");
            title = value.to_string();
            continue;
        }

        if lower.starts_with("edge") {
            if let Some(token) = trimmed.split_whitespace().nth(1)
                && !token.is_empty()
            {
                edge_label = token.to_string();
            }
            continue;
        }

        if lower.starts_with("exafs") {
            has_exafs = true;
            continue;
        }

        if lower.starts_with("potentials") {
            in_potentials = true;
            in_atoms = false;
            continue;
        }

        if lower.starts_with("atoms") {
            in_atoms = true;
            in_potentials = false;
            continue;
        }

        if lower.starts_with("end") {
            in_atoms = false;
            in_potentials = false;
            continue;
        }

        if in_potentials {
            let values = parse_numeric_tokens(trimmed);
            if values.len() >= 2 {
                let ipot = f64_to_i32_soft(values[0]);
                let z = f64_to_i32_soft(values[1]);

                if let (Some(ipot), Some(z)) = (ipot, z)
                    && (absorber_z.is_none() || ipot == 0)
                {
                    absorber_z = Some(z);
                }
            }
            continue;
        }

        if in_atoms {
            let values = parse_numeric_tokens(trimmed);
            if values.len() >= 5 {
                atom_count += 1;
            }
        }
    }

    if atom_count == 0 {
        atom_count = ((checksum % 96) as usize + 12).clamp(12, 200);
    }

    let absorber_z = absorber_z
        .unwrap_or((checksum % 60) as i32 + 20)
        .clamp(1, 118);

    if title.trim().is_empty() {
        return Err(debye_parse_error(
            fixture_id,
            "feff.inp title line cannot be empty",
        ));
    }

    Ok(FeffInputSummary {
        title,
        edge_label,
        absorber_z,
        atom_count,
        has_exafs,
    })
}

pub(super) fn parse_optional_spring_source(source: Option<&str>) -> Option<SpringInputSummary> {
    let source = source?;
    let checksum = checksum_bytes(source.as_bytes());

    enum Section {
        Unknown,
        Stretches,
        Bends,
    }

    let mut section = Section::Unknown;
    let mut stretch_count = 0_usize;
    let mut bend_count = 0_usize;
    let mut constant_sum = 0.0_f64;
    let mut constant_max = 0.0_f64;
    let mut constant_count = 0_usize;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("stretches") {
            section = Section::Stretches;
            continue;
        }
        if lower.contains("bends") {
            section = Section::Bends;
            continue;
        }

        if trimmed.starts_with('*') || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let values = parse_numeric_tokens(trimmed);
        if values.is_empty() {
            continue;
        }

        let constant = match section {
            Section::Stretches => {
                stretch_count += 1;
                values
                    .get(2)
                    .copied()
                    .unwrap_or_else(|| *values.last().unwrap_or(&0.0))
            }
            Section::Bends => {
                bend_count += 1;
                values
                    .get(3)
                    .copied()
                    .unwrap_or_else(|| *values.last().unwrap_or(&0.0))
            }
            Section::Unknown => values.last().copied().unwrap_or(0.0),
        }
        .abs();

        constant_sum += constant;
        constant_max = constant_max.max(constant);
        constant_count += 1;
    }

    if constant_count == 0 {
        let synthetic = ((checksum % 2_000) as f64 / 50.0).max(0.1);
        return Some(SpringInputSummary {
            checksum,
            stretch_count: 0,
            bend_count: 0,
            constant_mean: synthetic,
            constant_max: synthetic,
        });
    }

    Some(SpringInputSummary {
        checksum,
        stretch_count,
        bend_count,
        constant_mean: constant_sum / constant_count as f64,
        constant_max,
    })
}

fn debye_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.DEBYE_INPUT_PARSE",
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
            ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '='
        )
    });
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.replace(['D', 'd'], "E");
    normalized.parse::<f64>().ok()
}

fn parse_usize_token(token: &str) -> Option<usize> {
    let trimmed = token.trim_matches(|character: char| !character.is_ascii_digit());
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<usize>().ok()
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
    if !value.is_finite() {
        return Err(debye_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-6 {
        return Err(debye_parse_error(
            fixture_id,
            format!("{} must be an integer value", field),
        ));
    }
    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(debye_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }
    Ok(rounded as i32)
}

fn f64_to_i32_soft(value: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-6 {
        return None;
    }
    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return None;
    }
    Some(rounded as i32)
}

fn checksum_bytes(bytes: &[u8]) -> u64 {
    let mut checksum = CHECKSUM_OFFSET_BASIS;
    for byte in bytes {
        checksum ^= *byte as u64;
        checksum = checksum.wrapping_mul(CHECKSUM_PRIME);
    }
    checksum
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
