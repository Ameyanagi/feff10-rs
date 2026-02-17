use super::CRPA_REQUIRED_INPUTS;
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub(super) struct CrpaControlInput {
    pub(super) rcut: f64,
    pub(super) l_crpa: i32,
}

#[derive(Debug, Clone)]
pub(super) struct PotCrpaInput {
    pub(super) title: String,
    pub(super) gamach: f64,
    pub(super) rfms1: f64,
    pub(super) mean_folp: f64,
    pub(super) mean_xion: f64,
    pub(super) lmaxsc_max: i32,
}

#[derive(Debug, Clone)]
pub(super) struct GeomCrpaInput {
    pub(super) nat: usize,
    pub(super) nph: usize,
    pub(super) atoms: Vec<AtomSite>,
    pub(super) radius_mean: f64,
    pub(super) radius_rms: f64,
    pub(super) radius_max: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct AtomSite {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) z: f64,
    pub(super) ipot: i32,
}

#[derive(Debug, Clone, Copy)]
struct PotentialRow {
    lmaxsc: i32,
    xion: f64,
    folp: f64,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Crpa {
        return Err(FeffError::input_validation(
            "INPUT.CRPA_MODULE",
            format!("CRPA module expects CRPA, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.CRPA_INPUT_ARTIFACT",
                format!(
                    "CRPA module expects input artifact '{}' at '{}'",
                    CRPA_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(CRPA_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.CRPA_INPUT_ARTIFACT",
            format!(
                "CRPA module requires input artifact '{}' but received '{}'",
                CRPA_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.CRPA_INPUT_ARTIFACT",
            format!(
                "CRPA module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.CRPA_INPUT_READ",
            format!(
                "failed to read CRPA input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

pub(super) fn parse_crpa_source(fixture_id: &str, source: &str) -> ComputeResult<CrpaControlInput> {
    let lines: Vec<&str> = source.lines().collect();
    let mut do_crpa: Option<i32> = None;
    let mut rcut: Option<f64> = None;
    let mut l_crpa: Option<i32> = None;

    for index in 0..lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(raw_keyword) = trimmed.split_whitespace().next() else {
            continue;
        };
        let keyword = raw_keyword
            .trim_matches(|character: char| matches!(character, ':' | ',' | ';'))
            .to_ascii_lowercase();

        let value = parse_keyword_value(&lines, index).or_else(|| {
            next_nonempty_line(&lines, index + 1)
                .and_then(|(_, line)| parse_numeric_tokens(line).into_iter().next())
        });

        match keyword.as_str() {
            "do_crpa" => {
                if let Some(parsed) = value {
                    do_crpa = Some(f64_to_i32(parsed, fixture_id, "crpa.inp do_CRPA")?);
                }
            }
            "rcut" => {
                if let Some(parsed) = value {
                    rcut = Some(parsed.abs());
                }
            }
            "l_crpa" => {
                if let Some(parsed) = value {
                    l_crpa = Some(f64_to_i32(parsed, fixture_id, "crpa.inp l_crpa")?);
                }
            }
            _ => {}
        }
    }

    let do_crpa = do_crpa.ok_or_else(|| {
        crpa_parse_error(fixture_id, "crpa.inp missing required do_CRPA control flag")
    })?;
    if do_crpa <= 0 {
        return Err(crpa_parse_error(
            fixture_id,
            "crpa.inp requires do_CRPA = 1 for CRPA runtime execution",
        ));
    }

    Ok(CrpaControlInput {
        rcut: rcut.unwrap_or(1.5).max(1.0e-6),
        l_crpa: l_crpa.unwrap_or(3).max(1),
    })
}

pub(super) fn parse_pot_source(fixture_id: &str, source: &str) -> ComputeResult<PotCrpaInput> {
    let lines: Vec<&str> = source.lines().collect();
    let title = lines
        .iter()
        .map(|line| line.trim())
        .find(|line| {
            !line.is_empty()
                && parse_numeric_tokens(line).is_empty()
                && !line.contains(',')
                && !line.ends_with(':')
        })
        .unwrap_or("untitled")
        .to_string();

    let mut gamach = 1.0_f64;
    let mut rfms1 = 4.0_f64;
    let mut potential_rows = Vec::new();

    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("gamach") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 6 {
                    gamach = values[0];
                    rfms1 = values[5];
                }
            }
            index += 1;
            continue;
        }

        if lower.starts_with("iz") && lower.contains("lmaxsc") {
            index += 1;
            while index < lines.len() {
                let row = lines[index].trim();
                if row.is_empty() {
                    index += 1;
                    continue;
                }

                let values = parse_numeric_tokens(row);
                if values.len() < 5 {
                    break;
                }

                potential_rows.push(PotentialRow {
                    lmaxsc: f64_to_i32(values[1], fixture_id, "pot.inp potential lmaxsc")?,
                    xion: values[3],
                    folp: values[4],
                });
                index += 1;
            }
            continue;
        }

        index += 1;
    }

    if potential_rows.is_empty() {
        return Err(crpa_parse_error(
            fixture_id,
            "pot.inp does not contain any potential rows",
        ));
    }

    let folp_sum: f64 = potential_rows.iter().map(|row| row.folp).sum();
    let xion_sum: f64 = potential_rows.iter().map(|row| row.xion).sum();
    let lmaxsc_max = potential_rows
        .iter()
        .map(|row| row.lmaxsc)
        .max()
        .unwrap_or(1)
        .max(1);

    Ok(PotCrpaInput {
        title,
        gamach,
        rfms1,
        mean_folp: folp_sum / potential_rows.len() as f64,
        mean_xion: xion_sum / potential_rows.len() as f64,
        lmaxsc_max,
    })
}

pub(super) fn parse_geom_source(fixture_id: &str, source: &str) -> ComputeResult<GeomCrpaInput> {
    let mut nat: Option<usize> = None;
    let mut nph: Option<usize> = None;
    let mut atoms = Vec::new();

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
            atoms.push(AtomSite {
                x: values[1],
                y: values[2],
                z: values[3],
                ipot: f64_to_i32(values[4], fixture_id, "geom.dat atom ipot")?,
            });
        }
    }

    if atoms.is_empty() {
        return Err(crpa_parse_error(
            fixture_id,
            "geom.dat does not contain any atom rows",
        ));
    }

    let nat_value = nat.unwrap_or(atoms.len()).max(atoms.len());
    let nph_value = nph.unwrap_or(1).max(1);

    let mut radius_sum = 0.0_f64;
    let mut radius_sq_sum = 0.0_f64;
    let mut radius_max = 0.0_f64;
    for atom in &atoms {
        let radius = (atom.x * atom.x + atom.y * atom.y + atom.z * atom.z).sqrt();
        radius_sum += radius;
        radius_sq_sum += radius * radius;
        radius_max = radius_max.max(radius);
    }

    let atom_count = atoms.len() as f64;
    let radius_mean = radius_sum / atom_count;
    let radius_rms = (radius_sq_sum / atom_count).sqrt();

    Ok(GeomCrpaInput {
        nat: nat_value,
        nph: nph_value,
        atoms,
        radius_mean,
        radius_rms,
        radius_max,
    })
}

fn parse_keyword_value(lines: &[&str], index: usize) -> Option<f64> {
    let line = lines.get(index)?;
    line.split_whitespace()
        .skip(1)
        .find_map(parse_numeric_token)
}

fn next_nonempty_line<'a>(lines: &'a [&'a str], start_index: usize) -> Option<(usize, &'a str)> {
    for (offset, line) in lines.iter().enumerate().skip(start_index) {
        if !line.trim().is_empty() {
            return Some((offset, *line));
        }
    }

    None
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
    if !value.is_finite() {
        return Err(crpa_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }

    let rounded = value.round();
    if (value - rounded).abs() > 1.0e-6 {
        return Err(crpa_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }

    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(crpa_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }

    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(crpa_parse_error(
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

fn crpa_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.CRPA_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
