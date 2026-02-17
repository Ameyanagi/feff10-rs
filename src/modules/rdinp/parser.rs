use super::{RDINP_REQUIRED_INPUTS};
use crate::domain::{
    ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError, InputCard,
    InputDeck,
};
use std::fs;

#[derive(Debug, Clone)]
pub(super) struct PotentialEntry {
    pub(super) ipot: i32,
    pub(super) atomic_number: i32,
    pub(super) label: String,
    pub(super) explicit_xnatph: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct AtomSite {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) z: f64,
    pub(super) ipot: i32,
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Rdinp {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_MODULE",
            format!(
                "RDINP module expects RDINP, got {}",
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
                "INPUT.RDINP_INPUT_ARTIFACT",
                format!(
                    "RDINP module expects input artifact '{}' at '{}'",
                    RDINP_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;
    if !input_file_name.eq_ignore_ascii_case(RDINP_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_INPUT_ARTIFACT",
            format!(
                "RDINP module requires input artifact '{}' but received '{}'",
                RDINP_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }
    Ok(())
}

pub(super) fn read_input_source(input_path: &std::path::Path) -> ComputeResult<String> {
    fs::read_to_string(input_path).map_err(|source| {
        FeffError::io_system(
            "IO.RDINP_INPUT_READ",
            format!(
                "failed to read RDINP input '{}': {}",
                input_path.display(),
                source
            ),
        )
    })
}

pub(super) fn parse_potentials(deck: &InputDeck) -> ComputeResult<Vec<PotentialEntry>> {
    let mut rows = Vec::new();
    for card in deck
        .cards
        .iter()
        .filter(|card| card.keyword == "POTENTIALS" || card.keyword == "POTENTIAL")
    {
        if card.keyword == "POTENTIAL" && !card.values.is_empty() {
            rows.push((card.source_line, card.values.clone()));
        }
        for continuation in &card.continuations {
            if !continuation.values.is_empty() {
                rows.push((continuation.source_line, continuation.values.clone()));
            }
        }
    }

    if rows.is_empty() {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_POTENTIALS",
            "RDINP requires at least one POTENTIALS row",
        ));
    }

    let mut entries = Vec::with_capacity(rows.len());
    for (line, row) in rows {
        if row.len() < 2 {
            return Err(FeffError::input_validation(
                "INPUT.RDINP_POTENTIALS",
                format!(
                    "invalid POTENTIALS row at line {}: expected at least ipot and atomic number",
                    line
                ),
            ));
        }
        let ipot = parse_i32_token(&row[0], "POTENTIALS ipot", line)?;
        let atomic_number = parse_i32_token(&row[1], "POTENTIALS atomic number", line)?;
        let label = row.get(2).cloned().unwrap_or_else(|| format!("P{}", ipot));
        let explicit_xnatph = match row.get(5) {
            Some(token) => Some(parse_f64_token(token, "POTENTIALS xnatph", line)?),
            None => None,
        };
        entries.push(PotentialEntry {
            ipot,
            atomic_number,
            label,
            explicit_xnatph,
        });
    }
    entries.sort_by_key(|entry| entry.ipot);
    Ok(entries)
}

pub(super) fn parse_atoms(deck: &InputDeck) -> ComputeResult<Vec<AtomSite>> {
    let mut rows = Vec::new();
    for card in deck.cards.iter().filter(|card| card.keyword == "ATOMS") {
        if !card.values.is_empty() {
            rows.push((card.source_line, card.values.clone()));
        }
        for continuation in &card.continuations {
            if !continuation.values.is_empty() {
                rows.push((continuation.source_line, continuation.values.clone()));
            }
        }
    }

    if rows.is_empty() {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_ATOMS",
            "RDINP requires ATOMS entries",
        ));
    }

    let mut atoms = Vec::with_capacity(rows.len());
    for (line, row) in rows {
        if row.len() < 4 {
            return Err(FeffError::input_validation(
                "INPUT.RDINP_ATOMS",
                format!(
                    "invalid ATOMS row at line {}: expected x y z ipot fields",
                    line
                ),
            ));
        }
        atoms.push(AtomSite {
            x: parse_f64_token(&row[0], "ATOMS x", line)?,
            y: parse_f64_token(&row[1], "ATOMS y", line)?,
            z: parse_f64_token(&row[2], "ATOMS z", line)?,
            ipot: parse_i32_token(&row[3], "ATOMS ipot", line)?,
        });
    }
    Ok(atoms)
}

pub(super) fn sort_atoms_by_distance(mut atoms: Vec<AtomSite>) -> Vec<AtomSite> {
    if atoms.is_empty() {
        return atoms;
    }

    let absorber_index = atoms.iter().position(|atom| atom.ipot == 0).unwrap_or(0);
    let absorber = atoms[absorber_index];
    let mut distances: Vec<f64> = atoms
        .iter()
        .map(|atom| {
            ((atom.x - absorber.x).powi(2)
                + (atom.y - absorber.y).powi(2)
                + (atom.z - absorber.z).powi(2))
            .sqrt()
        })
        .collect();

    let mut index = 0;
    while index < atoms.len() {
        let mut swap_index = index;
        let mut minimum = distances[index];
        let mut candidate = index;
        while candidate < atoms.len() {
            if distances[candidate] < minimum {
                swap_index = candidate;
                minimum = distances[candidate];
            }
            candidate += 1;
        }
        distances.swap(index, swap_index);
        atoms.swap(index, swap_index);
        index += 1;
    }

    atoms
}

pub(super) fn card_value(deck: &InputDeck, keyword: &str, index: usize) -> ComputeResult<Option<f64>> {
    let Some(card) = first_card(deck, keyword) else {
        return Ok(None);
    };
    if index >= card.values.len() {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_CARD_VALUE",
            format!(
                "card '{}' at line {} is missing value index {}",
                keyword, card.source_line, index
            ),
        ));
    }
    let value = parse_f64_token(
        card.values[index].as_str(),
        &format!("{} value {}", keyword, index),
        card.source_line,
    )?;
    Ok(Some(value))
}

pub(super) fn required_card_value(deck: &InputDeck, keyword: &str, index: usize) -> ComputeResult<f64> {
    card_value(deck, keyword, index)?.ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.RDINP_CARD_VALUE",
            format!("missing required card '{}'", keyword),
        )
    })
}

pub(super) fn parse_f64_token(token: &str, field: &str, line: usize) -> ComputeResult<f64> {
    let normalized = token.replace('D', "E").replace('d', "e");
    normalized.parse::<f64>().map_err(|_| {
        FeffError::input_validation(
            "INPUT.RDINP_CARD_VALUE",
            format!(
                "invalid numeric token '{}' for {} at line {}",
                token, field, line
            ),
        )
    })
}

pub(super) fn parse_i32_token(token: &str, field: &str, line: usize) -> ComputeResult<i32> {
    if let Ok(value) = token.parse::<i32>() {
        return Ok(value);
    }
    let float_value = parse_f64_token(token, field, line)?;
    let rounded = float_value.round();
    if (float_value - rounded).abs() > 1.0e-9 {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_CARD_VALUE",
            format!(
                "token '{}' for {} at line {} is not an integer",
                token, field, line
            ),
        ));
    }
    Ok(rounded as i32)
}

pub(super) fn first_card<'a>(deck: &'a InputDeck, keyword: &str) -> Option<&'a InputCard> {
    deck.cards.iter().find(|card| card.keyword == keyword)
}

pub(super) fn has_card(deck: &InputDeck, keyword: &str) -> bool {
    first_card(deck, keyword).is_some()
}

pub(super) fn deck_title(deck: &InputDeck) -> String {
    if let Some(card) = first_card(deck, "TITLE")
        && !card.values.is_empty()
    {
        return card.values.join(" ");
    }
    if let Some(card) = first_card(deck, "CIF")
        && let Some(path) = card.values.first()
    {
        return format!("CIF {}", path);
    }
    "FEFF Input".to_string()
}

pub(super) fn deck_edge_label(deck: &InputDeck) -> String {
    first_card(deck, "EDGE")
        .and_then(|card| card.values.first())
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "NULL".to_string())
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
