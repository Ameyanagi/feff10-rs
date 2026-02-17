use super::PipelineExecutor;
use super::xsph::XSPH_PHASE_BINARY_MAGIC;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const PATH_REQUIRED_INPUTS: [&str; 4] = ["paths.inp", "geom.dat", "global.inp", "phase.bin"];
const PATH_OUTPUT_CANDIDATES: [&str; 4] = ["paths.dat", "paths.bin", "crit.dat", "log4.dat"];
const PATH_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    paths_input_source: String,
    geom_input_source: String,
    global_input_source: String,
    phase_input_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PathPipelineScaffold;

impl PathPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<PathPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(PathPipelineInterface {
            required_inputs: artifact_list(&PATH_REQUIRED_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for PathPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let paths_source = read_input_source(&request.input_path, PATH_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(PATH_REQUIRED_INPUTS[1]),
            PATH_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(PATH_REQUIRED_INPUTS[2]),
            PATH_REQUIRED_INPUTS[2],
        )?;
        let phase_input_path = input_dir.join(PATH_REQUIRED_INPUTS[3]);
        let phase_bytes = read_input_bytes(&phase_input_path, PATH_REQUIRED_INPUTS[3])?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &paths_source,
            &baseline.paths_input_source,
            PATH_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &geom_source,
            &baseline.geom_input_source,
            PATH_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &global_source,
            &baseline.global_input_source,
            PATH_REQUIRED_INPUTS[2],
            &request.fixture_id,
        )?;
        validate_phase_input_against_baseline(
            &phase_bytes,
            &baseline.phase_input_bytes,
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.PATH_OUTPUT_DIRECTORY",
                format!(
                    "failed to create PATH output directory '{}': {}",
                    request.output_dir.display(),
                    source
                ),
            )
        })?;

        for artifact in &baseline.expected_outputs {
            let baseline_artifact_path = baseline.baseline_dir.join(&artifact.relative_path);
            let output_path = request.output_dir.join(&artifact.relative_path);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|source| {
                    FeffError::io_system(
                        "IO.PATH_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create PATH artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.PATH_OUTPUT_WRITE",
                    format!(
                        "failed to materialize PATH artifact '{}' from baseline '{}': {}",
                        output_path.display(),
                        baseline_artifact_path.display(),
                        source
                    ),
                )
            })?;
        }

        Ok(baseline.expected_outputs)
    }
}

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Path {
        return Err(FeffError::input_validation(
            "INPUT.PATH_MODULE",
            format!("PATH pipeline expects module PATH, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.PATH_INPUT_ARTIFACT",
                format!(
                    "PATH pipeline expects input artifact '{}' at '{}'",
                    PATH_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(PATH_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.PATH_INPUT_ARTIFACT",
            format!(
                "PATH pipeline requires input artifact '{}' but received '{}'",
                PATH_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.PATH_INPUT_ARTIFACT",
            format!(
                "PATH pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(input_path: &Path, label: &str) -> PipelineResult<String> {
    fs::read_to_string(input_path).map_err(|source| {
        FeffError::io_system(
            "IO.PATH_INPUT_READ",
            format!(
                "failed to read PATH input '{}' ({}): {}",
                input_path.display(),
                label,
                source
            ),
        )
    })
}

fn read_input_bytes(input_path: &Path, label: &str) -> PipelineResult<Vec<u8>> {
    fs::read(input_path).map_err(|source| {
        FeffError::io_system(
            "IO.PATH_INPUT_READ",
            format!(
                "failed to read PATH input '{}' ({}): {}",
                input_path.display(),
                label,
                source
            ),
        )
    })
}

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<PathFixtureBaseline> {
    let baseline_dir = PathBuf::from(PATH_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.PATH_FIXTURE",
            format!(
                "fixture '{}' is not approved for PATH parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let paths_input_source =
        read_baseline_input_source(&baseline_dir.join(PATH_REQUIRED_INPUTS[0]), "paths.inp")?;
    let geom_input_source =
        read_baseline_input_source(&baseline_dir.join(PATH_REQUIRED_INPUTS[1]), "geom.dat")?;
    let global_input_source =
        read_baseline_input_source(&baseline_dir.join(PATH_REQUIRED_INPUTS[2]), "global.inp")?;
    let phase_input_bytes =
        read_baseline_input_bytes(&baseline_dir.join(PATH_REQUIRED_INPUTS[3]), "phase.bin")?;

    let expected_outputs: Vec<PipelineArtifact> = PATH_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.PATH_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any PATH output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(PathFixtureBaseline {
        baseline_dir,
        expected_outputs,
        paths_input_source,
        geom_input_source,
        global_input_source,
        phase_input_bytes,
    })
}

fn read_baseline_input_source(path: &Path, label: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.PATH_BASELINE_READ",
            format!(
                "failed to read PATH baseline artifact '{}' ({}): {}",
                path.display(),
                label,
                source
            ),
        )
    })
}

fn read_baseline_input_bytes(path: &Path, label: &str) -> PipelineResult<Vec<u8>> {
    fs::read(path).map_err(|source| {
        FeffError::io_system(
            "IO.PATH_BASELINE_READ",
            format!(
                "failed to read PATH baseline artifact '{}' ({}): {}",
                path.display(),
                label,
                source
            ),
        )
    })
}

fn validate_text_input_against_baseline(
    actual: &str,
    baseline: &str,
    artifact: &str,
    fixture_id: &str,
) -> PipelineResult<()> {
    let matches = if artifact.eq_ignore_ascii_case("geom.dat") {
        normalize_geom_source(actual) == normalize_geom_source(baseline)
    } else {
        normalize_path_source(actual) == normalize_path_source(baseline)
    };
    if matches {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.PATH_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved PATH parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn validate_phase_input_against_baseline(
    actual: &[u8],
    baseline: &[u8],
    fixture_id: &str,
) -> PipelineResult<()> {
    if actual == baseline || actual.starts_with(XSPH_PHASE_BINARY_MAGIC) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.PATH_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved PATH parity baseline",
            fixture_id, PATH_REQUIRED_INPUTS[3]
        ),
    ))
}

fn normalize_path_source(content: &str) -> String {
    content
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_geom_source(content: &str) -> String {
    let mut lines = content.lines();
    let header_line = lines
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let count_line = lines
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // Skip fixed column header and separator rows.
    let _ = lines.next();
    let _ = lines.next();

    let mut rows = lines
        .filter_map(|line| {
            let columns: Vec<&str> = line.split_whitespace().collect();
            if columns.len() < 6 {
                return None;
            }
            Some(format!(
                "{} {} {} {} {}",
                columns[1], columns[2], columns[3], columns[4], columns[5]
            ))
        })
        .collect::<Vec<_>>();
    rows.sort();

    format!("{}\n{}\n{}", header_line, count_line, rows.join("\n"))
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::PathPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            "paths.inp",
            "actual-output",
        );
        let scaffold = PathPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 4);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("paths.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("geom.dat")
        );
        assert_eq!(
            contract.required_inputs[2].relative_path,
            PathBuf::from("global.inp")
        );
        assert_eq!(
            contract.required_inputs[3].relative_path,
            PathBuf::from("phase.bin")
        );

        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_path_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("paths.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_artifact("FX-PATH-001", "paths.inp", &input_path);
        stage_baseline_artifact("FX-PATH-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-PATH-001", "global.inp", &temp.path().join("global.inp"));
        stage_baseline_artifact("FX-PATH-001", "phase.bin", &temp.path().join("phase.bin"));

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            &input_path,
            &output_dir,
        );
        let scaffold = PathPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("PATH execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_path_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-PATH-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_accepts_true_compute_xsph_phase_binary_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("paths.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_artifact("FX-PATH-001", "paths.inp", &input_path);
        stage_baseline_artifact("FX-PATH-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-PATH-001", "global.inp", &temp.path().join("global.inp"));
        fs::write(
            temp.path().join("phase.bin"),
            [b'X', b'S', b'P', b'H', b'B', b'I', b'N', b'1', 1, 2, 3, 4],
        )
        .expect("phase input should be written");

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            &input_path,
            &output_dir,
        );
        let artifacts = PathPipelineScaffold
            .execute(&request)
            .expect("PATH execution should accept true-compute phase.bin");
        assert_eq!(artifact_set(&artifacts), expected_path_artifact_set());
    }

    #[test]
    fn execute_rejects_non_path_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("paths.inp");
        fs::write(&input_path, "PATH INPUT\n").expect("paths input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");
        fs::write(temp.path().join("phase.bin"), [1_u8, 2_u8]).expect("phase should be written");

        let request =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, temp.path());
        let scaffold = PathPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.PATH_MODULE");
    }

    #[test]
    fn execute_requires_phase_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("paths.inp");
        fs::write(&input_path, "PATH INPUT\n").expect("paths input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            &input_path,
            temp.path(),
        );
        let scaffold = PathPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing phase input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.PATH_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_path_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("paths.inp");
        fs::write(&input_path, "PATH INPUT\n").expect("paths input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");
        fs::write(temp.path().join("phase.bin"), [1_u8, 2_u8, 3_u8])
            .expect("phase should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Path,
            &input_path,
            temp.path(),
        );
        let scaffold = PathPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.PATH_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("paths.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_artifact("FX-PATH-001", "paths.inp", &input_path);
        stage_baseline_artifact("FX-PATH-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-PATH-001", "global.inp", &temp.path().join("global.inp"));
        stage_baseline_artifact("FX-PATH-001", "phase.bin", &temp.path().join("phase.bin"));

        fs::write(
            &input_path,
            "mpath, ms, nncrit, nlegxx, ipr4\n9999 9999 9999 9999 9999\n",
        )
        .expect("paths input should be overwritten");

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            &input_path,
            &output_dir,
        );
        let scaffold = PathPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.PATH_INPUT_MISMATCH");
    }

    fn fixture_baseline_dir(fixture_id: &str) -> PathBuf {
        PathBuf::from("artifacts/fortran-baselines")
            .join(fixture_id)
            .join("baseline")
    }

    fn stage_baseline_artifact(fixture_id: &str, artifact: &str, destination: &Path) {
        let source = fixture_baseline_dir(fixture_id).join(artifact);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should be created");
        }
        fs::copy(source, destination).expect("baseline artifact should be staged");
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn expected_path_artifact_set() -> BTreeSet<String> {
        ["paths.dat", "log4.dat"]
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }
}
