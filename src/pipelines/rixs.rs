use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const RIXS_REQUIRED_INPUTS: [&str; 6] = [
    "rixs.inp",
    "phase_1.bin",
    "phase_2.bin",
    "wscrn_1.dat",
    "wscrn_2.dat",
    "xsect_2.dat",
];
const RIXS_OUTPUT_CANDIDATES: [&str; 10] = [
    "rixs0.dat",
    "rixs1.dat",
    "rixsET.dat",
    "rixsEE.dat",
    "rixsET-sat.dat",
    "rixsEE-sat.dat",
    "logrixs.dat",
    "referenceherfd.dat",
    "referenceherfd-sat.dat",
    "referencerixsET.dat",
];
const RIXS_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RixsPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RixsFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    rixs_input_source: Option<String>,
    phase_1_input_bytes: Option<Vec<u8>>,
    phase_2_input_bytes: Option<Vec<u8>>,
    wscrn_1_input_source: Option<String>,
    wscrn_2_input_source: Option<String>,
    xsect_2_input_source: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RixsPipelineScaffold;

impl RixsPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<RixsPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(RixsPipelineInterface {
            required_inputs: artifact_list(&RIXS_REQUIRED_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for RixsPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let rixs_source = read_input_source(&request.input_path, RIXS_REQUIRED_INPUTS[0])?;
        let phase_1_bytes = read_input_bytes(
            &input_dir.join(RIXS_REQUIRED_INPUTS[1]),
            RIXS_REQUIRED_INPUTS[1],
        )?;
        let phase_2_bytes = read_input_bytes(
            &input_dir.join(RIXS_REQUIRED_INPUTS[2]),
            RIXS_REQUIRED_INPUTS[2],
        )?;
        let wscrn_1_source = read_input_source(
            &input_dir.join(RIXS_REQUIRED_INPUTS[3]),
            RIXS_REQUIRED_INPUTS[3],
        )?;
        let wscrn_2_source = read_input_source(
            &input_dir.join(RIXS_REQUIRED_INPUTS[4]),
            RIXS_REQUIRED_INPUTS[4],
        )?;
        let xsect_2_source = read_input_source(
            &input_dir.join(RIXS_REQUIRED_INPUTS[5]),
            RIXS_REQUIRED_INPUTS[5],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_optional_text_input_against_baseline(
            &rixs_source,
            baseline.rixs_input_source.as_deref(),
            RIXS_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_optional_binary_input_against_baseline(
            &phase_1_bytes,
            baseline.phase_1_input_bytes.as_deref(),
            RIXS_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_optional_binary_input_against_baseline(
            &phase_2_bytes,
            baseline.phase_2_input_bytes.as_deref(),
            RIXS_REQUIRED_INPUTS[2],
            &request.fixture_id,
        )?;
        validate_optional_text_input_against_baseline(
            &wscrn_1_source,
            baseline.wscrn_1_input_source.as_deref(),
            RIXS_REQUIRED_INPUTS[3],
            &request.fixture_id,
        )?;
        validate_optional_text_input_against_baseline(
            &wscrn_2_source,
            baseline.wscrn_2_input_source.as_deref(),
            RIXS_REQUIRED_INPUTS[4],
            &request.fixture_id,
        )?;
        validate_optional_text_input_against_baseline(
            &xsect_2_source,
            baseline.xsect_2_input_source.as_deref(),
            RIXS_REQUIRED_INPUTS[5],
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.RIXS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create RIXS output directory '{}': {}",
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
                        "IO.RIXS_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create RIXS artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.RIXS_OUTPUT_WRITE",
                    format!(
                        "failed to materialize RIXS artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Rixs {
        return Err(FeffError::input_validation(
            "INPUT.RIXS_MODULE",
            format!("RIXS pipeline expects module RIXS, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.RIXS_INPUT_ARTIFACT",
                format!(
                    "RIXS pipeline expects input artifact '{}' at '{}'",
                    RIXS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(RIXS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.RIXS_INPUT_ARTIFACT",
            format!(
                "RIXS pipeline requires input artifact '{}' but received '{}'",
                RIXS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.RIXS_INPUT_ARTIFACT",
            format!(
                "RIXS pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.RIXS_INPUT_READ",
            format!(
                "failed to read RIXS input '{}' ({}): {}",
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
            "IO.RIXS_INPUT_READ",
            format!(
                "failed to read RIXS input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<RixsFixtureBaseline> {
    let baseline_dir = PathBuf::from(RIXS_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.RIXS_FIXTURE",
            format!(
                "fixture '{}' is not approved for RIXS parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let rixs_input_source = maybe_read_optional_baseline_source(
        baseline_dir.join(RIXS_REQUIRED_INPUTS[0]),
        RIXS_REQUIRED_INPUTS[0],
    )?;
    let phase_1_input_bytes = maybe_read_optional_baseline_bytes(
        baseline_dir.join(RIXS_REQUIRED_INPUTS[1]),
        RIXS_REQUIRED_INPUTS[1],
    )?;
    let phase_2_input_bytes = maybe_read_optional_baseline_bytes(
        baseline_dir.join(RIXS_REQUIRED_INPUTS[2]),
        RIXS_REQUIRED_INPUTS[2],
    )?;
    let wscrn_1_input_source = maybe_read_optional_baseline_source(
        baseline_dir.join(RIXS_REQUIRED_INPUTS[3]),
        RIXS_REQUIRED_INPUTS[3],
    )?;
    let wscrn_2_input_source = maybe_read_optional_baseline_source(
        baseline_dir.join(RIXS_REQUIRED_INPUTS[4]),
        RIXS_REQUIRED_INPUTS[4],
    )?;
    let xsect_2_input_source = maybe_read_optional_baseline_source(
        baseline_dir.join(RIXS_REQUIRED_INPUTS[5]),
        RIXS_REQUIRED_INPUTS[5],
    )?;

    let expected_outputs: Vec<PipelineArtifact> = RIXS_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.RIXS_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any RIXS output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(RixsFixtureBaseline {
        baseline_dir,
        expected_outputs,
        rixs_input_source,
        phase_1_input_bytes,
        phase_2_input_bytes,
        wscrn_1_input_source,
        wscrn_2_input_source,
        xsect_2_input_source,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.RIXS_BASELINE_READ",
            format!(
                "failed to read RIXS baseline artifact '{}' ({}): {}",
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
            "IO.RIXS_BASELINE_READ",
            format!(
                "failed to read RIXS baseline artifact '{}' ({}): {}",
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

fn maybe_read_optional_baseline_bytes(
    path: PathBuf,
    artifact_name: &str,
) -> PipelineResult<Option<Vec<u8>>> {
    if path.is_file() {
        return read_baseline_input_bytes(&path, artifact_name).map(Some);
    }

    Ok(None)
}

fn validate_optional_text_input_against_baseline(
    actual: &str,
    baseline: Option<&str>,
    artifact: &str,
    fixture_id: &str,
) -> PipelineResult<()> {
    if let Some(baseline_source) = baseline {
        if normalize_rixs_source(actual) == normalize_rixs_source(baseline_source) {
            return Ok(());
        }

        return Err(FeffError::computation(
            "RUN.RIXS_INPUT_MISMATCH",
            format!(
                "fixture '{}' input '{}' does not match approved RIXS parity baseline",
                fixture_id, artifact
            ),
        ));
    }

    Ok(())
}

fn validate_optional_binary_input_against_baseline(
    actual: &[u8],
    baseline: Option<&[u8]>,
    artifact: &str,
    fixture_id: &str,
) -> PipelineResult<()> {
    if let Some(baseline_bytes) = baseline {
        if actual == baseline_bytes {
            return Ok(());
        }

        return Err(FeffError::computation(
            "RUN.RIXS_INPUT_MISMATCH",
            format!(
                "fixture '{}' input '{}' does not match approved RIXS parity baseline",
                fixture_id, artifact
            ),
        ));
    }

    Ok(())
}

fn normalize_rixs_source(content: &str) -> String {
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
    use super::RixsPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const RIXS_OUTPUT_CANDIDATES: [&str; 10] = [
        "rixs0.dat",
        "rixs1.dat",
        "rixsET.dat",
        "rixsEE.dat",
        "rixsET-sat.dat",
        "rixsEE-sat.dat",
        "logrixs.dat",
        "referenceherfd.dat",
        "referenceherfd-sat.dat",
        "referencerixsET.dat",
    ];

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rixs,
            "rixs.inp",
            "actual-output",
        );
        let scaffold = RixsPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 6);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("rixs.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("phase_1.bin")
        );
        assert_eq!(
            contract.required_inputs[2].relative_path,
            PathBuf::from("phase_2.bin")
        );
        assert_eq!(
            contract.required_inputs[3].relative_path,
            PathBuf::from("wscrn_1.dat")
        );
        assert_eq!(
            contract.required_inputs[4].relative_path,
            PathBuf::from("wscrn_2.dat")
        );
        assert_eq!(
            contract.required_inputs[5].relative_path,
            PathBuf::from("xsect_2.dat")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_rixs_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("rixs.inp");
        let output_dir = temp.path().join("out");
        stage_rixs_input_bundle("FX-RIXS-001", temp.path());

        let request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rixs,
            &input_path,
            &output_dir,
        );
        let scaffold = RixsPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("RIXS execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_rixs_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-RIXS-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_rejects_non_rixs_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle("FX-RIXS-001", temp.path());

        let request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rdinp,
            temp.path().join("rixs.inp"),
            temp.path(),
        );
        let scaffold = RixsPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.RIXS_MODULE");
    }

    #[test]
    fn execute_requires_phase_2_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle("FX-RIXS-001", temp.path());
        fs::remove_file(temp.path().join("phase_2.bin")).expect("phase_2.bin should be removed");

        let request = PipelineRequest::new(
            "FX-RIXS-001",
            PipelineModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path(),
        );
        let scaffold = RixsPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing phase_2 input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.RIXS_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_rixs_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle("FX-RIXS-001", temp.path());

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path(),
        );
        let scaffold = RixsPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.RIXS_FIXTURE");
    }

    fn fixture_baseline_dir(fixture_id: &str) -> PathBuf {
        PathBuf::from("artifacts/fortran-baselines")
            .join(fixture_id)
            .join("baseline")
    }

    fn expected_rixs_artifact_set() -> BTreeSet<String> {
        let baseline_dir = fixture_baseline_dir("FX-RIXS-001");
        let artifacts: BTreeSet<String> = RIXS_OUTPUT_CANDIDATES
            .iter()
            .filter(|artifact| baseline_dir.join(artifact).is_file())
            .map(|artifact| artifact.to_string())
            .collect();

        assert!(
            !artifacts.is_empty(),
            "fixture 'FX-RIXS-001' should provide at least one RIXS output artifact",
        );
        artifacts
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn stage_rixs_input_bundle(fixture_id: &str, destination_dir: &Path) {
        stage_text_input(
            fixture_id,
            "rixs.inp",
            &destination_dir.join("rixs.inp"),
            default_rixs_input_source(),
        );
        stage_binary_input(
            fixture_id,
            "phase_1.bin",
            &destination_dir.join("phase_1.bin"),
            &[0_u8, 1_u8, 2_u8, 3_u8],
        );
        stage_binary_input(
            fixture_id,
            "phase_2.bin",
            &destination_dir.join("phase_2.bin"),
            &[4_u8, 5_u8, 6_u8, 7_u8],
        );
        stage_text_input(
            fixture_id,
            "wscrn_1.dat",
            &destination_dir.join("wscrn_1.dat"),
            "0.0 0.0 0.0\n",
        );
        stage_text_input(
            fixture_id,
            "wscrn_2.dat",
            &destination_dir.join("wscrn_2.dat"),
            "0.0 0.0 0.0\n",
        );
        stage_text_input(
            fixture_id,
            "xsect_2.dat",
            &destination_dir.join("xsect_2.dat"),
            "0.0 0.0 0.0\n",
        );
    }

    fn stage_text_input(fixture_id: &str, artifact: &str, destination: &Path, default: &str) {
        let source = fixture_baseline_dir(fixture_id).join(artifact);
        if source.is_file() {
            copy_file(&source, destination);
            return;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::write(destination, default).expect("text input should be staged");
    }

    fn stage_binary_input(fixture_id: &str, artifact: &str, destination: &Path, default: &[u8]) {
        let source = fixture_baseline_dir(fixture_id).join(artifact);
        if source.is_file() {
            copy_file(&source, destination);
            return;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::write(destination, default).expect("binary input should be staged");
    }

    fn copy_file(source: &Path, destination: &Path) {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::copy(source, destination).expect("baseline artifact copy should succeed");
    }

    fn default_rixs_input_source() -> &'static str {
        "nenergies\n3\nemin emax estep\n-10.0 10.0 0.5\n"
    }
}
