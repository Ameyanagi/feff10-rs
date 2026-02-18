use super::FULLSPECTRUM_REQUIRED_INPUTS;
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub(super) struct FullSpectrumControlInput {
    pub(super) run_mode: i32,
    pub(super) broadening_ev: f64,
    pub(super) drude_scale: f64,
    pub(super) oscillator_scale: f64,
    pub(super) epsilon_shift: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct XmuRow {
    pub(super) energy: f64,
    pub(super) mu: f64,
    pub(super) mu0: f64,
    pub(super) chi: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct XmuSummary {
    pub(super) row_count: usize,
    pub(super) energy_min: f64,
    pub(super) energy_max: f64,
    pub(super) mean_mu: f64,
    pub(super) mean_mu0: f64,
    pub(super) mean_chi: f64,
    pub(super) rms_chi: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct AuxiliarySpectrumSummary {
    pub(super) row_count: usize,
    pub(super) mean_energy: f64,
    pub(super) mean_signal: f64,
    pub(super) rms_signal: f64,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::FullSpectrum {
        return Err(FeffError::input_validation(
            "INPUT.FULLSPECTRUM_MODULE",
            format!(
                "FULLSPECTRUM module expects FULLSPECTRUM, got {}",
                request.module
            ),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.FULLSPECTRUM_INPUT_ARTIFACT",
                format!(
                    "FULLSPECTRUM module expects input artifact '{}' at '{}'",
                    FULLSPECTRUM_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(FULLSPECTRUM_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.FULLSPECTRUM_INPUT_ARTIFACT",
            format!(
                "FULLSPECTRUM module requires input artifact '{}' but received '{}'",
                FULLSPECTRUM_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.FULLSPECTRUM_INPUT_ARTIFACT",
            format!(
                "FULLSPECTRUM module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.FULLSPECTRUM_INPUT_READ",
            format!(
                "failed to read FULLSPECTRUM input '{}' ({}): {}",
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

pub(super) fn parse_fullspectrum_source(
    fixture_id: &str,
    source: &str,
) -> ComputeResult<FullSpectrumControlInput> {
    let numeric_rows: Vec<Vec<f64>> = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect();

    if numeric_rows.is_empty() {
        return Err(fullspectrum_parse_error(
            fixture_id,
            "fullspectrum.inp does not contain numeric control rows",
        ));
    }

    let run_mode = row_value(&numeric_rows, 0, 0).ok_or_else(|| {
        fullspectrum_parse_error(
            fixture_id,
            "fullspectrum.inp is missing mFullSpectrum run-mode value",
        )
    })?;

    let run_mode = f64_to_i32(run_mode, fixture_id, "fullspectrum run-mode")?;
    let broadening_ev = row_value(&numeric_rows, 1, 0)
        .unwrap_or(0.35)
        .abs()
        .max(1.0e-6);
    let drude_scale = row_value(&numeric_rows, 1, 1)
        .unwrap_or(1.0)
        .abs()
        .max(1.0e-6);
    let oscillator_scale = row_value(&numeric_rows, 2, 0)
        .unwrap_or(1.0)
        .abs()
        .max(1.0e-6);
    let epsilon_shift = row_value(&numeric_rows, 2, 1).unwrap_or(0.0);

    Ok(FullSpectrumControlInput {
        run_mode,
        broadening_ev,
        drude_scale,
        oscillator_scale,
        epsilon_shift,
    })
}

pub(super) fn parse_xmu_source(fixture_id: &str, source: &str) -> ComputeResult<Vec<XmuRow>> {
    let mut rows = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let values = parse_numeric_tokens(trimmed);
        if values.len() < 2 {
            continue;
        }

        let energy = values[0];
        let (mu, mu0, chi) = if values.len() >= 6 {
            (values[3], values[4], values[5])
        } else if values.len() >= 4 {
            (values[1], values[2], values[3])
        } else {
            let mu = values[1];
            let mu0 = values.get(2).copied().unwrap_or(mu * 1.01);
            let chi = mu - mu0;
            (mu, mu0, chi)
        };

        if !energy.is_finite() || !mu.is_finite() || !mu0.is_finite() || !chi.is_finite() {
            continue;
        }

        rows.push(XmuRow {
            energy,
            mu,
            mu0,
            chi,
        });
    }

    if rows.is_empty() {
        return Err(fullspectrum_parse_error(
            fixture_id,
            "xmu.dat does not contain any numeric spectral rows",
        ));
    }

    Ok(rows)
}

pub(super) fn parse_auxiliary_source(source: &str) -> AuxiliarySpectrumSummary {
    let mut rows: Vec<(f64, f64)> = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let values = parse_numeric_tokens(trimmed);
        if values.len() < 2 {
            continue;
        }

        rows.push((values[0], values[1]));
    }

    if rows.is_empty() {
        return AuxiliarySpectrumSummary {
            row_count: 0,
            mean_energy: 0.0,
            mean_signal: 0.0,
            rms_signal: 0.0,
        };
    }

    let row_count = rows.len();
    let mean_energy = rows.iter().map(|(energy, _)| energy).sum::<f64>() / row_count as f64;
    let mean_signal = rows.iter().map(|(_, signal)| signal).sum::<f64>() / row_count as f64;
    let rms_signal =
        (rows.iter().map(|(_, signal)| signal * signal).sum::<f64>() / row_count as f64).sqrt();

    AuxiliarySpectrumSummary {
        row_count,
        mean_energy,
        mean_signal,
        rms_signal,
    }
}

pub(super) fn summarize_xmu_rows(rows: &[XmuRow]) -> XmuSummary {
    let mut energy_min = f64::INFINITY;
    let mut energy_max = f64::NEG_INFINITY;
    let mut mu_sum = 0.0_f64;
    let mut mu0_sum = 0.0_f64;
    let mut chi_sum = 0.0_f64;
    let mut chi_sq_sum = 0.0_f64;

    for row in rows {
        energy_min = energy_min.min(row.energy);
        energy_max = energy_max.max(row.energy);
        mu_sum += row.mu;
        mu0_sum += row.mu0;
        chi_sum += row.chi;
        chi_sq_sum += row.chi * row.chi;
    }

    let row_count = rows.len().max(1);
    XmuSummary {
        row_count: rows.len(),
        energy_min: if energy_min.is_finite() {
            energy_min
        } else {
            0.0
        },
        energy_max: if energy_max.is_finite() {
            energy_max
        } else {
            1.0
        },
        mean_mu: mu_sum / row_count as f64,
        mean_mu0: mu0_sum / row_count as f64,
        mean_chi: chi_sum / row_count as f64,
        rms_chi: (chi_sq_sum / row_count as f64).sqrt(),
    }
}

fn row_value(rows: &[Vec<f64>], row_index: usize, column_index: usize) -> Option<f64> {
    rows.get(row_index)
        .and_then(|row| row.get(column_index))
        .copied()
}

fn parse_numeric_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(parse_numeric_token)
        .collect()
}

fn parse_numeric_token(token: &str) -> Option<f64> {
    let normalized = token
        .trim()
        .trim_end_matches([',', ';', ':'])
        .replace(['D', 'd'], "E");
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
    if !value.is_finite() {
        return Err(fullspectrum_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }

    let rounded = value.round();
    if (value - rounded).abs() > 1.0e-6 {
        return Err(fullspectrum_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }

    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(fullspectrum_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }

    Ok(rounded as i32)
}

fn fullspectrum_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::input_validation(
        "INPUT.FULLSPECTRUM_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
