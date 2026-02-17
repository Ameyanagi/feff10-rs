use super::parser::{
    AtomSite, PotentialEntry, card_value, deck_edge_label, deck_title, first_card, has_card,
    parse_atoms, parse_potentials, required_card_value, sort_atoms_by_distance,
};
use super::{
    BAND_INP_TEMPLATE, COMPTON_INP_TEMPLATE, CRPA_INP_TEMPLATE, DMDW_INP_TEMPLATE,
    EELS_INP_TEMPLATE, FULLSPECTRUM_INP_TEMPLATE, GENFMT_INP_TEMPLATE, GLOBAL_INP_TEMPLATE,
    RDINP_BASE_OUTPUTS_PREFIX, RDINP_BASE_OUTPUTS_SUFFIX, RDINP_OPTIONAL_SCREEN_OUTPUT,
    RECIPROCAL_INP_TEMPLATE, RIXS_INP_TEMPLATE,
};
use crate::common::edge::{core_hole_lifetime_ev, hole_code_from_edge_spec};
use crate::domain::{ComputeArtifact, ComputeResult, FeffError, InputDeck};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};

#[derive(Debug, Clone)]
pub(super) struct RdinpModel {
    title: String,
    potentials: Vec<PotentialEntry>,
    atoms: Vec<AtomSite>,
    has_xanes: bool,
    ispec: i32,
    nohole: i32,
    ihole: i32,
    gamach: f64,
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
    pub(super) expected_outputs: Vec<ComputeArtifact>,
}

impl RdinpModel {
    pub(super) fn from_deck(deck: &InputDeck) -> ComputeResult<Self> {
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

        let ihole = hole_code_from_input(deck)?;
        let gamach = core_hole_lifetime_ev(absorber_atomic_number(&potentials), ihole);
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
            ihole,
            gamach,
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

    pub(super) fn render_artifact(&self, artifact_path: &str) -> ComputeResult<String> {
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

    pub(super) fn write_artifact(
        &self,
        artifact_path: &str,
        output_path: &std::path::Path,
    ) -> ComputeResult<()> {
        let content = self.render_artifact(artifact_path)?;
        write_text_artifact(output_path, &content).map_err(|source| {
            FeffError::io_system(
                "IO.RDINP_OUTPUT_WRITE",
                format!(
                    "failed to write RDINP artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
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
            1, nph, 1, self.ihole, 0, 0, 0, self.ispec
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
            format_f64_13(self.gamach),
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
            format_f64_13(self.gamach),
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
        content.push_str(&format!(
            " Core hole lifetime set to{:>20.14}      eV.\n",
            self.gamach
        ));
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

fn absorber_atomic_number(potentials: &[PotentialEntry]) -> i32 {
    potentials
        .iter()
        .find(|entry| entry.ipot == 0)
        .or_else(|| potentials.first())
        .map(|entry| entry.atomic_number)
        .unwrap_or(0)
}

fn hole_code_from_input(deck: &InputDeck) -> ComputeResult<i32> {
    if let Some(edge) = first_card(deck, "EDGE").and_then(|card| card.values.first()) {
        return parse_hole_code_token(edge, "EDGE");
    }
    if let Some(hole) = first_card(deck, "HOLE").and_then(|card| card.values.first()) {
        return parse_hole_code_token(hole, "HOLE");
    }
    Ok(1)
}

fn parse_hole_code_token(token: &str, source_card: &str) -> ComputeResult<i32> {
    hole_code_from_edge_spec(token).ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.RDINP_EDGE",
            format!(
                "unrecognized {} value '{}': expected edge label (K, L1, ...) or hole code (0..40)",
                source_card, token
            ),
        )
    })
}

pub(super) fn expected_outputs_for_screen_card(has_screen_card: bool) -> Vec<ComputeArtifact> {
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
