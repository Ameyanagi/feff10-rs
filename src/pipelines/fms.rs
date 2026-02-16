use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const FMS_REQUIRED_INPUTS: [&str; 4] = ["fms.inp", "geom.dat", "global.inp", "phase.bin"];
const FMS_OUTPUT_CANDIDATES: [&str; 2] = ["gg.bin", "log3.dat"];
const FMS_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmsPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FmsFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    fms_input_source: String,
    geom_input_source: String,
    global_input_source: String,
    phase_input_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FmsPipelineScaffold;

impl FmsPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<FmsPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(FmsPipelineInterface {
            required_inputs: artifact_list(&FMS_REQUIRED_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for FmsPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let fms_source = read_input_source(&request.input_path, FMS_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(FMS_REQUIRED_INPUTS[1]),
            FMS_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(FMS_REQUIRED_INPUTS[2]),
            FMS_REQUIRED_INPUTS[2],
        )?;
        let phase_bytes = read_input_bytes(
            &input_dir.join(FMS_REQUIRED_INPUTS[3]),
            FMS_REQUIRED_INPUTS[3],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &fms_source,
            &baseline.fms_input_source,
            FMS_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &geom_source,
            &baseline.geom_input_source,
            FMS_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &global_source,
            &baseline.global_input_source,
            FMS_REQUIRED_INPUTS[2],
            &request.fixture_id,
        )?;
        validate_phase_input_against_baseline(
            &phase_bytes,
            &baseline.phase_input_bytes,
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.FMS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create FMS output directory '{}': {}",
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
                        "IO.FMS_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create FMS artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.FMS_OUTPUT_WRITE",
                    format!(
                        "failed to materialize FMS artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Fms {
        return Err(FeffError::input_validation(
            "INPUT.FMS_MODULE",
            format!("FMS pipeline expects module FMS, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.FMS_INPUT_ARTIFACT",
                format!(
                    "FMS pipeline expects input artifact '{}' at '{}'",
                    FMS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(FMS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.FMS_INPUT_ARTIFACT",
            format!(
                "FMS pipeline requires input artifact '{}' but received '{}'",
                FMS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.FMS_INPUT_ARTIFACT",
            format!(
                "FMS pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.FMS_INPUT_READ",
            format!(
                "failed to read FMS input '{}' ({}): {}",
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
            "IO.FMS_INPUT_READ",
            format!(
                "failed to read FMS input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<FmsFixtureBaseline> {
    let baseline_dir = PathBuf::from(FMS_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.FMS_FIXTURE",
            format!(
                "fixture '{}' is not approved for FMS parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let fms_input_source =
        read_baseline_input_source(&baseline_dir.join(FMS_REQUIRED_INPUTS[0]), "fms.inp")?;
    let geom_input_source =
        read_baseline_input_source(&baseline_dir.join(FMS_REQUIRED_INPUTS[1]), "geom.dat")?;
    let global_input_source =
        read_baseline_input_source(&baseline_dir.join(FMS_REQUIRED_INPUTS[2]), "global.inp")?;
    let phase_input_bytes =
        read_baseline_input_bytes(&baseline_dir.join(FMS_REQUIRED_INPUTS[3]), "phase.bin")?;

    let expected_outputs: Vec<PipelineArtifact> = FMS_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.FMS_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any FMS output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(FmsFixtureBaseline {
        baseline_dir,
        expected_outputs,
        fms_input_source,
        geom_input_source,
        global_input_source,
        phase_input_bytes,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.FMS_BASELINE_READ",
            format!(
                "failed to read FMS baseline artifact '{}' ({}): {}",
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
            "IO.FMS_BASELINE_READ",
            format!(
                "failed to read FMS baseline artifact '{}' ({}): {}",
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
    if normalize_fms_source(actual) == normalize_fms_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.FMS_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved FMS parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn validate_phase_input_against_baseline(
    actual: &[u8],
    baseline: &[u8],
    fixture_id: &str,
) -> PipelineResult<()> {
    if actual == baseline {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.FMS_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved FMS parity baseline",
            fixture_id, FMS_REQUIRED_INPUTS[3]
        ),
    ))
}

fn normalize_fms_source(content: &str) -> String {
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
    use super::FmsPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Fms,
            "fms.inp",
            "actual-output",
        );
        let scaffold = FmsPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 4);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("fms.inp")
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
            expected_fms_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("fms.inp");
        let output_dir = temp.path().join("out");
        stage_baseline_artifact("FX-FMS-001", "fms.inp", &input_path);
        stage_baseline_artifact("FX-FMS-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-FMS-001", "global.inp", &temp.path().join("global.inp"));
        stage_baseline_artifact("FX-FMS-001", "phase.bin", &temp.path().join("phase.bin"));

        let request =
            PipelineRequest::new("FX-FMS-001", PipelineModule::Fms, &input_path, &output_dir);
        let scaffold = FmsPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("FMS execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_fms_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-FMS-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_rejects_non_fms_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("fms.inp");
        fs::write(&input_path, "FMS INPUT\n").expect("fms input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");
        fs::write(temp.path().join("phase.bin"), [1_u8, 2_u8]).expect("phase should be written");

        let request =
            PipelineRequest::new("FX-FMS-001", PipelineModule::Path, &input_path, temp.path());
        let scaffold = FmsPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.FMS_MODULE");
    }

    #[test]
    fn execute_requires_phase_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("fms.inp");
        fs::write(&input_path, "FMS INPUT\n").expect("fms input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");

        let request =
            PipelineRequest::new("FX-FMS-001", PipelineModule::Fms, &input_path, temp.path());
        let scaffold = FmsPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing phase input should fail");
        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.FMS_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_fms_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("fms.inp");
        fs::write(&input_path, "FMS INPUT\n").expect("fms input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");
        fs::write(temp.path().join("phase.bin"), [1_u8, 2_u8, 3_u8])
            .expect("phase should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Fms,
            &input_path,
            temp.path(),
        );
        let scaffold = FmsPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.FMS_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("fms.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_artifact("FX-FMS-001", "fms.inp", &input_path);
        stage_baseline_artifact("FX-FMS-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-FMS-001", "global.inp", &temp.path().join("global.inp"));
        stage_baseline_artifact("FX-FMS-001", "phase.bin", &temp.path().join("phase.bin"));

        fs::write(&input_path, "FMS 9.9\nNLEG 999\nRCLUST 100.0\n")
            .expect("fms input should be overwritten");

        let request =
            PipelineRequest::new("FX-FMS-001", PipelineModule::Fms, &input_path, &output_dir);
        let scaffold = FmsPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.FMS_INPUT_MISMATCH");
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

    fn expected_fms_artifact_set() -> BTreeSet<String> {
        ["gg.bin", "log3.dat"]
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }
}
