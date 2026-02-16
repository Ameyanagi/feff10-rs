use crate::domain::{InputCard, InputDeck};

pub fn parse_input_deck(source: &str) -> InputDeck {
    let cards = source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| parse_card(index + 1, line))
        .collect();

    InputDeck { cards }
}

fn parse_card(source_line: usize, line: &str) -> Option<InputCard> {
    let trimmed = strip_inline_comment(line).trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let keyword = parts.next()?.to_ascii_uppercase();
    let values = parts.map(ToOwned::to_owned).collect();

    Some(InputCard {
        keyword,
        values,
        source_line,
    })
}

fn strip_inline_comment(line: &str) -> &str {
    if let Some((prefix, _)) = line.split_once('*') {
        prefix
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::parse_input_deck;

    #[test]
    fn parser_normalizes_keywords_and_ignores_blank_lines() {
        let deck = parse_input_deck("\n title copper test\n\n edge k\n");

        assert_eq!(deck.cards.len(), 2);
        assert_eq!(deck.cards[0].keyword, "TITLE");
        assert_eq!(deck.cards[0].values, vec!["copper", "test"]);
        assert_eq!(deck.cards[1].keyword, "EDGE");
        assert_eq!(deck.cards[1].source_line, 4);
    }

    #[test]
    fn parser_ignores_inline_comments() {
        let deck = parse_input_deck("CONTROL 1 1 1 * existing comment");

        assert_eq!(deck.cards.len(), 1);
        assert_eq!(deck.cards[0].keyword, "CONTROL");
        assert_eq!(deck.cards[0].values, vec!["1", "1", "1"]);
    }
}
