use super::POT_REQUIRED_INPUTS;
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub(super) struct PotControl {
    pub(super) mpot: i32,
    pub(super) nph: i32,
    pub(super) ntitle: i32,
    pub(super) ihole: i32,
    pub(super) ipr1: i32,
    pub(super) iafolp: i32,
    pub(super) ixc: i32,
    pub(super) ispec: i32,
    pub(super) nmix: i32,
    pub(super) nohole: i32,
    pub(super) jumprm: i32,
    pub(super) inters: i32,
    pub(super) nscmt: i32,
    pub(super) icoul: i32,
    pub(super) lfms1: i32,
    pub(super) iunf: i32,
    pub(super) gamach: f64,
    pub(super) rgrd: f64,
    pub(super) ca1: f64,
    pub(super) ecv: f64,
    pub(super) totvol: f64,
    pub(super) rfms1: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PotentialEntry {
    pub(super) atomic_number: i32,
    pub(super) lmaxsc: i32,
    pub(super) xnatph: f64,
    pub(super) xion: f64,
    pub(super) folp: f64,
}

#[derive(Debug, Clone)]
pub(super) struct GeomModel {
    pub(super) nat: usize,
    pub(super) nph: usize,
    pub(super) atoms: Vec<AtomSite>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct AtomSite {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) z: f64,
    pub(super) ipot: i32,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Pot {
        return Err(FeffError::input_validation(
            "INPUT.POT_MODULE",
            format!("POT module expects POT, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.POT_INPUT_ARTIFACT",
                format!(
                    "POT module expects input artifact '{}' at '{}'",
                    POT_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(POT_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.POT_INPUT_ARTIFACT",
            format!(
                "POT module requires input artifact '{}' but received '{}'",
                POT_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn geom_input_path(request: &ComputeRequest) -> ComputeResult<PathBuf> {
    request
        .input_path
        .parent()
        .map(|parent| parent.join(POT_REQUIRED_INPUTS[1]))
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.POT_INPUT_ARTIFACT",
                format!(
                    "POT module requires sibling '{}' for input '{}'",
                    POT_REQUIRED_INPUTS[1],
                    request.input_path.display()
                ),
            )
        })
}

pub(super) fn read_input_source(input_path: &Path, label: &str) -> ComputeResult<String> {
    fs::read_to_string(input_path).map_err(|source| {
        FeffError::io_system(
            "IO.POT_INPUT_READ",
            format!(
                "failed to read POT input '{}' ({}): {}",
                input_path.display(),
                label,
                source
            ),
        )
    })
}

pub(super) fn parse_pot_input(
    fixture_id: &str,
    source: &str,
) -> ComputeResult<(String, PotControl, Vec<PotentialEntry>)> {
    let lines = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    let (header_index, header_values) = find_i32_row(&lines, 8, 0)
        .ok_or_else(|| pot_contract_error(fixture_id, "missing POT control row with 8 integers"))?;
    let (scf_index, scf_values) = find_i32_row(&lines, 8, header_index + 1)
        .ok_or_else(|| pot_contract_error(fixture_id, "missing SCF control row with 8 integers"))?;
    let title = lines
        .iter()
        .skip(scf_index + 1)
        .find(|line| {
            line.chars()
                .any(|character| character.is_ascii_alphabetic())
        })
        .map(|line| (*line).to_string())
        .unwrap_or_else(|| "POT input".to_string());

    let gamma_header = lines
        .iter()
        .position(|line| line.to_ascii_lowercase().contains("gamach"))
        .ok_or_else(|| pot_contract_error(fixture_id, "missing 'gamach' control header"))?;
    let (_, gamma_values) = find_f64_row(&lines, 6, gamma_header + 1).ok_or_else(|| {
        pot_contract_error(
            fixture_id,
            "missing numeric row with 6 values after 'gamach' header",
        )
    })?;

    let potential_header = lines
        .iter()
        .position(|line| line.to_ascii_lowercase().contains("iz, lmaxsc"))
        .ok_or_else(|| pot_contract_error(fixture_id, "missing potential table header"))?;

    let mut potentials = Vec::new();
    for line in lines.iter().skip(potential_header + 1) {
        if line
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic())
        {
            if !potentials.is_empty() {
                break;
            }
            continue;
        }

        if let Some(entry) = parse_potential_entry(line) {
            potentials.push(entry);
            continue;
        }

        if !potentials.is_empty() {
            break;
        }
    }

    if potentials.is_empty() {
        return Err(pot_contract_error(
            fixture_id,
            "missing potential rows after potential table header",
        ));
    }

    let control = PotControl {
        mpot: header_values[0],
        nph: header_values[1],
        ntitle: header_values[2],
        ihole: header_values[3],
        ipr1: header_values[4],
        iafolp: header_values[5],
        ixc: header_values[6],
        ispec: header_values[7],
        nmix: scf_values[0],
        nohole: scf_values[1],
        jumprm: scf_values[2],
        inters: scf_values[3],
        nscmt: scf_values[4],
        icoul: scf_values[5],
        lfms1: scf_values[6],
        iunf: scf_values[7],
        gamach: gamma_values[0],
        rgrd: gamma_values[1],
        ca1: gamma_values[2],
        ecv: gamma_values[3],
        totvol: gamma_values[4],
        rfms1: gamma_values[5],
    };

    Ok((title, control, potentials))
}

pub(super) fn parse_geom_input(fixture_id: &str, source: &str) -> ComputeResult<GeomModel> {
    let mut lines = source.lines();
    let header_line = lines
        .next()
        .ok_or_else(|| pot_contract_error(fixture_id, "geom.dat is empty"))?;
    let header_values = parse_i32_tokens(header_line);
    if header_values.len() < 2 {
        return Err(pot_contract_error(
            fixture_id,
            "geom.dat header must contain nat and nph",
        ));
    }

    let nat = usize::try_from(header_values[0]).unwrap_or(0);
    let nph = usize::try_from(header_values[1]).unwrap_or(0);
    let mut atoms = Vec::new();

    for line in lines {
        if let Some(atom) = parse_geom_atom(line) {
            atoms.push(atom);
        }
    }

    if atoms.is_empty() {
        return Err(pot_contract_error(
            fixture_id,
            "geom.dat must include at least one atom row",
        ));
    }

    Ok(GeomModel {
        nat: nat.max(atoms.len()),
        nph: nph.max(1),
        atoms,
    })
}

fn find_i32_row(lines: &[&str], minimum_fields: usize, start: usize) -> Option<(usize, Vec<i32>)> {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, line)| {
            let values = parse_i32_tokens(line);
            if values.len() >= minimum_fields {
                Some((index, values))
            } else {
                None
            }
        })
}

fn find_f64_row(lines: &[&str], minimum_fields: usize, start: usize) -> Option<(usize, Vec<f64>)> {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, line)| {
            let values = parse_f64_tokens(line);
            if values.len() >= minimum_fields {
                Some((index, values))
            } else {
                None
            }
        })
}

fn parse_potential_entry(line: &str) -> Option<PotentialEntry> {
    let columns = line.split_whitespace().collect::<Vec<_>>();
    if columns.len() < 5 {
        return None;
    }

    Some(PotentialEntry {
        atomic_number: parse_i32_token(columns[0])?,
        lmaxsc: parse_i32_token(columns[1])?,
        xnatph: parse_f64_token(columns[2])?,
        xion: parse_f64_token(columns[3])?,
        folp: parse_f64_token(columns[4])?,
    })
}

fn parse_geom_atom(line: &str) -> Option<AtomSite> {
    let columns = line.split_whitespace().collect::<Vec<_>>();
    if columns.len() < 6 {
        return None;
    }

    let _iat = parse_i32_token(columns[0])?;
    Some(AtomSite {
        x: parse_f64_token(columns[1])?,
        y: parse_f64_token(columns[2])?,
        z: parse_f64_token(columns[3])?,
        ipot: parse_i32_token(columns[4])?,
    })
}

fn parse_i32_tokens(line: &str) -> Vec<i32> {
    line.split_whitespace()
        .filter_map(parse_i32_token)
        .collect()
}

fn parse_f64_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(parse_f64_token)
        .collect()
}

fn parse_i32_token(token: &str) -> Option<i32> {
    let cleaned = token.trim_matches(|character: char| matches!(character, ',' | ';' | ':'));
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<i32>().ok()
}

fn parse_f64_token(token: &str) -> Option<f64> {
    let cleaned = token.trim_matches(|character: char| matches!(character, ',' | ';' | ':'));
    if cleaned.is_empty() {
        return None;
    }
    cleaned.replace(['D', 'd'], "E").parse::<f64>().ok()
}

fn pot_contract_error(fixture_id: &str, reason: &str) -> FeffError {
    FeffError::computation(
        "RUN.POT_INPUT_MISMATCH",
        format!(
            "fixture '{}' input contract mismatch for POT compute path: {}",
            fixture_id, reason
        ),
    )
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
