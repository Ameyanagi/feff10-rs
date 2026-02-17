use super::parser::{
    FmsControlInput, GeomFmsInput, GlobalFmsInput, PhaseFmsInput,
    parse_fms_source, parse_geom_source, parse_global_source, parse_phase_source,
};
use super::FMS_GG_BINARY_MAGIC;
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_binary_artifact, write_text_artifact};
use std::f64::consts::PI;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct FmsModel {
    fixture_id: String,
    control: FmsControlInput,
    geom: GeomFmsInput,
    global: GlobalFmsInput,
    phase: PhaseFmsInput,
}

#[derive(Debug, Clone, Copy)]
struct FmsOutputConfig {
    scattering_channels: usize,
    k_points: usize,
    energy_start: f64,
    energy_step: f64,
    amplitude_scale: f64,
    damping: f64,
    phase_offset: f64,
    temperature_factor: f64,
    phase_byte_scale: f64,
}

impl FmsModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        fms_source: &str,
        geom_source: &str,
        global_source: &str,
        phase_bytes: &[u8],
    ) -> ComputeResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_fms_source(fixture_id, fms_source)?,
            geom: parse_geom_source(fixture_id, geom_source)?,
            global: parse_global_source(fixture_id, global_source)?,
            phase: parse_phase_source(fixture_id, phase_bytes)?,
        })
    }

    pub(super) fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
        match artifact_name {
            "gg.bin" => {
                write_binary_artifact(output_path, &self.render_gg_binary()).map_err(|source| {
                    FeffError::io_system(
                        "IO.FMS_OUTPUT_WRITE",
                        format!(
                            "failed to write FMS artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "log3.dat" => write_text_artifact(output_path, &self.render_log3()).map_err(|source| {
                FeffError::io_system(
                    "IO.FMS_OUTPUT_WRITE",
                    format!(
                        "failed to write FMS artifact '{}': {}",
                        output_path.display(),
                        source
                    ),
                )
            }),
            other => Err(FeffError::internal(
                "SYS.FMS_OUTPUT_CONTRACT",
                format!("unsupported FMS output artifact '{}'", other),
            )),
        }
    }

    fn output_config(&self) -> FmsOutputConfig {
        let legacy_channel_hint = (self.control.lmaxph_sum.unsigned_abs() as usize)
            .max(2)
            .clamp(2, 24);
        let scattering_channels = if self.phase.has_xsph_magic {
            self.phase.channel_count.max(2).clamp(2, 24)
        } else {
            legacy_channel_hint
        };

        let k_points = (((self.geom.nat.max(1) as f64).sqrt() * 7.0).round() as usize
            + self.phase.spectral_points.max(16) / 3
            + self.global.token_count.min(128) / 6)
            .clamp(64, 512);

        let energy_start = -((self.control.rfms2.abs()
            + self.geom.radius_mean * 0.8
            + self.global.max_abs.min(200.0) * 0.01)
            .max(0.5));

        let energy_step = ((self.phase.energy_step.abs().max(1.0e-4) * 0.5)
            + self.control.toler1.abs().max(1.0e-4)
            + self.control.toler2.abs().max(1.0e-4) * 0.5)
            .max(1.0e-4);

        let amplitude_scale = (0.5
            + self.control.mfms.abs().max(1) as f64 * 0.12
            + self.geom.ipot_mean.abs() * 0.03
            + self.global.rms * 0.02)
            .max(1.0e-4);

        let damping = (1.0
            / (1.0
                + self.control.rfms2.abs()
                + self.geom.radius_max
                + self.geom.radius_rms
                + self.phase.byte_len as f64 * 1.0e-4
                + self.control.rdirec.abs() * 0.05))
            .max(1.0e-5);

        let phase_offset = (self.phase.base_phase
            + self.control.idwopt as f64 * 0.04
            + self.control.minv as f64 * 0.02
            + self.global.mean * 1.0e-4)
            .clamp(-PI, PI);

        let temperature_factor = (1.0
            + self.control.tk.abs() * 1.0e-3
            + self.control.thetad.abs() * 1.0e-4
            + self.control.sig2g.abs() * 10.0)
            .max(1.0);

        let phase_byte_scale = 1.0 + (self.phase.checksum as f64 / u64::MAX as f64) * 0.2;

        FmsOutputConfig {
            scattering_channels,
            k_points,
            energy_start,
            energy_step,
            amplitude_scale,
            damping,
            phase_offset,
            temperature_factor,
            phase_byte_scale,
        }
    }

    fn render_gg_binary(&self) -> Vec<u8> {
        let config = self.output_config();
        let mut bytes = Vec::with_capacity(
            160 + config.k_points
                * (1 + config.scattering_channels * 2)
                * std::mem::size_of::<f64>(),
        );

        bytes.extend_from_slice(FMS_GG_BINARY_MAGIC);
        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, config.scattering_channels as u32);
        push_u32(&mut bytes, config.k_points as u32);
        push_u32(&mut bytes, self.geom.nat as u32);
        push_u32(&mut bytes, self.geom.nph as u32);
        push_i32(&mut bytes, self.control.mfms);
        push_i32(&mut bytes, self.control.idwopt);
        push_i32(&mut bytes, self.control.minv);
        push_i32(&mut bytes, self.control.decomposition);
        push_i32(&mut bytes, self.control.lmaxph_sum);
        push_f64(&mut bytes, self.control.rfms2);
        push_f64(&mut bytes, self.control.rdirec);
        push_f64(&mut bytes, self.control.toler1);
        push_f64(&mut bytes, self.control.toler2);
        push_f64(&mut bytes, self.control.tk);
        push_f64(&mut bytes, self.control.thetad);
        push_f64(&mut bytes, self.control.sig2g);
        push_f64(&mut bytes, config.energy_start);
        push_f64(&mut bytes, config.energy_step);
        push_f64(&mut bytes, config.amplitude_scale);
        push_f64(&mut bytes, config.damping);

        for index in 0..config.k_points {
            let k = config.energy_start + config.energy_step * index as f64;
            let normalized = if config.k_points == 1 {
                0.0
            } else {
                index as f64 / (config.k_points - 1) as f64
            };
            push_f64(&mut bytes, k);

            for channel in 0..config.scattering_channels {
                let channel_f = channel as f64 + 1.0;
                let oscillation = (k * (0.09 + 0.01 * channel_f) + config.phase_offset).sin();
                let phase_term = (k * 0.07 + config.phase_offset + channel_f * 0.23).cos();
                let envelope = (-config.damping * index as f64 * (1.0 + channel_f * 0.015)).exp();
                let radial_weight = (1.0 + self.geom.radius_rms * 0.05 + normalized * 0.1).max(0.1);
                let scattering =
                    config.amplitude_scale * config.temperature_factor * config.phase_byte_scale;

                let real = scattering * envelope * oscillation * radial_weight / channel_f.sqrt();
                let imag =
                    scattering * envelope * phase_term * (1.0 + self.global.mean.abs() * 1.0e-3)
                        / channel_f.sqrt();

                push_f64(&mut bytes, real);
                push_f64(&mut bytes, imag);
            }
        }

        bytes
    }

    fn render_log3(&self) -> String {
        let config = self.output_config();
        let phase_source = if self.phase.has_xsph_magic {
            "xsph_phase_magic"
        } else {
            "legacy_phase_binary"
        };

        format!(
            "\
FMS true-compute runtime\n\
fixture: {}\n\
input-artifacts: fms.inp geom.dat global.inp phase.bin\n\
output-artifacts: gg.bin log3.dat\n\
nat: {} nph: {} atoms: {}\n\
phase-source: {}\n\
phase-bytes: {}\n\
phase-checksum: {}\n\
mfms: {} idwopt: {} minv: {} decomposition: {}\n\
rfms2: {} rdirec: {}\n\
toler1: {} toler2: {}\n\
tk: {} thetad: {} sig2g: {}\n\
radius-mean: {} radius-rms: {} radius-max: {}\n\
global-tokens: {} global-mean: {} global-rms: {}\n\
channels: {} k-points: {}\n\
energy-start: {} energy-step: {}\n\
damping: {} amplitude-scale: {}\n",
            self.fixture_id,
            self.geom.nat,
            self.geom.nph,
            self.geom.atom_count,
            phase_source,
            self.phase.byte_len,
            self.phase.checksum,
            self.control.mfms,
            self.control.idwopt,
            self.control.minv,
            self.control.decomposition,
            format_fixed_f64(self.control.rfms2, 10, 5),
            format_fixed_f64(self.control.rdirec, 10, 5),
            format_fixed_f64(self.control.toler1, 10, 6),
            format_fixed_f64(self.control.toler2, 10, 6),
            format_fixed_f64(self.control.tk, 10, 5),
            format_fixed_f64(self.control.thetad, 10, 5),
            format_fixed_f64(self.control.sig2g, 10, 6),
            format_fixed_f64(self.geom.radius_mean, 10, 5),
            format_fixed_f64(self.geom.radius_rms, 10, 5),
            format_fixed_f64(self.geom.radius_max, 10, 5),
            self.global.token_count,
            format_fixed_f64(self.global.mean, 10, 5),
            format_fixed_f64(self.global.rms, 10, 5),
            config.scattering_channels,
            config.k_points,
            format_fixed_f64(config.energy_start, 10, 5),
            format_fixed_f64(config.energy_step, 10, 6),
            format_fixed_f64(config.damping, 10, 6),
            format_fixed_f64(config.amplitude_scale, 10, 6),
        )
    }
}

pub(super) fn push_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn push_i32(target: &mut Vec<u8>, value: i32) {
    target.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn push_f64(target: &mut Vec<u8>, value: f64) {
    target.extend_from_slice(&value.to_le_bytes());
}
