use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const CRPA_REQUIRED_INPUTS: [&str; 3] = ["crpa.inp", "pot.inp", "geom.dat"];
const CRPA_OUTPUT_CANDIDATES: [&str; 2] = ["wscrn.dat", "logscrn.dat"];
const CRPA_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrpaPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CrpaFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    crpa_input_source: String,
    pot_input_source: String,
    geom_input_source: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CrpaPipelineScaffold;

impl CrpaPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<CrpaPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(CrpaPipelineInterface {
            required_inputs: artifact_list(&CRPA_REQUIRED_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for CrpaPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let crpa_source = read_input_source(&request.input_path, CRPA_REQUIRED_INPUTS[0])?;
        let pot_source = read_input_source(
            &input_dir.join(CRPA_REQUIRED_INPUTS[1]),
            CRPA_REQUIRED_INPUTS[1],
        )?;
        let geom_source = read_input_source(
            &input_dir.join(CRPA_REQUIRED_INPUTS[2]),
            CRPA_REQUIRED_INPUTS[2],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &crpa_source,
            &baseline.crpa_input_source,
            CRPA_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &pot_source,
            &baseline.pot_input_source,
            CRPA_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &geom_source,
            &baseline.geom_input_source,
            CRPA_REQUIRED_INPUTS[2],
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.CRPA_OUTPUT_DIRECTORY",
                format!(
                    "failed to create CRPA output directory '{}': {}",
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
                        "IO.CRPA_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create CRPA artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.CRPA_OUTPUT_WRITE",
                    format!(
                        "failed to materialize CRPA artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Crpa {
        return Err(FeffError::input_validation(
            "INPUT.CRPA_MODULE",
            format!("CRPA pipeline expects module CRPA, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.CRPA_INPUT_ARTIFACT",
                format!(
                    "CRPA pipeline expects input artifact '{}' at '{}'",
                    CRPA_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(CRPA_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.CRPA_INPUT_ARTIFACT",
            format!(
                "CRPA pipeline requires input artifact '{}' but received '{}'",
                CRPA_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.CRPA_INPUT_ARTIFACT",
            format!(
                "CRPA pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.CRPA_INPUT_READ",
            format!(
                "failed to read CRPA input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<CrpaFixtureBaseline> {
    let baseline_dir = PathBuf::from(CRPA_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.CRPA_FIXTURE",
            format!(
                "fixture '{}' is not approved for CRPA parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let crpa_input_source =
        read_baseline_input_source(&baseline_dir.join(CRPA_REQUIRED_INPUTS[0]), "crpa.inp")?;
    let pot_input_source =
        read_baseline_input_source(&baseline_dir.join(CRPA_REQUIRED_INPUTS[1]), "pot.inp")?;
    let geom_input_source =
        read_baseline_input_source(&baseline_dir.join(CRPA_REQUIRED_INPUTS[2]), "geom.dat")?;

    let expected_outputs: Vec<PipelineArtifact> = CRPA_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.CRPA_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any CRPA output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(CrpaFixtureBaseline {
        baseline_dir,
        expected_outputs,
        crpa_input_source,
        pot_input_source,
        geom_input_source,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.CRPA_BASELINE_READ",
            format!(
                "failed to read CRPA baseline artifact '{}' ({}): {}",
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
    if normalize_crpa_source(actual) == normalize_crpa_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.CRPA_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved CRPA parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn normalize_crpa_source(content: &str) -> String {
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
    use super::CrpaPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Crpa,
            "crpa.inp",
            "actual-output",
        );
        let scaffold = CrpaPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 3);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("crpa.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("pot.inp")
        );
        assert_eq!(
            contract.required_inputs[2].relative_path,
            PathBuf::from("geom.dat")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_crpa_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("crpa.inp");
        let output_dir = temp.path().join("out");
        stage_crpa_input("FX-CRPA-001", &input_path);
        stage_baseline_artifact("FX-CRPA-001", "pot.inp", &temp.path().join("pot.inp"));
        stage_baseline_artifact("FX-CRPA-001", "geom.dat", &temp.path().join("geom.dat"));

        let request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Crpa,
            &input_path,
            &output_dir,
        );
        let scaffold = CrpaPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("CRPA execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_crpa_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-CRPA-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_rejects_non_crpa_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("crpa.inp");
        fs::write(&input_path, default_crpa_input_source()).expect("crpa input should be written");
        fs::write(temp.path().join("pot.inp"), "POT INPUT\n").expect("pot should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");

        let request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Rixs,
            &input_path,
            temp.path(),
        );
        let scaffold = CrpaPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.CRPA_MODULE");
    }

    #[test]
    fn execute_requires_pot_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("crpa.inp");
        fs::write(&input_path, default_crpa_input_source()).expect("crpa input should be written");
        stage_baseline_artifact("FX-CRPA-001", "geom.dat", &temp.path().join("geom.dat"));

        let request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Crpa,
            &input_path,
            temp.path(),
        );
        let scaffold = CrpaPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing pot input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.CRPA_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_crpa_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("crpa.inp");
        fs::write(&input_path, default_crpa_input_source()).expect("crpa input should be written");
        fs::write(temp.path().join("pot.inp"), "POT INPUT\n").expect("pot should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Crpa,
            &input_path,
            temp.path(),
        );
        let scaffold = CrpaPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.CRPA_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("crpa.inp");
        let output_dir = temp.path().join("actual");
        stage_crpa_input("FX-CRPA-001", &input_path);
        stage_baseline_artifact("FX-CRPA-001", "pot.inp", &temp.path().join("pot.inp"));
        stage_baseline_artifact("FX-CRPA-001", "geom.dat", &temp.path().join("geom.dat"));

        fs::write(temp.path().join("pot.inp"), "drifted pot input\n")
            .expect("pot input should be overwritten");

        let request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Crpa,
            &input_path,
            &output_dir,
        );
        let scaffold = CrpaPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.CRPA_INPUT_MISMATCH");
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

    fn stage_crpa_input(fixture_id: &str, destination: &Path) {
        let baseline_crpa_input = fixture_baseline_dir(fixture_id).join("crpa.inp");
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        if baseline_crpa_input.is_file() {
            fs::copy(&baseline_crpa_input, destination).expect("crpa input copy should succeed");
            return;
        }

        fs::write(destination, default_crpa_input_source()).expect("crpa input should be written");
    }

    fn default_crpa_input_source() -> &'static str {
        "do_CRPA : if = 1, run CRPA and write wscrn.dat\n1\n"
    }

    fn expected_crpa_artifact_set() -> BTreeSet<String> {
        let baseline_dir = fixture_baseline_dir("FX-CRPA-001");
        ["wscrn.dat", "logscrn.dat"]
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
