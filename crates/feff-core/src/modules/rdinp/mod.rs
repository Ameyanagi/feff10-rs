mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{
    ComputeArtifact, ComputeRequest, ComputeResult, FeffError,
};
use crate::parser::parse_input_deck;
use std::fs;

use model::RdinpModel;
use parser::{artifact_list, read_input_source, validate_request_shape};

pub(crate) const RDINP_REQUIRED_INPUTS: [&str; 1] = ["feff.inp"];
pub(crate) const RDINP_BASE_OUTPUTS_PREFIX: [&str; 5] = [
    "geom.dat",
    "global.inp",
    "reciprocal.inp",
    "pot.inp",
    "ldos.inp",
];
pub(crate) const RDINP_BASE_OUTPUTS_SUFFIX: [&str; 14] = [
    "xsph.inp",
    "fms.inp",
    "paths.inp",
    "genfmt.inp",
    "ff2x.inp",
    "sfconv.inp",
    "eels.inp",
    "compton.inp",
    "band.inp",
    "rixs.inp",
    "crpa.inp",
    "fullspectrum.inp",
    "dmdw.inp",
    "log.dat",
];
pub(crate) const RDINP_OPTIONAL_SCREEN_OUTPUT: &str = "screen.inp";

pub(crate) const GLOBAL_INP_TEMPLATE: &str = " nabs, iphabs - CFAVERAGE data
       1       0 100000.00000
 ipol, ispin, le2, elpty, angks, l2lp, do_nrixs, ldecmx, lj
    0    0    0      0.0000      0.0000    0    0   -1   -1
evec\t\t  xivec \t   spvec
      0.00000      0.00000      0.00000
      0.00000      0.00000      0.00000
      0.00000      0.00000      0.00000
 polarization tensor 
      0.33333      0.00000      0.00000      0.00000      0.00000      0.00000
      0.00000      0.00000      0.33333      0.00000      0.00000      0.00000
      0.00000      0.00000      0.00000      0.00000      0.33333      0.00000
evnorm, xivnorm, spvnorm - only used for nrixs
      0.00000      0.00000      0.00000
nq,    imdff,   qaverage,   mixdff,   qqmdff,   cos<q,q'>
           0           0 T F  -1.00000000000000     
 q-vectors : qx, qy, qz, q(norm), weight, qcosth, qsinth, qcosfi, qsinfi
";

pub(crate) const RECIPROCAL_INP_TEMPLATE: &str = "spacy
   1
";

pub(crate) const GENFMT_INP_TEMPLATE: &str = "mfeff, ipr5, iorder, critcw, wnstar
   1   0       2      4.00000    F
 the number of decomposi
   -1
";

pub(crate) const EELS_INP_TEMPLATE: &str = "calculate ELNES?
   0
average? relativistic? cross-terms? Which input?
   0   1   1   1   4
polarizations to be used ; min step max
   1   1   1
beam energy in eV
      0.00000
beam direction in arbitrary units
      0.00000      0.00000      0.00000
collection and convergence semiangle in rad
      0.00000      0.00000
qmesh - radial and angular grid size
   0   0
detector positions - two angles in rad
      0.00000      0.00000
calculate magic angle if magic=1
   0
energy for magic angle - eV above threshold
      0.00000
";

pub(crate) const DMDW_INP_TEMPLATE: &str = "-999
";

pub(crate) const COMPTON_INP_TEMPLATE: &str = "run compton module?
{{RUN_COMPTON}}
pqmax, npq
   5.000000            1000
ns, nphi, nz, nzp
  32  32  32 144
smax, phimax, zmax, zpmax
      0.00000      6.28319      0.00000     10.00000
jpq? rhozzp? force_recalc_jzzp?
 F F F
window_type (0=Step, 1=Hann), window_cutoff
           1  0.0000000E+00
temperature (in eV)
      0.00000
set_chemical_potential? chemical_potential(eV)
 F  0.0000000E+00
rho_xy? rho_yz? rho_xz? rho_vol? rho_line?
 F F F F F
qhat_x qhat_y qhat_z
  0.000000000000000E+000  0.000000000000000E+000   1.00000000000000     
";

pub(crate) const BAND_INP_TEMPLATE: &str = "mband : calculate bands if = 1
{{MBAND}}
emin, emax, estep : energy mesh
      0.00000      0.00000      0.00000
nkp : # points in k-path
   0
ikpath : type of k-path
  -1
freeprop :  empty lattice if = T
 F
";

pub(crate) const RIXS_INP_TEMPLATE: &str = " m_run
{{RUN_RIXS}}
 gam_ch, gam_exp(1), gam_exp(2)
        0.0001350512        0.0001350512        0.0001350512
 EMinI, EMaxI, EMinF, EMaxF
        0.0000000000        0.0000000000        0.0000000000        0.0000000000
 xmu
  -367493090.02742821     
 Readpoles, SkipCalc, MBConv, ReadSigma
 T F F F
 nEdges
           1
 Edge           1
{{EDGE}}
";

pub(crate) const CRPA_INP_TEMPLATE: &str = " do_CRPA{{RUN_CRPA}}
 rcut{{RCUT}}
 l_crpa           3
";

pub(crate) const FULLSPECTRUM_INP_TEMPLATE: &str = " mFullSpectrum
{{RUN_FULLSPECTRUM}}
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdinpContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RdinpModule;

impl RdinpModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<RdinpContract> {
        let model = model_for_request(request)?;
        Ok(RdinpContract {
            required_inputs: artifact_list(&RDINP_REQUIRED_INPUTS),
            expected_outputs: model.expected_outputs,
        })
    }
}

impl ModuleExecutor for RdinpModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        let model = model_for_request(request)?;
        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.RDINP_OUTPUT_DIRECTORY",
                format!(
                    "failed to create RDINP output directory '{}': {}",
                    request.output_dir.display(),
                    source
                ),
            )
        })?;

        for artifact in &model.expected_outputs {
            let output_path = request.output_dir.join(&artifact.relative_path);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|source| {
                    FeffError::io_system(
                        "IO.RDINP_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create RDINP artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            let artifact_path = artifact.relative_path.to_string_lossy().replace('\\', "/");
            model.write_artifact(&artifact_path, &output_path)?;
        }

        Ok(model.expected_outputs)
    }
}

fn model_for_request(request: &ComputeRequest) -> ComputeResult<RdinpModel> {
    validate_request_shape(request)?;
    let input_source = read_input_source(&request.input_path)?;
    let deck = parse_input_deck(&input_source)?;
    RdinpModel::from_deck(&deck)
}

#[cfg(test)]
mod tests {
    use super::{RdinpModule, model::expected_outputs_for_screen_card};
    use crate::domain::{FeffErrorCategory, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_rdinp_compatibility_interfaces() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("feff.inp");
        let output_dir = temp.path().join("out");
        fs::write(
            &input_path,
            "TITLE Cu\nPOTENTIALS\n0 29 Cu\n1 29 Cu\nATOMS\n0.0 0.0 0.0 0 Cu\n1.0 0.0 0.0 1 Cu\nEND\n",
        )
        .expect("input should be written");

        let request = ComputeRequest::new(
            "FX-RDINP-001",
            ComputeModule::Rdinp,
            &input_path,
            &output_dir,
        );
        let scaffold = RdinpModule;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 1);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("feff.inp")
        );
        assert_eq!(contract.expected_outputs.len(), 19);
        assert!(
            contract
                .expected_outputs
                .iter()
                .all(|artifact| artifact.relative_path.as_path() != Path::new("screen.inp"))
        );
        assert!(
            contract
                .expected_outputs
                .iter()
                .any(|artifact| artifact.relative_path.as_path() == Path::new("compton.inp"))
        );
        assert!(
            contract
                .expected_outputs
                .iter()
                .any(|artifact| artifact.relative_path.as_path() == Path::new("fullspectrum.inp"))
        );
    }

    #[test]
    fn contract_adds_screen_output_when_screen_card_is_present() {
        let outputs = expected_outputs_for_screen_card(true);
        assert_eq!(outputs.len(), 20);
        assert!(
            outputs
                .iter()
                .any(|artifact| artifact.relative_path.as_path() == Path::new("screen.inp"))
        );
    }

    #[test]
    fn execute_materializes_rdinp_artifacts() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("feff.inp");
        let output_dir = temp.path().join("actual");
        fs::write(
            &input_path,
            "TITLE Cu\nPOTENTIALS\n0 29 Cu\n1 29 Cu\nATOMS\n0.0 0.0 0.0 0 Cu\n1.0 0.0 0.0 1 Cu\nEND\n",
        )
        .expect("input should be written");

        let request = ComputeRequest::new(
            "FX-RDINP-001",
            ComputeModule::Rdinp,
            &input_path,
            &output_dir,
        );
        let scaffold = RdinpModule;
        let artifacts = scaffold
            .execute(&request)
            .expect("RDINP execution should succeed");

        assert_eq!(artifacts.len(), 19);
        let log_dat = output_dir.join("log.dat");
        assert!(log_dat.exists());
        let content = fs::read_to_string(log_dat).expect("log output should be readable");
        assert!(content.contains("Core hole lifetime"));
        assert!(content.contains("Cu"));

        for generated in [
            "compton.inp",
            "band.inp",
            "rixs.inp",
            "crpa.inp",
            "fullspectrum.inp",
        ] {
            assert!(
                output_dir.join(generated).is_file(),
                "expected '{}' to be generated",
                generated
            );
        }
    }

    #[test]
    fn execute_rejects_non_rdinp_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("feff.inp");
        fs::write(
            &input_path,
            "TITLE Cu\nPOTENTIALS\n0 29 Cu\n1 29 Cu\nATOMS\n0.0 0.0 0.0 0 Cu\n1.0 0.0 0.0 1 Cu\nEND\n",
        )
        .expect("input should be written");

        let request =
            ComputeRequest::new("FX-POT-001", ComputeModule::Pot, &input_path, temp.path());
        let scaffold = RdinpModule;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.RDINP_MODULE");
    }

    #[test]
    fn xnatph_uses_atom_count_when_potential_fraction_is_omitted() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("feff.inp");
        let output_dir = temp.path().join("actual");
        fs::write(
            &input_path,
            "TITLE Cu\nPOTENTIALS\n0 29 Cu\n1 29 Cu\nATOMS\n0.0 0.0 0.0 0 Cu\n1.0 0.0 0.0 1 Cu\n2.0 0.0 0.0 1 Cu\nEND\n",
        )
        .expect("input should be written");
        let request = ComputeRequest::new(
            "FX-RDINP-ATOMCOUNT",
            ComputeModule::Rdinp,
            &input_path,
            &output_dir,
        );

        let scaffold = RdinpModule;
        scaffold
            .execute(&request)
            .expect("execution should succeed");
        let pot_inp = fs::read_to_string(output_dir.join("pot.inp")).expect("pot.inp");
        assert!(
            pot_inp.contains("    2.00000"),
            "xnatph should include atom count"
        );
    }
}
