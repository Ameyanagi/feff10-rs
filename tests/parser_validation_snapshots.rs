use feff10_rs::domain::{FeffError, InputDeck};
use feff10_rs::parser::parse_input_deck;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

#[test]
fn parser_snapshot_valid_main_deck_with_defaults() {
    let input = "\
TITLE Cu
EDGE K
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
";

    let deck = parse_input_deck(input).expect("valid deck should parse");
    let snapshot = serde_json::to_string_pretty(&deck_snapshot(&deck))
        .expect("deck snapshot should serialize");
    assert_snapshot("valid_main_deck_with_defaults", &snapshot);
}

#[test]
fn parser_snapshot_invalid_missing_required_cards() {
    let input = "\
TITLE Cu
EDGE K
POTENTIALS
0 29 Cu
END
";

    assert_invalid_snapshot("invalid_missing_required_cards", input);
}

#[test]
fn parser_snapshot_invalid_empty_deck() {
    let input = "\
* full-line comments should be removed
  
* and this still stays empty
";
    assert_invalid_snapshot("invalid_empty_deck", input);
}

#[test]
fn parser_snapshot_invalid_orphaned_continuation() {
    let input = "\
300 0 1
";
    assert_invalid_snapshot("invalid_orphaned_continuation", input);
}

#[test]
fn parser_snapshot_invalid_duplicate_singleton_card() {
    let input = "\
CIF cu.cif
CONTROL 1 1 1 1 1 1
CONTROL 0 0 0 0 0 0
END
";
    assert_invalid_snapshot("invalid_duplicate_singleton_card", input);
}

fn deck_snapshot(deck: &InputDeck) -> Value {
    json!({
        "cardCount": deck.cards.len(),
        "cards": deck.cards.iter().map(|card| {
            json!({
                "keyword": card.keyword,
                "kind": format!("{:?}", card.kind),
                "sourceLine": card.source_line,
                "values": card.values,
                "continuations": card.continuations.iter().map(|continuation| {
                    json!({
                        "sourceLine": continuation.source_line,
                        "values": continuation.values,
                        "raw": continuation.raw
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>()
    })
}

fn assert_invalid_snapshot(name: &str, input: &str) {
    let error = parse_input_deck(input).expect_err("deck should fail parser validation");
    let snapshot = serde_json::to_string_pretty(&error_snapshot(&error))
        .expect("error snapshot should serialize");
    assert_snapshot(name, &snapshot);
}

fn error_snapshot(error: &FeffError) -> Value {
    json!({
        "category": format!("{:?}", error.category()),
        "placeholder": error.placeholder(),
        "message": error.message(),
        "diagnostic": error.diagnostic_line(),
        "fatalExitLine": error.fatal_exit_line(),
        "exitCode": error.exit_code()
    })
}

fn assert_snapshot(name: &str, actual: &str) {
    let snapshot_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join("parser")
        .join(format!("{}.snap", name));
    let expected = fs::read_to_string(&snapshot_path).unwrap_or_else(|error| {
        panic!(
            "snapshot file {} should be readable: {}",
            snapshot_path.display(),
            error
        )
    });
    assert_eq!(
        actual.trim_end(),
        expected.trim_end(),
        "snapshot mismatch for {}",
        name
    );
}
