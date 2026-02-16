use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::Path;

const PATH_REQUIRED_INPUTS: [&str; 4] = ["paths.inp", "geom.dat", "global.inp", "phase.bin"];
const PATH_EXPECTED_OUTPUTS: [&str; 4] = ["paths.dat", "paths.bin", "crit.dat", "log4.dat"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PathPipelineScaffold;

impl PathPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<PathPipelineInterface> {
        validate_request_shape(request)?;
        Ok(PathPipelineInterface {
            required_inputs: artifact_list(&PATH_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&PATH_EXPECTED_OUTPUTS),
        })
    }
}

impl PipelineExecutor for PathPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let paths_source = read_input_source(&request.input_path, PATH_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(PATH_REQUIRED_INPUTS[1]),
            PATH_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(PATH_REQUIRED_INPUTS[2]),
            PATH_REQUIRED_INPUTS[2],
        )?;
        let phase_size = read_input_bytes(&input_dir.join(PATH_REQUIRED_INPUTS[3]))?.len();

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.PATH_OUTPUT_DIRECTORY",
                format!(
                    "failed to create PATH output directory '{}': {}",
                    request.output_dir.display(),
                    source
                ),
            )
        })?;

        let outputs = artifact_list(&PATH_EXPECTED_OUTPUTS);
        for artifact in &outputs {
            let output_path = request.output_dir.join(&artifact.relative_path);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|source| {
                    FeffError::io_system(
                        "IO.PATH_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create PATH artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            let artifact_name = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let content = render_placeholder_output(
                &request.fixture_id,
                &artifact_name,
                non_empty_line_count(&paths_source),
                non_empty_line_count(&geom_source),
                non_empty_line_count(&global_source),
                phase_size,
            );

            fs::write(&output_path, content).map_err(|source| {
                FeffError::io_system(
                    "IO.PATH_OUTPUT_WRITE",
                    format!(
                        "failed to write PATH artifact '{}': {}",
                        output_path.display(),
                        source
                    ),
                )
            })?;
        }

        Ok(outputs)
    }
}

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Path {
        return Err(FeffError::input_validation(
            "INPUT.PATH_MODULE",
            format!("PATH pipeline expects module PATH, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.PATH_INPUT_ARTIFACT",
                format!(
                    "PATH pipeline expects input artifact '{}' at '{}'",
                    PATH_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(PATH_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.PATH_INPUT_ARTIFACT",
            format!(
                "PATH pipeline requires input artifact '{}' but received '{}'",
                PATH_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.PATH_INPUT_ARTIFACT",
            format!(
                "PATH pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(input_path: &Path, label: &str) -> PipelineResult<String> {
    fs::read_to_string(input_path).map_err(|source| {
        FeffError::io_system(
            "IO.PATH_INPUT_READ",
            format!(
                "failed to read PATH input '{}' ({}): {}",
                input_path.display(),
                label,
                source
            ),
        )
    })
}

fn read_input_bytes(input_path: &Path) -> PipelineResult<Vec<u8>> {
    fs::read(input_path).map_err(|source| {
        FeffError::io_system(
            "IO.PATH_INPUT_READ",
            format!(
                "failed to read PATH input '{}' (phase.bin): {}",
                input_path.display(),
                source
            ),
        )
    })
}

fn render_placeholder_output(
    fixture_id: &str,
    artifact_name: &str,
    paths_line_count: usize,
    geom_line_count: usize,
    global_line_count: usize,
    phase_byte_count: usize,
) -> String {
    format!(
        "PATH_SCAFFOLD\nmodule=PATH\nfixture={fixture_id}\nartifact={artifact_name}\ninput.paths_inp.lines={paths_line_count}\ninput.geom_dat.lines={geom_line_count}\ninput.global_inp.lines={global_line_count}\ninput.phase_bin.bytes={phase_byte_count}\n"
    )
}

fn non_empty_line_count(content: &str) -> usize {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::PathPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn contract_matches_path_compatibility_interfaces() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("paths.inp");
        let output_dir = temp.path().join("out");
        fs::write(&input_path, "PATHS INPUT\n").expect("paths input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");
        fs::write(temp.path().join("phase.bin"), [0_u8, 1_u8, 2_u8, 3_u8])
            .expect("phase input should be written");

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            &input_path,
            &output_dir,
        );
        let scaffold = PathPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 4);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("paths.inp")
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

        assert_eq!(contract.expected_outputs.len(), 4);
        assert!(
            contract
                .expected_outputs
                .iter()
                .any(|artifact| artifact.relative_path == PathBuf::from("paths.dat"))
        );
        assert!(
            contract
                .expected_outputs
                .iter()
                .any(|artifact| artifact.relative_path == PathBuf::from("paths.bin"))
        );
        assert!(
            contract
                .expected_outputs
                .iter()
                .any(|artifact| artifact.relative_path == PathBuf::from("crit.dat"))
        );
        assert!(
            contract
                .expected_outputs
                .iter()
                .any(|artifact| artifact.relative_path == PathBuf::from("log4.dat"))
        );
    }

    #[test]
    fn execute_materializes_path_scaffold_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("paths.inp");
        let output_dir = temp.path().join("actual");
        fs::write(&input_path, "path row\nnext row\n").expect("paths input should be written");
        fs::write(temp.path().join("geom.dat"), "nat, nph =   2    1\n")
            .expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "mphase, iprint\n1 0\n")
            .expect("global input should be written");
        fs::write(
            temp.path().join("phase.bin"),
            [0_u8, 1_u8, 2_u8, 3_u8, 4_u8],
        )
        .expect("phase input should be written");

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            &input_path,
            &output_dir,
        );
        let scaffold = PathPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("PATH execution should succeed");

        assert_eq!(artifacts.len(), 4);
        let log = output_dir.join("log4.dat");
        assert!(log.exists());
        let content = fs::read_to_string(log).expect("log should be readable");
        assert!(content.contains("module=PATH"));
        assert!(content.contains("fixture=FX-PATH-001"));
        assert!(content.contains("input.phase_bin.bytes=5"));
    }

    #[test]
    fn execute_rejects_non_path_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("paths.inp");
        fs::write(&input_path, "PATH INPUT\n").expect("paths input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");
        fs::write(temp.path().join("phase.bin"), [1_u8, 2_u8]).expect("phase should be written");

        let request =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, temp.path());
        let scaffold = PathPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.PATH_MODULE");
    }

    #[test]
    fn execute_requires_phase_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("paths.inp");
        fs::write(&input_path, "PATH INPUT\n").expect("paths input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            &input_path,
            temp.path(),
        );
        let scaffold = PathPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing phase input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.PATH_INPUT_READ");
    }
}
