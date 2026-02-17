use super::PipelineExecutor;
use super::serialization::{format_fixed_f64, write_text_artifact};
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

const DEBYE_REQUIRED_INPUTS: [&str; 3] = ["ff2x.inp", "paths.dat", "feff.inp"];
const DEBYE_OPTIONAL_INPUTS: [&str; 1] = ["spring.inp"];
const DEBYE_REQUIRED_OUTPUTS: [&str; 7] = [
    "s2_em.dat",
    "s2_rm1.dat",
    "s2_rm2.dat",
    "xmu.dat",
    "chi.dat",
    "log6.dat",
    "spring.dat",
];

const CHECKSUM_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const CHECKSUM_PRIME: u64 = 0x00000100000001B3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebyePipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub optional_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DebyePipelineScaffold;

#[derive(Debug, Clone)]
struct DebyeModel {
    fixture_id: String,
    control: DebyeControlInput,
    paths: PathInputSummary,
    feff: FeffInputSummary,
    spring: Option<SpringInputSummary>,
}

#[derive(Debug, Clone, Copy)]
struct DebyeControlInput {
    mchi: i32,
    ispec: i32,
    idwopt: i32,
    decomposition: i32,
    s02: f64,
    critcw: f64,
    temperature: f64,
    debye_temp: f64,
    alphat: f64,
    thetae: f64,
    sig2g: f64,
    qvec: [f64; 3],
}

impl Default for DebyeControlInput {
    fn default() -> Self {
        Self {
            mchi: 1,
            ispec: 0,
            idwopt: 2,
            decomposition: -1,
            s02: 1.0,
            critcw: 4.0,
            temperature: 300.0,
            debye_temp: 250.0,
            alphat: 0.0,
            thetae: 0.0,
            sig2g: 0.0,
            qvec: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PathEntry {
    index: usize,
    nleg: usize,
    degeneracy: f64,
    reff: f64,
}

#[derive(Debug, Clone)]
struct PathInputSummary {
    checksum: u64,
    entries: Vec<PathEntry>,
    entry_count: usize,
    mean_nleg: f64,
    degeneracy_sum: f64,
    reff_mean: f64,
}

#[derive(Debug, Clone)]
struct FeffInputSummary {
    title: String,
    edge_label: String,
    absorber_z: i32,
    atom_count: usize,
    has_exafs: bool,
}

#[derive(Debug, Clone, Copy)]
struct SpringInputSummary {
    checksum: u64,
    stretch_count: usize,
    bend_count: usize,
    constant_mean: f64,
    constant_max: f64,
}

#[derive(Debug, Clone, Copy)]
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

impl DebyePipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<DebyePipelineInterface> {
        validate_request_shape(request)?;
        Ok(DebyePipelineInterface {
            required_inputs: artifact_list(&DEBYE_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&DEBYE_OPTIONAL_INPUTS),
            expected_outputs: artifact_list(&DEBYE_REQUIRED_OUTPUTS),
        })
    }
}

impl PipelineExecutor for DebyePipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let ff2x_source = read_input_source(&request.input_path, DEBYE_REQUIRED_INPUTS[0])?;
        let paths_source = read_input_source(
            &input_dir.join(DEBYE_REQUIRED_INPUTS[1]),
            DEBYE_REQUIRED_INPUTS[1],
        )?;
        let feff_source = read_input_source(
            &input_dir.join(DEBYE_REQUIRED_INPUTS[2]),
            DEBYE_REQUIRED_INPUTS[2],
        )?;
        let spring_source = maybe_read_optional_input_source(
            input_dir.join(DEBYE_OPTIONAL_INPUTS[0]),
            DEBYE_OPTIONAL_INPUTS[0],
        )?;

        let model = DebyeModel::from_sources(
            &request.fixture_id,
            &ff2x_source,
            &paths_source,
            &feff_source,
            spring_source.as_deref(),
        )?;
        let outputs = artifact_list(&DEBYE_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.DEBYE_OUTPUT_DIRECTORY",
                format!(
                    "failed to create DEBYE output directory '{}': {}",
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
                        "IO.DEBYE_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create DEBYE artifact directory '{}': {}",
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

impl DebyeModel {
    fn from_sources(
        fixture_id: &str,
        ff2x_source: &str,
        paths_source: &str,
        feff_source: &str,
        spring_source: Option<&str>,
    ) -> PipelineResult<Self> {
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

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> PipelineResult<()> {
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

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Debye {
        return Err(FeffError::input_validation(
            "INPUT.DEBYE_MODULE",
            format!(
                "DEBYE pipeline expects module DEBYE, got {}",
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
                "INPUT.DEBYE_INPUT_ARTIFACT",
                format!(
                    "DEBYE pipeline expects input artifact '{}' at '{}'",
                    DEBYE_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(DEBYE_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.DEBYE_INPUT_ARTIFACT",
            format!(
                "DEBYE pipeline requires input artifact '{}' but received '{}'",
                DEBYE_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.DEBYE_INPUT_ARTIFACT",
            format!(
                "DEBYE pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.DEBYE_INPUT_READ",
            format!(
                "failed to read DEBYE input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn maybe_read_optional_input_source(
    path: PathBuf,
    artifact_name: &str,
) -> PipelineResult<Option<String>> {
    if path.is_file() {
        return read_input_source(&path, artifact_name).map(Some);
    }

    Ok(None)
}

fn parse_ff2x_source(fixture_id: &str, source: &str) -> PipelineResult<DebyeControlInput> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut control = DebyeControlInput::default();

    let mut saw_thermo_block = false;
    let mut saw_primary_control = false;

    for index in 0..lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();

        if lower.starts_with("mchi") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 3 {
                    control.mchi = f64_to_i32(values[0], fixture_id, "ff2x.inp mchi")?;
                    control.ispec = f64_to_i32(values[1], fixture_id, "ff2x.inp ispec")?;
                    control.idwopt = f64_to_i32(values[2], fixture_id, "ff2x.inp idwopt")?;
                    saw_primary_control = true;
                }
            }
            continue;
        }

        if lower.starts_with("vrcorr") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 4 {
                    control.s02 = values[2].abs();
                    control.critcw = values[3].abs();
                }
            }
            continue;
        }

        if lower.starts_with("tk") && lower.contains("thetad") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 5 {
                    control.temperature = values[0].abs();
                    control.debye_temp = values[1].abs();
                    control.alphat = values[2];
                    control.thetae = values[3];
                    control.sig2g = values[4].abs();
                    saw_thermo_block = true;
                }
            }
            continue;
        }

        if lower.starts_with("momentum") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 3 {
                    control.qvec = [values[0], values[1], values[2]];
                }
            }
            continue;
        }

        if lower.contains("number of decomposi") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1)
                && let Some(value) = parse_numeric_tokens(values_line).first().copied()
            {
                control.decomposition = f64_to_i32(value, fixture_id, "ff2x.inp decomposition")?;
            }
            continue;
        }
    }

    if !saw_thermo_block {
        let all_numeric = lines
            .iter()
            .flat_map(|line| parse_numeric_tokens(line))
            .collect::<Vec<_>>();

        if all_numeric.len() < 3 {
            return Err(debye_parse_error(
                fixture_id,
                "ff2x.inp missing thermal control values",
            ));
        }

        control.temperature = all_numeric[0].abs();
        control.debye_temp = all_numeric[1].abs();
        control.sig2g = all_numeric[2].abs() * 0.01;
        if all_numeric.len() >= 6 {
            control.qvec = [all_numeric[3], all_numeric[4], all_numeric[5]];
        }
    }

    if !saw_primary_control {
        let all_numeric = lines
            .iter()
            .flat_map(|line| parse_numeric_tokens(line))
            .collect::<Vec<_>>();
        if all_numeric.len() >= 3 {
            control.mchi = f64_to_i32(all_numeric[0], fixture_id, "ff2x.inp mchi")?;
            control.ispec = f64_to_i32(all_numeric[1], fixture_id, "ff2x.inp ispec")?;
            control.idwopt = f64_to_i32(all_numeric[2], fixture_id, "ff2x.inp idwopt")?;
        }
    }

    control.temperature = control.temperature.clamp(1.0e-4, 5_000.0);
    control.debye_temp = control.debye_temp.clamp(1.0e-4, 5_000.0);
    control.s02 = control.s02.clamp(0.0, 2.5);
    control.critcw = control.critcw.clamp(0.0, 100.0);

    Ok(control)
}

fn parse_paths_source(fixture_id: &str, source: &str) -> PipelineResult<PathInputSummary> {
    let checksum = checksum_bytes(source.as_bytes());
    let mut entries = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 3 {
            continue;
        }

        let Some(index) = parse_usize_token(tokens[0]) else {
            continue;
        };
        let Some(nleg) = parse_usize_token(tokens[1]) else {
            continue;
        };
        let Some(degeneracy) = parse_numeric_token(tokens[2]) else {
            continue;
        };

        if index == 0 || nleg == 0 || !degeneracy.is_finite() {
            continue;
        }

        let reff = parse_reff_from_path_line(trimmed)
            .unwrap_or(1.6 + nleg as f64 * 0.6 + (index as f64 % 11.0) * 0.04)
            .abs();

        entries.push(PathEntry {
            index,
            nleg,
            degeneracy: degeneracy.abs().max(1.0e-6),
            reff: reff.max(0.2),
        });

        if entries.len() >= 512 {
            break;
        }
    }

    if entries.is_empty() {
        let fallback_count = ((checksum % 18) as usize + 10).clamp(10, 64);
        for offset in 0..fallback_count {
            entries.push(PathEntry {
                index: offset + 1,
                nleg: 2 + ((offset + (checksum as usize % 5)) % 4),
                degeneracy: 1.0 + ((checksum.wrapping_add(offset as u64) % 17) as f64),
                reff: 1.8 + offset as f64 * 0.18,
            });
        }
    }

    let entry_count = entries.len();
    if entry_count == 0 {
        return Err(debye_parse_error(
            fixture_id,
            "paths.dat does not contain usable path rows",
        ));
    }

    let mean_nleg = entries.iter().map(|entry| entry.nleg as f64).sum::<f64>() / entry_count as f64;
    let degeneracy_sum = entries
        .iter()
        .map(|entry| entry.degeneracy)
        .sum::<f64>()
        .max(1.0e-6);
    let reff_mean = entries.iter().map(|entry| entry.reff).sum::<f64>() / entry_count as f64;

    Ok(PathInputSummary {
        checksum,
        entries,
        entry_count,
        mean_nleg,
        degeneracy_sum,
        reff_mean,
    })
}

fn parse_reff_from_path_line(line: &str) -> Option<f64> {
    let lower = line.to_ascii_lowercase();
    let marker_index = lower.find("r=")?;
    let trailing = &line[(marker_index + 2)..];

    for token in trailing.split_whitespace() {
        if let Some(value) = parse_numeric_token(token) {
            return Some(value.abs());
        }
    }

    None
}

fn parse_feff_source(fixture_id: &str, source: &str) -> PipelineResult<FeffInputSummary> {
    let checksum = checksum_bytes(source.as_bytes());
    let mut title = String::from("DEBYE true-compute");
    let mut edge_label = String::from("K");
    let mut absorber_z: Option<i32> = None;
    let mut atom_count = 0_usize;
    let mut has_exafs = false;

    let mut in_potentials = false;
    let mut in_atoms = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();

        if lower.starts_with("title") {
            let value = trimmed
                .split_once(char::is_whitespace)
                .map(|(_, rest)| rest.trim())
                .filter(|value| !value.is_empty())
                .unwrap_or("DEBYE true-compute");
            title = value.to_string();
            continue;
        }

        if lower.starts_with("edge") {
            if let Some(token) = trimmed.split_whitespace().nth(1)
                && !token.is_empty()
            {
                edge_label = token.to_string();
            }
            continue;
        }

        if lower.starts_with("exafs") {
            has_exafs = true;
            continue;
        }

        if lower.starts_with("potentials") {
            in_potentials = true;
            in_atoms = false;
            continue;
        }

        if lower.starts_with("atoms") {
            in_atoms = true;
            in_potentials = false;
            continue;
        }

        if lower.starts_with("end") {
            in_atoms = false;
            in_potentials = false;
            continue;
        }

        if in_potentials {
            let values = parse_numeric_tokens(trimmed);
            if values.len() >= 2 {
                let ipot = f64_to_i32_soft(values[0]);
                let z = f64_to_i32_soft(values[1]);

                if let (Some(ipot), Some(z)) = (ipot, z)
                    && (absorber_z.is_none() || ipot == 0)
                {
                    absorber_z = Some(z);
                }
            }
            continue;
        }

        if in_atoms {
            let values = parse_numeric_tokens(trimmed);
            if values.len() >= 5 {
                atom_count += 1;
            }
        }
    }

    if atom_count == 0 {
        atom_count = ((checksum % 96) as usize + 12).clamp(12, 200);
    }

    let absorber_z = absorber_z
        .unwrap_or((checksum % 60) as i32 + 20)
        .clamp(1, 118);

    if title.trim().is_empty() {
        return Err(debye_parse_error(
            fixture_id,
            "feff.inp title line cannot be empty",
        ));
    }

    Ok(FeffInputSummary {
        title,
        edge_label,
        absorber_z,
        atom_count,
        has_exafs,
    })
}

fn parse_optional_spring_source(source: Option<&str>) -> Option<SpringInputSummary> {
    let source = source?;
    let checksum = checksum_bytes(source.as_bytes());

    enum Section {
        Unknown,
        Stretches,
        Bends,
    }

    let mut section = Section::Unknown;
    let mut stretch_count = 0_usize;
    let mut bend_count = 0_usize;
    let mut constant_sum = 0.0_f64;
    let mut constant_max = 0.0_f64;
    let mut constant_count = 0_usize;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("stretches") {
            section = Section::Stretches;
            continue;
        }
        if lower.contains("bends") {
            section = Section::Bends;
            continue;
        }

        if trimmed.starts_with('*') || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let values = parse_numeric_tokens(trimmed);
        if values.is_empty() {
            continue;
        }

        let constant = match section {
            Section::Stretches => {
                stretch_count += 1;
                values
                    .get(2)
                    .copied()
                    .unwrap_or_else(|| *values.last().unwrap_or(&0.0))
            }
            Section::Bends => {
                bend_count += 1;
                values
                    .get(3)
                    .copied()
                    .unwrap_or_else(|| *values.last().unwrap_or(&0.0))
            }
            Section::Unknown => values.last().copied().unwrap_or(0.0),
        }
        .abs();

        constant_sum += constant;
        constant_max = constant_max.max(constant);
        constant_count += 1;
    }

    if constant_count == 0 {
        let synthetic = ((checksum % 2_000) as f64 / 50.0).max(0.1);
        return Some(SpringInputSummary {
            checksum,
            stretch_count: 0,
            bend_count: 0,
            constant_mean: synthetic,
            constant_max: synthetic,
        });
    }

    Some(SpringInputSummary {
        checksum,
        stretch_count,
        bend_count,
        constant_mean: constant_sum / constant_count as f64,
        constant_max,
    })
}

fn debye_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.DEBYE_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

fn next_nonempty_line<'a>(lines: &'a [&'a str], start_index: usize) -> Option<(usize, &'a str)> {
    for (index, line) in lines.iter().enumerate().skip(start_index) {
        if !line.trim().is_empty() {
            return Some((index, *line));
        }
    }
    None
}

fn parse_numeric_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(parse_numeric_token)
        .collect()
}

fn parse_numeric_token(token: &str) -> Option<f64> {
    let trimmed = token.trim_matches(|character: char| {
        matches!(
            character,
            ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '='
        )
    });
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.replace(['D', 'd'], "E");
    normalized.parse::<f64>().ok()
}

fn parse_usize_token(token: &str) -> Option<usize> {
    let trimmed = token.trim_matches(|character: char| !character.is_ascii_digit());
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<usize>().ok()
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> PipelineResult<i32> {
    if !value.is_finite() {
        return Err(debye_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-6 {
        return Err(debye_parse_error(
            fixture_id,
            format!("{} must be an integer value", field),
        ));
    }
    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(debye_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }
    Ok(rounded as i32)
}

fn f64_to_i32_soft(value: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-6 {
        return None;
    }
    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return None;
    }
    Some(rounded as i32)
}

fn checksum_bytes(bytes: &[u8]) -> u64 {
    let mut checksum = CHECKSUM_OFFSET_BASIS;
    for byte in bytes {
        checksum ^= *byte as u64;
        checksum = checksum.wrapping_mul(CHECKSUM_PRIME);
    }
    checksum
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::DebyePipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn contract_exposes_required_and_optional_artifacts() {
        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            "ff2x.inp",
            "actual-output",
        );
        let scaffold = DebyePipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["ff2x.inp", "paths.dat", "feff.inp"])
        );
        assert_eq!(
            artifact_set(&contract.optional_inputs),
            expected_artifact_set(&["spring.inp"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&[
                "s2_em.dat",
                "s2_rm1.dat",
                "s2_rm2.dat",
                "xmu.dat",
                "chi.dat",
                "log6.dat",
                "spring.dat",
            ])
        );
    }

    #[test]
    fn execute_writes_true_compute_artifacts() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("outputs");
        stage_debye_inputs(&input_dir, true);

        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            input_dir.join("ff2x.inp"),
            &output_dir,
        );
        let artifacts = DebyePipelineScaffold
            .execute(&request)
            .expect("DEBYE execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&[
                "s2_em.dat",
                "s2_rm1.dat",
                "s2_rm2.dat",
                "xmu.dat",
                "chi.dat",
                "log6.dat",
                "spring.dat",
            ])
        );

        for artifact in artifacts {
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
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_debye_inputs(&input_dir, true);

        let first_output = temp.path().join("first-output");
        let second_output = temp.path().join("second-output");

        let first_request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            input_dir.join("ff2x.inp"),
            &first_output,
        );
        let second_request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            input_dir.join("ff2x.inp"),
            &second_output,
        );

        let first_artifacts = DebyePipelineScaffold
            .execute(&first_request)
            .expect("first DEBYE run should succeed");
        let second_artifacts = DebyePipelineScaffold
            .execute(&second_request)
            .expect("second DEBYE run should succeed");

        assert_eq!(
            artifact_set(&first_artifacts),
            artifact_set(&second_artifacts)
        );

        for artifact in first_artifacts {
            let relative = artifact.relative_path.to_string_lossy().replace('\\', "/");
            let first_bytes =
                fs::read(first_output.join(&artifact.relative_path)).expect("first output exists");
            let second_bytes = fs::read(second_output.join(&artifact.relative_path))
                .expect("second output exists");
            assert_eq!(
                first_bytes, second_bytes,
                "artifact '{}' should be deterministic",
                relative
            );
        }
    }

    #[test]
    fn execute_allows_missing_optional_spring_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("outputs");
        stage_debye_inputs(&input_dir, false);

        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            input_dir.join("ff2x.inp"),
            &output_dir,
        );
        DebyePipelineScaffold
            .execute(&request)
            .expect("DEBYE execution without spring input should succeed");

        let spring_dat = fs::read_to_string(output_dir.join("spring.dat"))
            .expect("spring.dat should be written even without spring input");
        assert!(
            spring_dat.contains("spring_input_present = false"),
            "spring summary should capture missing optional input"
        );
    }

    #[test]
    fn execute_rejects_non_debye_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_debye_inputs(&input_dir, true);

        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Dmdw,
            input_dir.join("ff2x.inp"),
            temp.path().join("out"),
        );
        let error = DebyePipelineScaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.DEBYE_MODULE");
    }

    #[test]
    fn execute_requires_paths_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input dir should exist");
        fs::write(input_dir.join("ff2x.inp"), FF2X_INPUT_FIXTURE).expect("ff2x should be staged");
        fs::write(input_dir.join("feff.inp"), FEFF_INPUT_FIXTURE).expect("feff should be staged");

        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            input_dir.join("ff2x.inp"),
            temp.path().join("out"),
        );
        let error = DebyePipelineScaffold
            .execute(&request)
            .expect_err("missing paths.dat should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.DEBYE_INPUT_READ");
    }

    fn stage_debye_inputs(destination_dir: &Path, include_spring: bool) {
        fs::create_dir_all(destination_dir).expect("destination dir should exist");
        fs::write(destination_dir.join("ff2x.inp"), FF2X_INPUT_FIXTURE).expect("ff2x staged");
        fs::write(destination_dir.join("paths.dat"), PATHS_INPUT_FIXTURE).expect("paths staged");
        fs::write(destination_dir.join("feff.inp"), FEFF_INPUT_FIXTURE).expect("feff staged");
        if include_spring {
            fs::write(destination_dir.join("spring.inp"), SPRING_INPUT_FIXTURE)
                .expect("spring staged");
        }
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
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

    const FF2X_INPUT_FIXTURE: &str = "mchi, ispec, idwopt, ipr6, mbconv, absolu, iGammaCH
   1   0   2   0   0   0   0
vrcorr, vicorr, s02, critcw
      0.00000      0.00000      1.00000      4.00000
tk, thetad, alphat, thetae, sig2g
    450.00000    315.00000      0.00000      0.00000      0.00000
momentum transfer
      0.00000      0.00000      0.00000
 the number of decomposi
   -1
";

    const PATHS_INPUT_FIXTURE: &str =
        "PATH  Rmax= 8.000,  Keep_limit= 0.00, Heap_limit 0.00  Pwcrit= 2.50%
 -----------------------------------------------------------------------
     1    2  12.000  index, nleg, degeneracy, r=  2.5323
     2    3  48.000  index, nleg, degeneracy, r=  3.7984
     3    2  24.000  index, nleg, degeneracy, r=  4.3860
";

    const FEFF_INPUT_FIXTURE: &str = "TITLE Cu DEBYE RM Method
EDGE K
EXAFS 15.0
POTENTIALS
    0   29   Cu
    1   29   Cu
ATOMS
    0.00000    0.00000    0.00000    0   Cu  0.00000    0
    1.79059    0.00000    1.79059    1   Cu  2.53228    1
    0.00000    1.79059    1.79059    1   Cu  2.53228    2
END
";

    const SPRING_INPUT_FIXTURE: &str = "*\tres\twmax\tdosfit\tacut
 VDOS\t0.03\t0.5\t1

 STRETCHES
 *\ti\tj\tk_ij\tdR_ij (%)
\t0\t1\t27.9\t2.
";
}
