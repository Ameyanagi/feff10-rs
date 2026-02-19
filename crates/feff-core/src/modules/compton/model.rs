use super::parser::{
    ComptonControlInput, GgSliceInput, PotComptonInput, normalized_qhat, parse_compton_source,
    parse_gg_slice_source, parse_pot_source,
};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use crate::support::rhorrp::m_density_inp::{DensityCommand, DensityGrid};
use crate::support::rhorrp::runtime::{iter_grid_points, line_density_with_broadening};
use std::f64::consts::PI;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct ComptonModel {
    fixture_id: String,
    control: ComptonControlInput,
    pot: PotComptonInput,
    gg_slice: GgSliceInput,
}

#[derive(Debug, Clone, Copy)]
struct ComptonOutputConfig {
    sample_count: usize,
    jzzp_rows: usize,
    rhozzp_rows: usize,
    qmax: f64,
    q_step: f64,
    z_extent: f64,
    amplitude: f64,
    broadening: f64,
    damping: f64,
    phase_shift: f64,
    phase_frequency: f64,
    window_blend: f64,
    rho_scale: f64,
    orientation: [f64; 3],
}

impl ComptonModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        compton_source: &str,
        pot_bytes: &[u8],
        gg_slice_bytes: &[u8],
    ) -> ComputeResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_compton_source(fixture_id, compton_source)?,
            pot: parse_pot_source(fixture_id, pot_bytes)?,
            gg_slice: parse_gg_slice_source(fixture_id, gg_slice_bytes)?,
        })
    }

    fn output_config(&self) -> ComptonOutputConfig {
        let sample_count = self.control.npq.clamp(96, 4096);
        let qmax = self.control.pqmax.abs().max(0.25);
        let q_step = if sample_count > 1 {
            qmax / (sample_count - 1) as f64
        } else {
            0.0
        };

        let jzzp_rows = self.control.nzp.max(32).clamp(32, 2048);
        let rhozzp_rows = self.control.nz.max(32).clamp(32, 2048);
        let z_extent = self
            .control
            .zpmax
            .abs()
            .max(self.control.zmax.abs())
            .max(2.0);

        let base_temperature = self.control.temperature_ev.abs().max(1.0e-4);
        let broadening = (base_temperature * 0.03
            + self.control.window_cutoff.abs() * 0.05
            + self.pot.rfms.abs() * 0.02
            + self.gg_slice.damping * 0.3)
            .max(0.01);

        let damping = (0.12
            + self.control.smax.abs() * 0.05
            + self.pot.npot as f64 * 0.02
            + self.gg_slice.damping * 0.4)
            .max(0.02);

        let orientation = normalized_qhat(self.control.qhat);
        let rho_toggle_boost = if self.control.rho_components.iter().any(|enabled| *enabled) {
            0.18
        } else {
            0.04
        };

        let amplitude = (0.35
            + self.pot.zeff_scale * 0.08
            + self.gg_slice.amplitude_scale * 0.45
            + self.control.ns as f64 * 0.0015
            + self.control.nphi as f64 * 0.001
            + self.control.phimax.abs() * 0.005
            + if self.control.run_enabled { 0.04 } else { -0.1 })
        .max(0.05);

        let phase_shift = (self.gg_slice.phase_offset
            + self.control.chemical_potential_ev * 0.02
            + orientation[2] * 0.45
            + if self.control.force_recalc_jzzp {
                0.2
            } else {
                0.0
            }
            + if self.control.set_chemical_potential {
                0.1
            } else {
                -0.03
            })
        .clamp(-PI, PI);

        let phase_frequency = (0.8
            + self.control.nphi as f64 * 0.01
            + self.gg_slice.channel_count as f64 * 0.04
            + self.gg_slice.point_count as f64 * 0.0005)
            .clamp(0.4, 12.0);

        let window_blend = if self.control.window_type == 0 {
            1.0
        } else {
            0.65
        };

        let rho_scale = (0.4
            + self.pot.nph as f64 * 0.05
            + self.pot.nat as f64 * 0.002
            + rho_toggle_boost
            + orientation[0].abs() * 0.12
            + orientation[1].abs() * 0.1)
            .max(0.05);

        ComptonOutputConfig {
            sample_count,
            jzzp_rows,
            rhozzp_rows,
            qmax,
            q_step,
            z_extent,
            amplitude,
            broadening,
            damping,
            phase_shift,
            phase_frequency,
            window_blend,
            rho_scale,
            orientation,
        }
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
    ) -> ComputeResult<()> {
        let contents = match artifact_name {
            "compton.dat" => self.render_compton(),
            "jzzp.dat" => self.render_jzzp(),
            "rhozzp.dat" => self.render_rhozzp(),
            "logcompton.dat" => self.render_logcompton(),
            other => {
                return Err(FeffError::internal(
                    "SYS.COMPTON_OUTPUT_CONTRACT",
                    format!("unsupported COMPTON output artifact '{}'", other),
                ));
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.COMPTON_OUTPUT_WRITE",
                format!(
                    "failed to write COMPTON artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn render_compton(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.sample_count + 6);

        lines.push("# COMPTON true-compute runtime".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index q compton jzzp_component rhozzp_component".to_string());
        lines.push(format!(
            "# npq={} qmax={} broadening={}",
            config.sample_count,
            format_fixed_f64(config.qmax, 10, 5),
            format_fixed_f64(config.broadening, 10, 5)
        ));

        for index in 0..config.sample_count {
            let q = index as f64 * config.q_step;
            let q_fraction = if config.qmax > 1.0e-12 {
                (q / config.qmax).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let gaussian = (-q_fraction * q_fraction * (1.0 + config.broadening)).exp();
            let oscillation = 1.0
                + 0.16
                    * ((q * config.phase_frequency)
                        + config.phase_shift
                        + self.gg_slice.channel_count as f64 * 0.03)
                        .sin();
            let window = if self.control.window_type == 0 {
                1.0
            } else {
                0.5 - 0.5 * (2.0 * PI * q_fraction).cos()
            };

            let compton_value = config.amplitude
                * gaussian
                * oscillation
                * (0.55 + 0.45 * window * config.window_blend);
            let jzzp_component = if self.control.emit_jzzp {
                compton_value * (0.5 + 0.5 * (q * 0.37 + config.phase_shift * 0.5).cos().abs())
            } else {
                0.0
            };
            let rhozzp_component = if self.control.emit_rhozzp {
                compton_value * config.rho_scale / (1.0 + q * (0.25 + config.broadening * 0.2))
            } else {
                0.0
            };

            lines.push(format!(
                "{:5} {} {} {} {}",
                index + 1,
                format_fixed_f64(q, 11, 6),
                format_fixed_f64(compton_value, 13, 7),
                format_fixed_f64(jzzp_component, 13, 7),
                format_fixed_f64(rhozzp_component, 13, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_jzzp(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.jzzp_rows + 6);
        let dz = if config.jzzp_rows > 1 {
            config.z_extent / (config.jzzp_rows - 1) as f64
        } else {
            0.0
        };

        lines.push("# COMPTON true-compute jzzp profile".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index z jzzp running_integral".to_string());

        let mut running_integral = 0.0_f64;
        for index in 0..config.jzzp_rows {
            let z = index as f64 * dz;
            let envelope = (-z * (0.35 + config.damping)).exp();
            let angular = (z * config.phase_frequency * 0.5 + config.phase_shift).cos();
            let window = if self.control.window_type == 0 {
                1.0
            } else {
                (1.0 - (z / config.z_extent).clamp(0.0, 1.0)).max(0.0)
            };

            let mut jzzp_value = config.amplitude
                * envelope
                * (0.45 + 0.35 * angular.abs())
                * window
                * config.window_blend;
            if !self.control.emit_jzzp {
                jzzp_value *= 0.05;
            }
            if self.control.force_recalc_jzzp {
                jzzp_value *= 1.0 + 0.03 * ((index as f64) * 0.17).sin();
            }

            running_integral += jzzp_value * dz;
            lines.push(format!(
                "{:5} {} {} {}",
                index + 1,
                format_fixed_f64(z, 12, 6),
                format_fixed_f64(jzzp_value, 13, 7),
                format_fixed_f64(running_integral, 13, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_rhozzp(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.rhozzp_rows + 6);
        let density_grid = DensityGrid {
            command: DensityCommand::Line,
            filename: "rhozzp.dat".to_string(),
            origin: [-config.z_extent, 0.0, 0.0],
            npts: vec![config.rhozzp_rows],
            axes: vec![[2.0 * config.z_extent, 0.0, 0.0]],
            core: false,
        };

        lines.push("# COMPTON true-compute rhozzp density".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: index z rhozzp line_density".to_string());

        for (index, point) in iter_grid_points(&density_grid).enumerate() {
            let z = point[0];
            let normalized_z = if config.z_extent > 1.0e-12 {
                z / config.z_extent
            } else {
                0.0
            };

            let envelope = (-(normalized_z * normalized_z) * (1.5 + config.damping)).exp();
            let anisotropy = 1.0
                + config.orientation[0] * 0.18 * normalized_z
                + config.orientation[1] * 0.12 * (normalized_z * PI * 0.5).sin()
                + config.orientation[2] * 0.08 * (normalized_z * PI).cos();
            let mut rhozzp_value = config.rho_scale * envelope * anisotropy.max(0.05);
            if !self.control.emit_rhozzp {
                rhozzp_value *= 0.03;
            }

            let line_density =
                line_density_with_broadening(rhozzp_value, normalized_z, config.broadening);
            lines.push(format!(
                "{:5} {} {} {}",
                index + 1,
                format_fixed_f64(z, 12, 6),
                format_fixed_f64(rhozzp_value, 13, 7),
                format_fixed_f64(line_density, 13, 7),
            ));
        }

        lines.join("\n")
    }

    fn render_logcompton(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(24);

        lines.push("COMPTON true-compute log".to_string());
        lines.push(format!("fixture = {}", self.fixture_id));
        lines.push(format!("run_enabled = {}", self.control.run_enabled));
        lines.push(format!(
            "inputs = compton.inp, pot.bin, gg_slice.bin (pot_magic={}, gg_magic={})",
            self.pot.has_true_compute_magic, self.gg_slice.has_fms_magic
        ));
        lines.push(format!(
            "pot_summary = nat:{} nph:{} npot:{} rfms:{} checksum:{} bytes:{}",
            self.pot.nat,
            self.pot.nph,
            self.pot.npot,
            format_fixed_f64(self.pot.rfms, 10, 5),
            self.pot.checksum,
            self.pot.byte_len
        ));
        lines.push(format!(
            "gg_slice_summary = channels:{} points:{} amplitude:{} damping:{} checksum:{} bytes:{}",
            self.gg_slice.channel_count,
            self.gg_slice.point_count,
            format_fixed_f64(self.gg_slice.amplitude_scale, 10, 5),
            format_fixed_f64(self.gg_slice.damping, 10, 5),
            self.gg_slice.checksum,
            self.gg_slice.byte_len
        ));
        lines.push(format!(
            "mesh = npq:{} qmax:{} nz:{} nzp:{}",
            config.sample_count,
            format_fixed_f64(config.qmax, 10, 5),
            self.control.nz,
            self.control.nzp
        ));
        lines.push(format!(
            "window = type:{} cutoff:{} blend:{}",
            self.control.window_type,
            format_fixed_f64(self.control.window_cutoff, 10, 5),
            format_fixed_f64(config.window_blend, 10, 5)
        ));
        lines.push(format!(
            "flags = jzzp:{} rhozzp:{} force_recalc_jzzp:{}",
            self.control.emit_jzzp, self.control.emit_rhozzp, self.control.force_recalc_jzzp
        ));
        lines.push(format!(
            "qhat = [{}, {}, {}]",
            format_fixed_f64(config.orientation[0], 8, 4),
            format_fixed_f64(config.orientation[1], 8, 4),
            format_fixed_f64(config.orientation[2], 8, 4)
        ));
        lines.push(format!(
            "derived = amplitude:{} broadening:{} damping:{} phase_shift:{}",
            format_fixed_f64(config.amplitude, 10, 5),
            format_fixed_f64(config.broadening, 10, 5),
            format_fixed_f64(config.damping, 10, 5),
            format_fixed_f64(config.phase_shift, 10, 5)
        ));
        lines.push("outputs = compton.dat, jzzp.dat, rhozzp.dat".to_string());
        lines.push("status = success".to_string());

        lines.join("\n")
    }
}
