mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::RixsModel;
use parser::{artifact_list, input_parent_dir, read_input_bytes, read_input_source, validate_request_shape};

pub(crate) const RIXS_REQUIRED_INPUTS: [&str; 6] = [
    "rixs.inp",
    "phase_1.bin",
    "phase_2.bin",
    "wscrn_1.dat",
    "wscrn_2.dat",
    "xsect_2.dat",
];
pub(crate) const RIXS_REQUIRED_OUTPUTS: [&str; 7] = [
    "rixs0.dat",
    "rixs1.dat",
    "rixsET.dat",
    "rixsEE.dat",
    "rixsET-sat.dat",
    "rixsEE-sat.dat",
    "logrixs.dat",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RixsContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RixsModule;

impl RixsModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<RixsContract> {
        validate_request_shape(request)?;
        Ok(RixsContract {
            required_inputs: artifact_list(&RIXS_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&RIXS_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for RixsModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let rixs_source = read_input_source(&request.input_path, RIXS_REQUIRED_INPUTS[0])?;
        let phase_1_bytes = read_input_bytes(
            &input_dir.join(RIXS_REQUIRED_INPUTS[1]),
            RIXS_REQUIRED_INPUTS[1],
        )?;
        let phase_2_bytes = read_input_bytes(
            &input_dir.join(RIXS_REQUIRED_INPUTS[2]),
            RIXS_REQUIRED_INPUTS[2],
        )?;
        let wscrn_1_source = read_input_source(
            &input_dir.join(RIXS_REQUIRED_INPUTS[3]),
            RIXS_REQUIRED_INPUTS[3],
        )?;
        let wscrn_2_source = read_input_source(
            &input_dir.join(RIXS_REQUIRED_INPUTS[4]),
            RIXS_REQUIRED_INPUTS[4],
        )?;
        let xsect_2_source = read_input_source(
            &input_dir.join(RIXS_REQUIRED_INPUTS[5]),
            RIXS_REQUIRED_INPUTS[5],
        )?;

        let model = RixsModel::from_sources(
            &request.fixture_id,
            &rixs_source,
            &phase_1_bytes,
            &phase_2_bytes,
            &wscrn_1_source,
            &wscrn_2_source,
            &xsect_2_source,
        )?;
        let outputs = artifact_list(&RIXS_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.RIXS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create RIXS output directory '{}': {}",
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
                        "IO.RIXS_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create RIXS artifact directory '{}': {}",
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
    use super::RixsModule;
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const RIXS_OUTPUTS: [&str; 7] = [
        "rixs0.dat",
        "rixs1.dat",
        "rixsET.dat",
        "rixsEE.dat",
        "rixsET-sat.dat",
        "rixsEE-sat.dat",
        "logrixs.dat",
    ];

    const RIXS_INPUT: &str = "\
 m_run
           1
 gam_ch, gam_exp(1), gam_exp(2)
        0.0001350512        0.0001450512        0.0001550512
 EMinI, EMaxI, EMinF, EMaxF
      -12.0000000000       18.0000000000       -4.0000000000       16.0000000000
 xmu
  -367493090.02742821
 Readpoles, SkipCalc, MBConv, ReadSigma
 T F F T
 nEdges
           2
 Edge           1
 L3
 Edge           2
 L2
";

    const WSCRN_1_INPUT: &str = "\
# edge 1 screening profile
-6.0  0.11  0.95
-2.0  0.16  1.05
 0.0  0.18  1.15
 3.5  0.23  1.30
 8.0  0.31  1.45
";

    const WSCRN_2_INPUT: &str = "\
# edge 2 screening profile
-5.0  0.09  0.85
-1.5  0.14  0.95
 1.0  0.17  1.05
 4.0  0.21  1.22
 9.0  0.28  1.36
";

    const XSECT_2_INPUT: &str = "\
# xsect_2 seed table
0.0  1.2  0.1
2.0  1.0  0.2
4.0  0.9  0.3
6.0  0.8  0.4
8.0  0.7  0.5
";

    #[test]
    fn contract_exposes_required_inputs_and_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle(temp.path());

        let request = ComputeRequest::new(
            "FX-RIXS-001",
            ComputeModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path().join("out"),
        );
        let contract = RixsModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            contract.required_inputs,
            artifact_list(&[
                "rixs.inp",
                "phase_1.bin",
                "phase_2.bin",
                "wscrn_1.dat",
                "wscrn_2.dat",
                "xsect_2.dat"
            ])
        );
        assert_eq!(artifact_set(&contract.expected_outputs), expected_outputs());
    }

    #[test]
    fn execute_writes_true_compute_rixs_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle(temp.path());

        let request = ComputeRequest::new(
            "FX-RIXS-001",
            ComputeModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path().join("out"),
        );
        let artifacts = RixsModule
            .execute(&request)
            .expect("execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_outputs());
        for artifact in expected_outputs() {
            let output_path = request.output_dir.join(&artifact);
            assert!(
                output_path.is_file(),
                "{} should exist",
                output_path.display()
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "{} should not be empty",
                output_path.display()
            );
        }
    }

    #[test]
    fn execute_is_deterministic_for_identical_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle(temp.path());

        let first_request = ComputeRequest::new(
            "FX-RIXS-001",
            ComputeModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path().join("out-first"),
        );
        let second_request = ComputeRequest::new(
            "FX-RIXS-001",
            ComputeModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path().join("out-second"),
        );

        let first = RixsModule
            .execute(&first_request)
            .expect("first execution should succeed");
        let second = RixsModule
            .execute(&second_request)
            .expect("second execution should succeed");

        assert_eq!(artifact_set(&first), artifact_set(&second));
        for artifact in first {
            let first_bytes =
                fs::read(first_request.output_dir.join(&artifact.relative_path)).expect("first");
            let second_bytes =
                fs::read(second_request.output_dir.join(&artifact.relative_path)).expect("second");
            assert_eq!(
                first_bytes,
                second_bytes,
                "artifact '{}' should be deterministic",
                artifact.relative_path.display()
            );
        }
    }

    #[test]
    fn execute_responds_to_multi_edge_input_changes() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_root = temp.path().join("first");
        let second_root = temp.path().join("second");
        stage_rixs_input_bundle(&first_root);
        stage_rixs_input_bundle(&second_root);

        stage_binary(
            second_root.join("phase_2.bin"),
            &[255_u8, 254_u8, 0_u8, 8_u8, 21_u8, 34_u8, 55_u8, 89_u8],
        );
        stage_text(
            second_root.join("wscrn_2.dat"),
            "# altered edge 2 screening\n-5.0  0.40  2.10\n0.0  0.55  2.25\n5.0  0.70  2.40\n",
        );

        let first_request = ComputeRequest::new(
            "FX-RIXS-001",
            ComputeModule::Rixs,
            first_root.join("rixs.inp"),
            first_root.join("out"),
        );
        let second_request = ComputeRequest::new(
            "FX-RIXS-001",
            ComputeModule::Rixs,
            second_root.join("rixs.inp"),
            second_root.join("out"),
        );

        let first_artifacts = RixsModule
            .execute(&first_request)
            .expect("first execution should succeed");
        let second_artifacts = RixsModule
            .execute(&second_request)
            .expect("second execution should succeed");

        assert_eq!(artifact_set(&first_artifacts), expected_outputs());
        assert_eq!(artifact_set(&second_artifacts), expected_outputs());

        let first_rixs1 = fs::read(first_request.output_dir.join("rixs1.dat"))
            .expect("first rixs1.dat should be readable");
        let second_rixs1 = fs::read(second_request.output_dir.join("rixs1.dat"))
            .expect("second rixs1.dat should be readable");
        assert_ne!(
            first_rixs1, second_rixs1,
            "rixs1.dat should change when edge-2 staged inputs change"
        );

        let first_rixsee = fs::read(first_request.output_dir.join("rixsEE.dat"))
            .expect("first rixsEE.dat should be readable");
        let second_rixsee = fs::read(second_request.output_dir.join("rixsEE.dat"))
            .expect("second rixsEE.dat should be readable");
        assert_ne!(
            first_rixsee, second_rixsee,
            "rixsEE.dat should change when edge-2 staged inputs change"
        );
    }

    #[test]
    fn execute_rejects_non_rixs_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle(temp.path());

        let request = ComputeRequest::new(
            "FX-RIXS-001",
            ComputeModule::Rdinp,
            temp.path().join("rixs.inp"),
            temp.path().join("out"),
        );
        let error = RixsModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.RIXS_MODULE");
    }

    #[test]
    fn execute_requires_phase_2_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_rixs_input_bundle(temp.path());
        fs::remove_file(temp.path().join("phase_2.bin")).expect("phase_2.bin should be removed");

        let request = ComputeRequest::new(
            "FX-RIXS-001",
            ComputeModule::Rixs,
            temp.path().join("rixs.inp"),
            temp.path().join("out"),
        );
        let error = RixsModule
            .execute(&request)
            .expect_err("missing phase_2 input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.RIXS_INPUT_READ");
    }

    fn expected_outputs() -> BTreeSet<String> {
        RIXS_OUTPUTS
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

    fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
        paths.iter().copied().map(ComputeArtifact::new).collect()
    }

    fn stage_rixs_input_bundle(destination_dir: &Path) {
        stage_text(destination_dir.join("rixs.inp"), RIXS_INPUT);
        stage_binary(
            destination_dir.join("phase_1.bin"),
            &[3_u8, 5_u8, 8_u8, 13_u8, 21_u8, 34_u8, 55_u8, 89_u8],
        );
        stage_binary(
            destination_dir.join("phase_2.bin"),
            &[2_u8, 7_u8, 1_u8, 8_u8, 2_u8, 8_u8, 1_u8, 8_u8],
        );
        stage_text(destination_dir.join("wscrn_1.dat"), WSCRN_1_INPUT);
        stage_text(destination_dir.join("wscrn_2.dat"), WSCRN_2_INPUT);
        stage_text(destination_dir.join("xsect_2.dat"), XSECT_2_INPUT);
    }

    fn stage_text(destination: PathBuf, contents: &str) {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should exist");
        }
        fs::write(destination, contents).expect("text input should be written");
    }

    fn stage_binary(destination: PathBuf, bytes: &[u8]) {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should exist");
        }
        fs::write(destination, bytes).expect("binary input should be written");
    }
}
