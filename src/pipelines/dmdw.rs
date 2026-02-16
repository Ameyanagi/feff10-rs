use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const DMDW_REQUIRED_INPUTS: [&str; 2] = ["dmdw.inp", "feff.dym"];
const DMDW_OUTPUT_CANDIDATES: [&str; 1] = ["dmdw.out"];
const DMDW_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmdwPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DmdwFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    dmdw_input_source: String,
    feff_dym_input_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DmdwPipelineScaffold;

impl DmdwPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<DmdwPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(DmdwPipelineInterface {
            required_inputs: artifact_list(&DMDW_REQUIRED_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for DmdwPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let dmdw_source = read_input_source(&request.input_path, DMDW_REQUIRED_INPUTS[0])?;
        let feff_dym_bytes = read_input_bytes(
            &input_dir.join(DMDW_REQUIRED_INPUTS[1]),
            DMDW_REQUIRED_INPUTS[1],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &dmdw_source,
            &baseline.dmdw_input_source,
            DMDW_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_binary_input_against_baseline(
            &feff_dym_bytes,
            &baseline.feff_dym_input_bytes,
            DMDW_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.DMDW_OUTPUT_DIRECTORY",
                format!(
                    "failed to create DMDW output directory '{}': {}",
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
                        "IO.DMDW_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create DMDW artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.DMDW_OUTPUT_WRITE",
                    format!(
                        "failed to materialize DMDW artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Dmdw {
        return Err(FeffError::input_validation(
            "INPUT.DMDW_MODULE",
            format!("DMDW pipeline expects module DMDW, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.DMDW_INPUT_ARTIFACT",
                format!(
                    "DMDW pipeline expects input artifact '{}' at '{}'",
                    DMDW_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(DMDW_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.DMDW_INPUT_ARTIFACT",
            format!(
                "DMDW pipeline requires input artifact '{}' but received '{}'",
                DMDW_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.DMDW_INPUT_ARTIFACT",
            format!(
                "DMDW pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.DMDW_INPUT_READ",
            format!(
                "failed to read DMDW input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn read_input_bytes(path: &Path, artifact_name: &str) -> PipelineResult<Vec<u8>> {
    fs::read(path).map_err(|source| {
        FeffError::io_system(
            "IO.DMDW_INPUT_READ",
            format!(
                "failed to read DMDW input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<DmdwFixtureBaseline> {
    let baseline_dir = PathBuf::from(DMDW_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.DMDW_FIXTURE",
            format!(
                "fixture '{}' is not approved for DMDW parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let dmdw_input_source =
        read_baseline_input_source(&baseline_dir.join(DMDW_REQUIRED_INPUTS[0]), "dmdw.inp")?;
    let feff_dym_input_bytes =
        read_baseline_input_bytes(&baseline_dir.join(DMDW_REQUIRED_INPUTS[1]), "feff.dym")?;

    let expected_outputs: Vec<PipelineArtifact> = DMDW_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.DMDW_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any DMDW output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(DmdwFixtureBaseline {
        baseline_dir,
        expected_outputs,
        dmdw_input_source,
        feff_dym_input_bytes,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.DMDW_BASELINE_READ",
            format!(
                "failed to read DMDW baseline artifact '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn read_baseline_input_bytes(path: &Path, artifact_name: &str) -> PipelineResult<Vec<u8>> {
    fs::read(path).map_err(|source| {
        FeffError::io_system(
            "IO.DMDW_BASELINE_READ",
            format!(
                "failed to read DMDW baseline artifact '{}' ({}): {}",
                path.display(),
                artifact_name,
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
    if normalize_dmdw_source(actual) == normalize_dmdw_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.DMDW_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved DMDW parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn validate_binary_input_against_baseline(
    actual: &[u8],
    baseline: &[u8],
    artifact: &str,
    fixture_id: &str,
) -> PipelineResult<()> {
    if actual == baseline {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.DMDW_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved DMDW parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn normalize_dmdw_source(content: &str) -> String {
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
    use super::DmdwPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-DMDW-001",
            PipelineModule::Dmdw,
            "dmdw.inp",
            "actual-output",
        );
        let scaffold = DmdwPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 2);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("dmdw.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("feff.dym")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_dmdw_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        let output_dir = temp.path().join("out");
        stage_baseline_artifact("FX-DMDW-001", "dmdw.inp", &input_path);
        stage_baseline_artifact("FX-DMDW-001", "feff.dym", &temp.path().join("feff.dym"));

        let request = PipelineRequest::new(
            "FX-DMDW-001",
            PipelineModule::Dmdw,
            &input_path,
            &output_dir,
        );
        let scaffold = DmdwPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("DMDW execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_dmdw_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-DMDW-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_rejects_non_dmdw_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        fs::write(&input_path, "DMDW\n").expect("dmdw input should be written");
        fs::write(temp.path().join("feff.dym"), [0_u8, 1_u8, 2_u8])
            .expect("feff.dym should be written");

        let request = PipelineRequest::new(
            "FX-DMDW-001",
            PipelineModule::Debye,
            &input_path,
            temp.path(),
        );
        let scaffold = DmdwPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.DMDW_MODULE");
    }

    #[test]
    fn execute_requires_feff_dym_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        stage_baseline_artifact("FX-DMDW-001", "dmdw.inp", &input_path);

        let request = PipelineRequest::new(
            "FX-DMDW-001",
            PipelineModule::Dmdw,
            &input_path,
            temp.path(),
        );
        let scaffold = DmdwPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing feff.dym should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.DMDW_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_dmdw_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        fs::write(&input_path, "DMDW\n").expect("dmdw input should be written");
        fs::write(temp.path().join("feff.dym"), [0_u8, 1_u8, 2_u8])
            .expect("feff.dym should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Dmdw,
            &input_path,
            temp.path(),
        );
        let scaffold = DmdwPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.DMDW_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_artifact("FX-DMDW-001", "dmdw.inp", &input_path);
        stage_baseline_artifact("FX-DMDW-001", "feff.dym", &temp.path().join("feff.dym"));

        fs::write(&input_path, "drifted dmdw input\n").expect("dmdw input should be overwritten");

        let request = PipelineRequest::new(
            "FX-DMDW-001",
            PipelineModule::Dmdw,
            &input_path,
            &output_dir,
        );
        let scaffold = DmdwPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.DMDW_INPUT_MISMATCH");
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

    fn expected_dmdw_artifact_set() -> BTreeSet<String> {
        let baseline_dir = fixture_baseline_dir("FX-DMDW-001");
        ["dmdw.out"]
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
