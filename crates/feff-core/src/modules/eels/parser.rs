use super::EELS_REQUIRED_INPUTS;
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub(super) struct EelsControlInput {
    pub(super) run_mode: i32,
    pub(super) average: i32,
    pub(super) relativistic: i32,
    pub(super) cross_terms: i32,
    pub(super) polarization_min: i32,
    pub(super) polarization_step: i32,
    pub(super) polarization_max: i32,
    pub(super) beam_energy_ev: f64,
    pub(super) beam_direction: [f64; 3],
    pub(super) collection_semiangle_rad: f64,
    pub(super) convergence_semiangle_rad: f64,
    pub(super) qmesh_radial: usize,
    pub(super) qmesh_angular: usize,
    pub(super) detector_theta: f64,
    pub(super) detector_phi: f64,
    pub(super) magic_flag: bool,
    pub(super) magic_energy_offset_ev: f64,
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
pub(super) struct MagicInputSummary {
    pub(super) value_count: usize,
    pub(super) mean_value: f64,
    pub(super) rms_value: f64,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Eels {
        return Err(FeffError::input_validation(
            "INPUT.EELS_MODULE",
            format!("EELS module expects EELS, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.EELS_INPUT_ARTIFACT",
                format!(
                    "EELS module expects input artifact '{}' at '{}'",
                    EELS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(EELS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.EELS_INPUT_ARTIFACT",
            format!(
                "EELS module requires input artifact '{}' but received '{}'",
                EELS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.EELS_INPUT_ARTIFACT",
            format!(
                "EELS module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.EELS_INPUT_READ",
            format!(
                "failed to read EELS input '{}' ({}): {}",
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

pub(super) fn parse_eels_source(fixture_id: &str, source: &str) -> ComputeResult<EelsControlInput> {
    let numeric_rows: Vec<Vec<f64>> = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect();

    if numeric_rows.is_empty() {
        return Err(eels_parse_error(
            fixture_id,
            "eels.inp does not contain numeric control rows",
        ));
    }

    let run_mode = row_value(&numeric_rows, 0, 0).ok_or_else(|| {
        eels_parse_error(
            fixture_id,
            "eels.inp is missing the ELNES/EXELFS run-mode flag row",
        )
    })?;
    let run_mode = f64_to_i32(run_mode, fixture_id, "eels.inp run-mode")?;
    if run_mode <= 0 {
        return Err(eels_parse_error(
            fixture_id,
            "eels.inp requires ELNES/EXELFS run mode enabled (first value must be > 0)",
        ));
    }

    let average = parse_optional_i32(
        row_value(&numeric_rows, 1, 0),
        0,
        fixture_id,
        "eels.inp average flag",
    )?;
    let relativistic = parse_optional_i32(
        row_value(&numeric_rows, 1, 1),
        1,
        fixture_id,
        "eels.inp relativistic flag",
    )?;
    let cross_terms = parse_optional_i32(
        row_value(&numeric_rows, 1, 2),
        1,
        fixture_id,
        "eels.inp cross-term flag",
    )?;
    let polarization_min = parse_optional_i32(
        row_value(&numeric_rows, 2, 0),
        1,
        fixture_id,
        "eels.inp polarization min",
    )?;
    let polarization_step = parse_optional_i32(
        row_value(&numeric_rows, 2, 1),
        1,
        fixture_id,
        "eels.inp polarization step",
    )?
    .max(1);
    let polarization_max = parse_optional_i32(
        row_value(&numeric_rows, 2, 2),
        polarization_min,
        fixture_id,
        "eels.inp polarization max",
    )?;

    let beam_energy_ev = row_value(&numeric_rows, 3, 0).unwrap_or(300000.0).abs();
    let beam_direction = [
        row_value(&numeric_rows, 4, 0).unwrap_or(0.0),
        row_value(&numeric_rows, 4, 1).unwrap_or(0.0),
        row_value(&numeric_rows, 4, 2).unwrap_or(1.0),
    ];
    let collection_semiangle_rad = row_value(&numeric_rows, 5, 0).unwrap_or(0.0024).abs();
    let convergence_semiangle_rad = row_value(&numeric_rows, 5, 1).unwrap_or(0.0).abs();
    let qmesh_radial = parse_optional_usize(
        row_value(&numeric_rows, 6, 0),
        5,
        fixture_id,
        "eels.inp qmesh radial",
    )?
    .max(1);
    let qmesh_angular = parse_optional_usize(
        row_value(&numeric_rows, 6, 1),
        3,
        fixture_id,
        "eels.inp qmesh angular",
    )?
    .max(1);
    let detector_theta = row_value(&numeric_rows, 7, 0).unwrap_or(0.0);
    let detector_phi = row_value(&numeric_rows, 7, 1).unwrap_or(0.0);
    let magic_flag = parse_optional_i32(
        row_value(&numeric_rows, 8, 0),
        0,
        fixture_id,
        "eels.inp magic flag",
    )? > 0;
    let magic_energy_offset_ev = row_value(&numeric_rows, 9, 0).unwrap_or(0.0);

    Ok(EelsControlInput {
        run_mode,
        average,
        relativistic,
        cross_terms,
        polarization_min,
        polarization_step,
        polarization_max,
        beam_energy_ev,
        beam_direction,
        collection_semiangle_rad,
        convergence_semiangle_rad,
        qmesh_radial,
        qmesh_angular,
        detector_theta,
        detector_phi,
        magic_flag,
        magic_energy_offset_ev,
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
        let mu = if values.len() >= 4 {
            values[3]
        } else {
            *values.last().unwrap_or(&values[0])
        };
        let mu0 = values.get(4).copied().unwrap_or(mu * 1.02);
        let chi = values.get(5).copied().unwrap_or(mu - mu0);

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
        return Err(eels_parse_error(
            fixture_id,
            "xmu.dat does not contain any numeric spectral rows",
        ));
    }

    Ok(rows)
}

pub(super) fn parse_magic_input_source(source: &str) -> MagicInputSummary {
    let values: Vec<f64> = source.lines().flat_map(parse_numeric_tokens).collect();
    if values.is_empty() {
        return MagicInputSummary {
            value_count: 0,
            mean_value: 0.0,
            rms_value: 0.0,
        };
    }

    let mean_value = values.iter().sum::<f64>() / values.len() as f64;
    let rms_value =
        (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt();

    MagicInputSummary {
        value_count: values.len(),
        mean_value,
        rms_value,
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

fn parse_optional_i32(
    value: Option<f64>,
    default: i32,
    fixture_id: &str,
    field: &str,
) -> ComputeResult<i32> {
    match value {
        Some(value) => f64_to_i32(value, fixture_id, field),
        None => Ok(default),
    }
}

fn parse_optional_usize(
    value: Option<f64>,
    default: usize,
    fixture_id: &str,
    field: &str,
) -> ComputeResult<usize> {
    match value {
        Some(value) => f64_to_usize(value, fixture_id, field),
        None => Ok(default),
    }
}

fn parse_numeric_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(parse_numeric_token)
        .collect()
}

fn parse_numeric_token(token: &str) -> Option<f64> {
    let normalized = token
        .trim()
        .trim_end_matches([',', ';'])
        .replace(['D', 'd'], "E");
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
    if !value.is_finite() {
        return Err(eels_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }

    let rounded = value.round();
    if (value - rounded).abs() > 1.0e-6 {
        return Err(eels_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }

    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(eels_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }

    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(eels_parse_error(
            fixture_id,
            format!("{} must be non-negative", field),
        ));
    }
    Ok(integer as usize)
}

fn eels_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::input_validation(
        "INPUT.EELS_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
