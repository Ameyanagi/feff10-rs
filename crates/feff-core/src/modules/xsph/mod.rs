mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::XsphModel;
use parser::{
    artifact_list, input_parent_dir, maybe_read_optional_input_source, read_input_bytes,
    read_input_source, validate_request_shape,
};

pub(crate) const XSPH_REQUIRED_INPUTS: [&str; 4] =
    ["xsph.inp", "geom.dat", "global.inp", "pot.bin"];
pub(crate) const XSPH_OPTIONAL_INPUTS: [&str; 1] = ["wscrn.dat"];
pub(crate) const XSPH_REQUIRED_OUTPUTS: [&str; 3] = ["phase.bin", "xsect.dat", "log2.dat"];
pub(crate) const XSPH_OPTIONAL_OUTPUTS: [&str; 1] = ["phase.dat"];
pub const XSPH_PHASE_BINARY_MAGIC: &[u8; 8] = b"XSPHBIN1";

const POT_CONTROL_I32_COUNT: usize = 16;
const POT_CONTROL_F64_COUNT: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XsphContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub optional_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
    pub optional_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct XsphModule;

impl XsphModule {
    pub fn contract_for_request(&self, request: &ComputeRequest) -> ComputeResult<XsphContract> {
        validate_request_shape(request)?;
        Ok(XsphContract {
            required_inputs: artifact_list(&XSPH_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&XSPH_OPTIONAL_INPUTS),
            expected_outputs: artifact_list(&XSPH_REQUIRED_OUTPUTS),
            optional_outputs: artifact_list(&XSPH_OPTIONAL_OUTPUTS),
        })
    }
}

impl ModuleExecutor for XsphModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let xsph_source = read_input_source(&request.input_path, XSPH_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(XSPH_REQUIRED_INPUTS[1]),
            XSPH_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(XSPH_REQUIRED_INPUTS[2]),
            XSPH_REQUIRED_INPUTS[2],
        )?;
        let pot_bytes = read_input_bytes(
            &input_dir.join(XSPH_REQUIRED_INPUTS[3]),
            XSPH_REQUIRED_INPUTS[3],
        )?;
        let wscrn_source = maybe_read_optional_input_source(
            input_dir.join(XSPH_OPTIONAL_INPUTS[0]),
            XSPH_OPTIONAL_INPUTS[0],
        )?;

        let model = XsphModel::from_sources(
            &request.fixture_id,
            &xsph_source,
            &geom_source,
            &global_source,
            &pot_bytes,
            wscrn_source.as_deref(),
        )?;
        let outputs = artifact_list(&XSPH_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.XSPH_OUTPUT_DIRECTORY",
                format!(
                    "failed to create XSPH output directory '{}': {}",
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
                        "IO.XSPH_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create XSPH artifact directory '{}': {}",
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
    use super::parser::{push_f64, push_i32, push_u32};
    use super::{XSPH_PHASE_BINARY_MAGIC, XsphModule};
    use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, FeffErrorCategory};
    use crate::modules::ModuleExecutor;
    use crate::modules::pot::POT_BINARY_MAGIC;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const XSPH_INPUT_FIXTURE: &str = "mphase,ipr2,ixc,ixc0,ispec,lreal,lfms2,nph,l2lp,iPlsmn,NPoles,iGammaCH,iGrid
   1   0   0   0   1   0   0   1   0   0  80   0   0
vr0, vi0
      0.00000      0.00000
 lmaxph(0:nph)
   3   3
rgrd, rfms2, gamach, xkstep, xkmax, vixan, Eps0, EGap
      0.05000      4.00000      1.72919      0.07000      8.00000      0.00000      0.00000      0.00000
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

    const GLOBAL_INPUT_FIXTURE: &str = "edge emu efermi
   8979.00000      0.12000     -7.60000
   9000.00000      0.18000     -7.50000
";

    const WSCRN_INPUT_FIXTURE: &str = "    0.1507330463E-03    0.2672902675E+02    0.2916165288E+02
    0.1584612949E-03    0.2672902006E+02    0.2916164619E+02
    0.1665857792E-03    0.2672900634E+02    0.2916163247E+02
";

    #[test]
    fn contract_exposes_true_compute_xsph_artifact_contract() {
        let request = ComputeRequest::new("FX-XSPH-001", ComputeModule::Xsph, "xsph.inp", "out");
        let contract = XsphModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["xsph.inp", "geom.dat", "global.inp", "pot.bin"])
        );
        assert_eq!(
            artifact_set(&contract.optional_inputs),
            expected_artifact_set(&["wscrn.dat"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&["phase.bin", "xsect.dat", "log2.dat"])
        );
        assert_eq!(
            artifact_set(&contract.optional_outputs),
            expected_artifact_set(&["phase.dat"])
        );
    }

    #[test]
    fn execute_emits_true_compute_artifacts() {
        let temp = TempDir::new().expect("tempdir should be created");
        let (input_path, output_dir) = stage_xsph_inputs(temp.path(), true);

        let request =
            ComputeRequest::new("FX-XSPH-001", ComputeModule::Xsph, &input_path, &output_dir);
        let artifacts = XsphModule
            .execute(&request)
            .expect("XSPH execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["phase.bin", "xsect.dat", "log2.dat"])
        );

        for artifact in &artifacts {
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

        let phase_bytes = fs::read(output_dir.join("phase.bin")).expect("phase.bin should exist");
        assert!(
            phase_bytes.starts_with(XSPH_PHASE_BINARY_MAGIC),
            "phase.bin should use true-compute XSPH header"
        );
    }

    #[test]
    fn execute_supports_missing_optional_wscrn_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let (input_path, output_dir) = stage_xsph_inputs(temp.path(), false);

        let request =
            ComputeRequest::new("FX-XSPH-001", ComputeModule::Xsph, &input_path, &output_dir);
        let artifacts = XsphModule
            .execute(&request)
            .expect("XSPH execution should succeed without wscrn.dat");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["phase.bin", "xsect.dat", "log2.dat"])
        );
    }

    #[test]
    fn execute_uses_optional_wscrn_when_present() {
        let temp = TempDir::new().expect("tempdir should be created");

        let (with_input_path, with_output_dir) = stage_xsph_inputs(temp.path().join("with"), true);
        let with_request = ComputeRequest::new(
            "FX-XSPH-001",
            ComputeModule::Xsph,
            &with_input_path,
            &with_output_dir,
        );
        XsphModule
            .execute(&with_request)
            .expect("XSPH execution with wscrn should succeed");

        let (without_input_path, without_output_dir) =
            stage_xsph_inputs(temp.path().join("without"), false);
        let without_request = ComputeRequest::new(
            "FX-XSPH-001",
            ComputeModule::Xsph,
            &without_input_path,
            &without_output_dir,
        );
        XsphModule
            .execute(&without_request)
            .expect("XSPH execution without wscrn should succeed");

        let with_phase = fs::read(with_output_dir.join("phase.bin")).expect("phase output");
        let without_phase = fs::read(without_output_dir.join("phase.bin")).expect("phase output");
        assert_ne!(
            with_phase, without_phase,
            "optional wscrn.dat should influence phase.bin output"
        );

        let with_xsect = fs::read(with_output_dir.join("xsect.dat")).expect("xsect output");
        let without_xsect = fs::read(without_output_dir.join("xsect.dat")).expect("xsect output");
        assert_ne!(
            with_xsect, without_xsect,
            "optional wscrn.dat should influence xsect.dat output"
        );
    }

    #[test]
    fn execute_rejects_non_xsph_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let (input_path, output_dir) = stage_xsph_inputs(temp.path(), false);

        let request =
            ComputeRequest::new("FX-XSPH-001", ComputeModule::Path, &input_path, &output_dir);
        let error = XsphModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.XSPH_MODULE");
    }

    #[test]
    fn execute_requires_pot_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        let output_dir = temp.path().join("out");

        fs::create_dir_all(temp.path()).expect("temp dir should exist");
        fs::write(&input_path, XSPH_INPUT_FIXTURE).expect("xsph input should be written");
        fs::write(temp.path().join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");
        fs::write(temp.path().join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");

        let request =
            ComputeRequest::new("FX-XSPH-001", ComputeModule::Xsph, &input_path, &output_dir);
        let error = XsphModule
            .execute(&request)
            .expect_err("missing pot.bin should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.XSPH_INPUT_READ");
    }

    #[test]
    fn execute_rejects_invalid_xsph_input_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("xsph.inp");
        let output_dir = temp.path().join("out");

        fs::create_dir_all(temp.path()).expect("temp dir should exist");
        fs::write(&input_path, "invalid xsph input\n").expect("xsph input should be written");
        fs::write(temp.path().join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");
        fs::write(temp.path().join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");
        write_true_compute_pot_fixture(&temp.path().join("pot.bin"));

        let request =
            ComputeRequest::new("FX-XSPH-001", ComputeModule::Xsph, &input_path, &output_dir);
        let error = XsphModule
            .execute(&request)
            .expect_err("invalid xsph input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.XSPH_INPUT_PARSE");
    }

    fn stage_xsph_inputs(root: impl AsRef<Path>, include_wscrn: bool) -> (PathBuf, PathBuf) {
        let root = root.as_ref();
        fs::create_dir_all(root).expect("root should be created");

        let input_path = root.join("xsph.inp");
        let output_dir = root.join("out");
        fs::write(&input_path, XSPH_INPUT_FIXTURE).expect("xsph input should be written");
        fs::write(root.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom input should be written");
        fs::write(root.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");
        write_true_compute_pot_fixture(&root.join("pot.bin"));

        if include_wscrn {
            fs::write(root.join("wscrn.dat"), WSCRN_INPUT_FIXTURE)
                .expect("wscrn input should be written");
        }

        (input_path, output_dir)
    }

    fn write_true_compute_pot_fixture(path: &Path) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(POT_BINARY_MAGIC);

        for value in [1_i32, 1, 1, 1, 0, 0, 0, 1, 6, 2, 0, 0, 30, 0, 0, 0] {
            push_i32(&mut bytes, value);
        }
        for value in [1.72919_f64, 0.05, 0.2, -40.0, 0.0, 4.0] {
            push_f64(&mut bytes, value);
        }

        push_u32(&mut bytes, 4);
        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, 2);
        push_f64(&mut bytes, 2.0);
        push_f64(&mut bytes, 2.2);
        push_f64(&mut bytes, 3.6);

        for (index, zeff) in [(0_u32, 29.0_f64), (1_u32, 28.8_f64)] {
            push_u32(&mut bytes, index);
            push_i32(&mut bytes, 29);
            push_i32(&mut bytes, 2);
            push_f64(&mut bytes, 1.0);
            push_f64(&mut bytes, 0.0);
            push_f64(&mut bytes, 1.15);
            push_f64(&mut bytes, zeff);
            push_f64(&mut bytes, 0.12);
            push_f64(&mut bytes, -0.45);
            push_f64(&mut bytes, -0.08);
        }

        for (x, y, z, ipot) in [
            (0.0_f64, 0.0_f64, 0.0_f64, 0_i32),
            (1.805_f64, 1.805_f64, 0.0_f64, 1_i32),
            (-1.805_f64, 1.805_f64, 0.0_f64, 1_i32),
            (0.0_f64, 1.805_f64, 1.805_f64, 1_i32),
        ] {
            push_f64(&mut bytes, x);
            push_f64(&mut bytes, y);
            push_f64(&mut bytes, z);
            push_i32(&mut bytes, ipot);
        }

        fs::write(path, bytes).expect("pot fixture should be written");
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
}
