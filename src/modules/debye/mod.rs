mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::DebyeModel;
use parser::{
    artifact_list, input_parent_dir, maybe_read_optional_input_source,
    read_input_source, validate_request_shape,
};

pub(crate) const DEBYE_REQUIRED_INPUTS: [&str; 3] = ["ff2x.inp", "paths.dat", "feff.inp"];
pub(crate) const DEBYE_OPTIONAL_INPUTS: [&str; 1] = ["spring.inp"];
pub(crate) const DEBYE_REQUIRED_OUTPUTS: [&str; 7] = [
    "s2_em.dat",
    "s2_rm1.dat",
    "s2_rm2.dat",
    "xmu.dat",
    "chi.dat",
    "log6.dat",
    "spring.dat",
];

pub(crate) const CHECKSUM_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
pub(crate) const CHECKSUM_PRIME: u64 = 0x00000100000001B3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebyeContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub optional_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}


pub struct DebyeModule;


impl DebyeModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<DebyeContract> {
        validate_request_shape(request)?;
        Ok(DebyeContract {
            required_inputs: artifact_list(&DEBYE_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&DEBYE_OPTIONAL_INPUTS),
            expected_outputs: artifact_list(&DEBYE_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for DebyeModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let ff2x_source = read_input_source(&request.input_path, DEBYE_REQUIRED_INPUTS[0])?;
        let paths_source = read_input_source(
            &input_dir.join(DEBYE_REQUIRED_INPUTS[1]),
            DEBYE_REQUIRED_INPUTS[1],
        )?;
        let feff_source = read_input_source(
            &input_dir.join(DEBYE_REQUIRED_INPUTS[2]),
            DEBYE_REQUIRED_INPUTS[2],
        )?;
        let spring_source = maybe_read_optional_input_source(
            input_dir.join(DEBYE_OPTIONAL_INPUTS[0]),
            DEBYE_OPTIONAL_INPUTS[0],
        )?;

        let model = DebyeModel::from_sources(
            &request.fixture_id,
            &ff2x_source,
            &paths_source,
            &feff_source,
            spring_source.as_deref(),
        )?;
        let outputs = artifact_list(&DEBYE_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.DEBYE_OUTPUT_DIRECTORY",
                format!(
                    "failed to create DEBYE output directory '{}': {}",
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
                        "IO.DEBYE_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create DEBYE artifact directory '{}': {}",
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
    use super::DebyeModule;
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn contract_exposes_required_and_optional_artifacts() {
        let request = ComputeRequest::new(
            "FX-DEBYE-001",
            ComputeModule::Debye,
            "ff2x.inp",
            "actual-output",
        );
        let scaffold = DebyeModule;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["ff2x.inp", "paths.dat", "feff.inp"])
        );
        assert_eq!(
            artifact_set(&contract.optional_inputs),
            expected_artifact_set(&["spring.inp"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&[
                "s2_em.dat",
                "s2_rm1.dat",
                "s2_rm2.dat",
                "xmu.dat",
                "chi.dat",
                "log6.dat",
                "spring.dat",
            ])
        );
    }

    #[test]
    fn execute_writes_true_compute_artifacts() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("outputs");
        stage_debye_inputs(&input_dir, true);

        let request = ComputeRequest::new(
            "FX-DEBYE-001",
            ComputeModule::Debye,
            input_dir.join("ff2x.inp"),
            &output_dir,
        );
        let artifacts = DebyeModule
            .execute(&request)
            .expect("DEBYE execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&[
                "s2_em.dat",
                "s2_rm1.dat",
                "s2_rm2.dat",
                "xmu.dat",
                "chi.dat",
                "log6.dat",
                "spring.dat",
            ])
        );

        for artifact in artifacts {
            let path = output_dir.join(&artifact.relative_path);
            assert!(path.is_file(), "artifact '{}' should exist", path.display());
            assert!(
                !fs::read(&path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "artifact '{}' should not be empty",
                path.display()
            );
        }
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_debye_inputs(&input_dir, true);

        let first_output = temp.path().join("first-output");
        let second_output = temp.path().join("second-output");

        let first_request = ComputeRequest::new(
            "FX-DEBYE-001",
            ComputeModule::Debye,
            input_dir.join("ff2x.inp"),
            &first_output,
        );
        let second_request = ComputeRequest::new(
            "FX-DEBYE-001",
            ComputeModule::Debye,
            input_dir.join("ff2x.inp"),
            &second_output,
        );

        let first_artifacts = DebyeModule
            .execute(&first_request)
            .expect("first DEBYE run should succeed");
        let second_artifacts = DebyeModule
            .execute(&second_request)
            .expect("second DEBYE run should succeed");

        assert_eq!(
            artifact_set(&first_artifacts),
            artifact_set(&second_artifacts)
        );

        for artifact in first_artifacts {
            let relative = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let first_bytes =
                fs::read(first_output.join(&artifact.relative_path)).expect("first output exists");
            let second_bytes = fs::read(second_output.join(&artifact.relative_path))
                .expect("second output exists");
            assert_eq!(
                first_bytes, second_bytes,
                "artifact '{}' should be deterministic",
                relative
            );
        }
    }

    #[test]
    fn execute_allows_missing_optional_spring_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("outputs");
        stage_debye_inputs(&input_dir, false);

        let request = ComputeRequest::new(
            "FX-DEBYE-001",
            ComputeModule::Debye,
            input_dir.join("ff2x.inp"),
            &output_dir,
        );
        DebyeModule
            .execute(&request)
            .expect("DEBYE execution without spring input should succeed");

        let spring_dat = fs::read_to_string(output_dir.join("spring.dat"))
            .expect("spring.dat should be written even without spring input");
        assert!(
            spring_dat.contains("spring_input_present = false"),
            "spring summary should capture missing optional input"
        );
    }

    #[test]
    fn execute_rejects_non_debye_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_debye_inputs(&input_dir, true);

        let request = ComputeRequest::new(
            "FX-DEBYE-001",
            ComputeModule::Dmdw,
            input_dir.join("ff2x.inp"),
            temp.path().join("out"),
        );
        let error = DebyeModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.DEBYE_MODULE");
    }

    #[test]
    fn execute_requires_paths_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input dir should exist");
        fs::write(input_dir.join("ff2x.inp"), FF2X_INPUT_FIXTURE).expect("ff2x should be staged");
        fs::write(input_dir.join("feff.inp"), FEFF_INPUT_FIXTURE).expect("feff should be staged");

        let request = ComputeRequest::new(
            "FX-DEBYE-001",
            ComputeModule::Debye,
            input_dir.join("ff2x.inp"),
            temp.path().join("out"),
        );
        let error = DebyeModule
            .execute(&request)
            .expect_err("missing paths.dat should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.DEBYE_INPUT_READ");
    }

    fn stage_debye_inputs(destination_dir: &Path, include_spring: bool) {
        fs::create_dir_all(destination_dir).expect("destination dir should exist");
        fs::write(destination_dir.join("ff2x.inp"), FF2X_INPUT_FIXTURE).expect("ff2x staged");
        fs::write(destination_dir.join("paths.dat"), PATHS_INPUT_FIXTURE).expect("paths staged");
        fs::write(destination_dir.join("feff.inp"), FEFF_INPUT_FIXTURE).expect("feff staged");
        if include_spring {
            fs::write(destination_dir.join("spring.inp"), SPRING_INPUT_FIXTURE)
                .expect("spring staged");
        }
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
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

    const FF2X_INPUT_FIXTURE: &str = "mchi, ispec, idwopt, ipr6, mbconv, absolu, iGammaCH
   1   0   2   0   0   0   0
vrcorr, vicorr, s02, critcw
      0.00000      0.00000      1.00000      4.00000
tk, thetad, alphat, thetae, sig2g
    450.00000    315.00000      0.00000      0.00000      0.00000
momentum transfer
      0.00000      0.00000      0.00000
 the number of decomposi
   -1
";

    const PATHS_INPUT_FIXTURE: &str =
        "PATH  Rmax= 8.000,  Keep_limit= 0.00, Heap_limit 0.00  Pwcrit= 2.50%
 -----------------------------------------------------------------------
     1    2  12.000  index, nleg, degeneracy, r=  2.5323
     2    3  48.000  index, nleg, degeneracy, r=  3.7984
     3    2  24.000  index, nleg, degeneracy, r=  4.3860
";

    const FEFF_INPUT_FIXTURE: &str = "TITLE Cu DEBYE RM Method
EDGE K
EXAFS 15.0
POTENTIALS
    0   29   Cu
    1   29   Cu
ATOMS
    0.00000    0.00000    0.00000    0   Cu  0.00000    0
    1.79059    0.00000    1.79059    1   Cu  2.53228    1
    0.00000    1.79059    1.79059    1   Cu  2.53228    2
END
";

    const SPRING_INPUT_FIXTURE: &str = "*\tres\twmax\tdosfit\tacut
 VDOS\t0.03\t0.5\t1

 STRETCHES
 *\ti\tj\tk_ij\tdR_ij (%)
\t0\t1\t27.9\t2.
";
}
