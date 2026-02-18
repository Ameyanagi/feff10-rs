mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::EelsModel;
use parser::{
    artifact_list, input_parent_dir, maybe_read_optional_input_source, read_input_source,
    validate_request_shape,
};

pub(crate) const EELS_REQUIRED_INPUTS: [&str; 2] = ["eels.inp", "xmu.dat"];
pub(crate) const EELS_OPTIONAL_INPUTS: [&str; 1] = ["magic.inp"];
pub(crate) const EELS_REQUIRED_OUTPUTS: [&str; 2] = ["eels.dat", "logeels.dat"];
pub(crate) const EELS_OPTIONAL_OUTPUT: &str = "magic.dat";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EelsContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub optional_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EelsModule;

impl EelsModule {
    pub fn contract_for_request(&self, request: &ComputeRequest) -> ComputeResult<EelsContract> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let eels_source = read_input_source(&request.input_path, EELS_REQUIRED_INPUTS[0])?;
        let xmu_source = read_input_source(
            &input_dir.join(EELS_REQUIRED_INPUTS[1]),
            EELS_REQUIRED_INPUTS[1],
        )?;
        let magic_source = maybe_read_optional_input_source(
            input_dir.join(EELS_OPTIONAL_INPUTS[0]),
            EELS_OPTIONAL_INPUTS[0],
        )?;
        let model = EelsModel::from_sources(
            &request.fixture_id,
            &eels_source,
            &xmu_source,
            magic_source.as_deref(),
        )?;

        Ok(EelsContract {
            required_inputs: artifact_list(&EELS_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&EELS_OPTIONAL_INPUTS),
            expected_outputs: model.expected_outputs(),
        })
    }
}

impl ModuleExecutor for EelsModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let eels_source = read_input_source(&request.input_path, EELS_REQUIRED_INPUTS[0])?;
        let xmu_source = read_input_source(
            &input_dir.join(EELS_REQUIRED_INPUTS[1]),
            EELS_REQUIRED_INPUTS[1],
        )?;
        let magic_source = maybe_read_optional_input_source(
            input_dir.join(EELS_OPTIONAL_INPUTS[0]),
            EELS_OPTIONAL_INPUTS[0],
        )?;
        let model = EelsModel::from_sources(
            &request.fixture_id,
            &eels_source,
            &xmu_source,
            magic_source.as_deref(),
        )?;
        let outputs = model.expected_outputs();

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.EELS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create EELS output directory '{}': {}",
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
                        "IO.EELS_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create EELS artifact directory '{}': {}",
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
    use super::EelsModule;
    use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, FeffErrorCategory};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const EELS_INPUT_NO_MAGIC: &str = "\
calculate ELNES?
   1
average? relativistic? cross-terms? Which input?
   0   1   1   1   4
polarizations to be used ; min step max
   1   1   9
beam energy in eV
 300000.00000
beam direction in arbitrary units
      0.00000      1.00000      0.00000
collection and convergence semiangle in rad
      0.00240      0.00000
qmesh - radial and angular grid size
   5   3
detector positions - two angles in rad
      0.00000      0.00000
calculate magic angle if magic=1
   0
energy for magic angle - eV above threshold
      0.00000
";

    const EELS_INPUT_WITH_MAGIC_FLAG: &str = "\
calculate ELNES?
   1
average? relativistic? cross-terms? Which input?
   1   1   1   1   4
polarizations to be used ; min step max
   1   1   9
beam energy in eV
 200000.00000
beam direction in arbitrary units
      0.00000      0.00000      1.00000
collection and convergence semiangle in rad
      0.00150      0.00030
qmesh - radial and angular grid size
   6   4
detector positions - two angles in rad
      0.00100      0.00200
calculate magic angle if magic=1
   1
energy for magic angle - eV above threshold
      15.00000
";

    const XMU_INPUT: &str = "\
# omega e k mu mu0 chi
8979.411 -16.773 -1.540 5.56205E-06 6.25832E-06 -6.96262E-07
8980.979 -15.204 -1.400 6.61771E-06 7.52318E-06 -9.05473E-07
8982.398 -13.786 -1.260 7.99662E-06 9.19560E-06 -1.19897E-06
8983.667 -12.516 -1.120 9.85468E-06 1.14689E-05 -1.61419E-06
";

    const MAGIC_INPUT: &str = "\
magic energy offset
12.5
angular tweak
0.45
";

    #[test]
    fn contract_exposes_required_inputs_and_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let contract = EelsModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            contract.required_inputs,
            artifact_list(&["eels.inp", "xmu.dat"])
        );
        assert_eq!(contract.optional_inputs, artifact_list(&["magic.inp"]));
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_set(false)
        );
    }

    #[test]
    fn contract_includes_magic_output_when_requested_by_input_flag() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_WITH_MAGIC_FLAG);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let contract = EelsModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(artifact_set(&contract.expected_outputs), expected_set(true));
    }

    #[test]
    fn execute_writes_true_compute_eels_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let artifacts = EelsModule
            .execute(&request)
            .expect("execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_set(false));
        assert!(temp.path().join("out/eels.dat").is_file());
        assert!(temp.path().join("out/logeels.dat").is_file());
    }

    #[test]
    fn execute_optional_magic_input_emits_magic_artifact() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);
        stage_text(temp.path().join("magic.inp"), MAGIC_INPUT);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let artifacts = EelsModule
            .execute(&request)
            .expect("execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_set(true));
        let magic_contents =
            fs::read_to_string(temp.path().join("out/magic.dat")).expect("magic.dat should exist");
        assert!(
            magic_contents.contains("magic_angle_mrad"),
            "magic output should include table header"
        );
    }

    #[test]
    fn execute_is_deterministic_for_identical_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);
        stage_text(temp.path().join("magic.inp"), MAGIC_INPUT);

        let first_request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out-first"),
        );
        let second_request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out-second"),
        );

        let first = EelsModule
            .execute(&first_request)
            .expect("first execution should succeed");
        let second = EelsModule
            .execute(&second_request)
            .expect("second execution should succeed");

        assert_eq!(artifact_set(&first), artifact_set(&second));
        for artifact in first {
            let first_bytes = fs::read(first_request.output_dir.join(&artifact.relative_path))
                .expect("first bytes");
            let second_bytes = fs::read(second_request.output_dir.join(&artifact.relative_path))
                .expect("second bytes");
            assert_eq!(
                first_bytes,
                second_bytes,
                "artifact '{}' should be deterministic",
                artifact.relative_path.display()
            );
        }
    }

    #[test]
    fn execute_rejects_non_eels_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Ldos,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let error = EelsModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.EELS_MODULE");
    }

    #[test]
    fn execute_requires_xmu_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("eels.inp"), EELS_INPUT_NO_MAGIC);

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            temp.path().join("eels.inp"),
            temp.path().join("out"),
        );
        let error = EelsModule
            .execute(&request)
            .expect_err("missing xmu should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.EELS_INPUT_READ");
    }

    fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
        paths.iter().copied().map(ComputeArtifact::new).collect()
    }

    fn expected_set(include_magic: bool) -> BTreeSet<String> {
        let mut outputs = BTreeSet::from(["eels.dat".to_string(), "logeels.dat".to_string()]);
        if include_magic {
            outputs.insert("magic.dat".to_string());
        }
        outputs
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn stage_text(destination: PathBuf, contents: &str) {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should exist");
        }
        fs::write(destination, contents).expect("text input should be written");
    }
}
