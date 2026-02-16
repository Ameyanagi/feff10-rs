use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;

const POT_REQUIRED_INPUTS: [&str; 2] = ["pot.inp", "geom.dat"];
const POT_EXPECTED_OUTPUTS: [&str; 5] = [
    "pot.bin",
    "pot.dat",
    "log1.dat",
    "convergence.scf",
    "convergence.scf.fine",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PotPipelineScaffold;

impl PotPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<PotPipelineInterface> {
        validate_request_shape(request)?;
        Ok(PotPipelineInterface {
            required_inputs: artifact_list(&POT_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&POT_EXPECTED_OUTPUTS),
        })
    }
}

impl PipelineExecutor for PotPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;

        let pot_inp_source = read_input_source(&request.input_path, "pot.inp")?;
        let geom_path = geom_input_path(request)?;
        let geom_source = read_input_source(&geom_path, "geom.dat")?;

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

        let pot_line_count = non_empty_line_count(&pot_inp_source);
        let geom_line_count = non_empty_line_count(&geom_source);
        let outputs = artifact_list(&POT_EXPECTED_OUTPUTS);

        for artifact in &outputs {
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

            let artifact_name = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let content = render_placeholder_output(
                &request.fixture_id,
                &artifact_name,
                pot_line_count,
                geom_line_count,
            );

            fs::write(&output_path, content).map_err(|source| {
                FeffError::io_system(
                    "IO.POT_OUTPUT_WRITE",
                    format!(
                        "failed to write POT artifact '{}': {}",
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

fn geom_input_path(request: &PipelineRequest) -> PipelineResult<std::path::PathBuf> {
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

fn read_input_source(input_path: &std::path::Path, label: &str) -> PipelineResult<String> {
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

fn render_placeholder_output(
    fixture_id: &str,
    artifact_name: &str,
    pot_line_count: usize,
    geom_line_count: usize,
) -> String {
    format!(
        "POT_SCAFFOLD\nmodule=POT\nfixture={fixture_id}\nartifact={artifact_name}\ninput.pot_inp.lines={pot_line_count}\ninput.geom_dat.lines={geom_line_count}\n"
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
    use super::PotPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn contract_matches_pot_compatibility_interfaces() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("out");
        fs::write(&input_path, "POT INPUT\n").expect("input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");

        let request =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, &output_dir);
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

        assert_eq!(contract.expected_outputs.len(), 5);
        assert!(
            contract
                .expected_outputs
                .iter()
                .any(|artifact| artifact.relative_path == PathBuf::from("pot.bin"))
        );
        assert!(
            contract
                .expected_outputs
                .iter()
                .any(|artifact| artifact.relative_path == PathBuf::from("convergence.scf.fine"))
        );
    }

    #[test]
    fn execute_materializes_pot_scaffold_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("actual");
        fs::write(&input_path, "title line\nscf row\n").expect("pot input should be written");
        fs::write(temp.path().join("geom.dat"), "nat, nph =   2    1\n")
            .expect("geom should be written");

        let request =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, &output_dir);
        let scaffold = PotPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("POT execution should succeed");

        assert_eq!(artifacts.len(), 5);
        let log = output_dir.join("log1.dat");
        assert!(log.exists());
        let content = fs::read_to_string(log).expect("log should be readable");
        assert!(content.contains("module=POT"));
        assert!(content.contains("fixture=FX-POT-001"));
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
}
