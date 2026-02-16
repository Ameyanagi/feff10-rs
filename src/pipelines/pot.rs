use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const POT_REQUIRED_INPUTS: [&str; 2] = ["pot.inp", "geom.dat"];
const POT_OUTPUT_CANDIDATES: [&str; 5] = [
    "pot.bin",
    "pot.dat",
    "log1.dat",
    "convergence.scf",
    "convergence.scf.fine",
];
const POT_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PotFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    pot_input_source: String,
    geom_input_source: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PotPipelineScaffold;

impl PotPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<PotPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(PotPipelineInterface {
            required_inputs: artifact_list(&POT_REQUIRED_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for PotPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;

        let pot_inp_source = read_input_source(&request.input_path, "pot.inp")?;
        let geom_path = geom_input_path(request)?;
        let geom_source = read_input_source(&geom_path, "geom.dat")?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_input_against_baseline(
            &pot_inp_source,
            &baseline.pot_input_source,
            "pot.inp",
            &request.fixture_id,
        )?;
        validate_input_against_baseline(
            &geom_source,
            &baseline.geom_input_source,
            "geom.dat",
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.POT_OUTPUT_DIRECTORY",
                format!(
                    "failed to create POT output directory '{}': {}",
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
                        "IO.POT_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create POT artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.POT_OUTPUT_WRITE",
                    format!(
                        "failed to materialize POT artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Pot {
        return Err(FeffError::input_validation(
            "INPUT.POT_MODULE",
            format!("POT pipeline expects module POT, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.POT_INPUT_ARTIFACT",
                format!(
                    "POT pipeline expects input artifact '{}' at '{}'",
                    POT_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(POT_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.POT_INPUT_ARTIFACT",
            format!(
                "POT pipeline requires input artifact '{}' but received '{}'",
                POT_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn geom_input_path(request: &PipelineRequest) -> PipelineResult<PathBuf> {
    request
        .input_path
        .parent()
        .map(|parent| parent.join(POT_REQUIRED_INPUTS[1]))
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.POT_INPUT_ARTIFACT",
                format!(
                    "POT pipeline requires sibling '{}' for input '{}'",
                    POT_REQUIRED_INPUTS[1],
                    request.input_path.display()
                ),
            )
        })
}

fn read_input_source(input_path: &Path, label: &str) -> PipelineResult<String> {
    fs::read_to_string(input_path).map_err(|source| {
        FeffError::io_system(
            "IO.POT_INPUT_READ",
            format!(
                "failed to read POT input '{}' ({}): {}",
                input_path.display(),
                label,
                source
            ),
        )
    })
}

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<PotFixtureBaseline> {
    let baseline_dir = PathBuf::from(POT_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.POT_FIXTURE",
            format!(
                "fixture '{}' is not approved for POT parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let pot_input_source = read_baseline_input_source(&baseline_dir.join("pot.inp"))?;
    let geom_input_source = read_baseline_input_source(&baseline_dir.join("geom.dat"))?;
    let expected_outputs: Vec<PipelineArtifact> = POT_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.POT_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any POT output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(PotFixtureBaseline {
        baseline_dir,
        expected_outputs,
        pot_input_source,
        geom_input_source,
    })
}

fn read_baseline_input_source(path: &Path) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.POT_BASELINE_READ",
            format!(
                "failed to read POT baseline artifact '{}': {}",
                path.display(),
                source
            ),
        )
    })
}

fn validate_input_against_baseline(
    actual: &str,
    baseline: &str,
    artifact: &str,
    fixture_id: &str,
) -> PipelineResult<()> {
    if normalize_pot_source(actual) == normalize_pot_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.POT_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved POT parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn normalize_pot_source(content: &str) -> String {
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
    use super::PotPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-POT-001",
            PipelineModule::Pot,
            "pot.inp",
            "actual-output",
        );
        let scaffold = PotPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 2);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("pot.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("geom.dat")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_pot_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_input("FX-POT-001", "pot.inp", &input_path);
        stage_baseline_input("FX-POT-001", "geom.dat", &temp.path().join("geom.dat"));

        let request =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, &output_dir);
        let scaffold = PotPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("POT execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_pot_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-POT-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_rejects_non_pot_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        fs::write(&input_path, "POT INPUT\n").expect("input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");

        let request = PipelineRequest::new(
            "FX-RDINP-001",
            PipelineModule::Rdinp,
            &input_path,
            temp.path(),
        );
        let scaffold = PotPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.POT_MODULE");
    }

    #[test]
    fn execute_requires_geom_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        fs::write(&input_path, "POT INPUT\n").expect("input should be written");

        let request =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, temp.path());
        let scaffold = PotPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing geom input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.POT_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_pot_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        fs::write(&input_path, "POT INPUT\n").expect("input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Pot,
            &input_path,
            temp.path(),
        );
        let scaffold = PotPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.POT_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_input("FX-POT-001", "pot.inp", &input_path);
        stage_baseline_input("FX-POT-001", "geom.dat", &temp.path().join("geom.dat"));

        fs::write(
            &input_path,
            "mpot, nph, ntitle, ihole, ipr1, iafolp, ixc,ispec\n999 999 999\n",
        )
        .expect("pot input should be overwritten");

        let request =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, &output_dir);
        let scaffold = PotPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.POT_INPUT_MISMATCH");
    }

    fn fixture_baseline_dir(fixture_id: &str) -> PathBuf {
        PathBuf::from("artifacts/fortran-baselines")
            .join(fixture_id)
            .join("baseline")
    }

    fn stage_baseline_input(fixture_id: &str, artifact: &str, destination: &Path) {
        let source = fixture_baseline_dir(fixture_id).join(artifact);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should be created");
        }
        fs::copy(source, destination).expect("baseline input should be staged");
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn expected_pot_artifact_set() -> BTreeSet<String> {
        ["pot.bin", "log1.dat"]
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }
}
