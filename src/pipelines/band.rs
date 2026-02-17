use super::PipelineExecutor;
use super::xsph::XSPH_PHASE_BINARY_MAGIC;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const BAND_REQUIRED_INPUTS: [&str; 4] = ["band.inp", "geom.dat", "global.inp", "phase.bin"];
const BAND_OUTPUT_CANDIDATES: [&str; 4] =
    ["bandstructure.dat", "logband.dat", "list.dat", "log5.dat"];
const BAND_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BandPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BandFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    band_input_source: Option<String>,
    geom_input_source: String,
    global_input_source: String,
    phase_input_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BandPipelineScaffold;

impl BandPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<BandPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;
        Ok(BandPipelineInterface {
            required_inputs: artifact_list(&BAND_REQUIRED_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for BandPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let band_source = read_input_source(&request.input_path, BAND_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(BAND_REQUIRED_INPUTS[1]),
            BAND_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(BAND_REQUIRED_INPUTS[2]),
            BAND_REQUIRED_INPUTS[2],
        )?;
        let phase_bytes = read_input_bytes(
            &input_dir.join(BAND_REQUIRED_INPUTS[3]),
            BAND_REQUIRED_INPUTS[3],
        )?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_band_input_against_baseline(
            &band_source,
            baseline.band_input_source.as_deref(),
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &geom_source,
            &baseline.geom_input_source,
            BAND_REQUIRED_INPUTS[1],
            &request.fixture_id,
        )?;
        validate_text_input_against_baseline(
            &global_source,
            &baseline.global_input_source,
            BAND_REQUIRED_INPUTS[2],
            &request.fixture_id,
        )?;
        validate_phase_input_against_baseline(
            &phase_bytes,
            &baseline.phase_input_bytes,
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.BAND_OUTPUT_DIRECTORY",
                format!(
                    "failed to create BAND output directory '{}': {}",
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
                        "IO.BAND_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create BAND artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.BAND_OUTPUT_WRITE",
                    format!(
                        "failed to materialize BAND artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::Band {
        return Err(FeffError::input_validation(
            "INPUT.BAND_MODULE",
            format!("BAND pipeline expects module BAND, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.BAND_INPUT_ARTIFACT",
                format!(
                    "BAND pipeline expects input artifact '{}' at '{}'",
                    BAND_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(BAND_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.BAND_INPUT_ARTIFACT",
            format!(
                "BAND pipeline requires input artifact '{}' but received '{}'",
                BAND_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.BAND_INPUT_ARTIFACT",
            format!(
                "BAND pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.BAND_INPUT_READ",
            format!(
                "failed to read BAND input '{}' ({}): {}",
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
            "IO.BAND_INPUT_READ",
            format!(
                "failed to read BAND input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<BandFixtureBaseline> {
    let baseline_dir = PathBuf::from(BAND_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.BAND_FIXTURE",
            format!(
                "fixture '{}' is not approved for BAND parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let band_input_source = maybe_read_optional_baseline_source(
        baseline_dir.join(BAND_REQUIRED_INPUTS[0]),
        BAND_REQUIRED_INPUTS[0],
    )?;
    let geom_input_source =
        read_baseline_input_source(&baseline_dir.join(BAND_REQUIRED_INPUTS[1]), "geom.dat")?;
    let global_input_source =
        read_baseline_input_source(&baseline_dir.join(BAND_REQUIRED_INPUTS[2]), "global.inp")?;
    let phase_input_bytes =
        read_baseline_input_bytes(&baseline_dir.join(BAND_REQUIRED_INPUTS[3]), "phase.bin")?;

    let expected_outputs: Vec<PipelineArtifact> = BAND_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .copied()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.BAND_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any BAND output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(BandFixtureBaseline {
        baseline_dir,
        expected_outputs,
        band_input_source,
        geom_input_source,
        global_input_source,
        phase_input_bytes,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.BAND_BASELINE_READ",
            format!(
                "failed to read BAND baseline artifact '{}' ({}): {}",
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
            "IO.BAND_BASELINE_READ",
            format!(
                "failed to read BAND baseline artifact '{}' ({}): {}",
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

fn validate_band_input_against_baseline(
    actual: &str,
    baseline: Option<&str>,
    fixture_id: &str,
) -> PipelineResult<()> {
    if let Some(baseline_source) = baseline {
        if normalize_band_source(actual) == normalize_band_source(baseline_source) {
            return Ok(());
        }

        return Err(FeffError::computation(
            "RUN.BAND_INPUT_MISMATCH",
            format!(
                "fixture '{}' input '{}' does not match approved BAND parity baseline",
                fixture_id, BAND_REQUIRED_INPUTS[0]
            ),
        ));
    }

    Ok(())
}

fn validate_text_input_against_baseline(
    actual: &str,
    baseline: &str,
    artifact: &str,
    fixture_id: &str,
) -> PipelineResult<()> {
    if normalize_band_source(actual) == normalize_band_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.BAND_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved BAND parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn validate_phase_input_against_baseline(
    actual: &[u8],
    baseline: &[u8],
    fixture_id: &str,
) -> PipelineResult<()> {
    if actual == baseline || actual.starts_with(XSPH_PHASE_BINARY_MAGIC) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.BAND_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved BAND parity baseline",
            fixture_id, BAND_REQUIRED_INPUTS[3]
        ),
    ))
}

fn normalize_band_source(content: &str) -> String {
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
    use super::BandPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-BAND-001",
            PipelineModule::Band,
            "band.inp",
            "actual-output",
        );
        let scaffold = BandPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 4);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("band.inp")
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
            expected_band_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("band.inp");
        let output_dir = temp.path().join("out");
        stage_band_input("FX-BAND-001", &input_path);
        stage_baseline_artifact("FX-BAND-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-BAND-001", "global.inp", &temp.path().join("global.inp"));
        stage_baseline_artifact("FX-BAND-001", "phase.bin", &temp.path().join("phase.bin"));

        let request = PipelineRequest::new(
            "FX-BAND-001",
            PipelineModule::Band,
            &input_path,
            &output_dir,
        );
        let scaffold = BandPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("BAND execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_band_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-BAND-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_accepts_true_compute_xsph_phase_binary_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("band.inp");
        let output_dir = temp.path().join("out");
        stage_band_input("FX-BAND-001", &input_path);
        stage_baseline_artifact("FX-BAND-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-BAND-001", "global.inp", &temp.path().join("global.inp"));
        fs::write(
            temp.path().join("phase.bin"),
            [b'X', b'S', b'P', b'H', b'B', b'I', b'N', b'1', 1, 2, 3, 4],
        )
        .expect("phase input should be written");

        let request = PipelineRequest::new(
            "FX-BAND-001",
            PipelineModule::Band,
            &input_path,
            &output_dir,
        );
        let artifacts = BandPipelineScaffold
            .execute(&request)
            .expect("BAND execution should accept true-compute phase.bin");
        assert_eq!(artifact_set(&artifacts), expected_band_artifact_set());
    }

    #[test]
    fn execute_rejects_non_band_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("band.inp");
        fs::write(&input_path, default_band_input_source()).expect("band input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global should be written");
        fs::write(temp.path().join("phase.bin"), [1u8, 2u8]).expect("phase should be written");

        let request = PipelineRequest::new(
            "FX-BAND-001",
            PipelineModule::Rdinp,
            &input_path,
            temp.path(),
        );
        let scaffold = BandPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.BAND_MODULE");
    }

    #[test]
    fn execute_requires_phase_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("band.inp");
        fs::write(&input_path, default_band_input_source()).expect("band input should be written");
        stage_baseline_artifact("FX-BAND-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-BAND-001", "global.inp", &temp.path().join("global.inp"));

        let request = PipelineRequest::new(
            "FX-BAND-001",
            PipelineModule::Band,
            &input_path,
            temp.path(),
        );
        let scaffold = BandPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing phase input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.BAND_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_band_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("band.inp");
        fs::write(&input_path, default_band_input_source()).expect("band input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global should be written");
        fs::write(temp.path().join("phase.bin"), [1u8, 2u8]).expect("phase should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::Band,
            &input_path,
            temp.path(),
        );
        let scaffold = BandPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.BAND_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("band.inp");
        let output_dir = temp.path().join("actual");
        stage_band_input("FX-BAND-001", &input_path);
        stage_baseline_artifact("FX-BAND-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-BAND-001", "global.inp", &temp.path().join("global.inp"));
        stage_baseline_artifact("FX-BAND-001", "phase.bin", &temp.path().join("phase.bin"));

        fs::write(temp.path().join("geom.dat"), "drifted geometry\n")
            .expect("geom input should be overwritten");

        let request = PipelineRequest::new(
            "FX-BAND-001",
            PipelineModule::Band,
            &input_path,
            &output_dir,
        );
        let scaffold = BandPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.BAND_INPUT_MISMATCH");
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

    fn stage_band_input(fixture_id: &str, destination: &Path) {
        let baseline_band_input = fixture_baseline_dir(fixture_id).join("band.inp");
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        if baseline_band_input.is_file() {
            fs::copy(&baseline_band_input, destination).expect("band input copy should succeed");
            return;
        }

        fs::write(destination, default_band_input_source()).expect("band input should be written");
    }

    fn default_band_input_source() -> &'static str {
        "mband : calculate bands if = 1\n   0\nemin, emax, estep : energy mesh\n      0.00000      0.00000      0.00000\nnkp : # points in k-path\n   0\nikpath : type of k-path\n  -1\nfreeprop :  empty lattice if = T\n F\n"
    }

    fn expected_band_artifact_set() -> BTreeSet<String> {
        let baseline_dir = fixture_baseline_dir("FX-BAND-001");
        ["bandstructure.dat", "logband.dat", "list.dat", "log5.dat"]
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
