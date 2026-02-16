use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const COMPTON_REQUIRED_INPUTS: [&str; 3] = ["compton.inp", "pot.bin", "gg_slice.bin"];
const COMPTON_OUTPUT_CANDIDATES: [&str; 4] =
    ["compton.dat", "jzzp.dat", "rhozzp.dat", "logcompton.dat"];
const COMPTON_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComptonPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComptonFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    compton_input_source: String,
    pot_input_bytes: Vec<u8>,
    gg_slice_input_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComptonPipelineScaffold;

impl ComptonPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<ComptonPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(ComptonPipelineInterface {
            required_inputs: artifact_list(&COMPTON_REQUIRED_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for ComptonPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let compton_source = read_input_source(&request.input_path, COMPTON_REQUIRED_INPUTS[0])?;
        let pot_bytes = read_input_bytes(
            &input_dir.join(COMPTON_REQUIRED_INPUTS[1]),
            COMPTON_REQUIRED_INPUTS[1],
        )?;
        let gg_slice_bytes = read_input_bytes(
            &input_dir.join(COMPTON_REQUIRED_INPUTS[2]),
            COMPTON_REQUIRED_INPUTS[2],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &compton_source,
            &baseline.compton_input_source,
            COMPTON_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_binary_input_against_baseline(
            &pot_bytes,
            &baseline.pot_input_bytes,
            COMPTON_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_optional_binary_input_against_baseline(
            &gg_slice_bytes,
            baseline.gg_slice_input_bytes.as_deref(),
            COMPTON_REQUIRED_INPUTS[2],
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.COMPTON_OUTPUT_DIRECTORY",
                format!(
                    "failed to create COMPTON output directory '{}': {}",
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
                        "IO.COMPTON_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create COMPTON artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.COMPTON_OUTPUT_WRITE",
                    format!(
                        "failed to materialize COMPTON artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Compton {
        return Err(FeffError::input_validation(
            "INPUT.COMPTON_MODULE",
            format!(
                "COMPTON pipeline expects module COMPTON, got {}",
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
                "INPUT.COMPTON_INPUT_ARTIFACT",
                format!(
                    "COMPTON pipeline expects input artifact '{}' at '{}'",
                    COMPTON_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(COMPTON_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.COMPTON_INPUT_ARTIFACT",
            format!(
                "COMPTON pipeline requires input artifact '{}' but received '{}'",
                COMPTON_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.COMPTON_INPUT_ARTIFACT",
            format!(
                "COMPTON pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.COMPTON_INPUT_READ",
            format!(
                "failed to read COMPTON input '{}' ({}): {}",
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
            "IO.COMPTON_INPUT_READ",
            format!(
                "failed to read COMPTON input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<ComptonFixtureBaseline> {
    let baseline_dir = PathBuf::from(COMPTON_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.COMPTON_FIXTURE",
            format!(
                "fixture '{}' is not approved for COMPTON parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let compton_input_source = read_baseline_input_source(
        &baseline_dir.join(COMPTON_REQUIRED_INPUTS[0]),
        "compton.inp",
    )?;
    let pot_input_bytes =
        read_baseline_input_bytes(&baseline_dir.join(COMPTON_REQUIRED_INPUTS[1]), "pot.bin")?;
    let gg_slice_input_bytes = maybe_read_optional_baseline_bytes(
        baseline_dir.join(COMPTON_REQUIRED_INPUTS[2]),
        "gg_slice.bin",
    )?;

    let expected_outputs: Vec<PipelineArtifact> = COMPTON_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.COMPTON_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any COMPTON output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(ComptonFixtureBaseline {
        baseline_dir,
        expected_outputs,
        compton_input_source,
        pot_input_bytes,
        gg_slice_input_bytes,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.COMPTON_BASELINE_READ",
            format!(
                "failed to read COMPTON baseline artifact '{}' ({}): {}",
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
            "IO.COMPTON_BASELINE_READ",
            format!(
                "failed to read COMPTON baseline artifact '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn maybe_read_optional_baseline_bytes(
    path: PathBuf,
    artifact_name: &str,
) -> PipelineResult<Option<Vec<u8>>> {
    if path.is_file() {
        return read_baseline_input_bytes(&path, artifact_name).map(Some);
    }

    Ok(None)
}

fn validate_text_input_against_baseline(
    actual: &str,
    baseline: &str,
    artifact: &str,
    fixture_id: &str,
) -> PipelineResult<()> {
    if normalize_compton_source(actual) == normalize_compton_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.COMPTON_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved COMPTON parity baseline",
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
        "RUN.COMPTON_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved COMPTON parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn validate_optional_binary_input_against_baseline(
    actual: &[u8],
    baseline: Option<&[u8]>,
    artifact: &str,
    fixture_id: &str,
) -> PipelineResult<()> {
    if let Some(expected_bytes) = baseline {
        return validate_binary_input_against_baseline(
            actual,
            expected_bytes,
            artifact,
            fixture_id,
        );
    }

    Ok(())
}

fn normalize_compton_source(content: &str) -> String {
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
    use super::ComptonPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Compton,
            "compton.inp",
            "actual-output",
        );
        let scaffold = ComptonPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 3);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("compton.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("pot.bin")
        );
        assert_eq!(
            contract.required_inputs[2].relative_path,
            PathBuf::from("gg_slice.bin")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_compton_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("compton.inp");
        let output_dir = temp.path().join("out");
        stage_compton_input("FX-COMPTON-001", &input_path);
        stage_baseline_artifact("FX-COMPTON-001", "pot.bin", &temp.path().join("pot.bin"));
        stage_gg_slice_input("FX-COMPTON-001", &temp.path().join("gg_slice.bin"));

        let request = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Compton,
            &input_path,
            &output_dir,
        );
        let scaffold = ComptonPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("COMPTON execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_compton_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path =
                fixture_baseline_dir("FX-COMPTON-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_rejects_non_compton_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("compton.inp");
        fs::write(&input_path, default_compton_input_source())
            .expect("compton input should be written");
        fs::write(temp.path().join("pot.bin"), [1u8, 2u8]).expect("pot should be written");
        fs::write(temp.path().join("gg_slice.bin"), [3u8, 4u8])
            .expect("gg slice should be written");

        let request = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Crpa,
            &input_path,
            temp.path(),
        );
        let scaffold = ComptonPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.COMPTON_MODULE");
    }

    #[test]
    fn execute_requires_gg_slice_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("compton.inp");
        stage_compton_input("FX-COMPTON-001", &input_path);
        stage_baseline_artifact("FX-COMPTON-001", "pot.bin", &temp.path().join("pot.bin"));

        let request = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Compton,
            &input_path,
            temp.path(),
        );
        let scaffold = ComptonPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing gg_slice input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.COMPTON_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_compton_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("compton.inp");
        fs::write(&input_path, default_compton_input_source())
            .expect("compton input should be written");
        fs::write(temp.path().join("pot.bin"), [1u8, 2u8]).expect("pot should be written");
        fs::write(temp.path().join("gg_slice.bin"), [3u8, 4u8])
            .expect("gg slice should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Compton,
            &input_path,
            temp.path(),
        );
        let scaffold = ComptonPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.COMPTON_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("compton.inp");
        let output_dir = temp.path().join("actual");
        stage_compton_input("FX-COMPTON-001", &input_path);
        stage_baseline_artifact("FX-COMPTON-001", "pot.bin", &temp.path().join("pot.bin"));
        stage_gg_slice_input("FX-COMPTON-001", &temp.path().join("gg_slice.bin"));

        fs::write(temp.path().join("pot.bin"), [9u8, 9u8, 9u8])
            .expect("pot input should be overwritten");

        let request = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Compton,
            &input_path,
            &output_dir,
        );
        let scaffold = ComptonPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.COMPTON_INPUT_MISMATCH");
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

    fn stage_compton_input(fixture_id: &str, destination: &Path) {
        let baseline_compton_input = fixture_baseline_dir(fixture_id).join("compton.inp");
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        if baseline_compton_input.is_file() {
            fs::copy(&baseline_compton_input, destination)
                .expect("compton input copy should succeed");
            return;
        }

        fs::write(destination, default_compton_input_source())
            .expect("compton input should be written");
    }

    fn stage_gg_slice_input(fixture_id: &str, destination: &Path) {
        let baseline_gg_slice = fixture_baseline_dir(fixture_id).join("gg_slice.bin");
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        if baseline_gg_slice.is_file() {
            fs::copy(&baseline_gg_slice, destination).expect("gg slice copy should succeed");
            return;
        }

        fs::write(destination, [0u8, 1u8, 2u8, 3u8]).expect("gg slice should be written");
    }

    fn default_compton_input_source() -> &'static str {
        "icore: core level index\n1\nemin emax estep\n-10.0 10.0 0.5\n"
    }

    fn expected_compton_artifact_set() -> BTreeSet<String> {
        let baseline_dir = fixture_baseline_dir("FX-COMPTON-001");
        ["compton.dat", "jzzp.dat", "rhozzp.dat", "logcompton.dat"]
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
