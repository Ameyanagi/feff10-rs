use crate::domain::{
    FeffError, InputCard, InputCardContinuation, InputCardKind, InputDeck, ParserResult,
};

const DEFAULT_CONTROL_VALUES: [&str; 6] = ["1", "1", "1", "1", "1", "1"];
const DEFAULT_PRINT_VALUES: [&str; 6] = ["0", "0", "0", "0", "0", "0"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputTokenLine {
    pub source_line: usize,
    pub raw: String,
    pub tokens: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationProfile {
    MainDeck,
    DebyeSpringDeck,
}

pub fn tokenize_input_deck(source: &str) -> Vec<InputTokenLine> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| tokenize_line(index + 1, line))
        .collect()
}

pub fn parse_input_deck(source: &str) -> ParserResult<InputDeck> {
    let mut cards = Vec::new();
    for token_line in tokenize_input_deck(source) {
        let Some(first_token) = token_line.tokens.first().map(|token| token.as_str()) else {
            continue;
        };

        if is_valid_keyword(first_token) {
            let keyword = first_token.to_ascii_uppercase();
            let kind = InputCardKind::from_keyword(&keyword);
            let values = token_line.tokens.into_iter().skip(1).collect();
            cards.push(InputCard::new(
                keyword,
                kind,
                values,
                token_line.source_line,
            ));
            continue;
        }

        let Some(last_card) = cards.last_mut() else {
            return Err(FeffError::input_validation(
                "INPUT.INVALID_CARD",
                format!(
                    "invalid card keyword '{}' at line {}",
                    first_token, token_line.source_line
                ),
            ));
        };

        last_card.continuations.push(InputCardContinuation {
            source_line: token_line.source_line,
            values: token_line.tokens,
            raw: token_line.raw,
        });
    }

    validate_input_deck(InputDeck { cards })
}

fn validate_input_deck(mut deck: InputDeck) -> ParserResult<InputDeck> {
    if deck.cards.is_empty() {
        return Err(FeffError::input_validation(
            "INPUT.EMPTY_DECK",
            "input deck is empty after removing comments and blank lines",
        ));
    }

    match determine_validation_profile(&deck) {
        ValidationProfile::MainDeck => validate_main_deck(&mut deck)?,
        ValidationProfile::DebyeSpringDeck => validate_debye_spring_deck(&mut deck)?,
    }

    Ok(deck)
}

fn determine_validation_profile(deck: &InputDeck) -> ValidationProfile {
    let has_spring_cards = has_any_keyword(deck, &["VDOS", "STRETCHES"]);
    let has_non_spring_cards = deck.cards.iter().any(|card| {
        !matches!(
            card.kind,
            InputCardKind::Title
                | InputCardKind::Vdos
                | InputCardKind::Stretches
                | InputCardKind::End
        )
    });

    if has_spring_cards && !has_non_spring_cards {
        ValidationProfile::DebyeSpringDeck
    } else {
        ValidationProfile::MainDeck
    }
}

fn validate_main_deck(deck: &mut InputDeck) -> ParserResult<()> {
    ensure_singleton_card(deck, "CONTROL")?;
    ensure_singleton_card(deck, "PRINT")?;
    ensure_singleton_card(deck, "END")?;

    ensure_required_any(deck, &["TITLE", "CIF"])?;
    ensure_required_any(deck, &["ATOMS", "CIF"])?;
    ensure_required_any(deck, &["POTENTIALS", "POTENTIAL", "CIF"])?;

    ensure_default_card(deck, "CONTROL", &DEFAULT_CONTROL_VALUES);
    ensure_default_card(deck, "PRINT", &DEFAULT_PRINT_VALUES);
    ensure_default_card(deck, "END", &[]);

    Ok(())
}

fn validate_debye_spring_deck(deck: &mut InputDeck) -> ParserResult<()> {
    ensure_singleton_card(deck, "VDOS")?;
    ensure_singleton_card(deck, "STRETCHES")?;
    ensure_singleton_card(deck, "END")?;

    ensure_required_any(deck, &["VDOS"])?;
    ensure_required_any(deck, &["STRETCHES"])?;

    ensure_default_card(deck, "END", &[]);
    Ok(())
}

fn ensure_required_any(deck: &InputDeck, keywords: &[&str]) -> ParserResult<()> {
    if has_any_keyword(deck, keywords) {
        return Ok(());
    }

    Err(FeffError::input_validation(
        "INPUT.MISSING_REQUIRED_CARD",
        format!(
            "missing required card: expected one of {}",
            keywords.join(", ")
        ),
    ))
}

fn ensure_singleton_card(deck: &InputDeck, keyword: &str) -> ParserResult<()> {
    let count = deck
        .cards
        .iter()
        .filter(|card| card.keyword == keyword)
        .count();
    if count <= 1 {
        return Ok(());
    }

    Err(FeffError::input_validation(
        "INPUT.DUPLICATE_SINGLETON_CARD",
        format!("card '{}' may only appear once (found {})", keyword, count),
    ))
}

fn ensure_default_card(deck: &mut InputDeck, keyword: &str, values: &[&str]) {
    if deck.cards.iter().any(|card| card.keyword == keyword) {
        return;
    }

    let values = values.iter().map(|value| (*value).to_string()).collect();
    let card = InputCard::new(keyword, InputCardKind::from_keyword(keyword), values, 0);
    if let Some(end_index) = deck
        .cards
        .iter()
        .position(|existing| existing.keyword == "END")
    {
        deck.cards.insert(end_index, card);
    } else {
        deck.cards.push(card);
    }
}

fn has_any_keyword(deck: &InputDeck, keywords: &[&str]) -> bool {
    deck.cards
        .iter()
        .any(|card| keywords.iter().any(|keyword| *keyword == card.keyword))
}

fn tokenize_line(source_line: usize, line: &str) -> Option<InputTokenLine> {
    let normalized = strip_inline_comment(line).trim();
    if normalized.is_empty() {
        return None;
    }

    let tokens: Vec<String> = normalized
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect();
    if tokens.is_empty() {
        return None;
    }

    Some(InputTokenLine {
        source_line,
        raw: normalized.to_owned(),
        tokens,
    })
}

fn strip_inline_comment(line: &str) -> &str {
    if let Some((prefix, _)) = line.split_once('*') {
        prefix
    } else {
        line
    }
}

fn is_valid_keyword(keyword: &str) -> bool {
    let mut chars = keyword.chars();
    match chars.next() {
        Some(ch) if ch.is_ascii_alphabetic() || ch == '_' => {}
        _ => return false,
    }

    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::{parse_input_deck, tokenize_input_deck};
    use crate::domain::{FeffErrorCategory, InputCardKind};

    #[test]
    fn parser_normalizes_keywords_and_ignores_blank_lines() {
        let deck = parse_input_deck(
            "\n title copper test\n\n edge k\npotentials\n0 29 Cu\natoms\n0.0 0.0 0.0 0 Cu\n",
        )
        .expect("valid deck should parse");

        assert_eq!(deck.cards.len(), 7);
        assert_eq!(deck.cards[0].keyword, "TITLE");
        assert_eq!(deck.cards[0].kind, InputCardKind::Title);
        assert_eq!(deck.cards[0].values, vec!["copper", "test"]);
        assert_eq!(deck.cards[1].keyword, "EDGE");
        assert_eq!(deck.cards[1].source_line, 4);
        assert_eq!(deck.cards[4].keyword, "CONTROL");
        assert_eq!(deck.cards[4].source_line, 0);
        assert_eq!(deck.cards[4].values, vec!["1", "1", "1", "1", "1", "1"]);
        assert_eq!(deck.cards[5].keyword, "PRINT");
        assert_eq!(deck.cards[5].source_line, 0);
        assert_eq!(deck.cards[6].keyword, "END");
        assert_eq!(deck.cards[6].source_line, 0);
    }

    #[test]
    fn parser_ignores_inline_comments() {
        let deck = parse_input_deck(
            "CIF cu.cif\nPOTENTIAL 0 29 Cu\nCONTROL 1 1 1 * existing comment\nEND\n",
        )
        .expect("valid deck should parse");

        assert_eq!(deck.cards.len(), 5);
        assert_eq!(deck.cards[2].keyword, "CONTROL");
        assert_eq!(deck.cards[2].values, vec!["1", "1", "1"]);
        assert!(deck.cards[2].continuations.is_empty());
        assert_eq!(deck.cards[3].keyword, "PRINT");
        assert_eq!(deck.cards[3].values, vec!["0", "0", "0", "0", "0", "0"]);
    }

    #[test]
    fn parser_preserves_unknown_card_tokens_for_diagnostics() {
        let deck = parse_input_deck("CIF cu.cif\nfuturecard 1 2 3\nEND\n")
            .expect("unknown cards should be preserved");
        assert_eq!(deck.cards.len(), 5);
        assert_eq!(deck.cards[1].keyword, "FUTURECARD");
        assert_eq!(
            deck.cards[1].kind,
            InputCardKind::Unknown("FUTURECARD".to_string())
        );
        assert_eq!(deck.cards[1].values, vec!["1", "2", "3"]);
    }

    #[test]
    fn parser_attaches_numeric_continuation_lines_to_previous_card() {
        let deck = parse_input_deck(
            "TITLE Cu\nPOTENTIALS\n0 29 Cu\nELNES 5.0 0.05 0.05\n300\n0 0 1\nATOMS\n0.0 0.0 0.0 0 Cu\nEND\n",
        )
        .expect("deck with continuations should parse");

        assert_eq!(deck.cards[2].kind, InputCardKind::Elnes);
        assert_eq!(deck.cards[2].continuations.len(), 2);
        assert_eq!(deck.cards[2].continuations[0].values, vec!["300"]);
        assert_eq!(deck.cards[2].continuations[1].values, vec!["0", "0", "1"]);
        assert_eq!(deck.cards[3].kind, InputCardKind::Atoms);
        assert_eq!(deck.cards[3].continuations.len(), 1);
        assert_eq!(
            deck.cards[3].continuations[0].values,
            vec!["0.0", "0.0", "0.0", "0", "Cu"]
        );
    }

    #[test]
    fn parser_reports_orphaned_continuation_with_shared_error() {
        let error = parse_input_deck("300 0 1").expect_err("orphaned continuation should fail");
        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.INVALID_CARD");
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn parser_reports_missing_required_card_with_shared_error_contract() {
        let error = parse_input_deck("TITLE sample\nEDGE K\nPOTENTIALS\n0 29 Cu\n")
            .expect_err("deck should fail without structure card");
        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.MISSING_REQUIRED_CARD");
        assert_eq!(error.exit_code(), 2);
        assert_eq!(
            error.diagnostic_line(),
            "ERROR: [INPUT.MISSING_REQUIRED_CARD] missing required card: expected one of ATOMS, CIF"
        );
        assert_eq!(
            error.fatal_exit_line().as_deref(),
            Some("FATAL EXIT CODE: 2")
        );
    }

    #[test]
    fn parser_supports_spring_profile_with_default_end() {
        let deck = parse_input_deck("VDOS 0.03 0.5 1\nSTRETCHES\n0 1 27.9 2.0\n")
            .expect("spring deck should parse");
        assert_eq!(deck.cards.len(), 3);
        assert_eq!(deck.cards[0].kind, InputCardKind::Vdos);
        assert_eq!(deck.cards[1].kind, InputCardKind::Stretches);
        assert_eq!(deck.cards[2].kind, InputCardKind::End);
        assert_eq!(deck.cards[2].source_line, 0);
    }

    #[test]
    fn tokenizer_emits_normalized_non_comment_lines() {
        let tokens = tokenize_input_deck("TITLE Cu\n* comment\nEDGE K * inline\n");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].source_line, 1);
        assert_eq!(tokens[0].raw, "TITLE Cu");
        assert_eq!(tokens[0].tokens, vec!["TITLE", "Cu"]);
        assert_eq!(tokens[1].source_line, 3);
        assert_eq!(tokens[1].tokens, vec!["EDGE", "K"]);
    }
}
