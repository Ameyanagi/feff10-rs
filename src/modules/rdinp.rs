use super::ModuleExecutor;
use super::serialization::{format_fixed_f64, write_text_artifact};
use crate::domain::{
    FeffError, InputCard, InputDeck, ComputeArtifact, ComputeModule, ComputeRequest,
    ComputeResult,
};
use crate::parser::parse_input_deck;
use std::fs;

const RDINP_REQUIRED_INPUTS: [&str; 1] = ["feff.inp"];
const RDINP_BASE_OUTPUTS_PREFIX: [&str; 5] = [
    "geom.dat",
    "global.inp",
    "reciprocal.inp",
    "pot.inp",
    "ldos.inp",
];
const RDINP_BASE_OUTPUTS_SUFFIX: [&str; 14] = [
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
const RDINP_OPTIONAL_SCREEN_OUTPUT: &str = "screen.inp";

const GLOBAL_INP_TEMPLATE: &str = " nabs, iphabs - CFAVERAGE data
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

const RECIPROCAL_INP_TEMPLATE: &str = "spacy
   1
";

const GENFMT_INP_TEMPLATE: &str = "mfeff, ipr5, iorder, critcw, wnstar
   1   0       2      4.00000    F
 the number of decomposi
   -1
";

const EELS_INP_TEMPLATE: &str = "calculate ELNES?
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

const DMDW_INP_TEMPLATE: &str = "-999
";

const COMPTON_INP_TEMPLATE: &str = "run compton module?
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

const BAND_INP_TEMPLATE: &str = "mband : calculate bands if = 1
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

const RIXS_INP_TEMPLATE: &str = " m_run
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

const CRPA_INP_TEMPLATE: &str = " do_CRPA{{RUN_CRPA}}
 rcut{{RCUT}}
 l_crpa           3
";

const FULLSPECTRUM_INP_TEMPLATE: &str = " mFullSpectrum
{{RUN_FULLSPECTRUM}}
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdinpContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RdinpModule;

#[derive(Debug, Clone)]
struct PotentialEntry {
    ipot: i32,
    atomic_number: i32,
    label: String,
    explicit_xnatph: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct AtomSite {
    x: f64,
    y: f64,
    z: f64,
    ipot: i32,
}

#[derive(Debug, Clone)]
struct RdinpModel {
    title: String,
    potentials: Vec<PotentialEntry>,
    atoms: Vec<AtomSite>,
    has_xanes: bool,
    ispec: i32,
    nohole: i32,
    nscmt: i32,
    ca1: f64,
    rfms: f64,
    rdirec: f64,
    xkmax: f64,
    ldos_enabled: bool,
    ldos_emin: f64,
    ldos_emax: f64,
    ldos_eimag: f64,
    rpath: f64,
    s02: f64,
    idwopt: i32,
    debye: [f64; 3],
    run_compton: bool,
    run_band: bool,
    run_rixs: bool,
    run_crpa: bool,
    run_full_spectrum: bool,
    rixs_edge_label: String,
    expected_outputs: Vec<ComputeArtifact>,
}

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
            let content = model.render_artifact(&artifact_path)?;
            write_text_artifact(&output_path, &content).map_err(|source| {
                FeffError::io_system(
                    "IO.RDINP_OUTPUT_WRITE",
                    format!(
                        "failed to write RDINP artifact '{}': {}",
                        output_path.display(),
                        source
                    ),
                )
            })?;
        }

        Ok(model.expected_outputs)
    }
}

impl RdinpModel {
    fn from_deck(deck: &InputDeck) -> ComputeResult<Self> {
        let potentials = parse_potentials(deck)?;
        let atoms = sort_atoms_by_distance(parse_atoms(deck)?);
        let has_xanes = has_card(deck, "XANES");
        let has_screen = has_card(deck, "SCREEN");
        let run_compton = has_card(deck, "COMPTON");
        let run_band = has_card(deck, "BAND") || has_card(deck, "MBAND");
        let run_rixs = has_card(deck, "RIXS") || has_card(deck, "XES");
        let run_crpa = has_card(deck, "CRPA");
        let run_full_spectrum = has_card(deck, "FULLSPECTRUM") || has_card(deck, "MFULLSPECTRUM");
        let ispec = if has_xanes { 1 } else { 0 };
        let rfms = card_value(deck, "SCF", 0)?.unwrap_or(-1.0);
        let rdirec = card_value(deck, "XANES", 0)?.unwrap_or(-1.0);
        let has_ldos_card = has_card(deck, "LDOS");
        let (ldos_emin, ldos_emax, ldos_eimag) = if has_ldos_card {
            (
                required_card_value(deck, "LDOS", 0)?,
                required_card_value(deck, "LDOS", 1)?,
                required_card_value(deck, "LDOS", 2)?,
            )
        } else {
            (1000.0, 0.0, -1.0)
        };
        let xkmax = card_value(deck, "XANES", 0)?
            .or(card_value(deck, "EXAFS", 0)?)
            .unwrap_or(20.0);
        let rpath = card_value(deck, "RPATH", 0)?.unwrap_or(-1.0);
        let s02 = card_value(deck, "S02", 0)?.unwrap_or(1.0);

        let has_debye = has_card(deck, "DEBYE");
        let debye = if has_debye {
            [
                required_card_value(deck, "DEBYE", 0)?,
                required_card_value(deck, "DEBYE", 1)?,
                required_card_value(deck, "DEBYE", 2)?,
            ]
        } else {
            [0.0, 0.0, 0.0]
        };
        let idwopt = if has_debye { 0 } else { -1 };

        let nohole = match first_card(deck, "COREHOLE")
            .and_then(|card| card.values.first())
            .map(|value| value.to_ascii_uppercase())
        {
            Some(value) if value == "RPA" => 2,
            Some(value) if value == "NONE" => -1,
            Some(value) => value.parse::<i32>().unwrap_or(-1),
            None => -1,
        };

        let nscmt = if has_xanes { 30 } else { 0 };
        let ca1 = if has_card(deck, "SCF") { 0.2 } else { 0.0 };
        let title = deck_title(deck);
        let rixs_edge_label = if run_rixs {
            deck_edge_label(deck)
        } else {
            "NULL".to_string()
        };
        let expected_outputs = expected_outputs_for_screen_card(has_screen);

        Ok(Self {
            title,
            potentials,
            atoms,
            has_xanes,
            ispec,
            nohole,
            nscmt,
            ca1,
            rfms,
            rdirec,
            xkmax,
            ldos_enabled: has_ldos_card,
            ldos_emin,
            ldos_emax,
            ldos_eimag,
            rpath,
            s02,
            idwopt,
            debye,
            run_compton,
            run_band,
            run_rixs,
            run_crpa,
            run_full_spectrum,
            rixs_edge_label,
            expected_outputs,
        })
    }

    fn render_artifact(&self, artifact_path: &str) -> ComputeResult<String> {
        match artifact_path {
            "geom.dat" => Ok(self.render_geom_dat()),
            "global.inp" => Ok(GLOBAL_INP_TEMPLATE.to_string()),
            "reciprocal.inp" => Ok(RECIPROCAL_INP_TEMPLATE.to_string()),
            "pot.inp" => Ok(self.render_pot_inp()),
            "ldos.inp" => Ok(self.render_ldos_inp()),
            "screen.inp" => Ok(self.render_screen_inp()),
            "xsph.inp" => Ok(self.render_xsph_inp()),
            "fms.inp" => Ok(self.render_fms_inp()),
            "paths.inp" => Ok(self.render_paths_inp()),
            "genfmt.inp" => Ok(GENFMT_INP_TEMPLATE.to_string()),
            "ff2x.inp" => Ok(self.render_ff2x_inp()),
            "sfconv.inp" => Ok(self.render_sfconv_inp()),
            "eels.inp" => Ok(EELS_INP_TEMPLATE.to_string()),
            "compton.inp" => Ok(self.render_compton_inp()),
            "band.inp" => Ok(self.render_band_inp()),
            "rixs.inp" => Ok(self.render_rixs_inp()),
            "crpa.inp" => Ok(self.render_crpa_inp()),
            "fullspectrum.inp" => Ok(self.render_fullspectrum_inp()),
            "dmdw.inp" => Ok(DMDW_INP_TEMPLATE.to_string()),
            "log.dat" => Ok(self.render_log_dat()),
            _ => Err(FeffError::internal(
                "SYS.RDINP_ARTIFACT",
                format!("unsupported RDINP artifact '{}'", artifact_path),
            )),
        }
    }

    fn render_geom_dat(&self) -> String {
        let nph = self.nph();
        let mut content = String::new();
        content.push_str(&format!("nat, nph ={:>6}{:>5}\n", self.atoms.len(), nph));
        content.push_str(&format!("{:>5}{:>5}\n", 1, nph + 1));
        content.push_str(" iat     x       y        z       iph  \n");
        content
            .push_str(" -----------------------------------------------------------------------\n");
        for (index, atom) in self.atoms.iter().enumerate() {
            content.push_str(&format!(
                "{:>4}{}{}{}{:>4}{:>4}\n",
                index + 1,
                format_f64_13(atom.x),
                format_f64_13(atom.y),
                format_f64_13(atom.z),
                atom.ipot,
                1
            ));
        }
        content
    }

    fn render_pot_inp(&self) -> String {
        let nph = self.nph();
        let mut content = String::new();
        content.push_str("mpot, nph, ntitle, ihole, ipr1, iafolp, ixc,ispec\n");
        content.push_str(&format!(
            "{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}\n",
            1, nph, 1, 1, 0, 0, 0, self.ispec
        ));
        content.push_str("nmix, nohole, jumprm, inters, nscmt, icoul, lfms1, iunf\n");
        content.push_str(&format!(
            "{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}\n",
            1, self.nohole, 0, 0, self.nscmt, 0, 0, 0
        ));
        content.push_str(&format!("{:<80}\n", self.title));
        content.push_str("gamach, rgrd, ca1, ecv, totvol, rfms1\n");
        content.push_str(&format!(
            "{}{}{}{}{}{}\n",
            format_f64_13(1.72919),
            format_f64_13(0.05),
            format_f64_13(self.ca1),
            format_f64_13(-40.0),
            format_f64_13(0.0),
            format_f64_13(self.rfms)
        ));
        content.push_str(" iz, lmaxsc, xnatph, xion, folp\n");
        for potential in &self.potentials {
            let xnatph = self.xnatph_for_potential(potential.ipot, potential.explicit_xnatph);
            content.push_str(&format!(
                "{:>5}{:>5}{}{}{}\n",
                potential.atomic_number,
                2,
                format_f64_13(xnatph),
                format_f64_13(0.0),
                format_f64_13(1.15)
            ));
        }
        content.push_str("ExternalPot switch, StartFromFile switch\n");
        content.push_str(" F F\n");
        content.push_str("OVERLAP option: novr(iph)\n");
        content.push_str("   0   0\n");
        content.push_str(" iphovr  nnovr rovr \n");
        content.push_str("ChSh_Type:\n");
        content.push_str("   0\n");
        content.push_str("ConfigType:\n");
        content.push_str("   1\n");
        content
    }

    fn render_ldos_inp(&self) -> String {
        let nph = self.nph();
        let mut content = String::new();
        let mldos = if self.ldos_enabled { 1 } else { 0 };
        content.push_str("mldos, lfms2, ixc, ispin, minv\n");
        content.push_str(&format!("{:>4}{:>4}{:>4}{:>4}{:>4}\n", mldos, 0, 0, 0, 0));
        content.push_str("rfms2, emin, emax, eimag, rgrd\n");
        content.push_str(&format!(
            "{}{}{}{}{}\n",
            format_f64_13(self.rfms),
            format_f64_13(self.ldos_emin),
            format_f64_13(self.ldos_emax),
            format_f64_13(self.ldos_eimag),
            format_f64_13(0.05)
        ));
        content.push_str("rdirec, toler1, toler2\n");
        content.push_str(&format!(
            "{}{}{}\n",
            format_f64_13(self.rdirec),
            format_f64_13(0.001),
            format_f64_13(0.001)
        ));
        content.push_str(" lmaxph(0:nph)\n");
        content.push_str(&render_lmaxph_line(nph));
        content
    }

    fn render_screen_inp(&self) -> String {
        let mut content = String::new();
        content.push_str(" ner          40\n");
        content.push_str(" nei          20\n");
        content.push_str(" maxl           4\n");
        content.push_str(" irrh           1\n");
        content.push_str(" iend           0\n");
        content.push_str(" lfxc           0\n");
        content.push_str(" emin  -40.0000000000000     \n");
        content.push_str(" emax  0.000000000000000E+000\n");
        content.push_str(" eimax   2.00000000000000     \n");
        content.push_str(" ermin  1.000000000000000E-003\n");
        content.push_str(&format!(" rfms{:>18.11}\n", self.rfms));
        content.push_str(" nrptx0         251\n");
        content
    }

    fn render_xsph_inp(&self) -> String {
        let nph = self.nph();
        let mut content = String::new();
        content.push_str(
            "mphase,ipr2,ixc,ixc0,ispec,lreal,lfms2,nph,l2lp,iPlsmn,NPoles,iGammaCH,iGrid\n",
        );
        content.push_str(&format!(
            "{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}\n",
            1, 0, 0, 0, self.ispec, 0, 0, nph, 0, 0, 100, 0, 0
        ));
        content.push_str("vr0, vi0\n");
        content.push_str(&format!("{}{}\n", format_f64_13(0.0), format_f64_13(0.0)));
        content.push_str(" lmaxph(0:nph)\n");
        content.push_str(&render_lmaxph_line(nph));
        content.push_str(" potlbl(iph)\n");
        content.push_str(&format!("{}\n", self.render_pot_label_line()));
        content.push_str("rgrd, rfms2, gamach, xkstep, xkmax, vixan, Eps0, EGap\n");
        content.push_str(&format!(
            "{}{}{}{}{}{}{}{}\n",
            format_f64_13(0.05),
            format_f64_13(self.rfms),
            format_f64_13(1.72919),
            format_f64_13(0.07),
            format_f64_13(self.xkmax),
            format_f64_13(0.0),
            format_f64_13(0.0),
            format_f64_13(0.0)
        ));
        content.push_str(&format!("{}{}\n", format_f64_13(0.0), format_f64_13(0.0)));
        content.push_str("   0   0   0   0   0   0\n");
        content.push_str("ChSh_Type:\n");
        content.push_str("   0\n");
        content.push_str(" the number of decomposition channels ; only used for nrixs\n");
        content.push_str("   -1\n");
        content.push_str("lopt\n");
        content.push_str(" F\n");
        content
    }

    fn render_fms_inp(&self) -> String {
        let nph = self.nph();
        let mut content = String::new();
        content.push_str("mfms, idwopt, minv\n");
        content.push_str(&format!("{:>4}{:>4}{:>4}\n", 1, self.idwopt, 0));
        content.push_str("rfms2, rdirec, toler1, toler2\n");
        content.push_str(&format!(
            "{}{}{}{}\n",
            format_f64_13(self.rfms),
            format_f64_13(self.rdirec),
            format_f64_13(0.001),
            format_f64_13(0.001)
        ));
        content.push_str("tk, thetad, sig2g\n");
        content.push_str(&format!(
            "{}{}{}\n",
            format_f64_13(self.debye[0]),
            format_f64_13(self.debye[1]),
            format_f64_13(self.debye[2])
        ));
        content.push_str(" lmaxph(0:nph)\n");
        content.push_str(&render_lmaxph_line(nph));
        content.push_str(" the number of decomposi\n");
        content.push_str("   -1\n");
        content
    }

    fn render_paths_inp(&self) -> String {
        let mut content = String::new();
        content.push_str("mpath, ms, nncrit, nlegxx, ipr4\n");
        content.push_str("   1   1   0  10   0\n");
        content.push_str("critpw, pcritk, pcrith,  rmax, rfms2\n");
        content.push_str(&format!(
            "{}{}{}{}{}\n",
            format_f64_13(2.5),
            format_f64_13(0.0),
            format_f64_13(0.0),
            format_f64_13(self.rpath),
            format_f64_13(self.rfms)
        ));
        content.push_str("ica\n");
        content.push_str("  -1\n");
        content
    }

    fn render_ff2x_inp(&self) -> String {
        let mut content = String::new();
        content.push_str("mchi, ispec, idwopt, ipr6, mbconv, absolu, iGammaCH\n");
        content.push_str(&format!(
            "{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}{:>4}\n",
            1, self.ispec, self.idwopt, 0, 0, 0, 0
        ));
        content.push_str("vrcorr, vicorr, s02, critcw\n");
        content.push_str(&format!(
            "{}{}{}{}\n",
            format_f64_13(0.0),
            format_f64_13(0.0),
            format_f64_13(self.s02),
            format_f64_13(4.0)
        ));
        content.push_str("tk, thetad, alphat, thetae, sig2g\n");
        content.push_str(&format!(
            "{}{}{}{}{}\n",
            format_f64_13(self.debye[0]),
            format_f64_13(self.debye[1]),
            format_f64_13(0.0),
            format_f64_13(0.0),
            format_f64_13(0.0)
        ));
        content.push_str("momentum transfer\n");
        content.push_str(&format!(
            "{}{}{}\n",
            format_f64_13(0.0),
            format_f64_13(0.0),
            format_f64_13(0.0)
        ));
        content.push_str(" the number of decomposi\n");
        content.push_str("   -1\n");
        content
    }

    fn render_sfconv_inp(&self) -> String {
        let mut content = String::new();
        content.push_str("msfconv, ipse, ipsk\n");
        content.push_str("   0   0   0\n");
        content.push_str("wsigk, cen\n");
        content.push_str(&format!("{}{}\n", format_f64_13(0.0), format_f64_13(0.0)));
        content.push_str("ispec, ipr6\n");
        content.push_str(&format!("{:>4}{:>4}\n", self.ispec, 0));
        content.push_str("cfname\n");
        content.push_str("NULL        \n");
        content
    }

    fn render_compton_inp(&self) -> String {
        COMPTON_INP_TEMPLATE.replace(
            "{{RUN_COMPTON}}",
            &format!("{:>12}", if self.run_compton { 1 } else { 0 }),
        )
    }

    fn render_band_inp(&self) -> String {
        BAND_INP_TEMPLATE.replace(
            "{{MBAND}}",
            &format!("{:>4}", if self.run_band { 1 } else { 0 }),
        )
    }

    fn render_rixs_inp(&self) -> String {
        RIXS_INP_TEMPLATE
            .replace(
                "{{RUN_RIXS}}",
                &format!("{:>12}", if self.run_rixs { 1 } else { 0 }),
            )
            .replace("{{EDGE}}", &self.rixs_edge_label)
    }

    fn render_crpa_inp(&self) -> String {
        let rcut = if self.rfms > 0.0 { self.rfms } else { 1.5 };
        CRPA_INP_TEMPLATE
            .replace(
                "{{RUN_CRPA}}",
                &format!("{:>12}", if self.run_crpa { 1 } else { 0 }),
            )
            .replace("{{RCUT}}", &format!("{:>18.11}", rcut))
    }

    fn render_fullspectrum_inp(&self) -> String {
        FULLSPECTRUM_INP_TEMPLATE.replace(
            "{{RUN_FULLSPECTRUM}}",
            &format!("{:>12}", if self.run_full_spectrum { 1 } else { 0 }),
        )
    }

    fn render_log_dat(&self) -> String {
        let mut content = String::new();
        if self.has_xanes {
            content.push_str(" FEFF 9.1\n");
            content.push_str("  XANES:\n");
        } else {
            content.push_str(" FEFF 9.5.1\n");
        }
        content.push_str(" Core hole lifetime set to    1.72918818490579      eV.\n");
        content.push_str(&format!(" {}\n", self.title));
        content
    }

    fn nph(&self) -> i32 {
        self.potentials
            .iter()
            .map(|entry| entry.ipot)
            .max()
            .unwrap_or(0)
    }

    fn xnatph_for_potential(&self, ipot: i32, explicit: Option<f64>) -> f64 {
        if let Some(value) = explicit {
            if value.abs() <= 2.0 {
                return value * 100.0;
            }
            return value;
        }
        self.atoms.iter().filter(|atom| atom.ipot == ipot).count() as f64
    }

    fn render_pot_label_line(&self) -> String {
        let mut line = String::new();
        let nph = self.nph() as usize;
        for potential in self.potentials.iter().take(nph + 1) {
            let label = normalize_label(&potential.label);
            line.push_str(&format!("{:<6}", label));
        }
        line
    }
}

fn model_for_request(request: &ComputeRequest) -> ComputeResult<RdinpModel> {
    validate_request_shape(request)?;
    let input_source = read_input_source(&request.input_path)?;
    let deck = parse_input_deck(&input_source)?;
    RdinpModel::from_deck(&deck)
}

fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Rdinp {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_MODULE",
            format!(
                "RDINP module expects RDINP, got {}",
                request.module
            ),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.RDINP_INPUT_ARTIFACT",
                format!(
                    "RDINP module expects input artifact '{}' at '{}'",
                    RDINP_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;
    if !input_file_name.eq_ignore_ascii_case(RDINP_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_INPUT_ARTIFACT",
            format!(
                "RDINP module requires input artifact '{}' but received '{}'",
                RDINP_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }
    Ok(())
}

fn read_input_source(input_path: &std::path::Path) -> ComputeResult<String> {
    fs::read_to_string(input_path).map_err(|source| {
        FeffError::io_system(
            "IO.RDINP_INPUT_READ",
            format!(
                "failed to read RDINP input '{}': {}",
                input_path.display(),
                source
            ),
        )
    })
}

fn parse_potentials(deck: &InputDeck) -> ComputeResult<Vec<PotentialEntry>> {
    let mut rows = Vec::new();
    for card in deck
        .cards
        .iter()
        .filter(|card| card.keyword == "POTENTIALS" || card.keyword == "POTENTIAL")
    {
        if card.keyword == "POTENTIAL" && !card.values.is_empty() {
            rows.push((card.source_line, card.values.clone()));
        }
        for continuation in &card.continuations {
            if !continuation.values.is_empty() {
                rows.push((continuation.source_line, continuation.values.clone()));
            }
        }
    }

    if rows.is_empty() {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_POTENTIALS",
            "RDINP requires at least one POTENTIALS row",
        ));
    }

    let mut entries = Vec::with_capacity(rows.len());
    for (line, row) in rows {
        if row.len() < 2 {
            return Err(FeffError::input_validation(
                "INPUT.RDINP_POTENTIALS",
                format!(
                    "invalid POTENTIALS row at line {}: expected at least ipot and atomic number",
                    line
                ),
            ));
        }
        let ipot = parse_i32_token(&row[0], "POTENTIALS ipot", line)?;
        let atomic_number = parse_i32_token(&row[1], "POTENTIALS atomic number", line)?;
        let label = row.get(2).cloned().unwrap_or_else(|| format!("P{}", ipot));
        let explicit_xnatph = match row.get(5) {
            Some(token) => Some(parse_f64_token(token, "POTENTIALS xnatph", line)?),
            None => None,
        };
        entries.push(PotentialEntry {
            ipot,
            atomic_number,
            label,
            explicit_xnatph,
        });
    }
    entries.sort_by_key(|entry| entry.ipot);
    Ok(entries)
}

fn parse_atoms(deck: &InputDeck) -> ComputeResult<Vec<AtomSite>> {
    let mut rows = Vec::new();
    for card in deck.cards.iter().filter(|card| card.keyword == "ATOMS") {
        if !card.values.is_empty() {
            rows.push((card.source_line, card.values.clone()));
        }
        for continuation in &card.continuations {
            if !continuation.values.is_empty() {
                rows.push((continuation.source_line, continuation.values.clone()));
            }
        }
    }

    if rows.is_empty() {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_ATOMS",
            "RDINP requires ATOMS entries",
        ));
    }

    let mut atoms = Vec::with_capacity(rows.len());
    for (line, row) in rows {
        if row.len() < 4 {
            return Err(FeffError::input_validation(
                "INPUT.RDINP_ATOMS",
                format!(
                    "invalid ATOMS row at line {}: expected x y z ipot fields",
                    line
                ),
            ));
        }
        atoms.push(AtomSite {
            x: parse_f64_token(&row[0], "ATOMS x", line)?,
            y: parse_f64_token(&row[1], "ATOMS y", line)?,
            z: parse_f64_token(&row[2], "ATOMS z", line)?,
            ipot: parse_i32_token(&row[3], "ATOMS ipot", line)?,
        });
    }
    Ok(atoms)
}

fn sort_atoms_by_distance(mut atoms: Vec<AtomSite>) -> Vec<AtomSite> {
    if atoms.is_empty() {
        return atoms;
    }

    let absorber_index = atoms.iter().position(|atom| atom.ipot == 0).unwrap_or(0);
    let absorber = atoms[absorber_index];
    let mut distances: Vec<f64> = atoms
        .iter()
        .map(|atom| {
            ((atom.x - absorber.x).powi(2)
                + (atom.y - absorber.y).powi(2)
                + (atom.z - absorber.z).powi(2))
            .sqrt()
        })
        .collect();

    let mut index = 0;
    while index < atoms.len() {
        let mut swap_index = index;
        let mut minimum = distances[index];
        let mut candidate = index;
        while candidate < atoms.len() {
            if distances[candidate] < minimum {
                swap_index = candidate;
                minimum = distances[candidate];
            }
            candidate += 1;
        }
        distances.swap(index, swap_index);
        atoms.swap(index, swap_index);
        index += 1;
    }

    atoms
}

fn card_value(deck: &InputDeck, keyword: &str, index: usize) -> ComputeResult<Option<f64>> {
    let Some(card) = first_card(deck, keyword) else {
        return Ok(None);
    };
    if index >= card.values.len() {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_CARD_VALUE",
            format!(
                "card '{}' at line {} is missing value index {}",
                keyword, card.source_line, index
            ),
        ));
    }
    let value = parse_f64_token(
        card.values[index].as_str(),
        &format!("{} value {}", keyword, index),
        card.source_line,
    )?;
    Ok(Some(value))
}

fn required_card_value(deck: &InputDeck, keyword: &str, index: usize) -> ComputeResult<f64> {
    card_value(deck, keyword, index)?.ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.RDINP_CARD_VALUE",
            format!("missing required card '{}'", keyword),
        )
    })
}

fn parse_f64_token(token: &str, field: &str, line: usize) -> ComputeResult<f64> {
    let normalized = token.replace('D', "E").replace('d', "e");
    normalized.parse::<f64>().map_err(|_| {
        FeffError::input_validation(
            "INPUT.RDINP_CARD_VALUE",
            format!(
                "invalid numeric token '{}' for {} at line {}",
                token, field, line
            ),
        )
    })
}

fn parse_i32_token(token: &str, field: &str, line: usize) -> ComputeResult<i32> {
    if let Ok(value) = token.parse::<i32>() {
        return Ok(value);
    }
    let float_value = parse_f64_token(token, field, line)?;
    let rounded = float_value.round();
    if (float_value - rounded).abs() > 1.0e-9 {
        return Err(FeffError::input_validation(
            "INPUT.RDINP_CARD_VALUE",
            format!(
                "token '{}' for {} at line {} is not an integer",
                token, field, line
            ),
        ));
    }
    Ok(rounded as i32)
}

fn first_card<'a>(deck: &'a InputDeck, keyword: &str) -> Option<&'a InputCard> {
    deck.cards.iter().find(|card| card.keyword == keyword)
}

fn has_card(deck: &InputDeck, keyword: &str) -> bool {
    first_card(deck, keyword).is_some()
}

fn deck_title(deck: &InputDeck) -> String {
    if let Some(card) = first_card(deck, "TITLE")
        && !card.values.is_empty()
    {
        return card.values.join(" ");
    }
    if let Some(card) = first_card(deck, "CIF")
        && let Some(path) = card.values.first()
    {
        return format!("CIF {}", path);
    }
    "FEFF Input".to_string()
}

fn deck_edge_label(deck: &InputDeck) -> String {
    first_card(deck, "EDGE")
        .and_then(|card| card.values.first())
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "NULL".to_string())
}

fn normalize_label(label: &str) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return "X".to_string();
    }
    trimmed.chars().take(4).collect()
}

fn render_lmaxph_line(nph: i32) -> String {
    let count = (nph + 1).max(1) as usize;
    let mut line = String::new();
    for _ in 0..count {
        line.push_str("   3");
    }
    line.push('\n');
    line
}

fn format_f64_13(value: f64) -> String {
    format_fixed_f64(value, 13, 5)
}

fn expected_outputs_for_screen_card(has_screen_card: bool) -> Vec<ComputeArtifact> {
    let mut outputs = RDINP_BASE_OUTPUTS_PREFIX
        .iter()
        .copied()
        .map(ComputeArtifact::new)
        .collect::<Vec<_>>();
    if has_screen_card {
        outputs.push(ComputeArtifact::new(RDINP_OPTIONAL_SCREEN_OUTPUT));
    }
    outputs.extend(
        RDINP_BASE_OUTPUTS_SUFFIX
            .iter()
            .copied()
            .map(ComputeArtifact::new),
    );
    outputs
}

fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::{RdinpModule, expected_outputs_for_screen_card};
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
