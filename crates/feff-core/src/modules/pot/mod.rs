mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::PotModel;
use parser::{artifact_list, geom_input_path, read_input_source, validate_request_shape};

pub(crate) const POT_REQUIRED_INPUTS: [&str; 2] = ["pot.inp", "geom.dat"];
pub(crate) const POT_REQUIRED_OUTPUTS: [&str; 5] = [
    "pot.bin",
    "pot.dat",
    "log1.dat",
    "convergence.scf",
    "convergence.scf.fine",
];
pub const POT_BINARY_MAGIC: &[u8; 8] = b"POTBIN10";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PotModule;

impl PotModule {
    pub fn contract_for_request(&self, request: &ComputeRequest) -> ComputeResult<PotContract> {
        validate_request_shape(request)?;
        Ok(PotContract {
            required_inputs: artifact_list(&POT_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&POT_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for PotModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;

        let pot_inp_source = read_input_source(&request.input_path, POT_REQUIRED_INPUTS[0])?;
        let geom_path = geom_input_path(request)?;
        let geom_source = read_input_source(&geom_path, POT_REQUIRED_INPUTS[1])?;
        let model = PotModel::from_sources(&request.fixture_id, &pot_inp_source, &geom_source)?;
        let outputs = artifact_list(&POT_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.POT_OUTPUT_DIRECTORY",
                format!(
                    "failed to create POT output directory '{}': {}",
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
                        "IO.POT_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create POT artifact directory '{}': {}",
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
    use super::{PotModule, POT_BINARY_MAGIC};
    use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, FeffErrorCategory};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_exposes_required_inputs_and_outputs() {
        let request =
            ComputeRequest::new("FX-POT-001", ComputeModule::Pot, "pot.inp", "actual-output");
        let scaffold = PotModule;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 2);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("pot.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("geom.dat")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_pot_artifact_set()
        );
    }

    #[test]
    fn execute_writes_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("actual");
        stage_pot_inputs(&input_path, &temp.path().join("geom.dat"));

        let request =
            ComputeRequest::new("FX-POT-001", ComputeModule::Pot, &input_path, &output_dir);
        let scaffold = PotModule;
        let artifacts = scaffold
            .execute(&request)
            .expect("POT execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_pot_artifact_set());
        for artifact in artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            assert!(output_path.is_file(), "artifact should exist");
        }

        let pot_binary = fs::read(output_dir.join("pot.bin")).expect("pot.bin should be readable");
        assert!(
            pot_binary.starts_with(POT_BINARY_MAGIC),
            "pot.bin should use true-compute binary header"
        );

        let pot_dat =
            fs::read_to_string(output_dir.join("pot.dat")).expect("pot.dat should be readable");
        assert!(pot_dat.contains("POT true-compute summary"));
        assert!(pot_dat.contains("index iz lmaxsc"));
    }

    #[test]
    fn execute_populates_finite_atom_exchange_metrics() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("actual");
        stage_pot_inputs(&input_path, &temp.path().join("geom.dat"));

        let request =
            ComputeRequest::new("FX-POT-001", ComputeModule::Pot, &input_path, &output_dir);
        PotModule
            .execute(&request)
            .expect("POT execution should succeed");

        let pot_dat =
            fs::read_to_string(output_dir.join("pot.dat")).expect("pot.dat should be readable");
        let metric_rows = pot_dat
            .lines()
            .filter(|line| {
                line.trim_start()
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_digit())
            })
            .collect::<Vec<_>>();
        assert!(
            !metric_rows.is_empty(),
            "pot.dat should include numeric potential metrics"
        );

        for row in metric_rows {
            let columns = row.split_whitespace().collect::<Vec<_>>();
            assert!(
                columns.len() >= 10,
                "expected at least 10 columns in potential metrics row '{}'",
                row
            );
            for value in &columns[6..10] {
                let parsed = value.parse::<f64>().unwrap_or(f64::NAN);
                assert!(
                    parsed.is_finite(),
                    "metric value '{}' should be finite in row '{}'",
                    value,
                    row
                );
            }
        }
    }

    #[test]
    fn execute_is_deterministic_for_same_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let geom_path = temp.path().join("geom.dat");
        let output_a = temp.path().join("out-a");
        let output_b = temp.path().join("out-b");
        stage_pot_inputs(&input_path, &geom_path);

        let request_a =
            ComputeRequest::new("FX-POT-001", ComputeModule::Pot, &input_path, &output_a);
        let request_b =
            ComputeRequest::new("FX-POT-001", ComputeModule::Pot, &input_path, &output_b);

        PotModule
            .execute(&request_a)
            .expect("first execution should succeed");
        PotModule
            .execute(&request_b)
            .expect("second execution should succeed");

        for artifact in [
            "pot.bin",
            "pot.dat",
            "log1.dat",
            "convergence.scf",
            "convergence.scf.fine",
        ] {
            let first = fs::read(output_a.join(artifact)).expect("first output should be readable");
            let second =
                fs::read(output_b.join(artifact)).expect("second output should be readable");
            assert_eq!(
                first, second,
                "artifact '{}' should be deterministic",
                artifact
            );
        }
    }

    #[test]
    fn execute_rejects_non_pot_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        stage_pot_inputs(&input_path, &temp.path().join("geom.dat"));

        let request = ComputeRequest::new(
            "FX-RDINP-001",
            ComputeModule::Rdinp,
            &input_path,
            temp.path(),
        );
        let scaffold = PotModule;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.POT_MODULE");
    }

    #[test]
    fn execute_requires_geom_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        fs::write(&input_path, pot_input_fixture()).expect("input should be written");

        let request =
            ComputeRequest::new("FX-POT-001", ComputeModule::Pot, &input_path, temp.path());
        let scaffold = PotModule;
        let error = scaffold
            .execute(&request)
            .expect_err("missing geom input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.POT_INPUT_READ");
    }

    #[test]
    fn execute_rejects_invalid_pot_input_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("actual");
        fs::write(&input_path, "BROKEN POT INPUT\n").expect("pot input should be written");
        fs::write(
            temp.path().join("geom.dat"),
            "nat, nph =    1    1\n 1 1\n iat x y z iph\n ---\n 1 0 0 0 0 1\n",
        )
        .expect("geom input should be written");

        let request =
            ComputeRequest::new("FX-POT-001", ComputeModule::Pot, &input_path, &output_dir);
        let scaffold = PotModule;
        let error = scaffold
            .execute(&request)
            .expect_err("invalid POT input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.POT_INPUT_MISMATCH");
    }

    fn stage_pot_inputs(pot_path: &Path, geom_path: &Path) {
        fs::write(pot_path, pot_input_fixture()).expect("pot input should be written");
        fs::write(geom_path, geom_input_fixture()).expect("geom input should be written");
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn expected_pot_artifact_set() -> BTreeSet<String> {
        [
            "pot.bin",
            "pot.dat",
            "log1.dat",
            "convergence.scf",
            "convergence.scf.fine",
        ]
        .iter()
        .map(|artifact| artifact.to_string())
        .collect()
    }

    fn pot_input_fixture() -> &'static str {
        "mpot, nph, ntitle, ihole, ipr1, iafolp, ixc,ispec
   1   1   1   1   0   0   0   1
nmix, nohole, jumprm, inters, nscmt, icoul, lfms1, iunf
   6   2   0   0  30   0   0   0
Cu crystal
gamach, rgrd, ca1, ecv, totvol, rfms1
      1.72919      0.05000      0.20000    -40.00000      0.00000      4.00000
 iz, lmaxsc, xnatph, xion, folp
   29    2      1.00000      0.00000      1.15000
   29    2    100.00000      0.00000      1.15000
ExternalPot switch, StartFromFile switch
 F F
"
    }

    fn geom_input_fixture() -> &'static str {
        "nat, nph =    4    1
    1    2
 iat     x       y        z       iph
 -----------------------------------------------------------------------
   1      0.00000      0.00000      0.00000   0   1
   2      1.80500      1.80500      0.00000   1   1
   3     -1.80500      1.80500      0.00000   1   1
   4      0.00000      1.80500      1.80500   1   1
"
    }
}
