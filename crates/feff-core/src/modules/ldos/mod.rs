mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::LdosModel;
use parser::{artifact_list, input_parent_dir, read_input_bytes, read_input_source, validate_request_shape};

pub(crate) const LDOS_REQUIRED_INPUTS: [&str; 4] = ["ldos.inp", "geom.dat", "pot.bin", "reciprocal.inp"];
pub(crate) const LDOS_LOG_OUTPUT: &str = "logdos.dat";
pub(crate) const POT_BINARY_MAGIC: &[u8; 8] = b"POTBIN10";
pub(crate) const POT_CONTROL_I32_COUNT: usize = 16;
pub(crate) const POT_CONTROL_F64_COUNT: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdosContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LdosModule;

impl LdosModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<LdosContract> {
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
        let model = LdosModel::from_sources(
            &request.fixture_id,
            &ldos_source,
            &geom_source,
            &pot_bytes,
            &reciprocal_source,
        )?;

        Ok(LdosContract {
            required_inputs: artifact_list(&LDOS_REQUIRED_INPUTS),
            expected_outputs: model.expected_outputs(),
        })
    }
}

impl ModuleExecutor for LdosModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
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

        let model = LdosModel::from_sources(
            &request.fixture_id,
            &ldos_source,
            &geom_source,
            &pot_bytes,
            &reciprocal_source,
        )?;
        let outputs = model.expected_outputs();

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

        for artifact in &outputs {
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

            let artifact_name = artifact.relative_path.to_string_lossy().replace('\\', "/");
            model.write_artifact(&artifact_name, &output_path)?;
        }

        Ok(outputs)
    }
}

#[cfg(test)]
mod tests {
    use super::LdosModule;
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn contract_matches_true_compute_ldos_output_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_baseline_artifact("FX-LDOS-001", "ldos.inp", &temp.path().join("ldos.inp"));
        stage_baseline_artifact("FX-LDOS-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-LDOS-001", "pot.bin", &temp.path().join("pot.bin"));
        stage_baseline_artifact(
            "FX-LDOS-001",
            "reciprocal.inp",
            &temp.path().join("reciprocal.inp"),
        );

        let request = ComputeRequest::new(
            "FX-LDOS-001",
            ComputeModule::Ldos,
            temp.path().join("ldos.inp"),
            temp.path().join("actual-output"),
        );
        let scaffold = LdosModule;
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
            expected_artifact_set(&["ldos00.dat", "ldos01.dat", "ldos02.dat", "logdos.dat"])
        );
    }

    #[test]
    fn execute_emits_true_compute_ldos_outputs() {
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

        let request = ComputeRequest::new(
            "FX-LDOS-001",
            ComputeModule::Ldos,
            &input_path,
            &output_dir,
        );
        let scaffold = LdosModule;
        let artifacts = scaffold
            .execute(&request)
            .expect("LDOS execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["ldos00.dat", "ldos01.dat", "ldos02.dat", "logdos.dat"])
        );
        for artifact in artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            assert!(output_path.is_file(), "artifact should exist on disk");
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "artifact should not be empty"
            );
        }
    }

    #[test]
    fn execute_is_deterministic_for_identical_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_input_dir = temp.path().join("first-inputs");
        let second_input_dir = temp.path().join("second-inputs");
        let first_output_dir = temp.path().join("first-output");
        let second_output_dir = temp.path().join("second-output");

        for input_dir in [&first_input_dir, &second_input_dir] {
            stage_baseline_artifact("FX-LDOS-001", "ldos.inp", &input_dir.join("ldos.inp"));
            stage_baseline_artifact("FX-LDOS-001", "geom.dat", &input_dir.join("geom.dat"));
            stage_baseline_artifact("FX-LDOS-001", "pot.bin", &input_dir.join("pot.bin"));
            stage_baseline_artifact(
                "FX-LDOS-001",
                "reciprocal.inp",
                &input_dir.join("reciprocal.inp"),
            );
        }

        let scaffold = LdosModule;
        let first_request = ComputeRequest::new(
            "FX-LDOS-001",
            ComputeModule::Ldos,
            first_input_dir.join("ldos.inp"),
            &first_output_dir,
        );
        let first_artifacts = scaffold
            .execute(&first_request)
            .expect("first LDOS execution should succeed");

        let second_request = ComputeRequest::new(
            "FX-LDOS-001",
            ComputeModule::Ldos,
            second_input_dir.join("ldos.inp"),
            &second_output_dir,
        );
        let second_artifacts = scaffold
            .execute(&second_request)
            .expect("second LDOS execution should succeed");

        assert_eq!(
            artifact_set(&first_artifacts),
            artifact_set(&second_artifacts),
            "artifact contracts should match across runs"
        );
        for artifact in first_artifacts {
            let first = fs::read(first_output_dir.join(&artifact.relative_path))
                .expect("first artifact should be readable");
            let second = fs::read(second_output_dir.join(&artifact.relative_path))
                .expect("second artifact should be readable");
            assert_eq!(first, second, "artifact bytes should be deterministic");
        }
    }

    #[test]
    fn execute_accepts_rdinp_style_ldos_input_without_neldos() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ldos.inp");
        let output_dir = temp.path().join("out");
        fs::write(&input_path, LDOS_INPUT_WITHOUT_NELDOS).expect("ldos input should be staged");
        fs::write(temp.path().join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be staged");
        fs::write(temp.path().join("pot.bin"), [1_u8, 2_u8, 3_u8, 4_u8])
            .expect("pot input should be staged");
        fs::write(temp.path().join("reciprocal.inp"), RECIPROCAL_INPUT_FIXTURE)
            .expect("reciprocal input should be staged");

        let request = ComputeRequest::new(
            "FX-RDINP-COMPAT",
            ComputeModule::Ldos,
            &input_path,
            &output_dir,
        );
        let artifacts = LdosModule
            .execute(&request)
            .expect("LDOS should accept RDINP-style ldos.inp");

        let artifact_names = artifact_set(&artifacts);
        assert!(
            artifact_names.contains("logdos.dat"),
            "log output should be present"
        );
        assert!(
            artifact_names.iter().any(|name| name.starts_with("ldos")),
            "at least one ldosNN.dat output should be present"
        );
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

        let request = ComputeRequest::new(
            "FX-LDOS-001",
            ComputeModule::Band,
            &input_path,
            temp.path(),
        );
        let scaffold = LdosModule;
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

        let request = ComputeRequest::new(
            "FX-LDOS-001",
            ComputeModule::Ldos,
            &input_path,
            temp.path(),
        );
        let scaffold = LdosModule;
        let error = scaffold
            .execute(&request)
            .expect_err("missing pot input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.LDOS_INPUT_READ");
    }

    fn fixture_baseline_dir(fixture_id: &str) -> PathBuf {
        workspace_root()
            .join("artifacts/fortran-baselines")
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

    const LDOS_INPUT_WITHOUT_NELDOS: &str = "mldos, lfms2, ixc, ispin, minv\n\
   1   0   0   0   0\n\
rfms2, emin, emax, eimag, rgrd\n\
      4.00000    -20.00000     10.00000      0.10000      0.05000\n\
rdirec, toler1, toler2\n\
      8.00000      0.00100      0.00100\n\
 lmaxph(0:nph)\n\
   2   2\n";

    const GEOM_INPUT_FIXTURE: &str = "nat, nph =    4    1\n\
    1    2\n\
 iat     x       y        z       iph\n\
 -----------------------------------------------------------------------\n\
   1      0.00000      0.00000      0.00000   0   1\n\
   2      1.80500      1.80500      0.00000   1   1\n\
   3     -1.80500      1.80500      0.00000   1   1\n\
   4      0.00000      1.80500      1.80500   1   1\n";

    const RECIPROCAL_INPUT_FIXTURE: &str = "ispace\n\
   1\n";
}
