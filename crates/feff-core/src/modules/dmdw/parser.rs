use super::{DMDW_REQUIRED_INPUTS};
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use std::fs;
use std::path::Path;

const CHECKSUM_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const CHECKSUM_PRIME: u64 = 0x00000100000001B3;

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Dmdw {
        return Err(FeffError::input_validation(
            "INPUT.DMDW_MODULE",
            format!("DMDW module expects DMDW, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.DMDW_INPUT_ARTIFACT",
                format!(
                    "DMDW module expects input artifact '{}' at '{}'",
                    DMDW_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(DMDW_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.DMDW_INPUT_ARTIFACT",
            format!(
                "DMDW module requires input artifact '{}' but received '{}'",
                DMDW_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.DMDW_INPUT_ARTIFACT",
            format!(
                "DMDW module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.DMDW_INPUT_READ",
            format!(
                "failed to read DMDW input '{}' ({}): {}",
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
            "IO.DMDW_INPUT_READ",
            format!(
                "failed to read DMDW input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

#[derive(Debug, Clone)]
pub(super) struct DmdwControlInput {
    pub(super) mode_selector: i32,
    pub(super) lanczos_order: usize,
    pub(super) path_group_count: usize,
    pub(super) temperature: f64,
    pub(super) decomposition_flag: i32,
    pub(super) matrix_label: String,
    pub(super) block_count: usize,
    pub(super) path_start: usize,
    pub(super) path_end: usize,
    pub(super) path_step: usize,
    pub(super) reduced_mass: f64,
}

pub(super) fn parse_dmdw_source(fixture_id: &str, source: &str) -> ComputeResult<DmdwControlInput> {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    if lines.is_empty() {
        return Err(FeffError::input_validation(
            "INPUT.DMDW_INPUT_PARSE",
            format!(
                "fixture '{}' DMDW input is empty; expected '{}' content",
                fixture_id, DMDW_REQUIRED_INPUTS[0]
            ),
        ));
    }

    let mode_selector = parse_line_i32(lines.first().copied(), 1);
    let lanczos_order = parse_line_usize(lines.get(1).copied(), 6).clamp(1, 4096);

    let cardinality_row = parse_numeric_tokens(lines.get(2).copied().unwrap_or(""));
    let path_group_count = cardinality_row
        .first()
        .copied()
        .map(|value| as_positive_usize(value, 1))
        .unwrap_or(1)
        .clamp(1, 4096);
    let temperature = cardinality_row
        .get(1)
        .copied()
        .unwrap_or(300.0)
        .clamp(0.0, 5000.0);

    let decomposition_flag = parse_line_i32(lines.get(3).copied(), 0);
    let matrix_label = lines
        .get(4)
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or("feff.dym")
        .to_string();

    let block_count = parse_line_usize(lines.get(5).copied(), path_group_count).clamp(1, 4096);
    let path_row = parse_numeric_tokens(lines.get(6).copied().unwrap_or(""));

    let path_start = path_row
        .first()
        .copied()
        .map(|value| as_positive_usize(value, 1))
        .unwrap_or(1)
        .clamp(1, 65_535);
    let default_path_end = path_start.saturating_add(block_count.saturating_sub(1));
    let mut path_end = path_row
        .get(1)
        .copied()
        .map(|value| as_positive_usize(value, default_path_end))
        .unwrap_or(default_path_end)
        .clamp(path_start, 65_535);
    if path_end < path_start {
        path_end = path_start;
    }

    let path_step = path_row
        .get(2)
        .copied()
        .map(|value| as_positive_usize(value, 1))
        .unwrap_or(1)
        .clamp(1, 4096);
    let reduced_mass = path_row
        .get(3)
        .copied()
        .unwrap_or(28.0)
        .abs()
        .clamp(0.01, 10_000.0);

    Ok(DmdwControlInput {
        mode_selector,
        lanczos_order,
        path_group_count,
        temperature,
        decomposition_flag,
        matrix_label,
        block_count,
        path_start,
        path_end,
        path_step,
        reduced_mass,
    })
}

fn parse_line_i32(line: Option<&str>, fallback: i32) -> i32 {
    line.and_then(|content| parse_numeric_tokens(content).first().copied())
        .map(|value| value.round() as i32)
        .unwrap_or(fallback)
}

fn parse_line_usize(line: Option<&str>, fallback: usize) -> usize {
    line.and_then(|content| parse_numeric_tokens(content).first().copied())
        .map(|value| as_positive_usize(value, fallback))
        .unwrap_or(fallback)
}

fn as_positive_usize(value: f64, fallback: usize) -> usize {
    if !value.is_finite() {
        return fallback;
    }

    let rounded = value.round();
    if rounded <= 0.0 {
        return fallback;
    }

    if rounded >= usize::MAX as f64 {
        return usize::MAX;
    }

    rounded as usize
}

fn parse_numeric_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(parse_numeric_token)
        .collect()
}

fn parse_numeric_token(token: &str) -> Option<f64> {
    let normalized = token.replace('D', "E").replace('d', "e");
    normalized.parse::<f64>().ok()
}

#[derive(Debug, Clone, Copy)]
pub(super) struct DymInputSummary {
    pub(super) checksum: u64,
    pub(super) byte_count: usize,
    pub(super) line_count: usize,
    pub(super) mean_byte: f64,
    pub(super) rms: f64,
}

pub(super) fn summarize_dym_input(bytes: &[u8]) -> DymInputSummary {
    if bytes.is_empty() {
        return DymInputSummary {
            checksum: CHECKSUM_OFFSET_BASIS,
            byte_count: 0,
            line_count: 0,
            mean_byte: 0.0,
            rms: 0.0,
        };
    }

    let checksum = fnv1a64(bytes);
    let byte_count = bytes.len();
    let line_count = bytes.iter().filter(|byte| **byte == b'\n').count();
    let sum = bytes.iter().map(|byte| *byte as f64).sum::<f64>();
    let sum_sq = bytes
        .iter()
        .map(|byte| {
            let value = *byte as f64;
            value * value
        })
        .sum::<f64>();

    let mean_byte = sum / byte_count as f64;
    let rms = (sum_sq / byte_count as f64).sqrt() / 255.0;

    DymInputSummary {
        checksum,
        byte_count,
        line_count,
        mean_byte,
        rms,
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = CHECKSUM_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(CHECKSUM_PRIME);
    }
    hash
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
