use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const XSPH_REQUIRED_INPUTS: [&str; 4] = ["xsph.inp", "geom.dat", "global.inp", "pot.bin"];
const XSPH_OPTIONAL_INPUTS: [&str; 1] = ["wscrn.dat"];
const XSPH_EXPECTED_OUTPUTS: [&str; 3] = ["phase.bin", "xsect.dat", "log2.dat"];
const XSPH_OPTIONAL_OUTPUTS: [&str; 1] = ["phase.dat"];

const PHASE_BIN_PLACEHOLDER: &[u8] = b"XSPH_SCAFFOLD_PHASE_BIN\n";
const XSECT_DAT_PLACEHOLDER: &str =
    "# XSPH scaffold placeholder cross section\n# energy mu\n0.000000 0.000000\n";
const LOG2_DAT_PLACEHOLDER: &str = "XSPH scaffold placeholder log\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XsphPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub optional_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
    pub optional_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct XsphPipelineScaffold;

impl XsphPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<XsphPipelineInterface> {
        validate_request_shape(request)?;
        Ok(XsphPipelineInterface {
            required_inputs: artifact_list(&XSPH_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&XSPH_OPTIONAL_INPUTS),
            expected_outputs: artifact_list(&XSPH_EXPECTED_OUTPUTS),
            optional_outputs: artifact_list(&XSPH_OPTIONAL_OUTPUTS),
        })
    }
}

impl PipelineExecutor for XsphPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let _xsph_source = read_input_source(&request.input_path, XSPH_REQUIRED_INPUTS[0])?;
        let _geom_source = read_input_source(
            &input_dir.join(XSPH_REQUIRED_INPUTS[1]),
            XSPH_REQUIRED_INPUTS[1],
        )?;
        let _global_source = read_input_source(
            &input_dir.join(XSPH_REQUIRED_INPUTS[2]),
            XSPH_REQUIRED_INPUTS[2],
        )?;
        let _pot_bytes = read_input_bytes(
            &input_dir.join(XSPH_REQUIRED_INPUTS[3]),
            XSPH_REQUIRED_INPUTS[3],
        )?;

        maybe_read_optional_input(
            input_dir.join(XSPH_OPTIONAL_INPUTS[0]),
            XSPH_OPTIONAL_INPUTS[0],
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

        let artifacts = artifact_list(&XSPH_EXPECTED_OUTPUTS);
        for artifact in &artifacts {
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

            write_placeholder_artifact(&output_path, &artifact.relative_path)?;
        }

        Ok(artifacts)
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

fn maybe_read_optional_input(path: PathBuf, artifact_name: &str) -> PipelineResult<()> {
    if path.is_file() {
        let _ = read_input_bytes(&path, artifact_name)?;
    }

    Ok(())
}

fn write_placeholder_artifact(path: &Path, artifact_relative_path: &Path) -> PipelineResult<()> {
    let normalized = artifact_relative_path.to_string_lossy().replace('\\', "/");
    match normalized.as_str() {
        "phase.bin" => fs::write(path, PHASE_BIN_PLACEHOLDER),
        "xsect.dat" => fs::write(path, XSECT_DAT_PLACEHOLDER),
        "log2.dat" => fs::write(path, LOG2_DAT_PLACEHOLDER),
        _ => {
            return Err(FeffError::internal(
                "SYS.XSPH_ARTIFACT",
                format!("unsupported XSPH artifact '{}'", normalized),
            ));
        }
    }
    .map_err(|source| {
        FeffError::io_system(
            "IO.XSPH_OUTPUT_WRITE",
            format!(
                "failed to write XSPH artifact '{}': {}",
                path.display(),
                source
            ),
        )
    })
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        LOG2_DAT_PLACEHOLDER, PHASE_BIN_PLACEHOLDER, XSECT_DAT_PLACEHOLDER, XsphPipelineScaffold,
    };
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn contract_matches_compatibility_matrix_interfaces() {
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
            expected_artifact_set(&["phase.bin", "xsect.dat", "log2.dat"])
        );
        assert_eq!(
            artifact_set(&contract.optional_outputs),
            expected_artifact_set(&["phase.dat"])
        );
    }

    #[test]
    fn execute_materializes_deterministic_placeholder_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        let output_dir = temp.path().join("out");

        stage_required_inputs(&input_path);

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

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["phase.bin", "xsect.dat", "log2.dat"])
        );
        assert_eq!(
            fs::read(output_dir.join("phase.bin")).expect("phase.bin should exist"),
            PHASE_BIN_PLACEHOLDER
        );
        assert_eq!(
            fs::read_to_string(output_dir.join("xsect.dat")).expect("xsect.dat should exist"),
            XSECT_DAT_PLACEHOLDER
        );
        assert_eq!(
            fs::read_to_string(output_dir.join("log2.dat")).expect("log2.dat should exist"),
            LOG2_DAT_PLACEHOLDER
        );
    }

    #[test]
    fn execute_rejects_non_xsph_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        stage_required_inputs(&input_path);

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
    fn execute_requires_pot_bin_input_in_same_directory() {
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

    fn stage_required_inputs(xsph_input_path: &PathBuf) {
        let input_dir = xsph_input_path
            .parent()
            .expect("xsph input should have parent");
        fs::create_dir_all(input_dir).expect("input dir should exist");
        fs::write(xsph_input_path, "XSPH INPUT\n").expect("xsph input should be written");
        fs::write(input_dir.join("geom.dat"), "GEOM INPUT\n")
            .expect("geom input should be written");
        fs::write(input_dir.join("global.inp"), "GLOBAL INPUT\n")
            .expect("global input should be written");
        fs::write(input_dir.join("pot.bin"), [1_u8, 2_u8, 3_u8])
            .expect("pot.bin should be written");
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
}
