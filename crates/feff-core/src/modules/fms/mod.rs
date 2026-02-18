mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::FmsModel;
use parser::{
    artifact_list, input_parent_dir, read_input_bytes, read_input_source, validate_request_shape,
};

pub(crate) const FMS_REQUIRED_INPUTS: [&str; 4] =
    ["fms.inp", "geom.dat", "global.inp", "phase.bin"];
pub(crate) const FMS_REQUIRED_OUTPUTS: [&str; 2] = ["gg.bin", "log3.dat"];
pub const FMS_GG_BINARY_MAGIC: &[u8; 8] = b"FMSGBIN1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmsContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FmsModule;

impl FmsModule {
    pub fn contract_for_request(&self, request: &ComputeRequest) -> ComputeResult<FmsContract> {
        validate_request_shape(request)?;
        Ok(FmsContract {
            required_inputs: artifact_list(&FMS_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&FMS_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for FmsModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let fms_source = read_input_source(&request.input_path, FMS_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(FMS_REQUIRED_INPUTS[1]),
            FMS_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(FMS_REQUIRED_INPUTS[2]),
            FMS_REQUIRED_INPUTS[2],
        )?;
        let phase_bytes = read_input_bytes(
            &input_dir.join(FMS_REQUIRED_INPUTS[3]),
            FMS_REQUIRED_INPUTS[3],
        )?;

        let model = FmsModel::from_sources(
            &request.fixture_id,
            &fms_source,
            &geom_source,
            &global_source,
            &phase_bytes,
        )?;
        let outputs = artifact_list(&FMS_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.FMS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create FMS output directory '{}': {}",
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
                        "IO.FMS_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create FMS artifact directory '{}': {}",
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
    use super::{FMS_GG_BINARY_MAGIC, FmsModule};
    use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, FeffErrorCategory};
    use crate::modules::ModuleExecutor;
    use crate::modules::xsph::XSPH_PHASE_BINARY_MAGIC;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn contract_reports_required_true_compute_artifacts() {
        let request =
            ComputeRequest::new("FX-FMS-001", ComputeModule::Fms, "fms.inp", "actual-output");
        let contract = FmsModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["fms.inp", "geom.dat", "global.inp", "phase.bin"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&["gg.bin", "log3.dat"])
        );
    }

    #[test]
    fn execute_generates_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_inputs(&input_dir, &legacy_phase_bytes());

        let request = ComputeRequest::new(
            "FX-FMS-001",
            ComputeModule::Fms,
            input_dir.join("fms.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = FmsModule
            .execute(&request)
            .expect("FMS execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["gg.bin", "log3.dat"])
        );

        let output_dir = temp.path().join("outputs");
        let gg_bytes = fs::read(output_dir.join("gg.bin")).expect("gg.bin should be readable");
        assert!(
            gg_bytes.starts_with(FMS_GG_BINARY_MAGIC),
            "gg.bin should include deterministic FMS magic header"
        );
        assert!(
            gg_bytes.len() > FMS_GG_BINARY_MAGIC.len(),
            "gg.bin should include payload after header"
        );

        let log = fs::read_to_string(output_dir.join("log3.dat"))
            .expect("log3.dat should be readable as text");
        assert!(log.contains("FMS true-compute runtime"));
        assert!(log.contains("output-artifacts: gg.bin log3.dat"));
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");

        let first_inputs = temp.path().join("first-inputs");
        stage_inputs(&first_inputs, &legacy_phase_bytes());
        let first_output = temp.path().join("first-output");
        let first_request = ComputeRequest::new(
            "FX-FMS-001",
            ComputeModule::Fms,
            first_inputs.join("fms.inp"),
            &first_output,
        );
        FmsModule
            .execute(&first_request)
            .expect("first FMS execution should succeed");

        let second_inputs = temp.path().join("second-inputs");
        stage_inputs(&second_inputs, &legacy_phase_bytes());
        let second_output = temp.path().join("second-output");
        let second_request = ComputeRequest::new(
            "FX-FMS-001",
            ComputeModule::Fms,
            second_inputs.join("fms.inp"),
            &second_output,
        );
        FmsModule
            .execute(&second_request)
            .expect("second FMS execution should succeed");

        for artifact in ["gg.bin", "log3.dat"] {
            let first = fs::read(first_output.join(artifact)).expect("first artifact should exist");
            let second =
                fs::read(second_output.join(artifact)).expect("second artifact should exist");
            assert_eq!(
                first, second,
                "artifact '{}' should be deterministic",
                artifact
            );
        }
    }

    #[test]
    fn execute_accepts_true_compute_xsph_phase_binary_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_inputs(&input_dir, &xsph_phase_bytes());

        let request = ComputeRequest::new(
            "FX-FMS-001",
            ComputeModule::Fms,
            input_dir.join("fms.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = FmsModule
            .execute(&request)
            .expect("FMS execution should accept true-compute XSPH phase.bin");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["gg.bin", "log3.dat"])
        );
    }

    #[test]
    fn execute_rejects_non_fms_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_inputs(&input_dir, &legacy_phase_bytes());

        let request = ComputeRequest::new(
            "FX-FMS-001",
            ComputeModule::Path,
            input_dir.join("fms.inp"),
            temp.path(),
        );
        let error = FmsModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.FMS_MODULE");
    }

    #[test]
    fn execute_requires_phase_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input dir should exist");
        fs::write(input_dir.join("fms.inp"), FMS_INPUT_FIXTURE)
            .expect("fms input should be written");
        fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");
        fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");

        let request = ComputeRequest::new(
            "FX-FMS-001",
            ComputeModule::Fms,
            input_dir.join("fms.inp"),
            temp.path(),
        );
        let error = FmsModule
            .execute(&request)
            .expect_err("missing phase input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.FMS_INPUT_READ");
    }

    #[test]
    fn execute_reports_parse_failures_for_invalid_fms_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input dir should exist");
        fs::write(input_dir.join("fms.inp"), "mfms\n1\n").expect("fms input should be written");
        fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");
        fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");
        fs::write(input_dir.join("phase.bin"), legacy_phase_bytes())
            .expect("phase input should be written");

        let request = ComputeRequest::new(
            "FX-FMS-001",
            ComputeModule::Fms,
            input_dir.join("fms.inp"),
            temp.path().join("outputs"),
        );
        let error = FmsModule
            .execute(&request)
            .expect_err("invalid fms input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.FMS_INPUT_PARSE");
    }

    fn stage_inputs(root: &Path, phase_bytes: &[u8]) {
        fs::create_dir_all(root).expect("input root should exist");
        fs::write(root.join("fms.inp"), FMS_INPUT_FIXTURE).expect("fms input should be written");
        fs::write(root.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom input should be written");
        fs::write(root.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");
        fs::write(root.join("phase.bin"), phase_bytes).expect("phase input should be written");
    }

    fn legacy_phase_bytes() -> Vec<u8> {
        b"legacy-phase-binary-contract".to_vec()
    }

    fn xsph_phase_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(XSPH_PHASE_BINARY_MAGIC);
        super::model::push_u32(&mut bytes, 1);
        super::model::push_u32(&mut bytes, 6);
        super::model::push_u32(&mut bytes, 128);
        super::model::push_i32(&mut bytes, 1);
        super::model::push_i32(&mut bytes, 0);
        super::model::push_f64(&mut bytes, -25.0);
        super::model::push_f64(&mut bytes, 0.15);
        super::model::push_f64(&mut bytes, 0.2);
        bytes
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

    const FMS_INPUT_FIXTURE: &str = "mfms, idwopt, minv
   1  -1   0
rfms2, rdirec, toler1, toler2
      4.00000      8.00000      0.00100      0.00100
tk, thetad, sig2g
      0.00000      0.00000      0.00300
 lmaxph(0:nph)
   3   3
 the number of decomposi
   -1
";

    const GEOM_INPUT_FIXTURE: &str = "nat, nph =    4    1
    1    2
 iat     x       y        z       iph
 -----------------------------------------------------------------------
   1      0.00000      0.00000      0.00000   0   1
   2      1.80500      1.80500      0.00000   1   1
   3     -1.80500      1.80500      0.00000   1   1
   4      0.00000      1.80500      1.80500   1   1
";

    const GLOBAL_INPUT_FIXTURE: &str = " nabs, iphabs - CFAVERAGE data
       1       0 100000.00000
 ipol, ispin, le2, elpty, angks, l2lp, do_nrixs, ldecmx, lj
    0    0    0      0.0000      0.0000    0    0   -1   -1
evec xivec spvec
      0.00000      0.00000      1.00000
";
}
