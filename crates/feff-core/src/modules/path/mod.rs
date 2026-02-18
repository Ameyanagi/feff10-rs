mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::PathModel;
use parser::{
    artifact_list, input_parent_dir, read_input_bytes, read_input_source, validate_request_shape,
};

pub(crate) const PATH_REQUIRED_INPUTS: [&str; 4] =
    ["paths.inp", "geom.dat", "global.inp", "phase.bin"];
pub(crate) const PATH_REQUIRED_OUTPUTS: [&str; 4] =
    ["paths.dat", "paths.bin", "crit.dat", "log4.dat"];
pub const PATH_BINARY_MAGIC: &[u8; 8] = b"PATHBIN1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PathModule;

impl PathModule {
    pub fn contract_for_request(&self, request: &ComputeRequest) -> ComputeResult<PathContract> {
        validate_request_shape(request)?;
        Ok(PathContract {
            required_inputs: artifact_list(&PATH_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&PATH_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for PathModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let path_source = read_input_source(&request.input_path, PATH_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(PATH_REQUIRED_INPUTS[1]),
            PATH_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(PATH_REQUIRED_INPUTS[2]),
            PATH_REQUIRED_INPUTS[2],
        )?;
        let phase_bytes = read_input_bytes(
            &input_dir.join(PATH_REQUIRED_INPUTS[3]),
            PATH_REQUIRED_INPUTS[3],
        )?;

        let model = PathModel::from_sources(
            &request.fixture_id,
            &path_source,
            &geom_source,
            &global_source,
            &phase_bytes,
        )?;
        let outputs = artifact_list(&PATH_REQUIRED_OUTPUTS);
        let generated_paths = model.generated_paths();

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
            model.write_artifact(&artifact_name, &output_path, &generated_paths)?;
        }

        Ok(outputs)
    }
}

#[cfg(test)]
mod tests {
    use super::{PATH_BINARY_MAGIC, PathModule};
    use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, FeffErrorCategory};
    use crate::modules::ModuleExecutor;
    use crate::modules::path::PATH_BINARY_MAGIC as EXPORTED_PATH_BINARY_MAGIC;
    use crate::modules::xsph::XSPH_PHASE_BINARY_MAGIC;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    const PATH_INPUT_FIXTURE: &str = "mpath, ms, nncrit, nlegxx, ipr4
   1   1   0  10   0
critpw, pcritk, pcrith,  rmax, rfms2
      2.50000      0.00000      0.00000      5.50000      4.00000
ica
  -1
";

    const GEOM_INPUT_FIXTURE: &str = "nat, nph =    6    1
    1    2
 iat     x       y        z       iph
 -----------------------------------------------------------------------
   1      0.00000      0.00000      0.00000   0   1
   2      1.80500      1.80500      0.00000   1   1
   3     -1.80500      1.80500      0.00000   1   1
   4      1.80500     -1.80500      0.00000   1   1
   5     -1.80500     -1.80500      0.00000   1   1
   6      0.00000      0.00000      3.61000   1   1
";

    const GLOBAL_INPUT_FIXTURE: &str = "nabs iphabs
1 0 100000.0
ipol ispin le2 elpty angks
0 0 0 0.0 0.0
";

    #[test]
    fn contract_returns_required_path_compute_artifacts() {
        let request = ComputeRequest::new(
            "FX-PATH-001",
            ComputeModule::Path,
            "paths.inp",
            "actual-output",
        );
        let contract = PathModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["paths.inp", "geom.dat", "global.inp", "phase.bin"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&["paths.dat", "paths.bin", "crit.dat", "log4.dat"])
        );
    }

    #[test]
    fn execute_emits_required_true_compute_artifacts() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("actual");
        stage_path_inputs(&input_dir, &sample_xsph_phase_binary());

        let request = ComputeRequest::new(
            "FX-PATH-001",
            ComputeModule::Path,
            input_dir.join("paths.inp"),
            &output_dir,
        );
        let artifacts = PathModule
            .execute(&request)
            .expect("PATH execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["paths.dat", "paths.bin", "crit.dat", "log4.dat"])
        );
        for artifact in &artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            assert!(
                output_path.is_file(),
                "PATH artifact '{}' should exist",
                output_path.display()
            );
            let bytes = fs::read(&output_path).expect("artifact bytes should be readable");
            assert!(
                !bytes.is_empty(),
                "PATH artifact '{}' should not be empty",
                output_path.display()
            );
        }

        let path_bin = fs::read(output_dir.join("paths.bin")).expect("paths.bin should exist");
        assert!(
            path_bin.starts_with(PATH_BINARY_MAGIC),
            "paths.bin should use PATH binary magic header"
        );
        assert_eq!(PATH_BINARY_MAGIC, EXPORTED_PATH_BINARY_MAGIC);
    }

    #[test]
    fn execute_outputs_are_deterministic_across_runs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_path_inputs(&input_dir, &sample_xsph_phase_binary());

        let first_dir = temp.path().join("first");
        let first_request = ComputeRequest::new(
            "FX-PATH-001",
            ComputeModule::Path,
            input_dir.join("paths.inp"),
            &first_dir,
        );
        let first_artifacts = PathModule
            .execute(&first_request)
            .expect("first PATH run should succeed");

        let second_dir = temp.path().join("second");
        let second_request = ComputeRequest::new(
            "FX-PATH-001",
            ComputeModule::Path,
            input_dir.join("paths.inp"),
            &second_dir,
        );
        let second_artifacts = PathModule
            .execute(&second_request)
            .expect("second PATH run should succeed");

        assert_eq!(
            artifact_set(&first_artifacts),
            artifact_set(&second_artifacts),
            "artifact sets should match across runs"
        );
        for artifact in &first_artifacts {
            let first_bytes =
                fs::read(first_dir.join(&artifact.relative_path)).expect("first bytes should read");
            let second_bytes = fs::read(second_dir.join(&artifact.relative_path))
                .expect("second bytes should read");
            assert_eq!(
                first_bytes,
                second_bytes,
                "artifact '{}' should be deterministic",
                artifact.relative_path.display()
            );
        }
    }

    #[test]
    fn execute_accepts_legacy_phase_binary_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("actual");
        stage_path_inputs(
            &input_dir,
            &[1_u8, 2_u8, 3_u8, 4_u8, 5_u8, 6_u8, 7_u8, 8_u8],
        );

        let request = ComputeRequest::new(
            "FX-PATH-001",
            ComputeModule::Path,
            input_dir.join("paths.inp"),
            &output_dir,
        );
        let artifacts = PathModule
            .execute(&request)
            .expect("PATH execution should accept legacy phase.bin");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["paths.dat", "paths.bin", "crit.dat", "log4.dat"])
        );
    }

    #[test]
    fn execute_rejects_non_path_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("actual");
        stage_path_inputs(&input_dir, &sample_xsph_phase_binary());

        let request = ComputeRequest::new(
            "FX-PATH-001",
            ComputeModule::Pot,
            input_dir.join("paths.inp"),
            &output_dir,
        );
        let error = PathModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.PATH_MODULE");
    }

    #[test]
    fn execute_requires_phase_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input directory should be created");
        fs::write(input_dir.join("paths.inp"), PATH_INPUT_FIXTURE).expect("paths.inp should write");
        fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom.dat should write");
        fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global.inp should write");

        let request = ComputeRequest::new(
            "FX-PATH-001",
            ComputeModule::Path,
            input_dir.join("paths.inp"),
            temp.path().join("actual"),
        );
        let error = PathModule
            .execute(&request)
            .expect_err("missing phase.bin should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.PATH_INPUT_READ");
    }

    #[test]
    fn execute_rejects_invalid_paths_control_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input directory should be created");
        fs::write(input_dir.join("paths.inp"), "invalid path deck\n")
            .expect("paths.inp should write");
        fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom.dat should write");
        fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global.inp should write");
        fs::write(input_dir.join("phase.bin"), sample_xsph_phase_binary())
            .expect("phase.bin should write");

        let request = ComputeRequest::new(
            "FX-PATH-001",
            ComputeModule::Path,
            input_dir.join("paths.inp"),
            temp.path().join("actual"),
        );
        let error = PathModule
            .execute(&request)
            .expect_err("invalid paths input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.PATH_INPUT_PARSE");
    }

    fn stage_path_inputs(input_dir: &Path, phase_bytes: &[u8]) {
        fs::create_dir_all(input_dir).expect("input directory should be created");
        fs::write(input_dir.join("paths.inp"), PATH_INPUT_FIXTURE).expect("paths.inp should write");
        fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom.dat should write");
        fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global.inp should write");
        fs::write(input_dir.join("phase.bin"), phase_bytes).expect("phase.bin should write");
    }

    fn sample_xsph_phase_binary() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(XSPH_PHASE_BINARY_MAGIC);
        bytes.extend_from_slice(&0xA5A5A5A5_u32.to_le_bytes());
        bytes.extend_from_slice(&4_u32.to_le_bytes());
        bytes.extend_from_slice(&32_u32.to_le_bytes());
        bytes.extend_from_slice(&(-8.0_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.15_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.25_f64).to_le_bytes());
        bytes.extend_from_slice(&(1.10_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.03_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.005_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.001_f64).to_le_bytes());
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
}
