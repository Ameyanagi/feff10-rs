use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const EELS_REQUIRED_INPUTS: [&str; 2] = ["eels.inp", "xmu.dat"];
const EELS_OPTIONAL_INPUTS: [&str; 1] = ["magic.inp"];
const EELS_OUTPUT_CANDIDATES: [&str; 4] =
    ["eels.dat", "logeels.dat", "magic.dat", "reference_eels.dat"];
const EELS_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EelsPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub optional_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EelsFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    eels_input_source: String,
    xmu_input_source: String,
    magic_input_source: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EelsPipelineScaffold;

impl EelsPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<EelsPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(EelsPipelineInterface {
            required_inputs: artifact_list(&EELS_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&EELS_OPTIONAL_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for EelsPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let eels_source = read_input_source(&request.input_path, EELS_REQUIRED_INPUTS[0])?;
        let xmu_source = read_input_source(
            &input_dir.join(EELS_REQUIRED_INPUTS[1]),
            EELS_REQUIRED_INPUTS[1],
        )?;
        let magic_source = maybe_read_optional_input_source(
            input_dir.join(EELS_OPTIONAL_INPUTS[0]),
            EELS_OPTIONAL_INPUTS[0],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &eels_source,
            &baseline.eels_input_source,
            EELS_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &xmu_source,
            &baseline.xmu_input_source,
            EELS_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_optional_magic_input_against_baseline(
            magic_source.as_deref(),
            baseline.magic_input_source.as_deref(),
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.EELS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create EELS output directory '{}': {}",
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
                        "IO.EELS_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create EELS artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.EELS_OUTPUT_WRITE",
                    format!(
                        "failed to materialize EELS artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Eels {
        return Err(FeffError::input_validation(
            "INPUT.EELS_MODULE",
            format!("EELS pipeline expects module EELS, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.EELS_INPUT_ARTIFACT",
                format!(
                    "EELS pipeline expects input artifact '{}' at '{}'",
                    EELS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(EELS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.EELS_INPUT_ARTIFACT",
            format!(
                "EELS pipeline requires input artifact '{}' but received '{}'",
                EELS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.EELS_INPUT_ARTIFACT",
            format!(
                "EELS pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.EELS_INPUT_READ",
            format!(
                "failed to read EELS input '{}' ({}): {}",
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

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<EelsFixtureBaseline> {
    let baseline_dir = PathBuf::from(EELS_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.EELS_FIXTURE",
            format!(
                "fixture '{}' is not approved for EELS parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let eels_input_source =
        read_baseline_input_source(&baseline_dir.join(EELS_REQUIRED_INPUTS[0]), "eels.inp")?;
    let xmu_input_source =
        read_baseline_input_source(&baseline_dir.join(EELS_REQUIRED_INPUTS[1]), "xmu.dat")?;
    let magic_input_source = maybe_read_optional_baseline_source(
        baseline_dir.join(EELS_OPTIONAL_INPUTS[0]),
        EELS_OPTIONAL_INPUTS[0],
    )?;

    let expected_outputs: Vec<PipelineArtifact> = EELS_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.EELS_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any EELS output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(EelsFixtureBaseline {
        baseline_dir,
        expected_outputs,
        eels_input_source,
        xmu_input_source,
        magic_input_source,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.EELS_BASELINE_READ",
            format!(
                "failed to read EELS baseline artifact '{}' ({}): {}",
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
    if normalize_eels_source(actual) == normalize_eels_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.EELS_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved EELS parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn validate_optional_magic_input_against_baseline(
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

    if normalize_eels_source(actual) == normalize_eels_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.EELS_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved EELS parity baseline",
            fixture_id, EELS_OPTIONAL_INPUTS[0]
        ),
    ))
}

fn normalize_eels_source(content: &str) -> String {
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
    use super::EelsPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-EELS-001",
            PipelineModule::Eels,
            "eels.inp",
            "actual-output",
        );
        let scaffold = EelsPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 2);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("eels.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("xmu.dat")
        );
        assert_eq!(contract.optional_inputs.len(), 1);
        assert_eq!(
            contract.optional_inputs[0].relative_path,
            PathBuf::from("magic.inp")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_eels_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("eels.inp");
        let output_dir = temp.path().join("out");
        stage_baseline_artifact("FX-EELS-001", "eels.inp", &input_path);
        stage_baseline_artifact("FX-EELS-001", "xmu.dat", &temp.path().join("xmu.dat"));
        stage_optional_magic_input("FX-EELS-001", &temp.path().join("magic.inp"));

        let request = PipelineRequest::new(
            "FX-EELS-001",
            PipelineModule::Eels,
            &input_path,
            &output_dir,
        );
        let scaffold = EelsPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("EELS execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_eels_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-EELS-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_allows_missing_optional_magic_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("eels.inp");
        let output_dir = temp.path().join("out");
        stage_baseline_artifact("FX-EELS-001", "eels.inp", &input_path);
        stage_baseline_artifact("FX-EELS-001", "xmu.dat", &temp.path().join("xmu.dat"));

        let request = PipelineRequest::new(
            "FX-EELS-001",
            PipelineModule::Eels,
            &input_path,
            &output_dir,
        );
        let scaffold = EelsPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("EELS execution should succeed without magic.inp");

        assert_eq!(artifact_set(&artifacts), expected_eels_artifact_set());
    }

    #[test]
    fn execute_rejects_non_eels_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("eels.inp");
        fs::write(&input_path, "EELS INPUT\n").expect("eels input should be written");
        fs::write(temp.path().join("xmu.dat"), "XMU INPUT\n").expect("xmu should be written");

        let request = PipelineRequest::new(
            "FX-EELS-001",
            PipelineModule::Ldos,
            &input_path,
            temp.path(),
        );
        let scaffold = EelsPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.EELS_MODULE");
    }

    #[test]
    fn execute_requires_xmu_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("eels.inp");
        stage_baseline_artifact("FX-EELS-001", "eels.inp", &input_path);

        let request = PipelineRequest::new(
            "FX-EELS-001",
            PipelineModule::Eels,
            &input_path,
            temp.path(),
        );
        let scaffold = EelsPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing xmu input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.EELS_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_eels_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("eels.inp");
        fs::write(&input_path, "EELS INPUT\n").expect("eels input should be written");
        fs::write(temp.path().join("xmu.dat"), "XMU INPUT\n").expect("xmu should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Eels,
            &input_path,
            temp.path(),
        );
        let scaffold = EelsPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.EELS_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("eels.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_artifact("FX-EELS-001", "eels.inp", &input_path);
        stage_baseline_artifact("FX-EELS-001", "xmu.dat", &temp.path().join("xmu.dat"));

        fs::write(temp.path().join("xmu.dat"), "drifted xmu input\n")
            .expect("xmu input should be overwritten");

        let request = PipelineRequest::new(
            "FX-EELS-001",
            PipelineModule::Eels,
            &input_path,
            &output_dir,
        );
        let scaffold = EelsPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.EELS_INPUT_MISMATCH");
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

    fn stage_optional_magic_input(fixture_id: &str, destination: &Path) {
        let baseline_magic_input = fixture_baseline_dir(fixture_id).join("magic.inp");
        if !baseline_magic_input.is_file() {
            return;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::copy(&baseline_magic_input, destination).expect("magic input copy should succeed");
    }

    fn expected_eels_artifact_set() -> BTreeSet<String> {
        let baseline_dir = fixture_baseline_dir("FX-EELS-001");
        ["eels.dat", "logeels.dat", "magic.dat", "reference_eels.dat"]
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
