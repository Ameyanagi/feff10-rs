mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::ScreenModel;
use parser::{
    artifact_list, input_parent_dir, maybe_read_optional_input_source, read_input_source,
    validate_request_shape,
};

pub(crate) const SCREEN_REQUIRED_INPUTS: [&str; 3] = ["pot.inp", "geom.dat", "ldos.inp"];
pub(crate) const SCREEN_OPTIONAL_INPUTS: [&str; 1] = ["screen.inp"];
pub(crate) const SCREEN_REQUIRED_OUTPUTS: [&str; 2] = ["wscrn.dat", "logscreen.dat"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub optional_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenModule;

impl ScreenModule {
    pub fn contract_for_request(&self, request: &ComputeRequest) -> ComputeResult<ScreenContract> {
        validate_request_shape(request)?;
        Ok(ScreenContract {
            required_inputs: artifact_list(&SCREEN_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&SCREEN_OPTIONAL_INPUTS),
            expected_outputs: artifact_list(&SCREEN_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for ScreenModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let pot_source = read_input_source(&request.input_path, SCREEN_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(SCREEN_REQUIRED_INPUTS[1]),
            SCREEN_REQUIRED_INPUTS[1],
        )?;
        let ldos_source = read_input_source(
            &input_dir.join(SCREEN_REQUIRED_INPUTS[2]),
            SCREEN_REQUIRED_INPUTS[2],
        )?;
        let screen_source = maybe_read_optional_input_source(
            input_dir.join(SCREEN_OPTIONAL_INPUTS[0]),
            SCREEN_OPTIONAL_INPUTS[0],
        )?;

        let model = ScreenModel::from_sources(
            &request.fixture_id,
            &pot_source,
            &geom_source,
            &ldos_source,
            screen_source.as_deref(),
        )?;
        let outputs = artifact_list(&SCREEN_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.SCREEN_OUTPUT_DIRECTORY",
                format!(
                    "failed to create SCREEN output directory '{}': {}",
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
                        "IO.SCREEN_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create SCREEN artifact directory '{}': {}",
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
    use super::ScreenModule;
    use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, FeffErrorCategory};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const POT_INPUT_FIXTURE: &str = "mpot, nph, ntitle, ihole, ipr1, iafolp, ixc,ispec
   1   1   1   1   0   0   0   1
nmix, nohole, jumprm, inters, nscmt, icoul, lfms1, iunf
   1   2   0   0 100   0   0   0
Cu crystal
gamach, rgrd, ca1, ecv, totvol, rfms1
      1.72919      0.05000      0.20000    -40.00000      0.00000      4.00000
 iz, lmaxsc, xnatph, xion, folp
   29    2      1.00000      0.00000      1.15000
   29    3      1.00000      0.10000      1.35000
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

    const LDOS_INPUT_FIXTURE: &str = "mldos, lfms2, ixc, ispin, minv, neldos
   0   0   0   0   0     101
rfms2, emin, emax, eimag, rgrd
      6.00000   1000.00000      0.00000     -1.00000      0.05000
rdirec, toler1, toler2
     12.00000      0.00100      0.00100
 lmaxph(0:nph)
   3   3
";

    const SCREEN_OVERRIDE_FIXTURE: &str = "ner          40
nei          20
maxl           4
irrh           1
iend           0
lfxc           0
emin  -40.0000000000000
emax  0.000000000000000E+000
eimax   2.00000000000000
ermin  1.000000000000000E-003
rfms   4.00000000000000
nrptx0         251
";

    #[test]
    fn contract_exposes_true_compute_screen_artifact_contract() {
        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            "pot.inp",
            "actual-output",
        );
        let scaffold = ScreenModule;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_set(&["pot.inp", "geom.dat", "ldos.inp"])
        );
        assert_eq!(
            artifact_set(&contract.optional_inputs),
            expected_set(&["screen.inp"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_set(&["wscrn.dat", "logscreen.dat"])
        );
    }

    #[test]
    fn execute_emits_required_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");
        let input_path = stage_screen_inputs(temp.path(), Some(SCREEN_OVERRIDE_FIXTURE));

        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &input_path,
            &output_dir,
        );
        let artifacts = ScreenModule
            .execute(&request)
            .expect("SCREEN execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_set(&["wscrn.dat", "logscreen.dat"])
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
    fn execute_allows_missing_optional_screen_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");
        let input_path = stage_screen_inputs(temp.path(), None);

        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &input_path,
            &output_dir,
        );
        let artifacts = ScreenModule
            .execute(&request)
            .expect("SCREEN execution should succeed without screen.inp");

        assert_eq!(
            artifact_set(&artifacts),
            expected_set(&["wscrn.dat", "logscreen.dat"])
        );
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_root = temp.path().join("first");
        let second_root = temp.path().join("second");
        let first_input = stage_screen_inputs(&first_root, Some(SCREEN_OVERRIDE_FIXTURE));
        let second_input = stage_screen_inputs(&second_root, Some(SCREEN_OVERRIDE_FIXTURE));

        let first_output = first_root.join("out");
        let second_output = second_root.join("out");

        let first_request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &first_input,
            &first_output,
        );
        let second_request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &second_input,
            &second_output,
        );

        ScreenModule
            .execute(&first_request)
            .expect("first run should succeed");
        ScreenModule
            .execute(&second_request)
            .expect("second run should succeed");

        for artifact in ["wscrn.dat", "logscreen.dat"] {
            let first_bytes =
                fs::read(first_output.join(artifact)).expect("first output should exist");
            let second_bytes =
                fs::read(second_output.join(artifact)).expect("second output should exist");
            assert_eq!(
                first_bytes, second_bytes,
                "artifact '{}' should be deterministic across runs",
                artifact
            );
        }
    }

    #[test]
    fn execute_optional_screen_input_changes_screen_response() {
        let temp = TempDir::new().expect("tempdir should be created");

        let with_override_root = temp.path().join("with-override");
        let without_override_root = temp.path().join("without-override");
        let with_override_input =
            stage_screen_inputs(&with_override_root, Some(SCREEN_OVERRIDE_FIXTURE));
        let without_override_input = stage_screen_inputs(&without_override_root, None);

        let with_override_output = with_override_root.join("out");
        let without_override_output = without_override_root.join("out");

        let with_override_request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &with_override_input,
            &with_override_output,
        );
        let without_override_request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &without_override_input,
            &without_override_output,
        );

        ScreenModule
            .execute(&with_override_request)
            .expect("override run should succeed");
        ScreenModule
            .execute(&without_override_request)
            .expect("default run should succeed");

        let with_override =
            fs::read(with_override_output.join("wscrn.dat")).expect("override wscrn should exist");
        let without_override = fs::read(without_override_output.join("wscrn.dat"))
            .expect("default wscrn should exist");
        assert_ne!(
            with_override, without_override,
            "optional screen.inp should influence computed wscrn.dat"
        );
    }

    #[test]
    fn execute_rejects_non_screen_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = stage_screen_inputs(temp.path(), Some(SCREEN_OVERRIDE_FIXTURE));

        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Crpa,
            &input_path,
            temp.path(),
        );
        let error = ScreenModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.SCREEN_MODULE");
    }

    #[test]
    fn execute_requires_geom_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        fs::write(&input_path, POT_INPUT_FIXTURE).expect("pot input should be staged");
        fs::write(temp.path().join("ldos.inp"), LDOS_INPUT_FIXTURE)
            .expect("ldos input should be staged");

        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &input_path,
            temp.path(),
        );
        let error = ScreenModule
            .execute(&request)
            .expect_err("missing geom input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.SCREEN_INPUT_READ");
    }

    #[test]
    fn execute_rejects_invalid_ldos_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        fs::write(&input_path, POT_INPUT_FIXTURE).expect("pot input should be staged");
        fs::write(temp.path().join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be staged");
        fs::write(temp.path().join("ldos.inp"), "invalid ldos input\n")
            .expect("ldos input should be staged");

        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &input_path,
            temp.path(),
        );
        let error = ScreenModule
            .execute(&request)
            .expect_err("invalid ldos should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.SCREEN_INPUT_PARSE");
    }

    fn stage_screen_inputs(root: &Path, screen_override: Option<&str>) -> PathBuf {
        fs::create_dir_all(root).expect("root directory should exist");
        let pot_path = root.join("pot.inp");
        fs::write(&pot_path, POT_INPUT_FIXTURE).expect("pot input should be written");
        fs::write(root.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom input should be written");
        fs::write(root.join("ldos.inp"), LDOS_INPUT_FIXTURE).expect("ldos input should be written");

        if let Some(source) = screen_override {
            fs::write(root.join("screen.inp"), source).expect("screen override should be written");
        }

        pot_path
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
