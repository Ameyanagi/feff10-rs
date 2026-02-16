use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const LDOS_REQUIRED_INPUTS: [&str; 4] = ["ldos.inp", "geom.dat", "pot.bin", "reciprocal.inp"];
const LDOS_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdosPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LdosFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    ldos_input_source: String,
    geom_input_source: String,
    pot_input_bytes: Vec<u8>,
    reciprocal_input_source: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LdosPipelineScaffold;

impl LdosPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<LdosPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(LdosPipelineInterface {
            required_inputs: artifact_list(&LDOS_REQUIRED_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for LdosPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let ldos_source = read_input_source(&request.input_path, LDOS_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(LDOS_REQUIRED_INPUTS[1]),
            LDOS_REQUIRED_INPUTS[1],
        )?;
        let pot_bytes = read_input_bytes(
            &input_dir.join(LDOS_REQUIRED_INPUTS[2]),
            LDOS_REQUIRED_INPUTS[2],
        )?;
        let reciprocal_source = read_input_source(
            &input_dir.join(LDOS_REQUIRED_INPUTS[3]),
            LDOS_REQUIRED_INPUTS[3],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &ldos_source,
            &baseline.ldos_input_source,
            LDOS_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &geom_source,
            &baseline.geom_input_source,
            LDOS_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_binary_input_against_baseline(
            &pot_bytes,
            &baseline.pot_input_bytes,
            LDOS_REQUIRED_INPUTS[2],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &reciprocal_source,
            &baseline.reciprocal_input_source,
            LDOS_REQUIRED_INPUTS[3],
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.LDOS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create LDOS output directory '{}': {}",
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
                        "IO.LDOS_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create LDOS artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.LDOS_OUTPUT_WRITE",
                    format!(
                        "failed to materialize LDOS artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Ldos {
        return Err(FeffError::input_validation(
            "INPUT.LDOS_MODULE",
            format!("LDOS pipeline expects module LDOS, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.LDOS_INPUT_ARTIFACT",
                format!(
                    "LDOS pipeline expects input artifact '{}' at '{}'",
                    LDOS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(LDOS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.LDOS_INPUT_ARTIFACT",
            format!(
                "LDOS pipeline requires input artifact '{}' but received '{}'",
                LDOS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.LDOS_INPUT_ARTIFACT",
            format!(
                "LDOS pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.LDOS_INPUT_READ",
            format!(
                "failed to read LDOS input '{}' ({}): {}",
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
            "IO.LDOS_INPUT_READ",
            format!(
                "failed to read LDOS input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<LdosFixtureBaseline> {
    let baseline_dir = PathBuf::from(LDOS_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.LDOS_FIXTURE",
            format!(
                "fixture '{}' is not approved for LDOS parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let ldos_input_source =
        read_baseline_input_source(&baseline_dir.join(LDOS_REQUIRED_INPUTS[0]), "ldos.inp")?;
    let geom_input_source =
        read_baseline_input_source(&baseline_dir.join(LDOS_REQUIRED_INPUTS[1]), "geom.dat")?;
    let pot_input_bytes =
        read_baseline_input_bytes(&baseline_dir.join(LDOS_REQUIRED_INPUTS[2]), "pot.bin")?;
    let reciprocal_input_source = read_baseline_input_source(
        &baseline_dir.join(LDOS_REQUIRED_INPUTS[3]),
        "reciprocal.inp",
    )?;

    let expected_outputs = collect_expected_outputs(&baseline_dir, fixture_id)?;

    Ok(LdosFixtureBaseline {
        baseline_dir,
        expected_outputs,
        ldos_input_source,
        geom_input_source,
        pot_input_bytes,
        reciprocal_input_source,
    })
}

fn collect_expected_outputs(
    baseline_dir: &Path,
    fixture_id: &str,
) -> PipelineResult<Vec<PipelineArtifact>> {
    let entries = fs::read_dir(baseline_dir).map_err(|source| {
        FeffError::io_system(
            "IO.LDOS_BASELINE_READ",
            format!(
                "failed to enumerate LDOS baseline directory '{}' for fixture '{}': {}",
                baseline_dir.display(),
                fixture_id,
                source
            ),
        )
    })?;

    let mut outputs = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| {
            FeffError::io_system(
                "IO.LDOS_BASELINE_READ",
                format!(
                    "failed to read LDOS baseline directory entry under '{}' for fixture '{}': {}",
                    baseline_dir.display(),
                    fixture_id,
                    source
                ),
            )
        })?;

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if is_ldos_output_file_name(file_name) {
            outputs.push(PipelineArtifact::new(file_name));
        }
    }

    outputs.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    if outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.LDOS_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any LDOS output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(outputs)
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.LDOS_BASELINE_READ",
            format!(
                "failed to read LDOS baseline artifact '{}' ({}): {}",
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
            "IO.LDOS_BASELINE_READ",
            format!(
                "failed to read LDOS baseline artifact '{}' ({}): {}",
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
    if normalize_ldos_source(actual) == normalize_ldos_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.LDOS_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved LDOS parity baseline",
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
        "RUN.LDOS_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved LDOS parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn is_ldos_output_file_name(file_name: &str) -> bool {
    if file_name.eq_ignore_ascii_case("logdos.dat") {
        return true;
    }

    let lowered = file_name.to_ascii_lowercase();
    lowered.starts_with("ldos") && lowered.ends_with(".dat")
}

fn normalize_ldos_source(content: &str) -> String {
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
    use super::LdosPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-LDOS-001",
            PipelineModule::Ldos,
            "ldos.inp",
            "actual-output",
        );
        let scaffold = LdosPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 4);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("ldos.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("geom.dat")
        );
        assert_eq!(
            contract.required_inputs[2].relative_path,
            PathBuf::from("pot.bin")
        );
        assert_eq!(
            contract.required_inputs[3].relative_path,
            PathBuf::from("reciprocal.inp")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_ldos_artifact_set("FX-LDOS-001")
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ldos.inp");
        let output_dir = temp.path().join("out");
        stage_baseline_artifact("FX-LDOS-001", "ldos.inp", &input_path);
        stage_baseline_artifact("FX-LDOS-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-LDOS-001", "pot.bin", &temp.path().join("pot.bin"));
        stage_baseline_artifact(
            "FX-LDOS-001",
            "reciprocal.inp",
            &temp.path().join("reciprocal.inp"),
        );

        let request = PipelineRequest::new(
            "FX-LDOS-001",
            PipelineModule::Ldos,
            &input_path,
            &output_dir,
        );
        let scaffold = LdosPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("LDOS execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_ldos_artifact_set("FX-LDOS-001")
        );
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-LDOS-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_rejects_non_ldos_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ldos.inp");
        fs::write(&input_path, "LDOS INPUT\n").expect("ldos input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("pot.bin"), [1_u8, 2_u8]).expect("pot should be written");
        fs::write(temp.path().join("reciprocal.inp"), "R 0.0 0.0 0.0\n")
            .expect("reciprocal should be written");

        let request = PipelineRequest::new(
            "FX-LDOS-001",
            PipelineModule::Band,
            &input_path,
            temp.path(),
        );
        let scaffold = LdosPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.LDOS_MODULE");
    }

    #[test]
    fn execute_requires_pot_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ldos.inp");
        fs::write(&input_path, "LDOS INPUT\n").expect("ldos input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("reciprocal.inp"), "R 0.0 0.0 0.0\n")
            .expect("reciprocal should be written");

        let request = PipelineRequest::new(
            "FX-LDOS-001",
            PipelineModule::Ldos,
            &input_path,
            temp.path(),
        );
        let scaffold = LdosPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing pot input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.LDOS_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_ldos_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ldos.inp");
        fs::write(&input_path, "LDOS INPUT\n").expect("ldos input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("pot.bin"), [1_u8, 2_u8, 3_u8]).expect("pot should be written");
        fs::write(temp.path().join("reciprocal.inp"), "R 0.0 0.0 0.0\n")
            .expect("reciprocal should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Ldos,
            &input_path,
            temp.path(),
        );
        let scaffold = LdosPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.LDOS_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ldos.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_artifact("FX-LDOS-001", "ldos.inp", &input_path);
        stage_baseline_artifact("FX-LDOS-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-LDOS-001", "pot.bin", &temp.path().join("pot.bin"));
        stage_baseline_artifact(
            "FX-LDOS-001",
            "reciprocal.inp",
            &temp.path().join("reciprocal.inp"),
        );

        fs::write(temp.path().join("reciprocal.inp"), "R 999.0 999.0 999.0\n")
            .expect("reciprocal input should be overwritten");

        let request = PipelineRequest::new(
            "FX-LDOS-001",
            PipelineModule::Ldos,
            &input_path,
            &output_dir,
        );
        let scaffold = LdosPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.LDOS_INPUT_MISMATCH");
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

    fn expected_ldos_artifact_set(fixture_id: &str) -> BTreeSet<String> {
        let mut artifacts = BTreeSet::new();
        let baseline_dir = fixture_baseline_dir(fixture_id);
        let entries = fs::read_dir(&baseline_dir).expect("baseline directory should be readable");

        for entry in entries {
            let entry = entry.expect("directory entry should be readable");
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if super::is_ldos_output_file_name(name) {
                artifacts.insert(name.to_string());
            }
        }

        assert!(
            !artifacts.is_empty(),
            "fixture '{}' should provide at least one LDOS output artifact",
            fixture_id
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
