use crate::domain::{
    FeffError, InputCard, InputCardContinuation, InputCardKind, InputDeck, ParserResult,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputTokenLine {
    pub source_line: usize,
    pub raw: String,
    pub tokens: Vec<String>,
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

    Ok(InputDeck { cards })
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
        let deck =
            parse_input_deck("\n title copper test\n\n edge k\n").expect("valid deck should parse");

        assert_eq!(deck.cards.len(), 2);
        assert_eq!(deck.cards[0].keyword, "TITLE");
        assert_eq!(deck.cards[0].kind, InputCardKind::Title);
        assert_eq!(deck.cards[0].values, vec!["copper", "test"]);
        assert_eq!(deck.cards[1].keyword, "EDGE");
        assert_eq!(deck.cards[1].source_line, 4);
    }

    #[test]
    fn parser_ignores_inline_comments() {
        let deck =
            parse_input_deck("CONTROL 1 1 1 * existing comment").expect("valid deck should parse");

        assert_eq!(deck.cards.len(), 1);
        assert_eq!(deck.cards[0].keyword, "CONTROL");
        assert_eq!(deck.cards[0].values, vec!["1", "1", "1"]);
        assert!(deck.cards[0].continuations.is_empty());
    }

    #[test]
    fn parser_preserves_unknown_card_tokens_for_diagnostics() {
        let deck = parse_input_deck("futurecard 1 2 3").expect("unknown cards should be preserved");
        assert_eq!(deck.cards.len(), 1);
        assert_eq!(deck.cards[0].keyword, "FUTURECARD");
        assert_eq!(
            deck.cards[0].kind,
            InputCardKind::Unknown("FUTURECARD".to_string())
        );
        assert_eq!(deck.cards[0].values, vec!["1", "2", "3"]);
    }

    #[test]
    fn parser_attaches_numeric_continuation_lines_to_previous_card() {
        let deck =
            parse_input_deck("ELNES 5.0 0.05 0.05\n300\n0 0 1\nATOMS\n0.0 0.0 0.0 0 Cu\nEND\n")
                .expect("deck with continuations should parse");

        assert_eq!(deck.cards.len(), 3);
        assert_eq!(deck.cards[0].kind, InputCardKind::Elnes);
        assert_eq!(deck.cards[0].continuations.len(), 2);
        assert_eq!(deck.cards[0].continuations[0].values, vec!["300"]);
        assert_eq!(deck.cards[0].continuations[1].values, vec!["0", "0", "1"]);
        assert_eq!(deck.cards[1].kind, InputCardKind::Atoms);
        assert_eq!(deck.cards[1].continuations.len(), 1);
        assert_eq!(
            deck.cards[1].continuations[0].values,
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
