mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::ComptonModel;
use parser::{artifact_list, input_parent_dir, read_input_bytes, read_input_source, validate_request_shape};

pub(crate) const COMPTON_REQUIRED_INPUTS: [&str; 3] = ["compton.inp", "pot.bin", "gg_slice.bin"];
pub(crate) const COMPTON_REQUIRED_OUTPUTS: [&str; 4] =
    ["compton.dat", "jzzp.dat", "rhozzp.dat", "logcompton.dat"];
pub(crate) const POT_BINARY_MAGIC: &[u8; 8] = b"POTBIN10";
pub(crate) const POT_CONTROL_I32_COUNT: usize = 16;
pub(crate) const POT_CONTROL_F64_COUNT: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComptonContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComptonModule;

impl ComptonModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<ComptonContract> {
        validate_request_shape(request)?;
        Ok(ComptonContract {
            required_inputs: artifact_list(&COMPTON_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&COMPTON_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for ComptonModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let compton_source = read_input_source(&request.input_path, COMPTON_REQUIRED_INPUTS[0])?;
        let pot_bytes = read_input_bytes(
            &input_dir.join(COMPTON_REQUIRED_INPUTS[1]),
            COMPTON_REQUIRED_INPUTS[1],
        )?;
        let gg_slice_bytes = read_input_bytes(
            &input_dir.join(COMPTON_REQUIRED_INPUTS[2]),
            COMPTON_REQUIRED_INPUTS[2],
        )?;

        let model = ComptonModel::from_sources(
            &request.fixture_id,
            &compton_source,
            &pot_bytes,
            &gg_slice_bytes,
        )?;
        let outputs = artifact_list(&COMPTON_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.COMPTON_OUTPUT_DIRECTORY",
                format!(
                    "failed to create COMPTON output directory '{}': {}",
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
                        "IO.COMPTON_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create COMPTON artifact directory '{}': {}",
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
    use super::ComptonModule;
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    const COMPTON_INPUT_FIXTURE: &str = "run compton module?\n           1\npqmax, npq\n   5.000000            1000\nns, nphi, nz, nzp\n  32  32  32 120\nsmax, phimax, zmax, zpmax\n      0.00000      6.28319      0.00000     10.00000\njpq? rhozzp? force_recalc_jzzp?\n T T F\nwindow_type (0=Step, 1=Hann), window_cutoff\n           1  0.0000000E+00\ntemperature (in eV)\n      0.00000\nset_chemical_potential? chemical_potential(eV)\n F  0.0000000E+00\nrho_xy? rho_yz? rho_xz? rho_vol? rho_line?\n F F F F F\nqhat_x qhat_y qhat_z\n  0.000000000000000E+000  0.000000000000000E+000   1.00000000000000\n";

    #[test]
    fn contract_lists_required_inputs_and_outputs() {
        let request = ComputeRequest::new(
            "FX-COMPTON-001",
            ComputeModule::Compton,
            "compton.inp",
            "actual-output",
        );
        let scaffold = ComptonModule;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 3);
        assert_eq!(contract.expected_outputs.len(), 4);
        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["compton.inp", "pot.bin", "gg_slice.bin"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&["compton.dat", "jzzp.dat", "rhozzp.dat", "logcompton.dat"])
        );
    }

    #[test]
    fn execute_emits_required_true_compute_artifacts() {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");
        stage_inputs(&output_dir);

        let request = ComputeRequest::new(
            "FX-COMPTON-001",
            ComputeModule::Compton,
            output_dir.join("compton.inp"),
            &output_dir,
        );
        let scaffold = ComptonModule;
        let artifacts = scaffold
            .execute(&request)
            .expect("COMPTON execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["compton.dat", "jzzp.dat", "rhozzp.dat", "logcompton.dat"])
        );
        for artifact in artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            let bytes = fs::read(&output_path).expect("artifact should be readable");
            assert!(
                !bytes.is_empty(),
                "artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }

    #[test]
    fn execute_is_deterministic_across_runs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_dir = temp.path().join("first");
        let second_dir = temp.path().join("second");
        stage_inputs(&first_dir);
        stage_inputs(&second_dir);

        let request_one = ComputeRequest::new(
            "FX-COMPTON-001",
            ComputeModule::Compton,
            first_dir.join("compton.inp"),
            &first_dir,
        );
        let request_two = ComputeRequest::new(
            "FX-COMPTON-001",
            ComputeModule::Compton,
            second_dir.join("compton.inp"),
            &second_dir,
        );

        let scaffold = ComptonModule;
        let first_artifacts = scaffold
            .execute(&request_one)
            .expect("first run should succeed");
        let second_artifacts = scaffold
            .execute(&request_two)
            .expect("second run should succeed");

        assert_eq!(
            artifact_set(&first_artifacts),
            artifact_set(&second_artifacts)
        );

        for artifact in first_artifacts {
            let first = fs::read(first_dir.join(&artifact.relative_path))
                .expect("first artifact should be readable");
            let second = fs::read(second_dir.join(&artifact.relative_path))
                .expect("second artifact should be readable");
            assert_eq!(
                first,
                second,
                "artifact '{}' should be deterministic",
                artifact.relative_path.display()
            );
        }
    }

    #[test]
    fn execute_rejects_non_compton_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("compton.inp");
        fs::write(&input_path, COMPTON_INPUT_FIXTURE).expect("compton input should be written");
        fs::write(temp.path().join("pot.bin"), [1_u8, 2_u8]).expect("pot should be written");
        fs::write(temp.path().join("gg_slice.bin"), [3_u8, 4_u8])
            .expect("gg slice should be written");

        let request = ComputeRequest::new(
            "FX-COMPTON-001",
            ComputeModule::Crpa,
            &input_path,
            temp.path(),
        );
        let scaffold = ComptonModule;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.COMPTON_MODULE");
    }

    #[test]
    fn execute_requires_gg_slice_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("compton.inp");
        fs::write(&input_path, COMPTON_INPUT_FIXTURE).expect("compton input should be written");
        fs::write(temp.path().join("pot.bin"), [0_u8, 1_u8, 2_u8]).expect("pot should be written");

        let request = ComputeRequest::new(
            "FX-COMPTON-001",
            ComputeModule::Compton,
            &input_path,
            temp.path(),
        );
        let scaffold = ComptonModule;
        let error = scaffold
            .execute(&request)
            .expect_err("missing gg_slice input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.COMPTON_INPUT_READ");
    }

    #[test]
    fn execute_rejects_invalid_control_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("compton.inp");
        fs::write(&input_path, "run compton module?\n1\n")
            .expect("compton input should be written");
        fs::write(temp.path().join("pot.bin"), [0_u8, 1_u8, 2_u8]).expect("pot should be written");
        fs::write(temp.path().join("gg_slice.bin"), [3_u8, 4_u8, 5_u8])
            .expect("gg slice should be written");

        let request = ComputeRequest::new(
            "FX-COMPTON-001",
            ComputeModule::Compton,
            &input_path,
            temp.path(),
        );
        let scaffold = ComptonModule;
        let error = scaffold
            .execute(&request)
            .expect_err("invalid input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.COMPTON_INPUT_PARSE");
    }

    fn stage_inputs(destination_dir: &Path) {
        fs::create_dir_all(destination_dir).expect("destination dir should exist");
        fs::write(destination_dir.join("compton.inp"), COMPTON_INPUT_FIXTURE)
            .expect("compton input should be staged");
        fs::write(destination_dir.join("pot.bin"), pot_fixture_bytes())
            .expect("pot input should be staged");
        fs::write(
            destination_dir.join("gg_slice.bin"),
            gg_slice_fixture_bytes(),
        )
        .expect("gg_slice input should be staged");
    }

    fn pot_fixture_bytes() -> Vec<u8> {
        vec![0_u8, 1_u8, 2_u8, 3_u8, 4_u8, 5_u8, 6_u8, 7_u8, 8_u8]
    }

    fn gg_slice_fixture_bytes() -> Vec<u8> {
        vec![9_u8, 10_u8, 11_u8, 12_u8, 13_u8, 14_u8, 15_u8, 16_u8]
    }

    fn expected_artifact_set(artifacts: &[&str]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }
}
