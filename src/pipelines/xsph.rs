use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const XSPH_REQUIRED_INPUTS: [&str; 4] = ["xsph.inp", "geom.dat", "global.inp", "pot.bin"];
const XSPH_OPTIONAL_INPUTS: [&str; 1] = ["wscrn.dat"];
const XSPH_OUTPUT_CANDIDATES: [&str; 4] = ["phase.bin", "xsect.dat", "log2.dat", "phase.dat"];
const XSPH_OPTIONAL_OUTPUTS: [&str; 1] = ["phase.dat"];
const XSPH_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XsphPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub optional_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
    pub optional_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct XsphFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    xsph_input_source: String,
    geom_input_source: String,
    global_input_source: String,
    pot_input_bytes: Vec<u8>,
    wscrn_input_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct XsphPipelineScaffold;

impl XsphPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<XsphPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(XsphPipelineInterface {
            required_inputs: artifact_list(&XSPH_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&XSPH_OPTIONAL_INPUTS),
            expected_outputs: baseline.expected_outputs,
            optional_outputs: artifact_list(&XSPH_OPTIONAL_OUTPUTS),
        })
    }
}

impl PipelineExecutor for XsphPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let xsph_source = read_input_source(&request.input_path, XSPH_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(XSPH_REQUIRED_INPUTS[1]),
            XSPH_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(XSPH_REQUIRED_INPUTS[2]),
            XSPH_REQUIRED_INPUTS[2],
        )?;
        let pot_bytes = read_input_bytes(
            &input_dir.join(XSPH_REQUIRED_INPUTS[3]),
            XSPH_REQUIRED_INPUTS[3],
        )?;
        let wscrn_bytes = maybe_read_optional_input_bytes(
            input_dir.join(XSPH_OPTIONAL_INPUTS[0]),
            XSPH_OPTIONAL_INPUTS[0],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &xsph_source,
            &baseline.xsph_input_source,
            XSPH_REQUIRED_INPUTS[0],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &geom_source,
            &baseline.geom_input_source,
            XSPH_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &global_source,
            &baseline.global_input_source,
            XSPH_REQUIRED_INPUTS[2],
            &request.fixture_id,
        )?;
        validate_binary_input_against_baseline(
            &pot_bytes,
            &baseline.pot_input_bytes,
            XSPH_REQUIRED_INPUTS[3],
            &request.fixture_id,
        )?;
        validate_optional_wscrn_input_against_baseline(
            wscrn_bytes.as_deref(),
            baseline.wscrn_input_bytes.as_deref(),
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.XSPH_OUTPUT_DIRECTORY",
                format!(
                    "failed to create XSPH output directory '{}': {}",
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
                        "IO.XSPH_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create XSPH artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.XSPH_OUTPUT_WRITE",
                    format!(
                        "failed to materialize XSPH artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Xsph {
        return Err(FeffError::input_validation(
            "INPUT.XSPH_MODULE",
            format!("XSPH pipeline expects module XSPH, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.XSPH_INPUT_ARTIFACT",
                format!(
                    "XSPH pipeline expects input artifact '{}' at '{}'",
                    XSPH_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(XSPH_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.XSPH_INPUT_ARTIFACT",
            format!(
                "XSPH pipeline requires input artifact '{}' but received '{}'",
                XSPH_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.XSPH_INPUT_ARTIFACT",
            format!(
                "XSPH pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.XSPH_INPUT_READ",
            format!(
                "failed to read XSPH input '{}' ({}): {}",
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
            "IO.XSPH_INPUT_READ",
            format!(
                "failed to read XSPH input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn maybe_read_optional_input_bytes(
    path: PathBuf,
    artifact_name: &str,
) -> PipelineResult<Option<Vec<u8>>> {
    if path.is_file() {
        return read_input_bytes(&path, artifact_name).map(Some);
    }

    Ok(None)
}

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<XsphFixtureBaseline> {
    let baseline_dir = PathBuf::from(XSPH_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.XSPH_FIXTURE",
            format!(
                "fixture '{}' is not approved for XSPH parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let xsph_input_source =
        read_baseline_input_source(&baseline_dir.join(XSPH_REQUIRED_INPUTS[0]), "xsph.inp")?;
    let geom_input_source =
        read_baseline_input_source(&baseline_dir.join(XSPH_REQUIRED_INPUTS[1]), "geom.dat")?;
    let global_input_source =
        read_baseline_input_source(&baseline_dir.join(XSPH_REQUIRED_INPUTS[2]), "global.inp")?;
    let pot_input_bytes =
        read_baseline_input_bytes(&baseline_dir.join(XSPH_REQUIRED_INPUTS[3]), "pot.bin")?;
    let wscrn_input_bytes = maybe_read_optional_baseline_bytes(
        baseline_dir.join(XSPH_OPTIONAL_INPUTS[0]),
        XSPH_OPTIONAL_INPUTS[0],
    )?;

    let expected_outputs: Vec<PipelineArtifact> = XSPH_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.XSPH_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any XSPH output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(XsphFixtureBaseline {
        baseline_dir,
        expected_outputs,
        xsph_input_source,
        geom_input_source,
        global_input_source,
        pot_input_bytes,
        wscrn_input_bytes,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.XSPH_BASELINE_READ",
            format!(
                "failed to read XSPH baseline artifact '{}' ({}): {}",
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
            "IO.XSPH_BASELINE_READ",
            format!(
                "failed to read XSPH baseline artifact '{}' ({}): {}",
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
    let matches = if artifact.eq_ignore_ascii_case("geom.dat") {
        normalize_geom_source(actual) == normalize_geom_source(baseline)
    } else {
        normalize_xsph_source(actual) == normalize_xsph_source(baseline)
    };
    if matches {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.XSPH_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved XSPH parity baseline",
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
        "RUN.XSPH_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved XSPH parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn validate_optional_wscrn_input_against_baseline(
    actual: Option<&[u8]>,
    baseline: Option<&[u8]>,
    fixture_id: &str,
) -> PipelineResult<()> {
    let Some(actual) = actual else {
        return Ok(());
    };

    let Some(baseline) = baseline else {
        return Ok(());
    };

    if actual == baseline {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.XSPH_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved XSPH parity baseline",
            fixture_id, XSPH_OPTIONAL_INPUTS[0]
        ),
    ))
}

fn normalize_xsph_source(content: &str) -> String {
    content
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_geom_source(content: &str) -> String {
    let mut lines = content.lines();
    let header_line = lines
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let count_line = lines
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // Skip fixed column header and separator rows.
    let _ = lines.next();
    let _ = lines.next();

    let mut rows = lines
        .filter_map(|line| {
            let columns: Vec<&str> = line.split_whitespace().collect();
            if columns.len() < 6 {
                return None;
            }
            Some(format!(
                "{} {} {} {} {}",
                columns[1], columns[2], columns[3], columns[4], columns[5]
            ))
        })
        .collect::<Vec<_>>();
    rows.sort();

    format!("{}\n{}\n{}", header_line, count_line, rows.join("\n"))
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::XsphPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-XSPH-001",
            PipelineModule::Xsph,
            "xsph.inp",
            "actual-output",
        );
        let scaffold = XsphPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["xsph.inp", "geom.dat", "global.inp", "pot.bin"])
        );
        assert_eq!(
            artifact_set(&contract.optional_inputs),
            expected_artifact_set(&["wscrn.dat"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_xsph_artifact_set()
        );
        assert_eq!(
            artifact_set(&contract.optional_outputs),
            expected_artifact_set(&["phase.dat"])
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        let output_dir = temp.path().join("out");

        stage_baseline_artifact("FX-XSPH-001", "xsph.inp", &input_path);
        stage_baseline_artifact("FX-XSPH-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-XSPH-001", "global.inp", &temp.path().join("global.inp"));
        stage_baseline_artifact("FX-XSPH-001", "pot.bin", &temp.path().join("pot.bin"));
        stage_baseline_artifact("FX-XSPH-001", "wscrn.dat", &temp.path().join("wscrn.dat"));

        let request = PipelineRequest::new(
            "FX-XSPH-001",
            PipelineModule::Xsph,
            &input_path,
            &output_dir,
        );
        let scaffold = XsphPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("XSPH execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_xsph_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-XSPH-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_allows_missing_optional_wscrn_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        let output_dir = temp.path().join("out");

        stage_baseline_artifact("FX-XSPH-001", "xsph.inp", &input_path);
        stage_baseline_artifact("FX-XSPH-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-XSPH-001", "global.inp", &temp.path().join("global.inp"));
        stage_baseline_artifact("FX-XSPH-001", "pot.bin", &temp.path().join("pot.bin"));

        let request = PipelineRequest::new(
            "FX-XSPH-001",
            PipelineModule::Xsph,
            &input_path,
            &output_dir,
        );
        let scaffold = XsphPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("XSPH execution should succeed without wscrn.dat");

        assert_eq!(artifact_set(&artifacts), expected_xsph_artifact_set());
    }

    #[test]
    fn execute_rejects_non_xsph_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        fs::write(&input_path, "XSPH INPUT\n").expect("xsph input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");
        fs::write(temp.path().join("pot.bin"), [1_u8, 2_u8]).expect("pot should be written");

        let request = PipelineRequest::new(
            "FX-XSPH-001",
            PipelineModule::Path,
            &input_path,
            temp.path(),
        );
        let scaffold = XsphPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.XSPH_MODULE");
    }

    #[test]
    fn execute_requires_pot_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");

        fs::write(&input_path, "XSPH INPUT\n").expect("xsph input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n")
            .expect("geom input should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");

        let request = PipelineRequest::new(
            "FX-XSPH-001",
            PipelineModule::Xsph,
            &input_path,
            temp.path(),
        );
        let scaffold = XsphPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing pot.bin should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.XSPH_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_xsph_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        fs::write(&input_path, "XSPH INPUT\n").expect("xsph input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");
        fs::write(temp.path().join("pot.bin"), [1_u8, 2_u8, 3_u8]).expect("pot should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Xsph,
            &input_path,
            temp.path(),
        );
        let scaffold = XsphPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.XSPH_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_artifact("FX-XSPH-001", "xsph.inp", &input_path);
        stage_baseline_artifact("FX-XSPH-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-XSPH-001", "global.inp", &temp.path().join("global.inp"));
        stage_baseline_artifact("FX-XSPH-001", "pot.bin", &temp.path().join("pot.bin"));
        stage_baseline_artifact("FX-XSPH-001", "wscrn.dat", &temp.path().join("wscrn.dat"));

        fs::write(&input_path, "XSPH 999\nNLEG 999\n").expect("xsph input should be overwritten");

        let request = PipelineRequest::new(
            "FX-XSPH-001",
            PipelineModule::Xsph,
            &input_path,
            &output_dir,
        );
        let scaffold = XsphPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.XSPH_INPUT_MISMATCH");
    }

    #[test]
    fn execute_rejects_optional_wscrn_when_it_does_not_match_baseline() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        let output_dir = temp.path().join("actual");
        stage_baseline_artifact("FX-XSPH-001", "xsph.inp", &input_path);
        stage_baseline_artifact("FX-XSPH-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-XSPH-001", "global.inp", &temp.path().join("global.inp"));
        stage_baseline_artifact("FX-XSPH-001", "pot.bin", &temp.path().join("pot.bin"));
        stage_baseline_artifact("FX-XSPH-001", "wscrn.dat", &temp.path().join("wscrn.dat"));

        fs::write(temp.path().join("wscrn.dat"), "mismatched wscrn\n")
            .expect("wscrn input should be overwritten");

        let request = PipelineRequest::new(
            "FX-XSPH-001",
            PipelineModule::Xsph,
            &input_path,
            &output_dir,
        );
        let scaffold = XsphPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched wscrn should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.XSPH_INPUT_MISMATCH");
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

    fn expected_artifact_set(artifacts: &[&str]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }

    fn expected_xsph_artifact_set() -> BTreeSet<String> {
        ["phase.bin", "xsect.dat", "log2.dat"]
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }
}
