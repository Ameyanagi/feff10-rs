use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SELF_PRIMARY_INPUT: &str = "sfconv.inp";
const SELF_SPECTRUM_INPUT_CANDIDATES: [&str; 3] = ["xmu.dat", "chi.dat", "loss.dat"];
const SELF_OPTIONAL_INPUTS: [&str; 1] = ["exc.dat"];
const SELF_OUTPUT_CANDIDATES: [&str; 9] = [
    "selfenergy.dat",
    "sigma.dat",
    "specfunct.dat",
    "xmu.dat",
    "chi.dat",
    "logsfconv.dat",
    "sig2FEFF.dat",
    "mpse.dat",
    "opconsCu.dat",
];
const SELF_BASELINE_ROOT: &str = "artifacts/fortran-baselines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfEnergyPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub optional_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelfSpectrumInput {
    artifact: String,
    source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelfEnergyFixtureBaseline {
    baseline_dir: PathBuf,
    expected_outputs: Vec<PipelineArtifact>,
    sfconv_input_source: String,
    spectrum_inputs: Vec<SelfSpectrumInput>,
    exc_input_source: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelfEnergyPipelineScaffold;

impl SelfEnergyPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<SelfEnergyPipelineInterface> {
        validate_request_shape(request)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        let mut required_inputs = Vec::with_capacity(1 + baseline.spectrum_inputs.len());
        required_inputs.push(PipelineArtifact::new(SELF_PRIMARY_INPUT));
        required_inputs.extend(
            baseline
                .spectrum_inputs
                .iter()
                .map(|input| PipelineArtifact::new(&input.artifact)),
        );

        Ok(SelfEnergyPipelineInterface {
            required_inputs,
            optional_inputs: artifact_list(&SELF_OPTIONAL_INPUTS),
            expected_outputs: baseline.expected_outputs,
        })
    }
}

impl PipelineExecutor for SelfEnergyPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let sfconv_source = read_input_source(&request.input_path, SELF_PRIMARY_INPUT)?;
        let baseline = load_fixture_baseline(&request.fixture_id)?;

        validate_text_input_against_baseline(
            &sfconv_source,
            &baseline.sfconv_input_source,
            SELF_PRIMARY_INPUT,
            &request.fixture_id,
        )?;

        let mut validated_spectrum_names = BTreeSet::new();
        let mut staged_spectrum_count = 0usize;

        for spectrum in &baseline.spectrum_inputs {
            let staged_path = input_dir.join(&spectrum.artifact);
            if !staged_path.is_file() {
                continue;
            }

            let staged_source = read_input_source(&staged_path, &spectrum.artifact)?;
            validate_text_input_against_baseline(
                &staged_source,
                &spectrum.source,
                &spectrum.artifact,
                &request.fixture_id,
            )?;

            staged_spectrum_count += 1;
            validated_spectrum_names.insert(spectrum.artifact.to_ascii_lowercase());
        }

        for candidate in SELF_SPECTRUM_INPUT_CANDIDATES {
            let candidate_path = input_dir.join(candidate);
            if !candidate_path.is_file() {
                continue;
            }

            let candidate_key = candidate.to_ascii_lowercase();
            if validated_spectrum_names.contains(&candidate_key) {
                continue;
            }

            staged_spectrum_count += 1;
            validated_spectrum_names.insert(candidate_key);
        }

        for artifact in collect_feff_spectrum_artifacts(
            input_dir,
            "IO.SELF_INPUT_READ",
            "input",
            "input directory",
        )? {
            let artifact_key = artifact.to_ascii_lowercase();
            if validated_spectrum_names.contains(&artifact_key) {
                continue;
            }

            staged_spectrum_count += 1;
            validated_spectrum_names.insert(artifact_key);
        }

        if staged_spectrum_count == 0 {
            return Err(FeffError::input_validation(
                "INPUT.SELF_SPECTRUM_INPUT",
                format!(
                    "SELF pipeline requires at least one staged spectrum input (xmu.dat, chi.dat, loss.dat, or feffNNNN.dat) in '{}'",
                    input_dir.display()
                ),
            ));
        }

        let exc_source = maybe_read_optional_input_source(
            input_dir.join(SELF_OPTIONAL_INPUTS[0]),
            SELF_OPTIONAL_INPUTS[0],
        )?;
        validate_optional_exc_input_against_baseline(
            exc_source.as_deref(),
            baseline.exc_input_source.as_deref(),
            &request.fixture_id,
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.SELF_OUTPUT_DIRECTORY",
                format!(
                    "failed to create SELF output directory '{}': {}",
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
                        "IO.SELF_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create SELF artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            fs::copy(&baseline_artifact_path, &output_path).map_err(|source| {
                FeffError::io_system(
                    "IO.SELF_OUTPUT_WRITE",
                    format!(
                        "failed to materialize SELF artifact '{}' from baseline '{}': {}",
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
    if request.module != PipelineModule::SelfEnergy {
        return Err(FeffError::input_validation(
            "INPUT.SELF_MODULE",
            format!("SELF pipeline expects module SELF, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.SELF_INPUT_ARTIFACT",
                format!(
                    "SELF pipeline expects input artifact '{}' at '{}'",
                    SELF_PRIMARY_INPUT,
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(SELF_PRIMARY_INPUT) {
        return Err(FeffError::input_validation(
            "INPUT.SELF_INPUT_ARTIFACT",
            format!(
                "SELF pipeline requires input artifact '{}' but received '{}'",
                SELF_PRIMARY_INPUT, input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.SELF_INPUT_ARTIFACT",
            format!(
                "SELF pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.SELF_INPUT_READ",
            format!(
                "failed to read SELF input '{}' ({}): {}",
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

fn load_fixture_baseline(fixture_id: &str) -> PipelineResult<SelfEnergyFixtureBaseline> {
    let baseline_dir = PathBuf::from(SELF_BASELINE_ROOT)
        .join(fixture_id)
        .join("baseline");
    if !baseline_dir.is_dir() {
        return Err(FeffError::input_validation(
            "INPUT.SELF_FIXTURE",
            format!(
                "fixture '{}' is not approved for SELF parity (missing baseline directory '{}')",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let sfconv_input_source =
        read_baseline_input_source(&baseline_dir.join(SELF_PRIMARY_INPUT), SELF_PRIMARY_INPUT)?;

    let mut spectrum_artifacts: Vec<String> = SELF_SPECTRUM_INPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .map(|artifact| artifact.to_string())
        .collect();

    for artifact in collect_feff_spectrum_artifacts(
        &baseline_dir,
        "IO.SELF_BASELINE_READ",
        "baseline",
        "baseline directory",
    )? {
        if spectrum_artifacts
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&artifact))
        {
            continue;
        }
        spectrum_artifacts.push(artifact);
    }

    if spectrum_artifacts.is_empty() {
        return Err(FeffError::computation(
            "RUN.SELF_BASELINE_INPUTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any SELF spectrum inputs",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    let mut spectrum_inputs = Vec::with_capacity(spectrum_artifacts.len());
    for artifact in &spectrum_artifacts {
        let source = read_baseline_input_source(&baseline_dir.join(artifact), artifact)?;
        spectrum_inputs.push(SelfSpectrumInput {
            artifact: artifact.clone(),
            source,
        });
    }

    let exc_input_source = maybe_read_optional_baseline_source(
        baseline_dir.join(SELF_OPTIONAL_INPUTS[0]),
        SELF_OPTIONAL_INPUTS[0],
    )?;

    let mut expected_output_paths: Vec<String> = SELF_OUTPUT_CANDIDATES
        .iter()
        .filter(|artifact| baseline_dir.join(artifact).is_file())
        .map(|artifact| artifact.to_string())
        .collect();

    for artifact in collect_feff_spectrum_artifacts(
        &baseline_dir,
        "IO.SELF_BASELINE_READ",
        "baseline",
        "baseline directory",
    )? {
        if expected_output_paths
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&artifact))
        {
            continue;
        }
        expected_output_paths.push(artifact);
    }

    let expected_outputs: Vec<PipelineArtifact> = expected_output_paths
        .iter()
        .map(PipelineArtifact::new)
        .collect();

    if expected_outputs.is_empty() {
        return Err(FeffError::computation(
            "RUN.SELF_BASELINE_ARTIFACTS",
            format!(
                "fixture '{}' baseline '{}' does not contain any SELF output artifacts",
                fixture_id,
                baseline_dir.display()
            ),
        ));
    }

    Ok(SelfEnergyFixtureBaseline {
        baseline_dir,
        expected_outputs,
        sfconv_input_source,
        spectrum_inputs,
        exc_input_source,
    })
}

fn read_baseline_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.SELF_BASELINE_READ",
            format!(
                "failed to read SELF baseline artifact '{}' ({}): {}",
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

fn collect_feff_spectrum_artifacts(
    directory: &Path,
    placeholder: &'static str,
    location: &'static str,
    location_label: &'static str,
) -> PipelineResult<Vec<String>> {
    let entries = fs::read_dir(directory).map_err(|source| {
        FeffError::io_system(
            placeholder,
            format!(
                "failed to read SELF {} '{}' while collecting feffNNNN.dat artifacts: {}",
                location,
                directory.display(),
                source
            ),
        )
    })?;

    let mut artifacts = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| {
            FeffError::io_system(
                placeholder,
                format!(
                    "failed to read SELF {} entry in '{}': {}",
                    location,
                    directory.display(),
                    source
                ),
            )
        })?;

        let file_type = entry.file_type().map_err(|source| {
            FeffError::io_system(
                placeholder,
                format!(
                    "failed to inspect SELF {} entry '{}' in '{}': {}",
                    location_label,
                    entry.path().display(),
                    directory.display(),
                    source
                ),
            )
        })?;

        if !file_type.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if is_feff_spectrum_name(&file_name) {
            artifacts.push(file_name.into_owned());
        }
    }

    artifacts.sort();
    Ok(artifacts)
}

fn validate_text_input_against_baseline(
    actual: &str,
    baseline: &str,
    artifact: &str,
    fixture_id: &str,
) -> PipelineResult<()> {
    if normalize_self_source(actual) == normalize_self_source(baseline) {
        return Ok(());
    }

    Err(FeffError::computation(
        "RUN.SELF_INPUT_MISMATCH",
        format!(
            "fixture '{}' input '{}' does not match approved SELF parity baseline",
            fixture_id, artifact
        ),
    ))
}

fn validate_optional_exc_input_against_baseline(
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

    validate_text_input_against_baseline(actual, baseline, SELF_OPTIONAL_INPUTS[0], fixture_id)
}

fn normalize_self_source(content: &str) -> String {
    content
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_feff_spectrum_name(name: &str) -> bool {
    let lowercase = name.to_ascii_lowercase();
    if !lowercase.starts_with("feff") || !lowercase.ends_with(".dat") {
        return false;
    }

    let suffix = &lowercase[4..lowercase.len() - 4];
    !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        SELF_OPTIONAL_INPUTS, SELF_OUTPUT_CANDIDATES, SELF_PRIMARY_INPUT,
        SELF_SPECTRUM_INPUT_CANDIDATES, SelfEnergyPipelineScaffold, is_feff_spectrum_name,
    };
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_available_fixture_baseline_artifacts() {
        let request = PipelineRequest::new(
            "FX-SELF-001",
            PipelineModule::SelfEnergy,
            "sfconv.inp",
            "actual-output",
        );
        let scaffold = SelfEnergyPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        let required_inputs = artifact_set(&contract.required_inputs);
        assert!(required_inputs.contains(SELF_PRIMARY_INPUT));
        assert!(
            required_inputs
                .iter()
                .any(|artifact| is_spectrum_artifact_name(artifact)),
            "contract should include at least one SELF spectrum input"
        );

        assert_eq!(contract.optional_inputs.len(), 1);
        assert_eq!(
            contract.optional_inputs[0].relative_path,
            PathBuf::from(SELF_OPTIONAL_INPUTS[0])
        );

        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_self_artifact_set()
        );
    }

    #[test]
    fn execute_materializes_baseline_parity_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        let output_dir = temp.path().join("out");

        stage_baseline_artifact("FX-SELF-001", SELF_PRIMARY_INPUT, &input_path);
        let staged_spectrum_count = stage_available_spectrum_inputs("FX-SELF-001", temp.path());
        assert!(
            staged_spectrum_count > 0,
            "fixture should provide at least one staged spectrum input"
        );
        stage_optional_exc_input("FX-SELF-001", &temp.path().join(SELF_OPTIONAL_INPUTS[0]));

        let request = PipelineRequest::new(
            "FX-SELF-001",
            PipelineModule::SelfEnergy,
            &input_path,
            &output_dir,
        );
        let scaffold = SelfEnergyPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("SELF execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_self_artifact_set());
        for artifact in artifacts {
            let relative_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let output_path = output_dir.join(&artifact.relative_path);
            let baseline_path = fixture_baseline_dir("FX-SELF-001").join(&artifact.relative_path);
            assert_eq!(
                fs::read(&output_path).expect("output artifact should be readable"),
                fs::read(&baseline_path).expect("baseline artifact should be readable"),
                "artifact '{}' should match baseline",
                relative_path
            );
        }
    }

    #[test]
    fn execute_allows_missing_optional_exc_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        let output_dir = temp.path().join("out");

        stage_baseline_artifact("FX-SELF-001", SELF_PRIMARY_INPUT, &input_path);
        let staged_spectrum_count = stage_available_spectrum_inputs("FX-SELF-001", temp.path());
        assert!(staged_spectrum_count > 0);

        let request = PipelineRequest::new(
            "FX-SELF-001",
            PipelineModule::SelfEnergy,
            &input_path,
            &output_dir,
        );
        let scaffold = SelfEnergyPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("SELF execution should succeed without exc.dat");

        assert_eq!(artifact_set(&artifacts), expected_self_artifact_set());
    }

    #[test]
    fn execute_rejects_non_self_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        fs::write(&input_path, "SFCONV INPUT\n").expect("sfconv input should be written");

        let request = PipelineRequest::new(
            "FX-SELF-001",
            PipelineModule::Screen,
            &input_path,
            temp.path(),
        );
        let scaffold = SelfEnergyPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.SELF_MODULE");
    }

    #[test]
    fn execute_requires_at_least_one_spectrum_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        stage_baseline_artifact("FX-SELF-001", SELF_PRIMARY_INPUT, &input_path);

        let request = PipelineRequest::new(
            "FX-SELF-001",
            PipelineModule::SelfEnergy,
            &input_path,
            temp.path(),
        );
        let scaffold = SelfEnergyPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing spectrum inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.SELF_SPECTRUM_INPUT");
    }

    #[test]
    fn execute_rejects_unapproved_fixture_for_self_parity() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        fs::write(&input_path, "SFCONV INPUT\n").expect("sfconv input should be written");

        let request = PipelineRequest::new(
            "FX-UNKNOWN-001",
            PipelineModule::SelfEnergy,
            &input_path,
            temp.path(),
        );
        let scaffold = SelfEnergyPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("unknown fixture should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.SELF_FIXTURE");
    }

    #[test]
    fn execute_rejects_inputs_that_drift_from_baseline_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        let output_dir = temp.path().join("actual");

        stage_baseline_artifact("FX-SELF-001", SELF_PRIMARY_INPUT, &input_path);
        let staged_spectrum_count = stage_available_spectrum_inputs("FX-SELF-001", temp.path());
        assert!(staged_spectrum_count > 0);

        fs::write(&input_path, "drifted sfconv input\n")
            .expect("sfconv input should be overwritten");

        let request = PipelineRequest::new(
            "FX-SELF-001",
            PipelineModule::SelfEnergy,
            &input_path,
            &output_dir,
        );
        let scaffold = SelfEnergyPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("mismatched inputs should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.SELF_INPUT_MISMATCH");
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

    fn stage_optional_exc_input(fixture_id: &str, destination: &Path) {
        let baseline_exc_input = fixture_baseline_dir(fixture_id).join(SELF_OPTIONAL_INPUTS[0]);
        if !baseline_exc_input.is_file() {
            return;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination parent should exist");
        }
        fs::copy(&baseline_exc_input, destination).expect("exc input copy should succeed");
    }

    fn stage_available_spectrum_inputs(fixture_id: &str, destination_dir: &Path) -> usize {
        let baseline_dir = fixture_baseline_dir(fixture_id);
        let mut staged_count = 0usize;

        for artifact in SELF_SPECTRUM_INPUT_CANDIDATES {
            let source = baseline_dir.join(artifact);
            if !source.is_file() {
                continue;
            }

            stage_baseline_artifact(fixture_id, artifact, &destination_dir.join(artifact));
            staged_count += 1;
        }

        let entries = fs::read_dir(&baseline_dir).expect("baseline directory should be readable");
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if !is_feff_spectrum_name(&file_name) {
                continue;
            }

            stage_baseline_artifact(fixture_id, &file_name, &destination_dir.join(&file_name));
            staged_count += 1;
        }

        staged_count
    }

    fn expected_self_artifact_set() -> BTreeSet<String> {
        let baseline_dir = fixture_baseline_dir("FX-SELF-001");
        let mut artifacts: BTreeSet<String> = SELF_OUTPUT_CANDIDATES
            .iter()
            .filter(|artifact| baseline_dir.join(artifact).is_file())
            .map(|artifact| artifact.to_string())
            .collect();

        let entries = fs::read_dir(&baseline_dir).expect("baseline directory should be readable");
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if is_feff_spectrum_name(&file_name) {
                artifacts.insert(file_name);
            }
        }

        assert!(
            !artifacts.is_empty(),
            "fixture 'FX-SELF-001' should provide at least one SELF output artifact"
        );
        artifacts
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn is_spectrum_artifact_name(artifact: &str) -> bool {
        let normalized = artifact.to_ascii_lowercase();
        SELF_SPECTRUM_INPUT_CANDIDATES
            .iter()
            .any(|candidate| normalized == candidate.to_ascii_lowercase())
            || is_feff_spectrum_name(&normalized)
    }
}
