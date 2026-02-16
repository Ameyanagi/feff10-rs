use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const SCREEN_REQUIRED_INPUTS: [&str; 3] = ["pot.inp", "geom.dat", "ldos.inp"];
const SCREEN_OPTIONAL_INPUTS: [&str; 1] = ["screen.inp"];
const SCREEN_OUTPUT_CANDIDATES: [&str; 2] = ["wscrn.dat", "logscreen.dat"];
const SCREEN_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub optional_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScreenFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    pot_input_source: String,
    geom_input_source: String,
    ldos_input_source: String,
    screen_input_source: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenPipelineScaffold;

impl ScreenPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<ScreenPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(ScreenPipelineInterface {
            required_inputs: artifact_list(&SCREEN_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&SCREEN_OPTIONAL_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for ScreenPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let pot_source = read_input_source(&request.input_path, SCREEN_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(SCREEN_REQUIRED_INPUTS[1]),
            SCREEN_REQUIRED_INPUTS[1],
        )?;
        let ldos_source = read_input_source(
            &input_dir.join(SCREEN_REQUIRED_INPUTS[2]),
            SCREEN_REQUIRED_INPUTS[2],
        )?;
        let screen_source = maybe_read_optional_input_source(
            input_dir.join(SCREEN_OPTIONAL_INPUTS[0]),
            SCREEN_OPTIONAL_INPUTS[0],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &pot_source,
            &baseline.pot_input_source,
            SCREEN_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &geom_source,
            &baseline.geom_input_source,
            SCREEN_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &ldos_source,
            &baseline.ldos_input_source,
            SCREEN_REQUIRED_INPUTS[2],
            &request.fixture_id,
        )?;
        validate_optional_screen_input_against_baseline(
            screen_source.as_deref(),
            baseline.screen_input_source.as_deref(),
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.SCREEN_OUTPUT_DIRECTORY",
                format!(
                    "failed to create SCREEN output directory '{}': {}",
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
                        "IO.SCREEN_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create SCREEN artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.SCREEN_OUTPUT_WRITE",
                    format!(
                        "failed to materialize SCREEN artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Screen {
        return Err(FeffError::input_validation(
            "INPUT.SCREEN_MODULE",
            format!(
                "SCREEN pipeline expects module SCREEN, got {}",
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
                "INPUT.SCREEN_INPUT_ARTIFACT",
                format!(
                    "SCREEN pipeline expects input artifact '{}' at '{}'",
                    SCREEN_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(SCREEN_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.SCREEN_INPUT_ARTIFACT",
            format!(
                "SCREEN pipeline requires input artifact '{}' but received '{}'",
                SCREEN_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.SCREEN_INPUT_ARTIFACT",
            format!(
                "SCREEN pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.SCREEN_INPUT_READ",
            format!(
                "failed to read SCREEN input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn maybe_read_optional_input_source(
    path: PathBuf,
    artifact_name: &str,
) -> PipelineResult<Option<String>> {
    if path.is_file() {
        return read_input_source(&path, artifact_name).map(Some);
    }

    Ok(None)
}

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<ScreenFixtureBaseline> {
    let baseline_dir = PathBuf::from(SCREEN_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.SCREEN_FIXTURE",
            format!(
                "fixture '{}' is not approved for SCREEN parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let pot_input_source =
        read_baseline_input_source(&baseline_dir.join(SCREEN_REQUIRED_INPUTS[0]), "pot.inp")?;
    let geom_input_source =
        read_baseline_input_source(&baseline_dir.join(SCREEN_REQUIRED_INPUTS[1]), "geom.dat")?;
    let ldos_input_source =
        read_baseline_input_source(&baseline_dir.join(SCREEN_REQUIRED_INPUTS[2]), "ldos.inp")?;
    let screen_input_source = maybe_read_optional_baseline_source(
        baseline_dir.join(SCREEN_OPTIONAL_INPUTS[0]),
        SCREEN_OPTIONAL_INPUTS[0],
    )?;

    let expected_outputs: Vec<PipelineArtifact> = SCREEN_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.SCREEN_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any SCREEN output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(ScreenFixtureBaseline {
        baseline_dir,
        expected_outputs,
        pot_input_source,
        geom_input_source,
        ldos_input_source,
        screen_input_source,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.SCREEN_BASELINE_READ",
            format!(
                "failed to read SCREEN baseline artifact '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn maybe_read_optional_baseline_source(
    path: PathBuf,
    artifact_name: &str,
) -> PipelineResult<Option<String>> {
    if path.is_file() {
        return read_baseline_input_source(&path, artifact_name).map(Some);
    }

    Ok(None)
}

fn validate_text_input_against_baseline(
    actual: &str,
    baseline: &str,
    artifact: &str,
    fixture_id: &str,
) -> PipelineResult<()> {
    if normalize_screen_source(actual) == normalize_screen_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.SCREEN_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved SCREEN parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn validate_optional_screen_input_against_baseline(
    actual: Option<&str>,
    baseline: Option<&str>,
    fixture_id: &str,
) -> PipelineResult<()> {
    let Some(actual) = actual else {
        return Ok(());
    };

    let Some(baseline) = baseline else {
        return Ok(());
    };

    if normalize_screen_source(actual) == normalize_screen_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.SCREEN_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved SCREEN parity baseline",
            fixture_id, SCREEN_OPTIONAL_INPUTS[0]
        ),
    ))
}

fn normalize_screen_source(content: &str) -> String {
    content
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::ScreenPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-SCREEN-001",
            PipelineModule::Screen,
            "pot.inp",
            "actual-output",
        );
        let scaffold = ScreenPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 3);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("pot.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("geom.dat")
        );
        assert_eq!(
            contract.required_inputs[2].relative_path,
            PathBuf::from("ldos.inp")
        );
        assert_eq!(contract.optional_inputs.len(), 1);
        assert_eq!(
            contract.optional_inputs[0].relative_path,
            PathBuf::from("screen.inp")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_screen_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("out");
        stage_baseline_artifact("FX-SCREEN-001", "pot.inp", &input_path);
        stage_baseline_artifact("FX-SCREEN-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-SCREEN-001", "ldos.inp", &temp.path().join("ldos.inp"));
        stage_optional_screen_input("FX-SCREEN-001", &temp.path().join("screen.inp"));

        let request = PipelineRequest::new(
            "FX-SCREEN-001",
            PipelineModule::Screen,
            &input_path,
            &output_dir,
        );
        let scaffold = ScreenPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("SCREEN execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_screen_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-SCREEN-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_allows_missing_optional_screen_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("out");
        stage_baseline_artifact("FX-SCREEN-001", "pot.inp", &input_path);
        stage_baseline_artifact("FX-SCREEN-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-SCREEN-001", "ldos.inp", &temp.path().join("ldos.inp"));

        let request = PipelineRequest::new(
            "FX-SCREEN-001",
            PipelineModule::Screen,
            &input_path,
            &output_dir,
        );
        let scaffold = ScreenPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("SCREEN execution should succeed without screen.inp");

        assert_eq!(artifact_set(&artifacts), expected_screen_artifact_set());
    }

    #[test]
    fn execute_rejects_non_screen_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        fs::write(&input_path, "POT INPUT\n").expect("pot input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("ldos.inp"), "LDOS INPUT\n").expect("ldos should be written");

        let request = PipelineRequest::new(
            "FX-SCREEN-001",
            PipelineModule::Crpa,
            &input_path,
            temp.path(),
        );
        let scaffold = ScreenPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.SCREEN_MODULE");
    }

    #[test]
    fn execute_requires_geom_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        stage_baseline_artifact("FX-SCREEN-001", "pot.inp", &input_path);
        stage_baseline_artifact("FX-SCREEN-001", "ldos.inp", &temp.path().join("ldos.inp"));

        let request = PipelineRequest::new(
            "FX-SCREEN-001",
            PipelineModule::Screen,
            &input_path,
            temp.path(),
        );
        let scaffold = ScreenPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing geom input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.SCREEN_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_screen_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        fs::write(&input_path, "POT INPUT\n").expect("pot input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("ldos.inp"), "LDOS INPUT\n").expect("ldos should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Screen,
            &input_path,
            temp.path(),
        );
        let scaffold = ScreenPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.SCREEN_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_artifact("FX-SCREEN-001", "pot.inp", &input_path);
        stage_baseline_artifact("FX-SCREEN-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-SCREEN-001", "ldos.inp", &temp.path().join("ldos.inp"));
        stage_optional_screen_input("FX-SCREEN-001", &temp.path().join("screen.inp"));

        fs::write(temp.path().join("geom.dat"), "drifted geom input\n")
            .expect("geom input should be overwritten");

        let request = PipelineRequest::new(
            "FX-SCREEN-001",
            PipelineModule::Screen,
            &input_path,
            &output_dir,
        );
        let scaffold = ScreenPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.SCREEN_INPUT_MISMATCH");
    }

    fn fixture_baseline_dir(fixture_id: &str) -> PathBuf {
        PathBuf::from("artifacts/fortran-baselines")
            .join(fixture_id)
            .join("baseline")
    }

    fn stage_baseline_artifact(fixture_id: &str, artifact: &str, destination: &Path) {
        let source = fixture_baseline_dir(fixture_id).join(artifact);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::copy(&source, destination).expect("baseline artifact copy should succeed");
    }

    fn stage_optional_screen_input(fixture_id: &str, destination: &Path) {
        let baseline_screen_input = fixture_baseline_dir(fixture_id).join("screen.inp");
        if !baseline_screen_input.is_file() {
            return;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::copy(&baseline_screen_input, destination).expect("screen input copy should succeed");
    }

    fn expected_screen_artifact_set() -> BTreeSet<String> {
        let baseline_dir = fixture_baseline_dir("FX-SCREEN-001");
        ["wscrn.dat", "logscreen.dat"]
            .iter()
            .filter(|artifact| baseline_dir.join(artifact).is_file())
            .map(|artifact| artifact.to_string())
            .collect()
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }
}
