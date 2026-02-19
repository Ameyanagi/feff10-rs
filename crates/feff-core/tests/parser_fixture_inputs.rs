use feff_core::domain::InputCardKind;
use feff_core::parser::parse_input_deck;
use serde::Deserialize;
use std::fs;
use std::path::Path;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

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
    baseline_status: String,
}

#[test]
fn parser_supports_known_cards_in_fixture_entry_input_files() {
    let manifest = read_manifest("tasks/golden-fixture-manifest.json");
    let project_root = workspace_root();

    for fixture in &manifest.fixtures {
        for entry_file in &fixture.entry_files {
            if !entry_file.ends_with(".inp") {
                continue;
            }

            if fixture.baseline_status == "requires_fortran_capture"
                && entry_file.starts_with("REFERENCE/")
            {
                eprintln!(
                    "skipping capture-only fixture {} staged entry file {}",
                    fixture.id, entry_file
                );
                continue;
            }

            let source = match read_fixture_entry_source(&project_root, fixture, entry_file) {
                Ok(source) => source,
                Err(error) if fixture.baseline_status == "requires_fortran_capture" => {
                    eprintln!(
                        "skipping capture-only fixture {} entry file {}: {}",
                        fixture.id, entry_file, error
                    );
                    continue;
                }
                Err(error) => panic!(
                    "fixture {} entry file {} should resolve from fixture input directory or baseline snapshot: {}",
                    fixture.id, entry_file, error
                ),
            };
            let deck = parse_input_deck(&source).unwrap_or_else(|error| {
                panic!(
                    "fixture {} entry file {} should parse without unsupported-card failures: {}",
                    fixture.id, entry_file, error
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
                entry_file,
                unknown_cards
            );
        }
    }
}

fn read_fixture_entry_source(
    project_root: &Path,
    fixture: &FixtureDefinition,
    entry_file: &str,
) -> Result<String, String> {
    let direct_path = project_root.join(&fixture.input_directory).join(entry_file);
    if direct_path.exists() {
        return fs::read_to_string(&direct_path).map_err(|error| {
            format!(
                "failed to read fixture path '{}': {}",
                direct_path.display(),
                error
            )
        });
    }

    let Some(entry_basename) = Path::new(entry_file).file_name() else {
        return Err(format!(
            "entry file '{}' does not have a valid basename",
            entry_file
        ));
    };
    let baseline_path = project_root
        .join("artifacts/fortran-baselines")
        .join(&fixture.id)
        .join("baseline")
        .join(entry_basename);
    if baseline_path.exists() {
        return fs::read_to_string(&baseline_path).map_err(|error| {
            format!(
                "failed to read baseline snapshot path '{}': {}",
                baseline_path.display(),
                error
            )
        });
    }

    Err(format!(
        "missing both fixture path '{}' and baseline snapshot '{}'",
        direct_path.display(),
        baseline_path.display()
    ))
}

fn read_manifest(path: &str) -> FixtureManifest {
    let full_path = workspace_root().join(path);
    let source = fs::read_to_string(&full_path).unwrap_or_else(|error| {
        panic!(
            "fixture manifest {} should be readable: {}",
            full_path.display(),
            error
        )
    });
    serde_json::from_str(&source).unwrap_or_else(|error| {
        panic!(
            "fixture manifest {} should parse: {}",
            full_path.display(),
            error
        )
    })
}
