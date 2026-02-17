use super::{SELF_PRIMARY_INPUT, SELF_SPECTRUM_INPUT_CANDIDATES, FNV_OFFSET_BASIS, FNV_PRIME};
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StagedSpectrumSource {
    pub(super) artifact: String,
    pub(super) source: String,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SelfControlInput {
    pub(super) msfconv: i32,
    pub(super) ipse: i32,
    pub(super) ipsk: i32,
    pub(super) wsigk: f64,
    pub(super) cen: f64,
    pub(super) ispec: i32,
    pub(super) ipr6: i32,
}

impl Default for SelfControlInput {
    fn default() -> Self {
        Self {
            msfconv: 1,
            ipse: 0,
            ipsk: 0,
            wsigk: 0.0,
            cen: 0.0,
            ispec: 0,
            ipr6: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct SelfSpectrumInput {
    pub(super) artifact: String,
    pub(super) rows: Vec<SpectrumRow>,
    pub(super) checksum: u64,
    pub(super) mean_signal: f64,
    pub(super) rms_signal: f64,
    pub(super) energy_min: f64,
    pub(super) energy_max: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SpectrumRow {
    pub(super) energy: f64,
    pub(super) signal: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ExcInputSummary {
    pub(super) row_count: usize,
    pub(super) mean_weight: f64,
    pub(super) phase_bias: f64,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::SelfEnergy {
        return Err(FeffError::input_validation(
            "INPUT.SELF_MODULE",
            format!("SELF module expects SELF, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.SELF_INPUT_ARTIFACT",
                format!(
                    "SELF module expects input artifact '{}' at '{}'",
                    SELF_PRIMARY_INPUT,
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(SELF_PRIMARY_INPUT) {
        return Err(FeffError::input_validation(
            "INPUT.SELF_INPUT_ARTIFACT",
            format!(
                "SELF module requires input artifact '{}' but received '{}'",
                SELF_PRIMARY_INPUT, input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.SELF_INPUT_ARTIFACT",
            format!(
                "SELF module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    let bytes = fs::read(path).map_err(|source| {
        FeffError::io_system(
            "IO.SELF_INPUT_READ",
            format!(
                "failed to read SELF input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
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

pub(super) fn load_staged_spectrum_sources(directory: &Path) -> ComputeResult<Vec<StagedSpectrumSource>> {
    let artifacts = collect_staged_spectrum_artifacts(directory)?;
    if artifacts.is_empty() {
        return Err(FeffError::input_validation(
            "INPUT.SELF_SPECTRUM_INPUT",
            format!(
                "SELF module requires at least one staged spectrum input (xmu.dat, chi.dat, loss.dat, or feffNNNN.dat) in '{}'",
                directory.display()
            ),
        ));
    }

    let mut sources = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let source = read_input_source(&directory.join(&artifact), &artifact)?;
        sources.push(StagedSpectrumSource { artifact, source });
    }
    Ok(sources)
}

pub(super) fn parse_sfconv_source(source: &str) -> SelfControlInput {
    let mut control = SelfControlInput::default();
    let lines: Vec<&str> = source.lines().collect();

    if let Some(values) = parse_numbers_after_marker(&lines, "msfconv") {
        control.msfconv = values.first().copied().unwrap_or(1.0).round() as i32;
        control.ipse = values.get(1).copied().unwrap_or(0.0).round() as i32;
        control.ipsk = values.get(2).copied().unwrap_or(0.0).round() as i32;
    }
    if let Some(values) = parse_numbers_after_marker(&lines, "wsigk") {
        control.wsigk = values.first().copied().unwrap_or(0.0);
        control.cen = values.get(1).copied().unwrap_or(0.0);
    }
    if let Some(values) = parse_numbers_after_marker(&lines, "ispec") {
        control.ispec = values.first().copied().unwrap_or(0.0).round() as i32;
        control.ipr6 = values.get(1).copied().unwrap_or(0.0).round() as i32;
    }

    control
}

pub(super) fn parse_spectrum_source(
    fixture_id: &str,
    artifact: &str,
    source: &str,
) -> ComputeResult<SelfSpectrumInput> {
    let mut rows = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let values = parse_numeric_tokens(trimmed);
        if values.is_empty() {
            continue;
        }

        let energy = values.first().copied().unwrap_or(rows.len() as f64);
        let signal = if values.len() >= 2 {
            values[values.len() - 1]
        } else {
            values[0]
        };
        if !energy.is_finite() || !signal.is_finite() {
            continue;
        }
        rows.push(SpectrumRow { energy, signal });
    }

    if rows.is_empty() {
        return Err(self_parse_error(
            fixture_id,
            format!(
                "spectrum input '{}' does not contain numeric rows",
                artifact
            ),
        ));
    }

    let energy_min = rows
        .iter()
        .map(|row| row.energy)
        .fold(f64::INFINITY, f64::min);
    let energy_max = rows
        .iter()
        .map(|row| row.energy)
        .fold(f64::NEG_INFINITY, f64::max);
    let signal_sum = rows.iter().map(|row| row.signal).sum::<f64>();
    let signal_sq_sum = rows.iter().map(|row| row.signal * row.signal).sum::<f64>();
    let mean_signal = signal_sum / rows.len() as f64;
    let rms_signal = (signal_sq_sum / rows.len() as f64).sqrt();
    let checksum = fnv1a64(source.as_bytes());

    Ok(SelfSpectrumInput {
        artifact: artifact.to_string(),
        rows,
        checksum,
        mean_signal,
        rms_signal,
        energy_min,
        energy_max,
    })
}

pub(super) fn parse_exc_source(source: &str) -> ExcInputSummary {
    let mut row_count = 0usize;
    let mut weight_sum = 0.0_f64;
    let mut phase_sum = 0.0_f64;

    for line in source.lines() {
        let values = parse_numeric_tokens(line);
        if values.len() < 2 {
            continue;
        }

        let weight = if values.len() >= 3 {
            values[2]
        } else {
            values[1]
        };
        let phase = *values.last().unwrap_or(&0.0);
        if !weight.is_finite() || !phase.is_finite() {
            continue;
        }
        row_count += 1;
        weight_sum += weight;
        phase_sum += phase;
    }

    if row_count == 0 {
        return ExcInputSummary {
            row_count: 0,
            mean_weight: 0.0,
            phase_bias: 0.0,
        };
    }

    ExcInputSummary {
        row_count,
        mean_weight: weight_sum / row_count as f64,
        phase_bias: phase_sum / row_count as f64,
    }
}

pub(super) fn sample_spectrum_row(
    spectrum: &SelfSpectrumInput,
    sample_index: usize,
    sample_count: usize,
) -> SpectrumRow {
    if spectrum.rows.is_empty() {
        return SpectrumRow {
            energy: sample_index as f64,
            signal: 0.0,
        };
    }
    if spectrum.rows.len() == 1 || sample_count <= 1 {
        return spectrum.rows[0];
    }

    let ratio = sample_index as f64 / sample_count.saturating_sub(1) as f64;
    let scaled = ratio * spectrum.rows.len().saturating_sub(1) as f64;
    let lower = scaled.floor() as usize;
    let upper = scaled.ceil() as usize;
    let frac = scaled - lower as f64;

    let lower_row = spectrum.rows[lower];
    let upper_row = spectrum.rows[upper.min(spectrum.rows.len() - 1)];

    SpectrumRow {
        energy: lower_row.energy + (upper_row.energy - lower_row.energy) * frac,
        signal: lower_row.signal + (upper_row.signal - lower_row.signal) * frac,
    }
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}

pub(super) fn upsert_artifact(artifacts: &mut Vec<ComputeArtifact>, artifact: &str) {
    let normalized = artifact.to_ascii_lowercase();
    if artifacts.iter().any(|candidate| {
        candidate
            .relative_path
            .to_string_lossy()
            .to_ascii_lowercase()
            == normalized
    }) {
        return;
    }
    artifacts.push(ComputeArtifact::new(artifact));
}

fn collect_staged_spectrum_artifacts(directory: &Path) -> ComputeResult<Vec<String>> {
    let mut artifacts = Vec::new();
    let mut seen = BTreeSet::new();

    for candidate in SELF_SPECTRUM_INPUT_CANDIDATES {
        let candidate_path = directory.join(candidate);
        if !candidate_path.is_file() {
            continue;
        }

        let key = candidate.to_ascii_lowercase();
        if seen.insert(key) {
            artifacts.push(candidate.to_string());
        }
    }

    for artifact in collect_feff_spectrum_artifacts(
        directory,
        "IO.SELF_INPUT_READ",
        "input",
        "input directory",
    )? {
        let key = artifact.to_ascii_lowercase();
        if seen.insert(key) {
            artifacts.push(artifact);
        }
    }

    Ok(artifacts)
}

fn collect_feff_spectrum_artifacts(
    directory: &Path,
    placeholder: &'static str,
    location: &'static str,
    location_label: &'static str,
) -> ComputeResult<Vec<String>> {
    let entries = fs::read_dir(directory).map_err(|source| {
        FeffError::io_system(
            placeholder,
            format!(
                "failed to read SELF {} '{}' while collecting feffNNNN.dat artifacts: {}",
                location,
                directory.display(),
                source
            ),
        )
    })?;

    let mut artifacts = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| {
            FeffError::io_system(
                placeholder,
                format!(
                    "failed to read SELF {} entry in '{}': {}",
                    location,
                    directory.display(),
                    source
                ),
            )
        })?;

        let file_type = entry.file_type().map_err(|source| {
            FeffError::io_system(
                placeholder,
                format!(
                    "failed to inspect SELF {} entry '{}' in '{}': {}",
                    location_label,
                    entry.path().display(),
                    directory.display(),
                    source
                ),
            )
        })?;

        if !file_type.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if is_feff_spectrum_name(&file_name) {
            artifacts.push(file_name.into_owned());
        }
    }

    artifacts.sort();
    Ok(artifacts)
}

fn parse_numbers_after_marker(lines: &[&str], marker: &str) -> Option<Vec<f64>> {
    let marker = marker.to_ascii_lowercase();
    for (index, line) in lines.iter().enumerate() {
        if !line.to_ascii_lowercase().contains(&marker) {
            continue;
        }

        let (_, next_line) = next_nonempty_line(lines, index + 1)?;
        let values = parse_numeric_tokens(next_line);
        if !values.is_empty() {
            return Some(values);
        }
    }

    None
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

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn self_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.SELF_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

fn is_feff_spectrum_name(name: &str) -> bool {
    let lowercase = name.to_ascii_lowercase();
    if !lowercase.starts_with("feff") || !lowercase.ends_with(".dat") {
        return false;
    }

    let suffix = &lowercase[4..lowercase.len() - 4];
    !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
}
