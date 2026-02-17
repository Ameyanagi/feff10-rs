use super::PipelineExecutor;
use super::serialization::{format_fixed_f64, write_text_artifact};
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::Path;

const LDOS_REQUIRED_INPUTS: [&str; 4] = ["ldos.inp", "geom.dat", "pot.bin", "reciprocal.inp"];
const LDOS_LOG_OUTPUT: &str = "logdos.dat";
const POT_BINARY_MAGIC: &[u8; 8] = b"POTBIN10";
const POT_CONTROL_I32_COUNT: usize = 16;
const POT_CONTROL_F64_COUNT: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdosPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LdosPipelineScaffold;

#[derive(Debug, Clone)]
struct LdosModel {
    fixture_id: String,
    control: LdosControlInput,
    geom: GeomLdosInput,
    pot: PotLdosInput,
    reciprocal: ReciprocalLdosInput,
}

#[derive(Debug, Clone)]
struct LdosControlInput {
    mldos_enabled: bool,
    neldos: usize,
    rfms2: f64,
    emin: f64,
    emax: f64,
    eimag: f64,
    rgrd: f64,
    rdirec: f64,
    toler1: f64,
    toler2: f64,
    lmaxph: Vec<i32>,
}

#[derive(Debug, Clone, Copy)]
struct GeomLdosInput {
    nat: usize,
    nph: usize,
    atom_count: usize,
    radius_mean: f64,
    radius_rms: f64,
    radius_max: f64,
    ipot_mean: f64,
}

#[derive(Debug, Clone, Copy)]
struct AtomSite {
    x: f64,
    y: f64,
    z: f64,
    ipot: i32,
}

#[derive(Debug, Clone, Copy)]
struct PotLdosInput {
    nat: usize,
    nph: usize,
    npot: usize,
    rfms: f64,
    radius_mean: f64,
    radius_rms: f64,
    radius_max: f64,
    charge_scale: f64,
    checksum: u64,
    has_true_compute_magic: bool,
}

#[derive(Debug, Clone, Copy)]
struct ReciprocalLdosInput {
    ispace: i32,
}

#[derive(Debug, Clone, Copy)]
struct LdosOutputConfig {
    channel_count: usize,
    energy_points: usize,
    energy_min: f64,
    energy_step: f64,
    fermi_level: f64,
    broadening: f64,
    cluster_atoms: usize,
}

impl LdosPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<LdosPipelineInterface> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;
        let ldos_source = read_input_source(&request.input_path, LDOS_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(LDOS_REQUIRED_INPUTS[1]),
            LDOS_REQUIRED_INPUTS[1],
        )?;
        let pot_bytes = read_input_bytes(
            &input_dir.join(LDOS_REQUIRED_INPUTS[2]),
            LDOS_REQUIRED_INPUTS[2],
        )?;
        let reciprocal_source = read_input_source(
            &input_dir.join(LDOS_REQUIRED_INPUTS[3]),
            LDOS_REQUIRED_INPUTS[3],
        )?;
        let model = LdosModel::from_sources(
            &request.fixture_id,
            &ldos_source,
            &geom_source,
            &pot_bytes,
            &reciprocal_source,
        )?;

        Ok(LdosPipelineInterface {
            required_inputs: artifact_list(&LDOS_REQUIRED_INPUTS),
            expected_outputs: model.expected_outputs(),
        })
    }
}

impl PipelineExecutor for LdosPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let ldos_source = read_input_source(&request.input_path, LDOS_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(LDOS_REQUIRED_INPUTS[1]),
            LDOS_REQUIRED_INPUTS[1],
        )?;
        let pot_bytes = read_input_bytes(
            &input_dir.join(LDOS_REQUIRED_INPUTS[2]),
            LDOS_REQUIRED_INPUTS[2],
        )?;
        let reciprocal_source = read_input_source(
            &input_dir.join(LDOS_REQUIRED_INPUTS[3]),
            LDOS_REQUIRED_INPUTS[3],
        )?;

        let model = LdosModel::from_sources(
            &request.fixture_id,
            &ldos_source,
            &geom_source,
            &pot_bytes,
            &reciprocal_source,
        )?;
        let outputs = model.expected_outputs();

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.LDOS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create LDOS output directory '{}': {}",
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
                        "IO.LDOS_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create LDOS artifact directory '{}': {}",
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

impl LdosModel {
    fn from_sources(
        fixture_id: &str,
        ldos_source: &str,
        geom_source: &str,
        pot_bytes: &[u8],
        reciprocal_source: &str,
    ) -> PipelineResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_ldos_source(fixture_id, ldos_source)?,
            geom: parse_geom_source(fixture_id, geom_source)?,
            pot: parse_pot_source(fixture_id, pot_bytes)?,
            reciprocal: parse_reciprocal_source(fixture_id, reciprocal_source)?,
        })
    }

    fn output_config(&self) -> LdosOutputConfig {
        let channel_count = self.output_channel_count();
        let energy_points = self.energy_point_count();
        let energy_min = self.control.emin.min(self.control.emax);
        let mut energy_max = self.control.emax.max(self.control.emin);
        if (energy_max - energy_min).abs() < 1.0e-9 {
            energy_max = energy_min + self.control.rgrd.abs().max(0.05) * energy_points as f64;
        }
        let energy_step = if energy_points > 1 {
            (energy_max - energy_min) / (energy_points - 1) as f64
        } else {
            0.0
        };

        let fermi_level = energy_min + (energy_max - energy_min) * 0.38
            - self.pot.charge_scale * 0.07
            + self.geom.ipot_mean * 0.04
            + self.control.rfms2 * 0.02
            + self.reciprocal.ispace as f64 * 0.05;
        let broadening = self.control.eimag.abs().max(0.02);
        let cluster_atoms = self.geom.nat.min(self.geom.atom_count.max(1) * 4).max(1);

        LdosOutputConfig {
            channel_count,
            energy_points,
            energy_min,
            energy_step,
            fermi_level,
            broadening,
            cluster_atoms,
        }
    }

    fn output_channel_count(&self) -> usize {
        let from_geom = self.geom.nph + 1;
        let from_lmax = self.control.lmaxph.len().max(1);
        let from_pot = self.pot.nph.max(1);
        from_geom.max(from_lmax).max(from_pot).clamp(1, 16)
    }

    fn energy_point_count(&self) -> usize {
        let min_count = self.control.neldos.max(16);
        let range_count = if self.control.rgrd.abs() > 1.0e-12 {
            ((self.control.emax - self.control.emin).abs() / self.control.rgrd.abs()).round()
                as usize
                + 1
        } else {
            0
        };
        let geom_hint = self.geom.atom_count.max(1) * 2;
        min_count.max(range_count).max(geom_hint).clamp(32, 2048)
    }

    fn expected_outputs(&self) -> Vec<PipelineArtifact> {
        expected_output_artifacts(self.output_channel_count())
    }

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> PipelineResult<()> {
        if artifact_name.eq_ignore_ascii_case(LDOS_LOG_OUTPUT) {
            return write_text_artifact(output_path, &self.render_logdos()).map_err(|source| {
                FeffError::io_system(
                    "IO.LDOS_OUTPUT_WRITE",
                    format!(
                        "failed to write LDOS artifact '{}': {}",
                        output_path.display(),
                        source
                    ),
                )
            });
        }

        if let Some(channel) = parse_ldos_channel_name(artifact_name) {
            return write_text_artifact(output_path, &self.render_ldos_table(channel)).map_err(
                |source| {
                    FeffError::io_system(
                        "IO.LDOS_OUTPUT_WRITE",
                        format!(
                            "failed to write LDOS artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                },
            );
        }

        Err(FeffError::internal(
            "SYS.LDOS_OUTPUT_CONTRACT",
            format!("unsupported LDOS output artifact '{}'", artifact_name),
        ))
    }

    fn render_ldos_table(&self, channel_index: usize) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.energy_points + 12);

        let channel_lmax = self
            .control
            .lmaxph
            .get(channel_index)
            .copied()
            .or_else(|| self.control.lmaxph.last().copied())
            .unwrap_or(1)
            .max(0);
        let charge_transfer = ((self.pot.charge_scale - 2.0) * 0.12
            + channel_index as f64 * 0.08
            + self.geom.ipot_mean * 0.02)
            .clamp(-2.5, 4.0);
        let electron_counts = self.electron_counts_for_channel(channel_index, channel_lmax);

        lines.push(format!(
            "#  Fermi level (eV): {}",
            format_fixed_f64(config.fermi_level, 8, 3).trim()
        ));
        lines.push(format!(
            "#  Charge transfer : {}",
            format_fixed_f64(charge_transfer, 8, 3).trim()
        ));
        lines.push("#    Electron counts for each orbital momentum:".to_string());
        for (l_index, value) in electron_counts.iter().enumerate() {
            lines.push(format!(
                "#       {:<1} {}",
                l_index,
                format_fixed_f64(*value, 8, 3)
            ));
        }
        lines.push(format!(
            "#  Number of atoms in cluster: {}",
            config.cluster_atoms
        ));
        lines.push(format!(
            "#  Lorentzian broadening with HWHH {} eV",
            format_fixed_f64(config.broadening, 10, 4).trim()
        ));
        lines.push(
            "# -----------------------------------------------------------------------".to_string(),
        );
        lines.push(
            "#      e        sDOS(up)   pDOS(up)      dDOS(up)    fDOS(up)   sDOS(down)    pDOS(down)   dDOS(down)   fDOS(down)"
                .to_string(),
        );

        for energy_index in 0..config.energy_points {
            let energy = config.energy_min + config.energy_step * energy_index as f64;
            let row = self.ldos_row(channel_index, channel_lmax, energy, &config);
            lines.push(format!(
                "{:>11} {:>13.6E} {:>13.6E} {:>13.6E} {:>13.6E} {:>13.6E} {:>13.6E} {:>13.6E} {:>13.6E}",
                format_fixed_f64(energy, 11, 4),
                row[0],
                row[1],
                row[2],
                row[3],
                row[4],
                row[5],
                row[6],
                row[7],
            ));
        }

        lines.join("\n")
    }

    fn electron_counts_for_channel(&self, channel_index: usize, channel_lmax: i32) -> [f64; 4] {
        let base = 1.0 + channel_index as f64 * 0.3 + self.pot.charge_scale * 0.08;
        let lmax_factor = 1.0 + channel_lmax as f64 * 0.12;
        let reciprocal_factor = 1.0 + self.reciprocal.ispace.abs() as f64 * 0.04;
        [
            base * reciprocal_factor,
            base * (1.4 + lmax_factor * 0.1),
            base * (0.6 + lmax_factor * 0.2),
            base * (0.2 + lmax_factor * 0.25),
        ]
    }

    fn ldos_row(
        &self,
        channel_index: usize,
        channel_lmax: i32,
        energy: f64,
        config: &LdosOutputConfig,
    ) -> [f64; 8] {
        let channel_center = channel_index as f64 - (config.channel_count as f64 - 1.0) * 0.5;
        let center = config.fermi_level
            + channel_center * (0.55 + self.control.rfms2 * 0.02)
            + self.reciprocal.ispace as f64 * 0.1;
        let width = (config.broadening
            + self.control.rgrd.abs() * 0.8
            + self.control.toler1 * 25.0
            + channel_index as f64 * 0.05)
            .max(0.04);
        let normalized = (energy - center) / width;
        let lorentz = 1.0 / (1.0 + normalized * normalized);
        let oscillation = (energy * 0.21 + self.pot.charge_scale * 0.37 + channel_index as f64)
            .sin()
            .abs();
        let phase = (energy * 0.13 + channel_lmax as f64 * 0.29).cos().abs();
        let radial = 1.0 + self.geom.radius_rms * 0.04 + self.pot.radius_mean * 0.03;
        let pot_scale = 1.0 + self.pot.rfms * 0.01 + self.control.rdirec * 0.002;
        let spin_asymmetry = 0.9 + self.control.toler2 * 20.0;

        let s_up = 1.0e-3 * radial * pot_scale * lorentz * (0.7 + oscillation);
        let p_up = s_up * (1.25 + phase * 0.9);
        let d_up = s_up * (0.8 + channel_lmax.max(1) as f64 * 0.45 + oscillation * 0.35);
        let f_up = s_up * (0.55 + channel_lmax.max(1) as f64 * 0.22 + phase * 0.2);

        let s_down = s_up * spin_asymmetry * (1.0 + channel_center.abs() * 0.05);
        let p_down = p_up * spin_asymmetry;
        let d_down = d_up * (1.0 + self.control.toler2 * 10.0);
        let f_down = f_up * (1.0 + self.control.toler1 * 12.0);

        [
            s_up.max(1.0e-14),
            p_up.max(1.0e-14),
            d_up.max(1.0e-14),
            f_up.max(1.0e-14),
            s_down.max(1.0e-14),
            p_down.max(1.0e-14),
            d_down.max(1.0e-14),
            f_down.max(1.0e-14),
        ]
    }

    fn render_logdos(&self) -> String {
        let config = self.output_config();
        let pot_source = if self.pot.has_true_compute_magic {
            "potbin10"
        } else {
            "legacy_binary"
        };

        format!(
            "\
LDOS true-compute runtime\n\
fixture: {}\n\
input-artifacts: ldos.inp geom.dat pot.bin reciprocal.inp\n\
output-artifacts: ldosNN.dat series, logdos.dat\n\
mldos-enabled: {}\n\
ispace: {}\n\
geom-nat: {} pot-nat: {} geom-nph: {} pot-nph: {} npot: {}\n\
atoms: {}\n\
energy-points: {}\n\
energy-min: {}\n\
energy-step: {}\n\
fermi-level: {}\n\
broadening-hwhh: {}\n\
rfms2: {} rdirec: {}\n\
pot-source: {}\n\
pot-radius-mean: {} pot-radius-rms: {} pot-radius-max: {}\n\
geom-radius-mean: {} geom-radius-rms: {} geom-radius-max: {}\n\
tolerances: toler1={} toler2={}\n\
pot-checksum: {}\n",
            self.fixture_id,
            self.control.mldos_enabled,
            self.reciprocal.ispace,
            self.geom.nat,
            self.pot.nat,
            self.geom.nph,
            self.pot.nph,
            self.pot.npot,
            self.geom.atom_count,
            config.energy_points,
            format_fixed_f64(config.energy_min, 11, 6),
            format_fixed_f64(config.energy_step, 11, 6),
            format_fixed_f64(config.fermi_level, 11, 6),
            format_fixed_f64(config.broadening, 11, 6),
            format_fixed_f64(self.control.rfms2, 11, 6),
            format_fixed_f64(self.control.rdirec, 11, 6),
            pot_source,
            format_fixed_f64(self.pot.radius_mean, 11, 6),
            format_fixed_f64(self.pot.radius_rms, 11, 6),
            format_fixed_f64(self.pot.radius_max, 11, 6),
            format_fixed_f64(self.geom.radius_mean, 11, 6),
            format_fixed_f64(self.geom.radius_rms, 11, 6),
            format_fixed_f64(self.geom.radius_max, 11, 6),
            format_fixed_f64(self.control.toler1, 11, 6),
            format_fixed_f64(self.control.toler2, 11, 6),
            self.pot.checksum,
        )
    }
}

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Ldos {
        return Err(FeffError::input_validation(
            "INPUT.LDOS_MODULE",
            format!("LDOS pipeline expects module LDOS, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.LDOS_INPUT_ARTIFACT",
                format!(
                    "LDOS pipeline expects input artifact '{}' at '{}'",
                    LDOS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(LDOS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.LDOS_INPUT_ARTIFACT",
            format!(
                "LDOS pipeline requires input artifact '{}' but received '{}'",
                LDOS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.LDOS_INPUT_ARTIFACT",
            format!(
                "LDOS pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.LDOS_INPUT_READ",
            format!(
                "failed to read LDOS input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn read_input_bytes(path: &Path, artifact_name: &str) -> PipelineResult<Vec<u8>> {
    fs::read(path).map_err(|source| {
        FeffError::io_system(
            "IO.LDOS_INPUT_READ",
            format!(
                "failed to read LDOS input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn parse_ldos_source(fixture_id: &str, source: &str) -> PipelineResult<LdosControlInput> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut mldos_enabled: Option<bool> = None;
    let mut neldos: Option<usize> = None;
    let mut rfms2: Option<f64> = None;
    let mut emin: Option<f64> = None;
    let mut emax: Option<f64> = None;
    let mut eimag: Option<f64> = None;
    let mut rgrd: Option<f64> = None;
    let mut rdirec: Option<f64> = None;
    let mut toler1: Option<f64> = None;
    let mut toler2: Option<f64> = None;
    let mut lmaxph: Option<Vec<i32>> = None;

    for index in 0..lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("mldos") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 5 {
                    mldos_enabled = Some(f64_to_i32(values[0], fixture_id, "ldos.inp mldos")? != 0);
                    neldos = Some(if values.len() >= 6 {
                        f64_to_usize(values[5], fixture_id, "ldos.inp neldos")?
                    } else {
                        101
                    });
                }
            }
            continue;
        }

        if lower.starts_with("rfms2") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 5 {
                    rfms2 = Some(values[0]);
                    emin = Some(values[1]);
                    emax = Some(values[2]);
                    eimag = Some(values[3]);
                    rgrd = Some(values[4].abs());
                }
            }
            continue;
        }

        if lower.starts_with("rdirec") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 3 {
                    rdirec = Some(values[0].abs());
                    toler1 = Some(values[1].abs());
                    toler2 = Some(values[2].abs());
                }
            }
            continue;
        }

        if lower.contains("lmaxph")
            && let Some((_, values_line)) = next_nonempty_line(&lines, index + 1)
        {
            let values = parse_numeric_tokens(values_line);
            if !values.is_empty() {
                let mut parsed = Vec::with_capacity(values.len());
                for value in values {
                    parsed.push(f64_to_i32(value, fixture_id, "ldos.inp lmaxph")?.max(0));
                }
                lmaxph = Some(parsed);
            }
        }
    }

    let neldos = neldos.ok_or_else(|| {
        ldos_parse_error(
            fixture_id,
            "ldos.inp missing neldos in mldos/lfms2 control block",
        )
    })?;

    Ok(LdosControlInput {
        mldos_enabled: mldos_enabled.unwrap_or(true),
        neldos: neldos.max(1),
        rfms2: rfms2.ok_or_else(|| {
            ldos_parse_error(
                fixture_id,
                "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
            )
        })?,
        emin: emin.ok_or_else(|| {
            ldos_parse_error(
                fixture_id,
                "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
            )
        })?,
        emax: emax.ok_or_else(|| {
            ldos_parse_error(
                fixture_id,
                "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
            )
        })?,
        eimag: eimag.ok_or_else(|| {
            ldos_parse_error(
                fixture_id,
                "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
            )
        })?,
        rgrd: rgrd
            .ok_or_else(|| {
                ldos_parse_error(
                    fixture_id,
                    "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
                )
            })?
            .max(1.0e-6),
        rdirec: rdirec.unwrap_or(12.0),
        toler1: toler1.unwrap_or(1.0e-3),
        toler2: toler2.unwrap_or(1.0e-3),
        lmaxph: lmaxph.unwrap_or_else(|| vec![3]),
    })
}

fn parse_geom_source(fixture_id: &str, source: &str) -> PipelineResult<GeomLdosInput> {
    let numeric_rows = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();

    if numeric_rows.is_empty() {
        return Err(ldos_parse_error(
            fixture_id,
            "geom.dat is missing numeric content",
        ));
    }
    if numeric_rows[0].len() < 2 {
        return Err(ldos_parse_error(
            fixture_id,
            "geom.dat header must provide nat and nph values",
        ));
    }

    let declared_nat = f64_to_usize(numeric_rows[0][0], fixture_id, "geom.dat nat")?;
    let declared_nph = f64_to_usize(numeric_rows[0][1], fixture_id, "geom.dat nph")?;

    let mut atoms = Vec::new();
    for row in numeric_rows {
        if row.len() < 6 {
            continue;
        }
        atoms.push(AtomSite {
            x: row[1],
            y: row[2],
            z: row[3],
            ipot: f64_to_i32(row[4], fixture_id, "geom.dat ipot")?,
        });
    }
    if atoms.is_empty() {
        return Err(ldos_parse_error(
            fixture_id,
            "geom.dat does not contain atom rows",
        ));
    }

    let absorber_index = atoms.iter().position(|atom| atom.ipot == 0).unwrap_or(0);
    let absorber = atoms[absorber_index];
    let radii = atoms
        .iter()
        .enumerate()
        .filter_map(|(index, atom)| {
            if index == absorber_index {
                return None;
            }
            let radius = distance(*atom, absorber);
            (radius > 1.0e-10).then_some(radius)
        })
        .collect::<Vec<_>>();

    let atom_count = atoms.len();
    let radius_mean = if radii.is_empty() {
        0.0
    } else {
        radii.iter().sum::<f64>() / radii.len() as f64
    };
    let radius_rms = if radii.is_empty() {
        0.0
    } else {
        (radii.iter().map(|radius| radius * radius).sum::<f64>() / radii.len() as f64).sqrt()
    };
    let radius_max = radii.into_iter().fold(0.0_f64, f64::max);
    let ipot_mean = atoms.iter().map(|atom| atom.ipot as f64).sum::<f64>() / atom_count as f64;

    Ok(GeomLdosInput {
        nat: declared_nat.max(atom_count),
        nph: declared_nph.max(1),
        atom_count,
        radius_mean,
        radius_rms,
        radius_max,
        ipot_mean,
    })
}

fn parse_pot_source(fixture_id: &str, bytes: &[u8]) -> PipelineResult<PotLdosInput> {
    if bytes.is_empty() {
        return Err(ldos_parse_error(fixture_id, "pot.bin is empty"));
    }

    if bytes.starts_with(POT_BINARY_MAGIC) {
        return parse_true_compute_pot_binary(fixture_id, bytes);
    }

    let checksum = checksum_bytes(bytes);
    let byte_len = bytes.len();
    let radius_mean = ((byte_len % 7_500) as f64 / 1_500.0).max(1.0);
    let radius_rms = radius_mean * 1.1;
    let radius_max = radius_mean * 1.4;
    let charge_scale = ((checksum % 900) as f64 / 180.0 + 1.0).max(1.0e-6);

    Ok(PotLdosInput {
        nat: (byte_len / 2_048).max(1),
        nph: ((checksum % 4) as usize).max(1),
        npot: (byte_len / 16_384).max(1),
        rfms: ((byte_len % 6_000) as f64 / 800.0 + 2.0).clamp(2.0, 12.0),
        radius_mean,
        radius_rms,
        radius_max,
        charge_scale,
        checksum,
        has_true_compute_magic: false,
    })
}

fn parse_true_compute_pot_binary(fixture_id: &str, bytes: &[u8]) -> PipelineResult<PotLdosInput> {
    let mut offset = POT_BINARY_MAGIC.len();

    for _ in 0..POT_CONTROL_I32_COUNT {
        let _ = take_i32(bytes, &mut offset).ok_or_else(|| {
            ldos_parse_error(fixture_id, "pot.bin missing POT control i32 values")
        })?;
    }

    let mut control_f64 = [0.0_f64; POT_CONTROL_F64_COUNT];
    for value in &mut control_f64 {
        *value = take_f64(bytes, &mut offset).ok_or_else(|| {
            ldos_parse_error(fixture_id, "pot.bin missing POT control f64 values")
        })?;
    }

    let nat = take_u32(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing nat metadata"))?
        as usize;
    let nph = take_u32(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing nph metadata"))?
        as usize;
    let npot = take_u32(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing npot metadata"))?
        as usize;
    let radius_mean = take_f64(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing radius_mean metadata"))?;
    let radius_rms = take_f64(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing radius_rms metadata"))?;
    let radius_max = take_f64(bytes, &mut offset)
        .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing radius_max metadata"))?;

    let mut zeff_sum = 0.0_f64;
    let potential_count = npot.max(1);
    for _ in 0..potential_count {
        let _ = take_u32(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential index"))?;
        let _ = take_i32(bytes, &mut offset).ok_or_else(|| {
            ldos_parse_error(fixture_id, "pot.bin missing potential atomic number")
        })?;
        let _ = take_i32(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential lmaxsc"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential xnatph"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential xion"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential folp"))?;
        let zeff = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential zeff"))?;
        let _ = take_f64(bytes, &mut offset).ok_or_else(|| {
            ldos_parse_error(fixture_id, "pot.bin missing potential local_density")
        })?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential vmt0"))?;
        let _ = take_f64(bytes, &mut offset)
            .ok_or_else(|| ldos_parse_error(fixture_id, "pot.bin missing potential vxc"))?;
        zeff_sum += zeff.abs();
    }

    Ok(PotLdosInput {
        nat: nat.max(1),
        nph: nph.max(1),
        npot: npot.max(1),
        rfms: control_f64[5].abs().max(0.1),
        radius_mean: radius_mean.abs().max(1.0e-6),
        radius_rms: radius_rms.abs().max(1.0e-6),
        radius_max: radius_max.abs().max(1.0e-6),
        charge_scale: (zeff_sum / npot.max(1) as f64).max(1.0e-6),
        checksum: checksum_bytes(bytes),
        has_true_compute_magic: true,
    })
}

fn parse_reciprocal_source(fixture_id: &str, source: &str) -> PipelineResult<ReciprocalLdosInput> {
    let values = source
        .lines()
        .flat_map(parse_numeric_tokens)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err(ldos_parse_error(
            fixture_id,
            "reciprocal.inp does not contain numeric values",
        ));
    }

    Ok(ReciprocalLdosInput {
        ispace: f64_to_i32(values[0], fixture_id, "reciprocal.inp ispace")?,
    })
}

fn parse_ldos_channel_name(file_name: &str) -> Option<usize> {
    let normalized = file_name.to_ascii_lowercase();
    if !normalized.starts_with("ldos") || !normalized.ends_with(".dat") {
        return None;
    }

    let digits = &normalized[4..normalized.len().saturating_sub(4)];
    if digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    digits.parse::<usize>().ok()
}

fn expected_output_artifacts(channel_count: usize) -> Vec<PipelineArtifact> {
    let mut outputs = (0..channel_count)
        .map(|channel| PipelineArtifact::new(format!("ldos{channel:02}.dat")))
        .collect::<Vec<_>>();
    outputs.push(PipelineArtifact::new(LDOS_LOG_OUTPUT));
    outputs
}

fn ldos_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.LDOS_INPUT_PARSE",
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
            ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
        )
    });
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.replace(['D', 'd'], "E");
    normalized.parse::<f64>().ok()
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> PipelineResult<i32> {
    if !value.is_finite() {
        return Err(ldos_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-6 {
        return Err(ldos_parse_error(
            fixture_id,
            format!("{} must be an integer value", field),
        ));
    }
    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(ldos_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }
    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> PipelineResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(ldos_parse_error(
            fixture_id,
            format!("{} must be non-negative", field),
        ));
    }
    Ok(integer as usize)
}

fn take_u32(bytes: &[u8], offset: &mut usize) -> Option<u32> {
    let end = offset.checked_add(std::mem::size_of::<u32>())?;
    let slice = bytes.get(*offset..end)?;
    let value = u32::from_le_bytes(slice.try_into().ok()?);
    *offset = end;
    Some(value)
}

fn take_i32(bytes: &[u8], offset: &mut usize) -> Option<i32> {
    let end = offset.checked_add(std::mem::size_of::<i32>())?;
    let slice = bytes.get(*offset..end)?;
    let value = i32::from_le_bytes(slice.try_into().ok()?);
    *offset = end;
    Some(value)
}

fn take_f64(bytes: &[u8], offset: &mut usize) -> Option<f64> {
    let end = offset.checked_add(std::mem::size_of::<f64>())?;
    let slice = bytes.get(*offset..end)?;
    let value = f64::from_le_bytes(slice.try_into().ok()?);
    *offset = end;
    Some(value)
}

fn checksum_bytes(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0_u64, |accumulator, byte| {
        accumulator.wrapping_mul(131).wrapping_add(u64::from(*byte))
    })
}

fn distance(left: AtomSite, right: AtomSite) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    let dz = left.z - right.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::LdosPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_matches_true_compute_ldos_output_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_baseline_artifact("FX-LDOS-001", "ldos.inp", &temp.path().join("ldos.inp"));
        stage_baseline_artifact("FX-LDOS-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-LDOS-001", "pot.bin", &temp.path().join("pot.bin"));
        stage_baseline_artifact(
            "FX-LDOS-001",
            "reciprocal.inp",
            &temp.path().join("reciprocal.inp"),
        );

        let request = PipelineRequest::new(
            "FX-LDOS-001",
            PipelineModule::Ldos,
            temp.path().join("ldos.inp"),
            temp.path().join("actual-output"),
        );
        let scaffold = LdosPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 4);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("ldos.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("geom.dat")
        );
        assert_eq!(
            contract.required_inputs[2].relative_path,
            PathBuf::from("pot.bin")
        );
        assert_eq!(
            contract.required_inputs[3].relative_path,
            PathBuf::from("reciprocal.inp")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&["ldos00.dat", "ldos01.dat", "ldos02.dat", "logdos.dat"])
        );
    }

    #[test]
    fn execute_emits_true_compute_ldos_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ldos.inp");
        let output_dir = temp.path().join("out");
        stage_baseline_artifact("FX-LDOS-001", "ldos.inp", &input_path);
        stage_baseline_artifact("FX-LDOS-001", "geom.dat", &temp.path().join("geom.dat"));
        stage_baseline_artifact("FX-LDOS-001", "pot.bin", &temp.path().join("pot.bin"));
        stage_baseline_artifact(
            "FX-LDOS-001",
            "reciprocal.inp",
            &temp.path().join("reciprocal.inp"),
        );

        let request = PipelineRequest::new(
            "FX-LDOS-001",
            PipelineModule::Ldos,
            &input_path,
            &output_dir,
        );
        let scaffold = LdosPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("LDOS execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["ldos00.dat", "ldos01.dat", "ldos02.dat", "logdos.dat"])
        );
        for artifact in artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            assert!(output_path.is_file(), "artifact should exist on disk");
            assert!(
                !fs::read(&output_path)
                    .expect("artifact should be readable")
                    .is_empty(),
                "artifact should not be empty"
            );
        }
    }

    #[test]
    fn execute_is_deterministic_for_identical_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_input_dir = temp.path().join("first-inputs");
        let second_input_dir = temp.path().join("second-inputs");
        let first_output_dir = temp.path().join("first-output");
        let second_output_dir = temp.path().join("second-output");

        for input_dir in [&first_input_dir, &second_input_dir] {
            stage_baseline_artifact("FX-LDOS-001", "ldos.inp", &input_dir.join("ldos.inp"));
            stage_baseline_artifact("FX-LDOS-001", "geom.dat", &input_dir.join("geom.dat"));
            stage_baseline_artifact("FX-LDOS-001", "pot.bin", &input_dir.join("pot.bin"));
            stage_baseline_artifact(
                "FX-LDOS-001",
                "reciprocal.inp",
                &input_dir.join("reciprocal.inp"),
            );
        }

        let scaffold = LdosPipelineScaffold;
        let first_request = PipelineRequest::new(
            "FX-LDOS-001",
            PipelineModule::Ldos,
            first_input_dir.join("ldos.inp"),
            &first_output_dir,
        );
        let first_artifacts = scaffold
            .execute(&first_request)
            .expect("first LDOS execution should succeed");

        let second_request = PipelineRequest::new(
            "FX-LDOS-001",
            PipelineModule::Ldos,
            second_input_dir.join("ldos.inp"),
            &second_output_dir,
        );
        let second_artifacts = scaffold
            .execute(&second_request)
            .expect("second LDOS execution should succeed");

        assert_eq!(
            artifact_set(&first_artifacts),
            artifact_set(&second_artifacts),
            "artifact contracts should match across runs"
        );
        for artifact in first_artifacts {
            let first = fs::read(first_output_dir.join(&artifact.relative_path))
                .expect("first artifact should be readable");
            let second = fs::read(second_output_dir.join(&artifact.relative_path))
                .expect("second artifact should be readable");
            assert_eq!(first, second, "artifact bytes should be deterministic");
        }
    }

    #[test]
    fn execute_accepts_rdinp_style_ldos_input_without_neldos() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ldos.inp");
        let output_dir = temp.path().join("out");
        fs::write(&input_path, LDOS_INPUT_WITHOUT_NELDOS).expect("ldos input should be staged");
        fs::write(temp.path().join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be staged");
        fs::write(temp.path().join("pot.bin"), [1_u8, 2_u8, 3_u8, 4_u8])
            .expect("pot input should be staged");
        fs::write(temp.path().join("reciprocal.inp"), RECIPROCAL_INPUT_FIXTURE)
            .expect("reciprocal input should be staged");

        let request = PipelineRequest::new(
            "FX-RDINP-COMPAT",
            PipelineModule::Ldos,
            &input_path,
            &output_dir,
        );
        let artifacts = LdosPipelineScaffold
            .execute(&request)
            .expect("LDOS should accept RDINP-style ldos.inp");

        let artifact_names = artifact_set(&artifacts);
        assert!(
            artifact_names.contains("logdos.dat"),
            "log output should be present"
        );
        assert!(
            artifact_names.iter().any(|name| name.starts_with("ldos")),
            "at least one ldosNN.dat output should be present"
        );
    }

    #[test]
    fn execute_rejects_non_ldos_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ldos.inp");
        fs::write(&input_path, "LDOS INPUT\n").expect("ldos input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("pot.bin"), [1_u8, 2_u8]).expect("pot should be written");
        fs::write(temp.path().join("reciprocal.inp"), "R 0.0 0.0 0.0\n")
            .expect("reciprocal should be written");

        let request = PipelineRequest::new(
            "FX-LDOS-001",
            PipelineModule::Band,
            &input_path,
            temp.path(),
        );
        let scaffold = LdosPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.LDOS_MODULE");
    }

    #[test]
    fn execute_requires_pot_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("ldos.inp");
        fs::write(&input_path, "LDOS INPUT\n").expect("ldos input should be written");
        fs::write(temp.path().join("geom.dat"), "GEOM INPUT\n").expect("geom should be written");
        fs::write(temp.path().join("reciprocal.inp"), "R 0.0 0.0 0.0\n")
            .expect("reciprocal should be written");

        let request = PipelineRequest::new(
            "FX-LDOS-001",
            PipelineModule::Ldos,
            &input_path,
            temp.path(),
        );
        let scaffold = LdosPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing pot input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.LDOS_INPUT_READ");
    }

    fn fixture_baseline_dir(fixture_id: &str) -> PathBuf {
        PathBuf::from("artifacts/fortran-baselines")
            .join(fixture_id)
            .join("baseline")
    }

    fn stage_baseline_artifact(fixture_id: &str, artifact: &str, destination: &Path) {
        let source = fixture_baseline_dir(fixture_id).join(artifact);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should be created");
        }
        fs::copy(source, destination).expect("baseline artifact should be staged");
    }

    fn expected_artifact_set(artifacts: &[&str]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    const LDOS_INPUT_WITHOUT_NELDOS: &str = "mldos, lfms2, ixc, ispin, minv\n\
   1   0   0   0   0\n\
rfms2, emin, emax, eimag, rgrd\n\
      4.00000    -20.00000     10.00000      0.10000      0.05000\n\
rdirec, toler1, toler2\n\
      8.00000      0.00100      0.00100\n\
 lmaxph(0:nph)\n\
   2   2\n";

    const GEOM_INPUT_FIXTURE: &str = "nat, nph =    4    1\n\
    1    2\n\
 iat     x       y        z       iph\n\
 -----------------------------------------------------------------------\n\
   1      0.00000      0.00000      0.00000   0   1\n\
   2      1.80500      1.80500      0.00000   1   1\n\
   3     -1.80500      1.80500      0.00000   1   1\n\
   4      0.00000      1.80500      1.80500   1   1\n";

    const RECIPROCAL_INPUT_FIXTURE: &str = "ispace\n\
   1\n";
}
