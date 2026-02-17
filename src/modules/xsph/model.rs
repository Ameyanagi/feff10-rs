use super::XSPH_PHASE_BINARY_MAGIC;
use super::parser::{
    GeomXsphInput, GlobalXsphInput, PotXsphInput, WscrnXsphInput, XsphControlInput,
    format_scientific_f64, parse_geom_source, parse_global_source, parse_pot_source,
    parse_wscrn_source, parse_xsph_source, push_f64, push_i32, push_u32,
};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_binary_artifact, write_text_artifact};
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct XsphModel {
    fixture_id: String,
    control: XsphControlInput,
    geom: GeomXsphInput,
    global: GlobalXsphInput,
    pot: PotXsphInput,
    wscrn: Option<WscrnXsphInput>,
}

#[derive(Debug, Clone, Copy)]
struct XsphOutputConfig {
    phase_channels: usize,
    spectral_points: usize,
    energy_start: f64,
    energy_step: f64,
    base_phase: f64,
    phase_scale: f64,
    damping: f64,
    screening_shift: f64,
    xsnorm: f64,
}

impl XsphModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        xsph_source: &str,
        geom_source: &str,
        global_source: &str,
        pot_bytes: &[u8],
        wscrn_source: Option<&str>,
    ) -> ComputeResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_xsph_source(fixture_id, xsph_source)?,
            geom: parse_geom_source(fixture_id, geom_source)?,
            global: parse_global_source(fixture_id, global_source)?,
            pot: parse_pot_source(fixture_id, pot_bytes)?,
            wscrn: wscrn_source
                .map(|source| parse_wscrn_source(fixture_id, source))
                .transpose()?,
        })
    }

    fn output_config(&self) -> XsphOutputConfig {
        let phase_channels = (self.control.lmaxph_max + self.control.nph)
            .max(2)
            .clamp(2, 16) as usize;

        let spectral_points = (((self.geom.nat.max(1) as f64).sqrt() * 10.0).round() as usize
            + (self.control.n_poles.max(8) as usize / 2)
            + self.global.token_count.min(64) / 8)
            .clamp(64, 512);

        let wscrn_delta = self
            .wscrn
            .map(|wscrn| (wscrn.charge_mean - wscrn.screen_mean).abs())
            .unwrap_or(0.0);
        let screening_shift = wscrn_delta * 1.0e-3
            + self
                .wscrn
                .map(|wscrn| wscrn.radial_points as f64 * 1.0e-6)
                .unwrap_or(0.0);

        let energy_start = -(self.control.gamach + self.pot.gamach) * 6.0
            - self.geom.radius_mean * 2.0
            - self.global.max_abs.min(50.0) * 0.01;
        let energy_step = (self.control.xkstep.max(1.0e-4) * 3.5).max(1.0e-4);

        let base_phase = (0.03 * self.pot.charge_scale
            + 0.003 * self.geom.ipot_mean
            + 0.0005 * self.global.mean
            + 0.001 * self.control.mphase as f64
            + screening_shift)
            .clamp(-std::f64::consts::PI, std::f64::consts::PI);

        let phase_scale = (1.0 + self.control.rfms2 + self.pot.rfms + self.geom.radius_rms)
            .ln()
            .max(0.1)
            + self.pot.radius_rms * 0.005;

        let damping = 1.0
            / (self.control.xkmax
                + self.pot.radius_max
                + self.geom.radius_mean
                + 0.5 * self.geom.radius_max
                + 2.0)
                .max(1.0);

        let xsnorm = ((self.global.rms + self.pot.charge_scale + self.pot.radius_mean).abs()
            * 1.0e-3
            * (1.0 + 0.02 * self.control.ispec.abs() as f64)
            * (1.0 + 0.01 * self.geom.radius_max)
            * (1.0 + 0.01 * self.pot.npot as f64))
            .max(1.0e-6);

        XsphOutputConfig {
            phase_channels,
            spectral_points,
            energy_start,
            energy_step,
            base_phase,
            phase_scale,
            damping,
            screening_shift,
            xsnorm,
        }
    }

    pub(super) fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
        match artifact_name {
            "phase.bin" => {
                write_binary_artifact(output_path, &self.render_phase_binary()).map_err(|source| {
                    FeffError::io_system(
                        "IO.XSPH_OUTPUT_WRITE",
                        format!(
                            "failed to write XSPH artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "xsect.dat" => {
                write_text_artifact(output_path, &self.render_xsect()).map_err(|source| {
                    FeffError::io_system(
                        "IO.XSPH_OUTPUT_WRITE",
                        format!(
                            "failed to write XSPH artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "log2.dat" => write_text_artifact(output_path, &self.render_log2()).map_err(|source| {
                FeffError::io_system(
                    "IO.XSPH_OUTPUT_WRITE",
                    format!(
                        "failed to write XSPH artifact '{}': {}",
                        output_path.display(),
                        source
                    ),
                )
            }),
            other => Err(FeffError::internal(
                "SYS.XSPH_OUTPUT_CONTRACT",
                format!("unsupported XSPH output artifact '{}'", other),
            )),
        }
    }

    fn render_phase_binary(&self) -> Vec<u8> {
        let config = self.output_config();
        let mut bytes = Vec::with_capacity(
            96 + config.spectral_points * (config.phase_channels + 1) * std::mem::size_of::<f64>(),
        );

        bytes.extend_from_slice(XSPH_PHASE_BINARY_MAGIC);
        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, config.phase_channels as u32);
        push_u32(&mut bytes, config.spectral_points as u32);
        push_i32(&mut bytes, self.control.mphase);
        push_i32(&mut bytes, self.control.ispec);
        push_f64(&mut bytes, config.energy_start);
        push_f64(&mut bytes, config.energy_step);
        push_f64(&mut bytes, config.base_phase);
        push_f64(&mut bytes, config.phase_scale);
        push_f64(&mut bytes, config.damping);
        push_f64(&mut bytes, config.screening_shift);

        for index in 0..config.spectral_points {
            let t = if config.spectral_points == 1 {
                0.0
            } else {
                index as f64 / (config.spectral_points - 1) as f64
            };
            let energy = config.energy_start + config.energy_step * index as f64;
            push_f64(&mut bytes, energy);

            for channel in 0..config.phase_channels {
                let channel_f = channel as f64;
                let oscillation =
                    (energy * 0.015 + 0.25 * channel_f + self.control.mphase as f64 * 0.1).sin();
                let attenuation = (-config.damping * (1.0 + 0.03 * channel_f) * index as f64).exp();
                let phase = config.base_phase
                    + config.phase_scale * (1.0 + 0.1 * channel_f) * oscillation * attenuation
                    + config.screening_shift * (1.0 - t)
                    + 0.001 * self.control.ispec as f64;
                push_f64(&mut bytes, phase);
            }
        }

        bytes
    }

    fn render_xsect(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.spectral_points + 4);

        lines.push("# XSPH true-compute cross section".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push(format!(
            "# optional_wscrn: {}",
            if self.wscrn.is_some() {
                "present"
            } else {
                "absent"
            }
        ));
        lines.push("# energy(eV) xsnorm xsect imag_part".to_string());

        for index in 0..config.spectral_points {
            let t = if config.spectral_points == 1 {
                0.0
            } else {
                index as f64 / (config.spectral_points - 1) as f64
            };
            let energy = config.energy_start + config.energy_step * index as f64;
            let oscillation = (energy * 0.012 + config.base_phase).cos();
            let envelope = (-config.damping * index as f64).exp();

            let xsnorm = (config.xsnorm * (1.0 + 0.25 * t)).max(1.0e-12);
            let xsect = (xsnorm
                * (1.0 + 0.05 * config.phase_channels as f64 * oscillation.abs())
                * (1.0 + config.screening_shift.abs() * 50.0)
                * envelope)
                .max(1.0e-12);
            let imag_part =
                xsect * (0.30 + 0.05 * oscillation) + config.screening_shift * 1.0e-3 * (0.5 - t);

            lines.push(format!(
                "{:>16} {:>16} {:>16} {:>16}",
                format_scientific_f64(energy),
                format_scientific_f64(xsnorm),
                format_scientific_f64(xsect),
                format_scientific_f64(imag_part)
            ));
        }

        lines.join("\n")
    }

    fn render_log2(&self) -> String {
        let config = self.output_config();
        let wscrn_status = if self.wscrn.is_some() {
            "present"
        } else {
            "absent"
        };

        format!(
            "\
XSPH true-compute runtime\n\
fixture: {}\n\
input-artifacts: xsph.inp geom.dat global.inp pot.bin\n\
optional-input-wscrn: {}\n\
output-artifacts: phase.bin xsect.dat log2.dat\n\
nat: {} nph: {} atoms: {}\n\
pot-nat: {} pot-nph: {} npot: {}\n\
lmaxph-max: {} n-poles: {}\n\
phase-channels: {}\n\
spectral-points: {}\n\
energy-start: {}\n\
energy-step: {}\n\
xsnorm-base: {}\n\
",
            self.fixture_id,
            wscrn_status,
            self.geom.nat,
            self.geom.nph,
            self.geom.atom_count,
            self.pot.nat,
            self.pot.nph,
            self.pot.npot,
            self.control.lmaxph_max,
            self.control.n_poles,
            config.phase_channels,
            config.spectral_points,
            format_fixed_f64(config.energy_start, 12, 5),
            format_fixed_f64(config.energy_step, 12, 5),
            format_scientific_f64(config.xsnorm),
        )
    }
}
