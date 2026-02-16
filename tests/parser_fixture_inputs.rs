use feff10_rs::domain::InputCardKind;
use feff10_rs::parser::parse_input_deck;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct FixtureManifest {
    fixtures: Vec<FixtureDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureDefinition {
    id: String,
    input_directory: String,
    entry_files: Vec<String>,
}

#[test]
fn parser_supports_known_cards_in_fixture_entry_input_files() {
    let manifest = read_manifest("tasks/golden-fixture-manifest.json");
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    for fixture in &manifest.fixtures {
        for entry_file in &fixture.entry_files {
            if !entry_file.ends_with(".inp") {
                continue;
            }

            let input_path = project_root.join(&fixture.input_directory).join(entry_file);
            if !input_path.exists() {
                continue;
            }

            let source = fs::read_to_string(&input_path).unwrap_or_else(|error| {
                panic!(
                    "fixture {} entry file {} should be readable: {}",
                    fixture.id,
                    input_path.display(),
                    error
                )
            });
            let deck = parse_input_deck(&source).unwrap_or_else(|error| {
                panic!(
                    "fixture {} entry file {} should parse: {}",
                    fixture.id,
                    input_path.display(),
                    error
                )
            });

            let unknown_cards: Vec<String> = deck
                .cards
                .iter()
                .filter_map(|card| match &card.kind {
                    InputCardKind::Unknown(keyword) => Some(keyword.clone()),
                    _ => None,
                })
                .collect();
            assert!(
                unknown_cards.is_empty(),
                "fixture {} entry file {} has uncovered cards: {:?}",
                fixture.id,
                input_path.display(),
                unknown_cards
            );
        }
    }
}

fn read_manifest(path: &str) -> FixtureManifest {
    let source = fs::read_to_string(Path::new(path))
        .unwrap_or_else(|error| panic!("fixture manifest {} should be readable: {}", path, error));
    serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("fixture manifest {} should parse: {}", path, error))
}
