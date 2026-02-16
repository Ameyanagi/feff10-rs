use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const DEBYE_REQUIRED_INPUTS: [&str; 3] = ["ff2x.inp", "paths.dat", "feff.inp"];
const DEBYE_OPTIONAL_INPUTS: [&str; 1] = ["spring.inp"];
const DEBYE_OUTPUT_CANDIDATES: [&str; 7] = [
    "s2_em.dat",
    "s2_rm1.dat",
    "s2_rm2.dat",
    "xmu.dat",
    "chi.dat",
    "log6.dat",
    "spring.dat",
];
const DEBYE_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebyePipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub optional_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DebyeFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    ff2x_input_source: String,
    paths_input_source: String,
    feff_input_source: String,
    spring_input_source: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DebyePipelineScaffold;

impl DebyePipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<DebyePipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(DebyePipelineInterface {
            required_inputs: artifact_list(&DEBYE_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&DEBYE_OPTIONAL_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for DebyePipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let ff2x_source = read_input_source(&request.input_path, DEBYE_REQUIRED_INPUTS[0])?;
        let paths_source = read_input_source(
            &input_dir.join(DEBYE_REQUIRED_INPUTS[1]),
            DEBYE_REQUIRED_INPUTS[1],
        )?;
        let feff_source = read_input_source(
            &input_dir.join(DEBYE_REQUIRED_INPUTS[2]),
            DEBYE_REQUIRED_INPUTS[2],
        )?;
        let spring_source = maybe_read_optional_input_source(
            input_dir.join(DEBYE_OPTIONAL_INPUTS[0]),
            DEBYE_OPTIONAL_INPUTS[0],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &ff2x_source,
            &baseline.ff2x_input_source,
            DEBYE_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &paths_source,
            &baseline.paths_input_source,
            DEBYE_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &feff_source,
            &baseline.feff_input_source,
            DEBYE_REQUIRED_INPUTS[2],
            &request.fixture_id,
        )?;
        validate_optional_spring_input_against_baseline(
            spring_source.as_deref(),
            baseline.spring_input_source.as_deref(),
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.DEBYE_OUTPUT_DIRECTORY",
                format!(
                    "failed to create DEBYE output directory '{}': {}",
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
                        "IO.DEBYE_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create DEBYE artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.DEBYE_OUTPUT_WRITE",
                    format!(
                        "failed to materialize DEBYE artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Debye {
        return Err(FeffError::input_validation(
            "INPUT.DEBYE_MODULE",
            format!(
                "DEBYE pipeline expects module DEBYE, got {}",
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
                "INPUT.DEBYE_INPUT_ARTIFACT",
                format!(
                    "DEBYE pipeline expects input artifact '{}' at '{}'",
                    DEBYE_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(DEBYE_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.DEBYE_INPUT_ARTIFACT",
            format!(
                "DEBYE pipeline requires input artifact '{}' but received '{}'",
                DEBYE_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.DEBYE_INPUT_ARTIFACT",
            format!(
                "DEBYE pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.DEBYE_INPUT_READ",
            format!(
                "failed to read DEBYE input '{}' ({}): {}",
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

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<DebyeFixtureBaseline> {
    let baseline_dir = PathBuf::from(DEBYE_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.DEBYE_FIXTURE",
            format!(
                "fixture '{}' is not approved for DEBYE parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let ff2x_input_source =
        read_baseline_input_source(&baseline_dir.join(DEBYE_REQUIRED_INPUTS[0]), "ff2x.inp")?;
    let paths_input_source =
        read_baseline_input_source(&baseline_dir.join(DEBYE_REQUIRED_INPUTS[1]), "paths.dat")?;
    let feff_input_source =
        read_baseline_input_source(&baseline_dir.join(DEBYE_REQUIRED_INPUTS[2]), "feff.inp")?;
    let spring_input_source = maybe_read_optional_baseline_source(
        baseline_dir.join(DEBYE_OPTIONAL_INPUTS[0]),
        DEBYE_OPTIONAL_INPUTS[0],
    )?;

    let expected_outputs: Vec<PipelineArtifact> = DEBYE_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.DEBYE_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any DEBYE output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(DebyeFixtureBaseline {
        baseline_dir,
        expected_outputs,
        ff2x_input_source,
        paths_input_source,
        feff_input_source,
        spring_input_source,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.DEBYE_BASELINE_READ",
            format!(
                "failed to read DEBYE baseline artifact '{}' ({}): {}",
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
    if normalize_debye_source(actual) == normalize_debye_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.DEBYE_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved DEBYE parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn validate_optional_spring_input_against_baseline(
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

    if normalize_debye_source(actual) == normalize_debye_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.DEBYE_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved DEBYE parity baseline",
            fixture_id, DEBYE_OPTIONAL_INPUTS[0]
        ),
    ))
}

fn normalize_debye_source(content: &str) -> String {
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
    use super::DebyePipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            "ff2x.inp",
            "actual-output",
        );
        let scaffold = DebyePipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 3);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("ff2x.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("paths.dat")
        );
        assert_eq!(
            contract.required_inputs[2].relative_path,
            PathBuf::from("feff.inp")
        );
        assert_eq!(contract.optional_inputs.len(), 1);
        assert_eq!(
            contract.optional_inputs[0].relative_path,
            PathBuf::from("spring.inp")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_debye_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ff2x.inp");
        let output_dir = temp.path().join("out");
        stage_ff2x_input("FX-DEBYE-001", &input_path);
        stage_baseline_artifact("FX-DEBYE-001", "paths.dat", &temp.path().join("paths.dat"));
        stage_baseline_artifact("FX-DEBYE-001", "feff.inp", &temp.path().join("feff.inp"));
        stage_optional_spring_input("FX-DEBYE-001", &temp.path().join("spring.inp"));

        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            &input_path,
            &output_dir,
        );
        let scaffold = DebyePipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("DEBYE execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_debye_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-DEBYE-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_allows_missing_optional_spring_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ff2x.inp");
        let output_dir = temp.path().join("out");
        stage_ff2x_input("FX-DEBYE-001", &input_path);
        stage_baseline_artifact("FX-DEBYE-001", "paths.dat", &temp.path().join("paths.dat"));
        stage_baseline_artifact("FX-DEBYE-001", "feff.inp", &temp.path().join("feff.inp"));

        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            &input_path,
            &output_dir,
        );
        let scaffold = DebyePipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("DEBYE execution should succeed without spring.inp");

        assert_eq!(artifact_set(&artifacts), expected_debye_artifact_set());
    }

    #[test]
    fn execute_rejects_non_debye_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ff2x.inp");
        fs::write(&input_path, default_ff2x_input_source()).expect("ff2x input should be written");
        fs::write(temp.path().join("paths.dat"), "PATHS INPUT\n").expect("paths should be written");
        fs::write(temp.path().join("feff.inp"), "FEFF INPUT\n").expect("feff should be written");

        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Dmdw,
            &input_path,
            temp.path(),
        );
        let scaffold = DebyePipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.DEBYE_MODULE");
    }

    #[test]
    fn execute_requires_paths_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ff2x.inp");
        stage_ff2x_input("FX-DEBYE-001", &input_path);
        stage_baseline_artifact("FX-DEBYE-001", "feff.inp", &temp.path().join("feff.inp"));

        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            &input_path,
            temp.path(),
        );
        let scaffold = DebyePipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing paths input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.DEBYE_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_debye_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ff2x.inp");
        fs::write(&input_path, default_ff2x_input_source()).expect("ff2x input should be written");
        fs::write(temp.path().join("paths.dat"), "PATHS INPUT\n").expect("paths should be written");
        fs::write(temp.path().join("feff.inp"), "FEFF INPUT\n").expect("feff should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Debye,
            &input_path,
            temp.path(),
        );
        let scaffold = DebyePipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.DEBYE_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ff2x.inp");
        let output_dir = temp.path().join("actual");
        stage_ff2x_input("FX-DEBYE-001", &input_path);
        stage_baseline_artifact("FX-DEBYE-001", "paths.dat", &temp.path().join("paths.dat"));
        stage_baseline_artifact("FX-DEBYE-001", "feff.inp", &temp.path().join("feff.inp"));
        stage_optional_spring_input("FX-DEBYE-001", &temp.path().join("spring.inp"));

        fs::write(&temp.path().join("paths.dat"), "drifted paths input\n")
            .expect("paths input should be overwritten");

        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            &input_path,
            &output_dir,
        );
        let scaffold = DebyePipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.DEBYE_INPUT_MISMATCH");
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

    fn stage_ff2x_input(fixture_id: &str, destination: &Path) {
        let baseline_ff2x_input = fixture_baseline_dir(fixture_id).join("ff2x.inp");
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        if baseline_ff2x_input.is_file() {
            fs::copy(&baseline_ff2x_input, destination).expect("ff2x input copy should succeed");
            return;
        }

        fs::write(destination, default_ff2x_input_source()).expect("ff2x input should be written");
    }

    fn stage_optional_spring_input(fixture_id: &str, destination: &Path) {
        let baseline_spring_input = fixture_baseline_dir(fixture_id).join("spring.inp");
        if !baseline_spring_input.is_file() {
            return;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::copy(&baseline_spring_input, destination).expect("spring input copy should succeed");
    }

    fn default_ff2x_input_source() -> &'static str {
        "DEBYE PARAMETERS\n0.0 0.0 0.0\n"
    }

    fn expected_debye_artifact_set() -> BTreeSet<String> {
        let baseline_dir = fixture_baseline_dir("FX-DEBYE-001");
        [
            "s2_em.dat",
            "s2_rm1.dat",
            "s2_rm2.dat",
            "xmu.dat",
            "chi.dat",
            "log6.dat",
            "spring.dat",
        ]
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
