use super::SCREEN_REQUIRED_INPUTS;
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(super) struct PotScreenInput {
    pub(super) title: String,
    pub(super) gamach: f64,
    pub(super) rfms1: f64,
    pub(super) mean_folp: f64,
    pub(super) mean_xion: f64,
    pub(super) lmaxsc_max: i32,
}

#[derive(Debug, Clone)]
pub(super) struct GeomScreenInput {
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

#[derive(Debug, Clone)]
pub(super) struct LdosScreenInput {
    pub(super) neldos: i32,
    pub(super) rfms2: f64,
    pub(super) emin: f64,
    pub(super) emax: f64,
    pub(super) eimag: f64,
    pub(super) rgrd: f64,
    pub(super) toler1: f64,
    pub(super) toler2: f64,
    pub(super) lmaxph_max: i32,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ScreenOverrideInput {
    pub(super) ner: Option<i32>,
    pub(super) nei: Option<i32>,
    pub(super) maxl: Option<i32>,
    pub(super) irrh: Option<i32>,
    pub(super) iend: Option<i32>,
    pub(super) lfxc: Option<i32>,
    pub(super) emin: Option<f64>,
    pub(super) emax: Option<f64>,
    pub(super) eimax: Option<f64>,
    pub(super) ermin: Option<f64>,
    pub(super) rfms: Option<f64>,
    pub(super) nrptx0: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
struct PotentialRow {
    lmaxsc: i32,
    xion: f64,
    folp: f64,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Screen {
        return Err(FeffError::input_validation(
            "INPUT.SCREEN_MODULE",
            format!(
                "SCREEN module expects SCREEN, got {}",
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
                "INPUT.SCREEN_INPUT_ARTIFACT",
                format!(
                    "SCREEN module expects input artifact '{}' at '{}'",
                    SCREEN_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(SCREEN_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.SCREEN_INPUT_ARTIFACT",
            format!(
                "SCREEN module requires input artifact '{}' but received '{}'",
                SCREEN_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.SCREEN_INPUT_ARTIFACT",
            format!(
                "SCREEN module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.SCREEN_INPUT_READ",
            format!(
                "failed to read SCREEN input '{}' ({}): {}",
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

pub(super) fn parse_pot_source(fixture_id: &str, source: &str) -> ComputeResult<PotScreenInput> {
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
        return Err(screen_parse_error(
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

    Ok(PotScreenInput {
        title,
        gamach,
        rfms1,
        mean_folp: folp_sum / potential_rows.len() as f64,
        mean_xion: xion_sum / potential_rows.len() as f64,
        lmaxsc_max,
    })
}

pub(super) fn parse_geom_source(fixture_id: &str, source: &str) -> ComputeResult<GeomScreenInput> {
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
        return Err(screen_parse_error(
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

    Ok(GeomScreenInput {
        nat: nat_value,
        nph: nph_value,
        atoms,
        radius_mean,
        radius_rms,
        radius_max,
    })
}

pub(super) fn parse_ldos_source(fixture_id: &str, source: &str) -> ComputeResult<LdosScreenInput> {
    let lines: Vec<&str> = source.lines().collect();

    let mut neldos: Option<i32> = None;
    let mut rfms2: Option<f64> = None;
    let mut emin: Option<f64> = None;
    let mut emax: Option<f64> = None;
    let mut eimag: Option<f64> = None;
    let mut rgrd: Option<f64> = None;
    let mut toler1: Option<f64> = None;
    let mut toler2: Option<f64> = None;
    let mut lmaxph_max: Option<i32> = None;

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
                    neldos = if values.len() >= 6 {
                        Some(f64_to_i32(values[5], fixture_id, "ldos.inp neldos")?)
                    } else {
                        Some(101)
                    };
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
                    rgrd = Some(values[4]);
                }
            }
            continue;
        }

        if lower.starts_with("rdirec") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 3 {
                    toler1 = Some(values[1]);
                    toler2 = Some(values[2]);
                }
            }
            continue;
        }

        if lower.contains("lmaxph")
            && let Some((_, values_line)) = next_nonempty_line(&lines, index + 1)
        {
            let values = parse_numeric_tokens(values_line);
            if !values.is_empty() {
                let mut local_max = i32::MIN;
                for value in values {
                    local_max = local_max.max(f64_to_i32(value, fixture_id, "ldos.inp lmaxph")?);
                }
                lmaxph_max = Some(local_max.max(1));
            }
        }
    }

    let neldos = neldos.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing neldos in mldos/lfms2 control block",
        )
    })?;
    let rfms2 = rfms2.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
        )
    })?;
    let emin = emin.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
        )
    })?;
    let emax = emax.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
        )
    })?;
    let eimag = eimag.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
        )
    })?;
    let rgrd = rgrd.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
        )
    })?;

    Ok(LdosScreenInput {
        neldos,
        rfms2,
        emin,
        emax,
        eimag,
        rgrd,
        toler1: toler1.unwrap_or(1.0e-3),
        toler2: toler2.unwrap_or(1.0e-3),
        lmaxph_max: lmaxph_max.unwrap_or(1).max(1),
    })
}

pub(super) fn parse_screen_override_source(
    fixture_id: &str,
    source: &str,
) -> ComputeResult<ScreenOverrideInput> {
    let mut parsed = ScreenOverrideInput::default();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let mut tokens = trimmed.split_whitespace();
        let Some(raw_key) = tokens.next() else {
            continue;
        };
        let Some(raw_value) = tokens.next() else {
            continue;
        };

        let key = raw_key
            .trim_matches(|character: char| matches!(character, ':' | ',' | ';'))
            .to_ascii_lowercase();
        let Some(value) = parse_numeric_token(raw_value) else {
            continue;
        };

        match key.as_str() {
            "ner" => parsed.ner = Some(f64_to_i32(value, fixture_id, "screen.inp ner")?),
            "nei" => parsed.nei = Some(f64_to_i32(value, fixture_id, "screen.inp nei")?),
            "maxl" => parsed.maxl = Some(f64_to_i32(value, fixture_id, "screen.inp maxl")?),
            "irrh" => parsed.irrh = Some(f64_to_i32(value, fixture_id, "screen.inp irrh")?),
            "iend" => parsed.iend = Some(f64_to_i32(value, fixture_id, "screen.inp iend")?),
            "lfxc" => parsed.lfxc = Some(f64_to_i32(value, fixture_id, "screen.inp lfxc")?),
            "emin" => parsed.emin = Some(value),
            "emax" => parsed.emax = Some(value),
            "eimax" => parsed.eimax = Some(value),
            "ermin" => parsed.ermin = Some(value),
            "rfms" => parsed.rfms = Some(value),
            "nrptx0" => parsed.nrptx0 = Some(f64_to_i32(value, fixture_id, "screen.inp nrptx0")?),
            _ => {}
        }
    }

    Ok(parsed)
}

pub(super) fn format_scientific_f64(value: f64) -> String {
    format!("{value:.10E}")
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
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
        return Err(screen_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }

    let rounded = value.round();
    if (value - rounded).abs() > 1.0e-6 {
        return Err(screen_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }

    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(screen_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }

    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(screen_parse_error(
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

fn screen_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.SCREEN_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}
