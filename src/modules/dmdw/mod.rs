mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::DmdwModel;
use parser::{artifact_list, input_parent_dir, read_input_bytes, read_input_source, validate_request_shape};

pub(crate) const DMDW_REQUIRED_INPUTS: [&str; 2] = ["dmdw.inp", "feff.dym"];
pub(crate) const DMDW_REQUIRED_OUTPUTS: [&str; 1] = ["dmdw.out"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmdwContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DmdwModule;

impl DmdwModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<DmdwContract> {
        validate_request_shape(request)?;
        Ok(DmdwContract {
            required_inputs: artifact_list(&DMDW_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&DMDW_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for DmdwModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let dmdw_source = read_input_source(&request.input_path, DMDW_REQUIRED_INPUTS[0])?;
        let feff_dym_bytes = read_input_bytes(
            &input_dir.join(DMDW_REQUIRED_INPUTS[1]),
            DMDW_REQUIRED_INPUTS[1],
        )?;

        let model = DmdwModel::from_inputs(&request.fixture_id, &dmdw_source, &feff_dym_bytes)?;
        let outputs = artifact_list(&DMDW_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.DMDW_OUTPUT_DIRECTORY",
                format!(
                    "failed to create DMDW output directory '{}': {}",
                    request.output_dir.display(),
                    source
                ),
            )
        })?;

        for artifact in &outputs {
            let output_path = request.output_dir.join(&artifact.relative_path);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|source| {
                    FeffError::io_system(
                        "IO.DMDW_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create DMDW artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            let artifact_name = artifact.relative_path.to_string_lossy().replace('\\', "/");
            model.write_artifact(&artifact_name, &output_path)?;
        }

        Ok(outputs)
    }
}

#[cfg(test)]
mod tests {
    use super::DmdwModule;
    use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, FeffErrorCategory};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    const DMDW_INPUT_FIXTURE: &str =
        "   1\n   6\n   1    450.000\n   0\nfeff.dym\n   1\n   2   1   0          29.78\n";

    #[test]
    fn contract_declares_required_inputs_and_outputs() {
        let request = ComputeRequest::new(
            "FX-DMDW-001",
            ComputeModule::Dmdw,
            "dmdw.inp",
            "actual-output",
        );
        let contract = DmdwModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            artifact_set_from_names(&["dmdw.inp", "feff.dym"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            artifact_set_from_names(&["dmdw.out"])
        );
    }

    #[test]
    fn execute_emits_true_compute_dmdw_output() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        let output_dir = temp.path().join("out");
        stage_input_files(
            &input_path,
            &temp.path().join("feff.dym"),
            &[0_u8, 1_u8, 2_u8, 3_u8],
        );

        let request = ComputeRequest::new(
            "FX-DMDW-001",
            ComputeModule::Dmdw,
            &input_path,
            &output_dir,
        );
        let artifacts = DmdwModule
            .execute(&request)
            .expect("DMDW execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            artifact_set_from_names(&["dmdw.out"])
        );
        let out =
            fs::read_to_string(output_dir.join("dmdw.out")).expect("dmdw output should exist");
        assert!(
            out.contains("DMDW true-compute"),
            "output should include compute-mode banner"
        );
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        stage_input_files(
            &input_path,
            &temp.path().join("feff.dym"),
            &[9_u8, 10_u8, 11_u8, 12_u8, 13_u8],
        );

        let first = temp.path().join("first");
        let second = temp.path().join("second");

        let first_request =
            ComputeRequest::new("FX-DMDW-001", ComputeModule::Dmdw, &input_path, &first);
        DmdwModule
            .execute(&first_request)
            .expect("first run should succeed");

        let second_request =
            ComputeRequest::new("FX-DMDW-001", ComputeModule::Dmdw, &input_path, &second);
        DmdwModule
            .execute(&second_request)
            .expect("second run should succeed");

        let first_bytes = fs::read(first.join("dmdw.out")).expect("first output should exist");
        let second_bytes = fs::read(second.join("dmdw.out")).expect("second output should exist");
        assert_eq!(
            first_bytes, second_bytes,
            "DMDW output should be deterministic"
        );
    }

    #[test]
    fn execute_output_changes_when_feff_dym_changes() {
        let temp = TempDir::new().expect("tempdir should be created");

        let first_dir = temp.path().join("first-input");
        let second_dir = temp.path().join("second-input");
        fs::create_dir_all(&first_dir).expect("first input dir should exist");
        fs::create_dir_all(&second_dir).expect("second input dir should exist");

        let first_input = first_dir.join("dmdw.inp");
        let second_input = second_dir.join("dmdw.inp");
        stage_input_files(
            &first_input,
            &first_dir.join("feff.dym"),
            &[1_u8, 2_u8, 3_u8, 4_u8],
        );
        stage_input_files(
            &second_input,
            &second_dir.join("feff.dym"),
            &[10_u8, 11_u8, 12_u8, 13_u8],
        );

        let first_output = temp.path().join("first-output");
        let second_output = temp.path().join("second-output");

        DmdwModule
            .execute(&ComputeRequest::new(
                "FX-DMDW-001",
                ComputeModule::Dmdw,
                &first_input,
                &first_output,
            ))
            .expect("first run should succeed");
        DmdwModule
            .execute(&ComputeRequest::new(
                "FX-DMDW-001",
                ComputeModule::Dmdw,
                &second_input,
                &second_output,
            ))
            .expect("second run should succeed");

        let first_out = fs::read(first_output.join("dmdw.out")).expect("first output should exist");
        let second_out =
            fs::read(second_output.join("dmdw.out")).expect("second output should exist");
        assert_ne!(
            first_out, second_out,
            "DMDW output should depend on feff.dym input bytes"
        );
    }

    #[test]
    fn execute_rejects_non_dmdw_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        stage_input_files(
            &input_path,
            &temp.path().join("feff.dym"),
            &[0_u8, 1_u8, 2_u8],
        );

        let request = ComputeRequest::new(
            "FX-DMDW-001",
            ComputeModule::Debye,
            &input_path,
            temp.path(),
        );
        let error = DmdwModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.DMDW_MODULE");
    }

    #[test]
    fn execute_requires_feff_dym_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        fs::write(&input_path, DMDW_INPUT_FIXTURE).expect("dmdw input should be written");

        let request = ComputeRequest::new(
            "FX-DMDW-001",
            ComputeModule::Dmdw,
            &input_path,
            temp.path(),
        );
        let error = DmdwModule
            .execute(&request)
            .expect_err("missing feff.dym should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.DMDW_INPUT_READ");
    }

    #[test]
    fn execute_rejects_empty_input_source() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        fs::write(&input_path, "\n\n\n").expect("dmdw input should be written");
        fs::write(temp.path().join("feff.dym"), [0_u8, 1_u8, 2_u8])
            .expect("feff.dym should be written");

        let request = ComputeRequest::new(
            "FX-DMDW-001",
            ComputeModule::Dmdw,
            &input_path,
            temp.path(),
        );
        let error = DmdwModule
            .execute(&request)
            .expect_err("empty input should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.DMDW_INPUT_PARSE");
    }

    fn stage_input_files(dmdw_input: &Path, feff_dym: &Path, feff_dym_bytes: &[u8]) {
        if let Some(parent) = dmdw_input.parent() {
            fs::create_dir_all(parent).expect("dmdw input parent should exist");
        }
        fs::write(dmdw_input, DMDW_INPUT_FIXTURE).expect("dmdw input should be written");

        if let Some(parent) = feff_dym.parent() {
            fs::create_dir_all(parent).expect("feff.dym parent should exist");
        }
        fs::write(feff_dym, feff_dym_bytes).expect("feff.dym should be written");
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn artifact_set_from_names(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }
}
