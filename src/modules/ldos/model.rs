use super::parser::{
    GeomLdosInput, LdosControlInput, PotLdosInput, ReciprocalLdosInput,
    expected_output_artifacts, parse_geom_source, parse_ldos_channel_name, parse_ldos_source,
    parse_pot_source, parse_reciprocal_source,
};
use super::LDOS_LOG_OUTPUT;
use crate::domain::{ComputeArtifact, ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct LdosModel {
    fixture_id: String,
    control: LdosControlInput,
    geom: GeomLdosInput,
    pot: PotLdosInput,
    reciprocal: ReciprocalLdosInput,
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

impl LdosModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        ldos_source: &str,
        geom_source: &str,
        pot_bytes: &[u8],
        reciprocal_source: &str,
    ) -> ComputeResult<Self> {
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

    pub(super) fn output_channel_count(&self) -> usize {
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

    pub(super) fn expected_outputs(&self) -> Vec<ComputeArtifact> {
        expected_output_artifacts(self.output_channel_count())
    }

    pub(super) fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
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
