mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::SelfModel;
use parser::{
    artifact_list, input_parent_dir, load_staged_spectrum_sources,
    maybe_read_optional_input_source, read_input_source, validate_request_shape,
};

pub(crate) const SELF_PRIMARY_INPUT: &str = "sfconv.inp";
pub(crate) const SELF_SPECTRUM_INPUT_CANDIDATES: [&str; 3] = ["xmu.dat", "chi.dat", "loss.dat"];
pub(crate) const SELF_OPTIONAL_INPUTS: [&str; 1] = ["exc.dat"];
pub(crate) const SELF_REQUIRED_OUTPUTS: [&str; 7] = [
    "selfenergy.dat",
    "sigma.dat",
    "specfunct.dat",
    "logsfconv.dat",
    "sig2FEFF.dat",
    "mpse.dat",
    "opconsCu.dat",
];
pub(crate) const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
pub(crate) const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfEnergyContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub optional_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelfEnergyModule;

impl SelfEnergyModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<SelfEnergyContract> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let sfconv_source = read_input_source(&request.input_path, SELF_PRIMARY_INPUT)?;
        let spectrum_sources = load_staged_spectrum_sources(input_dir)?;
        let exc_source = maybe_read_optional_input_source(
            input_dir.join(SELF_OPTIONAL_INPUTS[0]),
            SELF_OPTIONAL_INPUTS[0],
        )?;
        let model = SelfModel::from_sources(
            &request.fixture_id,
            &sfconv_source,
            spectrum_sources,
            exc_source.as_deref(),
        )?;

        let mut required_inputs = vec![ComputeArtifact::new(SELF_PRIMARY_INPUT)];
        required_inputs.extend(
            model
                .spectrum_artifact_names()
                .iter()
                .map(ComputeArtifact::new),
        );

        Ok(SelfEnergyContract {
            required_inputs,
            optional_inputs: artifact_list(&SELF_OPTIONAL_INPUTS),
            expected_outputs: model.expected_outputs(),
        })
    }
}

impl ModuleExecutor for SelfEnergyModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let sfconv_source = read_input_source(&request.input_path, SELF_PRIMARY_INPUT)?;
        let spectrum_sources = load_staged_spectrum_sources(input_dir)?;
        let exc_source = maybe_read_optional_input_source(
            input_dir.join(SELF_OPTIONAL_INPUTS[0]),
            SELF_OPTIONAL_INPUTS[0],
        )?;
        let model = SelfModel::from_sources(
            &request.fixture_id,
            &sfconv_source,
            spectrum_sources,
            exc_source.as_deref(),
        )?;
        let outputs = model.expected_outputs();
        let state = model.compute_state()?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.SELF_OUTPUT_DIRECTORY",
                format!(
                    "failed to create SELF output directory '{}': {}",
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
                        "IO.SELF_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create SELF artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            let artifact_name = artifact.relative_path.to_string_lossy().replace('\\', "/");
            model.write_artifact(&artifact_name, &output_path, &state)?;
        }

        Ok(outputs)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SELF_OPTIONAL_INPUTS, SELF_PRIMARY_INPUT, SELF_REQUIRED_OUTPUTS, SelfEnergyModule,
    };
    use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, FeffErrorCategory};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const SFCONV_INPUT_FIXTURE: &str = "msfconv, ipse, ipsk
   1   0   0
wsigk, cen
      0.00000      0.00000
ispec, ipr6
   1   0
cfname
NULL
";

    const XMU_INPUT_FIXTURE: &str = "# omega e k mu mu0 chi
    8979.411  -16.765  -1.406  1.46870E-02  1.79897E-02 -3.30270E-03
    8980.979  -15.197  -1.252  2.93137E-02  3.59321E-02 -6.61845E-03
    8982.398  -13.778  -1.093  3.93900E-02  4.92748E-02 -9.88483E-03
";

    const LOSS_INPUT_FIXTURE: &str = "# E(eV) Loss
  2.50658E-03 2.58411E-02
  4.69344E-03 6.11057E-02
  7.56059E-03 1.37874E-01
";

    const FEFF_INPUT_FIXTURE: &str = " 1.00000E+00 3.00000E-01
 2.00000E+00 2.00000E-01
 3.00000E+00 1.00000E-01
";

    const EXC_INPUT_FIXTURE: &str =
        "  0.1414210018E-01  0.1000000000E+00  0.8481210460E-01  0.9420256311E-01
  0.2467626159E-01  0.1000000000E+00  0.5134531114E-01  0.9951100800E-01
  0.4683986560E-01  0.1000000000E+00  0.1271572855E-01  0.4677866877E-01
";

    #[test]
    fn contract_requires_staged_spectrum_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        fs::write(&input_path, SFCONV_INPUT_FIXTURE).expect("sfconv input should be written");

        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::SelfEnergy,
            &input_path,
            temp.path().join("out"),
        );
        let error = SelfEnergyModule
            .contract_for_request(&request)
            .expect_err("missing spectra should fail contract");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.SELF_SPECTRUM_INPUT");
    }

    #[test]
    fn contract_reflects_staged_spectrum_and_output_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        fs::write(&input_path, SFCONV_INPUT_FIXTURE).expect("sfconv input should be written");
        fs::write(temp.path().join("xmu.dat"), XMU_INPUT_FIXTURE).expect("xmu should be written");
        fs::write(temp.path().join("loss.dat"), LOSS_INPUT_FIXTURE)
            .expect("loss should be written");

        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::SelfEnergy,
            &input_path,
            temp.path().join("out"),
        );
        let contract = SelfEnergyModule
            .contract_for_request(&request)
            .expect("contract should build");

        let required_inputs = artifact_set(&contract.required_inputs);
        assert!(required_inputs.contains(SELF_PRIMARY_INPUT));
        assert!(required_inputs.contains("xmu.dat"));
        assert!(required_inputs.contains("loss.dat"));

        assert_eq!(contract.optional_inputs.len(), 1);
        assert_eq!(
            contract.optional_inputs[0].relative_path.to_string_lossy(),
            SELF_OPTIONAL_INPUTS[0]
        );

        let expected_outputs = artifact_set(&contract.expected_outputs);
        for required in SELF_REQUIRED_OUTPUTS {
            assert!(expected_outputs.contains(required));
        }
        assert!(expected_outputs.contains("xmu.dat"));
        assert!(expected_outputs.contains("loss.dat"));
    }

    #[test]
    fn execute_emits_required_outputs_and_rewrites_staged_spectra() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        let output_dir = temp.path().join("out");

        fs::write(&input_path, SFCONV_INPUT_FIXTURE).expect("sfconv input should be written");
        fs::write(temp.path().join("xmu.dat"), XMU_INPUT_FIXTURE).expect("xmu should be written");
        fs::write(temp.path().join(SELF_OPTIONAL_INPUTS[0]), EXC_INPUT_FIXTURE)
            .expect("exc input should be written");

        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::SelfEnergy,
            &input_path,
            &output_dir,
        );
        let artifacts = SelfEnergyModule
            .execute(&request)
            .expect("SELF execution should succeed");

        let emitted = artifact_set(&artifacts);
        for required in SELF_REQUIRED_OUTPUTS {
            assert!(
                emitted.contains(required),
                "missing required output '{}'",
                required
            );
        }
        assert!(emitted.contains("xmu.dat"));

        for artifact in &artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            assert!(
                output_path.is_file(),
                "artifact '{}' should exist",
                output_path.display()
            );
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "artifact '{}' should not be empty",
                output_path.display()
            );
        }

        let log = fs::read_to_string(output_dir.join("logsfconv.dat"))
            .expect("logsfconv.dat should be readable");
        assert!(log.contains("status = success"));
    }

    #[test]
    fn execute_accepts_feff_spectrum_inputs_when_named_spectra_are_absent() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        fs::write(&input_path, SFCONV_INPUT_FIXTURE).expect("sfconv input should be written");
        fs::write(temp.path().join("feff0001.dat"), FEFF_INPUT_FIXTURE)
            .expect("feff spectrum should be written");

        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::SelfEnergy,
            &input_path,
            temp.path().join("out"),
        );
        let artifacts = SelfEnergyModule
            .execute(&request)
            .expect("SELF execution should accept feffNNNN spectrum input");
        let emitted = artifact_set(&artifacts);

        assert!(emitted.contains("feff0001.dat"));
        assert!(emitted.contains("selfenergy.dat"));
        assert!(emitted.contains("sigma.dat"));
    }

    #[test]
    fn execute_rejects_non_self_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join(SELF_PRIMARY_INPUT);
        fs::write(&input_path, SFCONV_INPUT_FIXTURE).expect("sfconv input should be written");
        fs::write(temp.path().join("xmu.dat"), XMU_INPUT_FIXTURE).expect("xmu should be written");

        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::Screen,
            &input_path,
            temp.path().join("out"),
        );
        let error = SelfEnergyModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.SELF_MODULE");
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_output = run_self_case(temp.path(), "first");
        let second_output = run_self_case(temp.path(), "second");

        for artifact in expected_artifact_set(&["xmu.dat", "loss.dat"]) {
            let first =
                fs::read(first_output.join(&artifact)).expect("first artifact should exist");
            let second =
                fs::read(second_output.join(&artifact)).expect("second artifact should exist");
            assert_eq!(
                first, second,
                "artifact '{}' should be deterministic",
                artifact
            );
        }
    }

    fn run_self_case(root: &Path, subdir: &str) -> PathBuf {
        let case_root = root.join(subdir);
        fs::create_dir_all(&case_root).expect("case root should exist");
        fs::write(case_root.join(SELF_PRIMARY_INPUT), SFCONV_INPUT_FIXTURE)
            .expect("sfconv input should be written");
        fs::write(case_root.join("xmu.dat"), XMU_INPUT_FIXTURE).expect("xmu should be written");
        fs::write(case_root.join("loss.dat"), LOSS_INPUT_FIXTURE).expect("loss should be written");
        fs::write(case_root.join(SELF_OPTIONAL_INPUTS[0]), EXC_INPUT_FIXTURE)
            .expect("exc should be written");

        let output_dir = case_root.join("out");
        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::SelfEnergy,
            case_root.join(SELF_PRIMARY_INPUT),
            &output_dir,
        );
        let artifacts = SelfEnergyModule
            .execute(&request)
            .expect("SELF execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["xmu.dat", "loss.dat"]),
            "SELF output artifact set should match expected contract"
        );
        output_dir
    }

    fn expected_artifact_set(spectrum_artifacts: &[&str]) -> BTreeSet<String> {
        let mut artifacts: BTreeSet<String> = SELF_REQUIRED_OUTPUTS
            .iter()
            .map(|artifact| artifact.to_string())
            .collect();
        artifacts.extend(
            spectrum_artifacts
                .iter()
                .map(|artifact| artifact.to_string()),
        );
        artifacts
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }
}
