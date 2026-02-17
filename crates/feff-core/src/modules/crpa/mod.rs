mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::CrpaModel;
use parser::{
    artifact_list, input_parent_dir, maybe_read_optional_input_source, read_input_source,
    validate_request_shape,
};

pub(crate) const CRPA_REQUIRED_INPUTS: [&str; 3] = ["crpa.inp", "pot.inp", "geom.dat"];
pub(crate) const CRPA_OPTIONAL_INPUTS: [&str; 2] = ["wscrn.dat", "logscreen.dat"];
pub(crate) const CRPA_REQUIRED_OUTPUTS: [&str; 2] = ["wscrn.dat", "logscrn.dat"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrpaContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CrpaModule;

impl CrpaModule {
    pub fn contract_for_request(&self, request: &ComputeRequest) -> ComputeResult<CrpaContract> {
        validate_request_shape(request)?;
        Ok(CrpaContract {
            required_inputs: artifact_list(&CRPA_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&CRPA_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for CrpaModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let crpa_source = read_input_source(&request.input_path, CRPA_REQUIRED_INPUTS[0])?;
        let pot_source = read_input_source(
            &input_dir.join(CRPA_REQUIRED_INPUTS[1]),
            CRPA_REQUIRED_INPUTS[1],
        )?;
        let geom_source = read_input_source(
            &input_dir.join(CRPA_REQUIRED_INPUTS[2]),
            CRPA_REQUIRED_INPUTS[2],
        )?;
        let screen_wscrn_source = maybe_read_optional_input_source(
            input_dir.join(CRPA_OPTIONAL_INPUTS[0]),
            CRPA_OPTIONAL_INPUTS[0],
        )?;
        let screen_log_source = maybe_read_optional_input_source(
            input_dir.join(CRPA_OPTIONAL_INPUTS[1]),
            CRPA_OPTIONAL_INPUTS[1],
        )?;

        let model = CrpaModel::from_sources(
            &request.fixture_id,
            &crpa_source,
            &pot_source,
            &geom_source,
            screen_wscrn_source.as_deref(),
            screen_log_source.as_deref(),
        )?;
        let outputs = artifact_list(&CRPA_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.CRPA_OUTPUT_DIRECTORY",
                format!(
                    "failed to create CRPA output directory '{}': {}",
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
                        "IO.CRPA_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create CRPA artifact directory '{}': {}",
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
    use super::{CRPA_OPTIONAL_INPUTS, CrpaModule};
    use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, FeffErrorCategory};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const CRPA_INPUT_FIXTURE: &str = " do_CRPA           1
 rcut   1.49000000000000
 l_crpa           3
";

    const POT_INPUT_FIXTURE: &str = "mpot, nph, ntitle, ihole, ipr1, iafolp, ixc,ispec
   1   1   1   4   0   0   0   1
nmix, nohole, jumprm, inters, nscmt, icoul, lfms1, iunf
   1  -1   0   0 100   0   0   1
Ce example
gamach, rgrd, ca1, ecv, totvol, rfms1, corval_emin
      3.26955      0.05000      0.20000    -40.00000      0.00000      4.00000    -70.00000
 iz, lmaxsc, xnatph, xion, folp
   58    3      1.00000      0.00000      1.15000
   58    3    100.00000      0.00000      1.15000
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

    const SCREEN_WSCRN_FIXTURE: &str = "# r       w_scrn(r)      v_ch(r)
  1.0000000000E-04  2.5000000000E-01  3.1000000000E-01
  1.7500000000E-01  2.2500000000E-01  2.9000000000E-01
  3.5000000000E-01  1.9000000000E-01  2.5500000000E-01
  5.2500000000E-01  1.5500000000E-01  2.1000000000E-01
  7.0000000000E-01  1.2500000000E-01  1.8000000000E-01
";

    const SCREEN_LOG_FIXTURE: &str = "SCREEN true-compute runtime
fixture: FX-SCREEN-001
";

    #[test]
    fn contract_exposes_true_compute_crpa_artifact_contract() {
        let request = ComputeRequest::new(
            "FX-CRPA-001",
            ComputeModule::Crpa,
            "crpa.inp",
            "actual-output",
        );
        let scaffold = CrpaModule;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_set(&["crpa.inp", "pot.inp", "geom.dat"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_set(&["wscrn.dat", "logscrn.dat"])
        );
    }

    #[test]
    fn execute_emits_required_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");
        let input_path = stage_crpa_inputs(temp.path(), CRPA_INPUT_FIXTURE);

        let request =
            ComputeRequest::new("FX-CRPA-001", ComputeModule::Crpa, &input_path, &output_dir);
        let artifacts = CrpaModule
            .execute(&request)
            .expect("CRPA execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_set(&["wscrn.dat", "logscrn.dat"])
        );
        for artifact in artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            assert!(
                output_path.is_file(),
                "output artifact '{}' should exist",
                output_path.display()
            );
            assert!(
                !fs::read(&output_path)
                    .expect("output artifact should be readable")
                    .is_empty(),
                "output artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_root = temp.path().join("first");
        let second_root = temp.path().join("second");
        let first_input = stage_crpa_inputs(&first_root, CRPA_INPUT_FIXTURE);
        let second_input = stage_crpa_inputs(&second_root, CRPA_INPUT_FIXTURE);
        let first_output = first_root.join("out");
        let second_output = second_root.join("out");

        let first_request = ComputeRequest::new(
            "FX-CRPA-001",
            ComputeModule::Crpa,
            &first_input,
            &first_output,
        );
        let second_request = ComputeRequest::new(
            "FX-CRPA-001",
            ComputeModule::Crpa,
            &second_input,
            &second_output,
        );

        CrpaModule
            .execute(&first_request)
            .expect("first run should succeed");
        CrpaModule
            .execute(&second_request)
            .expect("second run should succeed");

        for artifact in ["wscrn.dat", "logscrn.dat"] {
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
    fn execute_optional_screen_wscrn_changes_crpa_response() {
        let temp = TempDir::new().expect("tempdir should be created");
        let with_screen_root = temp.path().join("with-screen-wscrn");
        let without_screen_root = temp.path().join("without-screen-wscrn");
        let with_screen_input = stage_crpa_inputs(&with_screen_root, CRPA_INPUT_FIXTURE);
        let without_screen_input = stage_crpa_inputs(&without_screen_root, CRPA_INPUT_FIXTURE);
        fs::write(
            with_screen_root.join(CRPA_OPTIONAL_INPUTS[0]),
            SCREEN_WSCRN_FIXTURE,
        )
        .expect("optional wscrn input should be written");
        fs::write(
            with_screen_root.join(CRPA_OPTIONAL_INPUTS[1]),
            SCREEN_LOG_FIXTURE,
        )
        .expect("optional logscreen input should be written");

        let with_screen_output = with_screen_root.join("out");
        let without_screen_output = without_screen_root.join("out");
        let with_screen_request = ComputeRequest::new(
            "FX-CRPA-001",
            ComputeModule::Crpa,
            &with_screen_input,
            &with_screen_output,
        );
        let without_screen_request = ComputeRequest::new(
            "FX-CRPA-001",
            ComputeModule::Crpa,
            &without_screen_input,
            &without_screen_output,
        );

        CrpaModule
            .execute(&with_screen_request)
            .expect("CRPA run with optional wscrn.dat should succeed");
        CrpaModule
            .execute(&without_screen_request)
            .expect("CRPA run without optional wscrn.dat should succeed");

        let with_screen = fs::read(with_screen_output.join("wscrn.dat"))
            .expect("wscrn output with optional screen data should exist");
        let without_screen = fs::read(without_screen_output.join("wscrn.dat"))
            .expect("wscrn output without optional screen data should exist");
        assert_ne!(
            with_screen, without_screen,
            "optional SCREEN wscrn.dat should influence CRPA wscrn.dat output"
        );
    }

    #[test]
    fn execute_rejects_non_crpa_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = stage_crpa_inputs(temp.path(), CRPA_INPUT_FIXTURE);

        let request = ComputeRequest::new(
            "FX-CRPA-001",
            ComputeModule::Screen,
            &input_path,
            temp.path(),
        );
        let error = CrpaModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.CRPA_MODULE");
    }

    #[test]
    fn execute_requires_pot_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("crpa.inp");
        fs::write(&input_path, CRPA_INPUT_FIXTURE).expect("crpa input should be staged");
        fs::write(temp.path().join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be staged");

        let request =
            ComputeRequest::new("FX-CRPA-001", ComputeModule::Crpa, &input_path, temp.path());
        let error = CrpaModule
            .execute(&request)
            .expect_err("missing pot input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.CRPA_INPUT_READ");
    }

    #[test]
    fn execute_rejects_disabled_crpa_flag() {
        let temp = TempDir::new().expect("tempdir should be created");
        let disabled_crpa_input = " do_CRPA           0
 rcut   1.49000000000000
 l_crpa           3
";
        let input_path = stage_crpa_inputs(temp.path(), disabled_crpa_input);

        let request =
            ComputeRequest::new("FX-CRPA-001", ComputeModule::Crpa, &input_path, temp.path());
        let error = CrpaModule
            .execute(&request)
            .expect_err("disabled CRPA flag should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.CRPA_INPUT_PARSE");
    }

    fn stage_crpa_inputs(root: &Path, crpa_input_source: &str) -> PathBuf {
        fs::create_dir_all(root).expect("root should exist");
        let crpa_path = root.join("crpa.inp");
        fs::write(&crpa_path, crpa_input_source).expect("crpa input should be written");
        fs::write(root.join("pot.inp"), POT_INPUT_FIXTURE).expect("pot input should be written");
        fs::write(root.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom input should be written");
        crpa_path
    }

    fn expected_set(entries: &[&str]) -> BTreeSet<String> {
        entries.iter().map(|entry| entry.to_string()).collect()
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }
}
