use crate::domain::{FeffError, InputCard, InputDeck, ParserResult};

pub fn parse_input_deck(source: &str) -> ParserResult<InputDeck> {
    let mut cards = Vec::new();
    for (index, line) in source.lines().enumerate() {
        if let Some(card) = parse_card(index + 1, line)? {
            cards.push(card);
        }
    }

    Ok(InputDeck { cards })
}

fn parse_card(source_line: usize, line: &str) -> ParserResult<Option<InputCard>> {
    let trimmed = strip_inline_comment(line).trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let mut parts = trimmed.split_whitespace();
    let Some(keyword) = parts.next() else {
        return Ok(None);
    };
    if !is_valid_keyword(keyword) {
        return Err(FeffError::input_validation(
            "INPUT.INVALID_CARD",
            format!("invalid card keyword '{}' at line {}", keyword, source_line),
        ));
    }

    let keyword = keyword.to_ascii_uppercase();
    let values = parts.map(ToOwned::to_owned).collect();

    Ok(Some(InputCard {
        keyword,
        values,
        source_line,
    }))
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
        Some(ch) if ch.is_ascii_alphabetic() => {}
        _ => return false,
    }

    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::parse_input_deck;
    use crate::domain::FeffErrorCategory;

    #[test]
    fn parser_normalizes_keywords_and_ignores_blank_lines() {
        let deck =
            parse_input_deck("\n title copper test\n\n edge k\n").expect("valid deck should parse");

        assert_eq!(deck.cards.len(), 2);
        assert_eq!(deck.cards[0].keyword, "TITLE");
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
    }

    #[test]
    fn parser_reports_invalid_card_keyword_with_shared_error() {
        let error = parse_input_deck("9bad 1 2 3").expect_err("invalid card should fail");
        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.INVALID_CARD");
        assert_eq!(error.exit_code(), 2);
    }
}
