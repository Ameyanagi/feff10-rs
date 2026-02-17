mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::BandModel;
use parser::{artifact_list, input_parent_dir, read_input_bytes, read_input_source, validate_request_shape};

pub(crate) const BAND_REQUIRED_INPUTS: [&str; 4] = ["band.inp", "geom.dat", "global.inp", "phase.bin"];
pub(crate) const BAND_REQUIRED_OUTPUTS: [&str; 2] = ["bandstructure.dat", "logband.dat"];

#[cfg(test)]
use super::xsph::XSPH_PHASE_BINARY_MAGIC;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BandContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BandModule;

impl BandModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<BandContract> {
        validate_request_shape(request)?;
        Ok(BandContract {
            required_inputs: artifact_list(&BAND_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&BAND_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for BandModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let band_source = read_input_source(&request.input_path, BAND_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(BAND_REQUIRED_INPUTS[1]),
            BAND_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(BAND_REQUIRED_INPUTS[2]),
            BAND_REQUIRED_INPUTS[2],
        )?;
        let phase_bytes = read_input_bytes(
            &input_dir.join(BAND_REQUIRED_INPUTS[3]),
            BAND_REQUIRED_INPUTS[3],
        )?;

        let model = BandModel::from_sources(
            &request.fixture_id,
            &band_source,
            &geom_source,
            &global_source,
            &phase_bytes,
        )?;
        let outputs = artifact_list(&BAND_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.BAND_OUTPUT_DIRECTORY",
                format!(
                    "failed to create BAND output directory '{}': {}",
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
                        "IO.BAND_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create BAND artifact directory '{}': {}",
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
    use super::BandModule;
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    const EXPECTED_BAND_OUTPUTS: [&str; 2] = ["bandstructure.dat", "logband.dat"];

    #[test]
    fn contract_matches_true_compute_band_outputs() {
        let request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            "band.inp",
            "actual-output",
        );
        let contract = BandModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["band.inp", "geom.dat", "global.inp", "phase.bin"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&EXPECTED_BAND_OUTPUTS)
        );
    }

    #[test]
    fn execute_emits_required_outputs_without_baseline_dependencies() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("outputs");
        stage_required_inputs(&input_dir, &legacy_phase_bytes());

        let request = ComputeRequest::new(
            "FX-NONBASELINE-001",
            ComputeModule::Band,
            input_dir.join("band.inp"),
            &output_dir,
        );
        let artifacts = BandModule
            .execute(&request)
            .expect("BAND execution should succeed without fixture baseline lookup");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&EXPECTED_BAND_OUTPUTS)
        );
        for artifact in EXPECTED_BAND_OUTPUTS {
            let output_path = output_dir.join(artifact);
            assert!(
                output_path.is_file(),
                "artifact '{}' should exist",
                artifact
            );
            assert!(
                !fs::read(&output_path)
                    .expect("output artifact should be readable")
                    .is_empty(),
                "artifact '{}' should not be empty",
                artifact
            );
        }
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_input = temp.path().join("first-input");
        let first_output = temp.path().join("first-output");
        let second_input = temp.path().join("second-input");
        let second_output = temp.path().join("second-output");
        let phase_bytes = xsph_phase_fixture_bytes();
        stage_required_inputs(&first_input, &phase_bytes);
        stage_required_inputs(&second_input, &phase_bytes);

        let first_request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            first_input.join("band.inp"),
            &first_output,
        );
        BandModule
            .execute(&first_request)
            .expect("first BAND execution should succeed");

        let second_request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            second_input.join("band.inp"),
            &second_output,
        );
        BandModule
            .execute(&second_request)
            .expect("second BAND execution should succeed");

        for artifact in EXPECTED_BAND_OUTPUTS {
            let first = fs::read(first_output.join(artifact)).expect("first output should exist");
            let second =
                fs::read(second_output.join(artifact)).expect("second output should exist");
            assert_eq!(
                first, second,
                "artifact '{}' should be deterministic across runs",
                artifact
            );
        }
    }

    #[test]
    fn execute_accepts_true_compute_xsph_phase_binary_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("outputs");
        stage_required_inputs(&input_dir, &xsph_phase_fixture_bytes());

        let request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            input_dir.join("band.inp"),
            &output_dir,
        );
        let artifacts = BandModule
            .execute(&request)
            .expect("BAND execution should accept true-compute phase.bin");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&EXPECTED_BAND_OUTPUTS)
        );
    }

    #[test]
    fn execute_rejects_non_band_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_required_inputs(&input_dir, &legacy_phase_bytes());

        let request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Rdinp,
            input_dir.join("band.inp"),
            temp.path(),
        );
        let error = BandModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.BAND_MODULE");
    }

    #[test]
    fn execute_requires_phase_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input directory should exist");
        fs::write(input_dir.join("band.inp"), default_band_input_source())
            .expect("band input should be written");
        fs::write(input_dir.join("geom.dat"), default_geom_source())
            .expect("geom input should be written");
        fs::write(input_dir.join("global.inp"), default_global_source())
            .expect("global input should be written");

        let request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            input_dir.join("band.inp"),
            temp.path().join("out"),
        );
        let error = BandModule
            .execute(&request)
            .expect_err("missing phase input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.BAND_INPUT_READ");
    }

    #[test]
    fn execute_rejects_malformed_band_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input directory should exist");
        fs::write(
            input_dir.join("band.inp"),
            "mband : calculate bands if = 1\n",
        )
        .expect("band input should be written");
        fs::write(input_dir.join("geom.dat"), default_geom_source())
            .expect("geom input should be written");
        fs::write(input_dir.join("global.inp"), default_global_source())
            .expect("global input should be written");
        fs::write(input_dir.join("phase.bin"), legacy_phase_bytes())
            .expect("phase input should be written");

        let request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            input_dir.join("band.inp"),
            temp.path().join("out"),
        );
        let error = BandModule
            .execute(&request)
            .expect_err("malformed input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.BAND_INPUT_PARSE");
    }

    fn stage_required_inputs(destination_dir: &Path, phase_bytes: &[u8]) {
        fs::create_dir_all(destination_dir).expect("destination directory should exist");
        fs::write(
            destination_dir.join("band.inp"),
            default_band_input_source(),
        )
        .expect("band input should be written");
        fs::write(destination_dir.join("geom.dat"), default_geom_source())
            .expect("geom input should be written");
        fs::write(destination_dir.join("global.inp"), default_global_source())
            .expect("global input should be written");
        fs::write(destination_dir.join("phase.bin"), phase_bytes)
            .expect("phase input should exist");
    }

    fn default_band_input_source() -> &'static str {
        "mband : calculate bands if = 1\n   1\nemin, emax, estep : energy mesh\n    -8.00000      6.00000      0.05000\nnkp : # points in k-path\n  121\nikpath : type of k-path\n   2\nfreeprop :  empty lattice if = T\n F\n"
    }

    fn default_geom_source() -> &'static str {
        "nat, nph =    4    2\n\
  iat      x        y        z       ipot  iz\n\
    1    0.00000  0.00000  0.00000    0   29\n\
    2    1.80500  1.80500  0.00000    1   29\n\
    3   -1.80500  1.80500  0.00000    1   29\n\
    4    0.00000  1.80500  1.80500    2   14\n"
    }

    fn default_global_source() -> &'static str {
        " nabs, iphabs - CFAVERAGE data\n\
       1       0 100000.00000\n\
 ipol, ispin, le2, elpty, angks, l2lp, do_nrixs, ldecmx, lj\n\
    0    0    0      0.0000      0.0000    0    0   -1   -1\n\
evec xivec spvec\n\
      0.00000      0.00000      1.00000\n"
    }

    fn legacy_phase_bytes() -> Vec<u8> {
        (0_u8..=127_u8).collect()
    }

    fn xsph_phase_fixture_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(super::XSPH_PHASE_BINARY_MAGIC);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&6_u32.to_le_bytes());
        bytes.extend_from_slice(&128_u32.to_le_bytes());
        bytes.extend_from_slice(&1_i32.to_le_bytes());
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        bytes.extend_from_slice(&(-12.0_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.15_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.2_f64).to_le_bytes());
        bytes.extend_from_slice(&(1.5_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.05_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.0_f64).to_le_bytes());
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
