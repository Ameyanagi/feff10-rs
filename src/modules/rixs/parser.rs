use super::RIXS_REQUIRED_INPUTS;
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct RixsControlInput {
    pub(super) run_enabled: bool,
    pub(super) energy_rows: usize,
    pub(super) incident_min: f64,
    pub(super) incident_max: f64,
    pub(super) incident_step: f64,
    pub(super) emitted_min: f64,
    pub(super) emitted_max: f64,
    pub(super) emitted_step: f64,
    pub(super) n_edges: usize,
    pub(super) gamma_core: f64,
    pub(super) gamma_edge_1: f64,
    pub(super) gamma_edge_2: f64,
    pub(super) xmu_shift: f64,
    pub(super) read_poles: bool,
    pub(super) skip_calc: bool,
    pub(super) read_sigma: bool,
    pub(super) edge_labels: [String; 2],
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
pub(super) struct BinaryInputSummary {
    pub(super) byte_len: usize,
    pub(super) checksum: u64,
    pub(super) mean: f64,
    pub(super) rms: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TableInputSummary {
    pub(super) value_count: usize,
    pub(super) min: f64,
    pub(super) max: f64,
    pub(super) mean: f64,
    pub(super) rms: f64,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Rixs {
        return Err(FeffError::input_validation(
            "INPUT.RIXS_MODULE",
            format!("RIXS module expects RIXS, got {}", request.module),
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
                    "RIXS module expects input artifact '{}' at '{}'",
                    RIXS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(RIXS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.RIXS_INPUT_ARTIFACT",
            format!(
                "RIXS module requires input artifact '{}' but received '{}'",
                RIXS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.RIXS_INPUT_ARTIFACT",
            format!(
                "RIXS module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
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

pub(super) fn read_input_bytes(path: &Path, artifact_name: &str) -> ComputeResult<Vec<u8>> {
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

pub(super) fn parse_rixs_source(fixture_id: &str, source: &str) -> ComputeResult<RixsControlInput> {
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

pub(super) fn parse_binary_source(
    fixture_id: &str,
    artifact_name: &str,
    bytes: &[u8],
) -> ComputeResult<BinaryInputSummary> {
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

pub(super) fn parse_table_source(
    fixture_id: &str,
    artifact_name: &str,
    source: &str,
) -> ComputeResult<TableInputSummary> {
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

pub(super) fn checksum_to_unit(checksum: u64) -> f64 {
    (checksum as f64 / u64::MAX as f64).clamp(0.0, 1.0)
}

pub(super) fn normalized_index(index: usize, count: usize) -> f64 {
    if count <= 1 {
        return 0.0;
    }

    index as f64 / (count - 1) as f64
}

pub(super) fn format_scientific_f64(value: f64) -> String {
    format!("{:>16.8E}", value)
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
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

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
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
