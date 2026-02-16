use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const FULLSPECTRUM_REQUIRED_INPUTS: [&str; 2] = ["fullspectrum.inp", "xmu.dat"];
const FULLSPECTRUM_OPTIONAL_INPUTS: [&str; 2] = ["prexmu.dat", "referencexmu.dat"];
const FULLSPECTRUM_OUTPUT_CANDIDATES: [&str; 9] = [
    "xmu.dat",
    "osc_str.dat",
    "eps.dat",
    "drude.dat",
    "background.dat",
    "fine_st.dat",
    "logfullspectrum.dat",
    "prexmu.dat",
    "referencexmu.dat",
];
const FULLSPECTRUM_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullSpectrumPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub optional_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FullSpectrumFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    fullspectrum_input_source: String,
    xmu_input_source: String,
    prexmu_input_source: Option<String>,
    referencexmu_input_source: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FullSpectrumPipelineScaffold;

impl FullSpectrumPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<FullSpectrumPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(FullSpectrumPipelineInterface {
            required_inputs: artifact_list(&FULLSPECTRUM_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&FULLSPECTRUM_OPTIONAL_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for FullSpectrumPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let fullspectrum_source =
            read_input_source(&request.input_path, FULLSPECTRUM_REQUIRED_INPUTS[0])?;
        let xmu_source = read_input_source(
            &input_dir.join(FULLSPECTRUM_REQUIRED_INPUTS[1]),
            FULLSPECTRUM_REQUIRED_INPUTS[1],
        )?;
        let prexmu_source = maybe_read_optional_input_source(
            input_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[0]),
            FULLSPECTRUM_OPTIONAL_INPUTS[0],
        )?;
        let referencexmu_source = maybe_read_optional_input_source(
            input_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[1]),
            FULLSPECTRUM_OPTIONAL_INPUTS[1],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &fullspectrum_source,
            &baseline.fullspectrum_input_source,
            FULLSPECTRUM_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &xmu_source,
            &baseline.xmu_input_source,
            FULLSPECTRUM_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_optional_component_input_against_baseline(
            prexmu_source.as_deref(),
            baseline.prexmu_input_source.as_deref(),
            FULLSPECTRUM_OPTIONAL_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_optional_component_input_against_baseline(
            referencexmu_source.as_deref(),
            baseline.referencexmu_input_source.as_deref(),
            FULLSPECTRUM_OPTIONAL_INPUTS[1],
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.FULLSPECTRUM_OUTPUT_DIRECTORY",
                format!(
                    "failed to create FULLSPECTRUM output directory '{}': {}",
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
                        "IO.FULLSPECTRUM_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create FULLSPECTRUM artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.FULLSPECTRUM_OUTPUT_WRITE",
                    format!(
                        "failed to materialize FULLSPECTRUM artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::FullSpectrum {
        return Err(FeffError::input_validation(
            "INPUT.FULLSPECTRUM_MODULE",
            format!(
                "FULLSPECTRUM pipeline expects module FULLSPECTRUM, got {}",
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
                "INPUT.FULLSPECTRUM_INPUT_ARTIFACT",
                format!(
                    "FULLSPECTRUM pipeline expects input artifact '{}' at '{}'",
                    FULLSPECTRUM_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(FULLSPECTRUM_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.FULLSPECTRUM_INPUT_ARTIFACT",
            format!(
                "FULLSPECTRUM pipeline requires input artifact '{}' but received '{}'",
                FULLSPECTRUM_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.FULLSPECTRUM_INPUT_ARTIFACT",
            format!(
                "FULLSPECTRUM pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.FULLSPECTRUM_INPUT_READ",
            format!(
                "failed to read FULLSPECTRUM input '{}' ({}): {}",
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

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<FullSpectrumFixtureBaseline> {
    let baseline_dir = PathBuf::from(FULLSPECTRUM_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.FULLSPECTRUM_FIXTURE",
            format!(
                "fixture '{}' is not approved for FULLSPECTRUM parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let fullspectrum_input_source = read_baseline_input_source(
        &baseline_dir.join(FULLSPECTRUM_REQUIRED_INPUTS[0]),
        FULLSPECTRUM_REQUIRED_INPUTS[0],
    )?;
    let xmu_input_source = read_baseline_input_source(
        &baseline_dir.join(FULLSPECTRUM_REQUIRED_INPUTS[1]),
        FULLSPECTRUM_REQUIRED_INPUTS[1],
    )?;
    let prexmu_input_source = maybe_read_optional_baseline_source(
        baseline_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[0]),
        FULLSPECTRUM_OPTIONAL_INPUTS[0],
    )?;
    let referencexmu_input_source = maybe_read_optional_baseline_source(
        baseline_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[1]),
        FULLSPECTRUM_OPTIONAL_INPUTS[1],
    )?;

    let expected_outputs: Vec<PipelineArtifact> = FULLSPECTRUM_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.FULLSPECTRUM_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any FULLSPECTRUM output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(FullSpectrumFixtureBaseline {
        baseline_dir,
        expected_outputs,
        fullspectrum_input_source,
        xmu_input_source,
        prexmu_input_source,
        referencexmu_input_source,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.FULLSPECTRUM_BASELINE_READ",
            format!(
                "failed to read FULLSPECTRUM baseline artifact '{}' ({}): {}",
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
    if normalize_fullspectrum_source(actual) == normalize_fullspectrum_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.FULLSPECTRUM_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved FULLSPECTRUM parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn validate_optional_component_input_against_baseline(
    actual: Option<&str>,
    baseline: Option<&str>,
    artifact: &str,
    fixture_id: &str,
) -> PipelineResult<()> {
    let Some(actual) = actual else {
        return Ok(());
    };

    let Some(baseline) = baseline else {
        return Ok(());
    };

    if normalize_fullspectrum_source(actual) == normalize_fullspectrum_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.FULLSPECTRUM_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved FULLSPECTRUM parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn normalize_fullspectrum_source(content: &str) -> String {
    content
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn artifact_list(artifacts: &[&str]) -> Vec<PipelineArtifact> {
    artifacts
        .iter()
        .copied()
        .map(PipelineArtifact::new)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        FULLSPECTRUM_OPTIONAL_INPUTS, FULLSPECTRUM_OUTPUT_CANDIDATES, FullSpectrumPipelineScaffold,
    };
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_reports_fullspectrum_artifact_expectations() {
        let request = PipelineRequest::new(
            "FX-FULLSPECTRUM-001",
            PipelineModule::FullSpectrum,
            "fullspectrum.inp",
            "out",
        );
        let scaffold = FullSpectrumPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("FULLSPECTRUM contract should load");

        assert_eq!(contract.required_inputs.len(), 2);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("fullspectrum.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("xmu.dat")
        );
        assert_eq!(contract.optional_inputs.len(), 2);
        assert_eq!(
            contract.optional_inputs[0].relative_path,
            PathBuf::from(FULLSPECTRUM_OPTIONAL_INPUTS[0])
        );
        assert_eq!(
            contract.optional_inputs[1].relative_path,
            PathBuf::from(FULLSPECTRUM_OPTIONAL_INPUTS[1])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_fullspectrum_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("fullspectrum.inp");
        let output_dir = temp.path().join("out");
        stage_baseline_artifact("FX-FULLSPECTRUM-001", "fullspectrum.inp", &input_path);
        stage_baseline_artifact(
            "FX-FULLSPECTRUM-001",
            "xmu.dat",
            &temp.path().join("xmu.dat"),
        );
        stage_optional_component_input(
            "FX-FULLSPECTRUM-001",
            FULLSPECTRUM_OPTIONAL_INPUTS[0],
            &temp.path().join(FULLSPECTRUM_OPTIONAL_INPUTS[0]),
        );
        stage_optional_component_input(
            "FX-FULLSPECTRUM-001",
            FULLSPECTRUM_OPTIONAL_INPUTS[1],
            &temp.path().join(FULLSPECTRUM_OPTIONAL_INPUTS[1]),
        );

        let request = PipelineRequest::new(
            "FX-FULLSPECTRUM-001",
            PipelineModule::FullSpectrum,
            &input_path,
            &output_dir,
        );
        let scaffold = FullSpectrumPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("FULLSPECTRUM execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_fullspectrum_artifact_set()
        );
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path =
                fixture_baseline_dir("FX-FULLSPECTRUM-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_allows_missing_optional_component_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("fullspectrum.inp");
        let output_dir = temp.path().join("out");
        stage_baseline_artifact("FX-FULLSPECTRUM-001", "fullspectrum.inp", &input_path);
        stage_baseline_artifact(
            "FX-FULLSPECTRUM-001",
            "xmu.dat",
            &temp.path().join("xmu.dat"),
        );

        let request = PipelineRequest::new(
            "FX-FULLSPECTRUM-001",
            PipelineModule::FullSpectrum,
            &input_path,
            &output_dir,
        );
        let scaffold = FullSpectrumPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("FULLSPECTRUM execution should succeed without optional component inputs");

        assert_eq!(
            artifact_set(&artifacts),
            expected_fullspectrum_artifact_set()
        );
    }

    #[test]
    fn execute_rejects_non_fullspectrum_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("fullspectrum.inp");
        fs::write(&input_path, "mFullSpectrum\n0\n").expect("fullspectrum input should be written");
        fs::write(temp.path().join("xmu.dat"), "0.0 0.0\n").expect("xmu should be written");

        let request = PipelineRequest::new(
            "FX-FULLSPECTRUM-001",
            PipelineModule::Eels,
            &input_path,
            temp.path(),
        );
        let scaffold = FullSpectrumPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.FULLSPECTRUM_MODULE");
    }

    #[test]
    fn execute_requires_xmu_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("fullspectrum.inp");
        stage_baseline_artifact("FX-FULLSPECTRUM-001", "fullspectrum.inp", &input_path);

        let request = PipelineRequest::new(
            "FX-FULLSPECTRUM-001",
            PipelineModule::FullSpectrum,
            &input_path,
            temp.path(),
        );
        let scaffold = FullSpectrumPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing xmu input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.FULLSPECTRUM_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_fullspectrum_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("fullspectrum.inp");
        fs::write(&input_path, "mFullSpectrum\n0\n").expect("fullspectrum input should be written");
        fs::write(temp.path().join("xmu.dat"), "0.0 0.0\n").expect("xmu should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::FullSpectrum,
            &input_path,
            temp.path(),
        );
        let scaffold = FullSpectrumPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.FULLSPECTRUM_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("fullspectrum.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_artifact("FX-FULLSPECTRUM-001", "fullspectrum.inp", &input_path);
        stage_baseline_artifact(
            "FX-FULLSPECTRUM-001",
            "xmu.dat",
            &temp.path().join("xmu.dat"),
        );
        stage_optional_component_input(
            "FX-FULLSPECTRUM-001",
            FULLSPECTRUM_OPTIONAL_INPUTS[0],
            &temp.path().join(FULLSPECTRUM_OPTIONAL_INPUTS[0]),
        );
        stage_optional_component_input(
            "FX-FULLSPECTRUM-001",
            FULLSPECTRUM_OPTIONAL_INPUTS[1],
            &temp.path().join(FULLSPECTRUM_OPTIONAL_INPUTS[1]),
        );

        fs::write(temp.path().join("xmu.dat"), "drifted xmu input\n")
            .expect("xmu input should be overwritten");

        let request = PipelineRequest::new(
            "FX-FULLSPECTRUM-001",
            PipelineModule::FullSpectrum,
            &input_path,
            &output_dir,
        );
        let scaffold = FullSpectrumPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.FULLSPECTRUM_INPUT_MISMATCH");
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

    fn stage_optional_component_input(fixture_id: &str, artifact: &str, destination: &Path) {
        let source = fixture_baseline_dir(fixture_id).join(artifact);
        if !source.is_file() {
            return;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::copy(&source, destination).expect("optional component input copy should succeed");
    }

    fn expected_fullspectrum_artifact_set() -> BTreeSet<String> {
        let baseline_dir = fixture_baseline_dir("FX-FULLSPECTRUM-001");
        let artifacts: BTreeSet<String> = FULLSPECTRUM_OUTPUT_CANDIDATES
            .iter()
            .filter(|artifact| baseline_dir.join(artifact).is_file())
            .map(|artifact| artifact.to_string())
            .collect();

        assert!(
            !artifacts.is_empty(),
            "FX-FULLSPECTRUM-001 should provide at least one FULLSPECTRUM output"
        );
        artifacts
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }
}
