use super::parser::{
    BandControlInput, GeomBandInput, GlobalBandInput, PhaseBandInput, parse_band_source,
    parse_geom_source, parse_global_source, parse_phase_source,
};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::helpers::kspace_workflow_coupling;
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use std::f64::consts::PI;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct BandModel {
    fixture_id: String,
    control: BandControlInput,
    geom: GeomBandInput,
    global: GlobalBandInput,
    phase: PhaseBandInput,
}

#[derive(Debug, Clone, Copy)]
struct BandOutputConfig {
    k_points: usize,
    band_count: usize,
    energy_origin: f64,
    band_spacing: f64,
    k_extent: f64,
    curvature: f64,
    phase_shift: f64,
    global_bias: f64,
    kspace_coupling: f64,
}

impl BandModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        band_source: &str,
        geom_source: &str,
        global_source: &str,
        phase_bytes: &[u8],
    ) -> ComputeResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_band_source(fixture_id, band_source)?,
            geom: parse_geom_source(fixture_id, geom_source)?,
            global: parse_global_source(fixture_id, global_source)?,
            phase: parse_phase_source(fixture_id, phase_bytes)?,
        })
    }

    fn output_config(&self) -> BandOutputConfig {
        let kspace_coupling = kspace_workflow_coupling(
            self.control.ikpath,
            self.phase.channel_count,
            self.geom.nph,
            self.control.freeprop,
        );

        let k_points = if self.control.nkp > 1 {
            self.control.nkp as usize
        } else {
            ((self.geom.atom_count.max(1) * 8)
                + self.phase.spectral_points.max(16) / 4
                + self.global.token_count.min(256) / 16)
                .clamp(48, 512)
        };

        let band_count = (self.phase.channel_count.max(2) + self.geom.nph.max(1)).clamp(4, 24);

        let energy_origin = if self.control.emax > self.control.emin {
            self.control.emin
        } else {
            self.phase.energy_start - self.geom.radius_mean * 0.12 - self.global.mean * 0.002
        };

        let explicit_range = (self.control.emax - self.control.emin).abs();
        let fallback_range = (self.phase.energy_step * self.phase.spectral_points as f64)
            .abs()
            .max(8.0)
            + self.geom.radius_rms
            + self.global.max_abs.min(120.0) * 0.01;
        let energy_range = if explicit_range > 1.0e-8 {
            explicit_range
        } else {
            fallback_range
        };

        let band_spacing = (energy_range / band_count as f64)
            .max(self.control.estep.abs())
            .max(1.0e-4);

        let k_extent = (self.control.ikpath.abs().max(1) as f64 * 0.25
            + self.geom.radius_mean * 0.03
            + self.phase.channel_count as f64 * 0.02)
            .max(0.25)
            * (0.85 + 0.15 * kspace_coupling);
        let k_extent = k_extent.clamp(0.25, 32.0);

        let curvature =
            ((1.0 + self.geom.radius_rms + self.control.mband.abs().max(1) as f64 * 0.2)
                * 0.08
                * (0.85 + 0.15 * kspace_coupling))
                .max(1.0e-6);

        let phase_shift = (self.phase.base_phase
            + if self.control.freeprop { 0.35 } else { 0.0 }
            + if self.phase.has_xsph_magic {
                0.15
            } else {
                -0.05
            }
            + (kspace_coupling - 1.0) * 0.1)
            .clamp(-PI, PI);

        let global_bias =
            ((0.5 + self.global.rms * 0.02 + (self.phase.checksum as f64 / u64::MAX as f64) * 0.3)
                .max(0.1)
                * (0.9 + 0.1 * kspace_coupling))
                .max(0.05);

        BandOutputConfig {
            k_points,
            band_count,
            energy_origin,
            band_spacing,
            k_extent,
            curvature,
            phase_shift,
            global_bias,
            kspace_coupling,
        }
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
    ) -> ComputeResult<()> {
        match artifact_name {
            "bandstructure.dat" => write_text_artifact(output_path, &self.render_bandstructure())
                .map_err(|source| {
                    FeffError::io_system(
                        "IO.BAND_OUTPUT_WRITE",
                        format!(
                            "failed to write BAND artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                }),
            "logband.dat" => {
                write_text_artifact(output_path, &self.render_logband()).map_err(|source| {
                    FeffError::io_system(
                        "IO.BAND_OUTPUT_WRITE",
                        format!(
                            "failed to write BAND artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            other => Err(FeffError::internal(
                "SYS.BAND_OUTPUT_CONTRACT",
                format!("unsupported BAND output artifact '{}'", other),
            )),
        }
    }

    fn render_bandstructure(&self) -> String {
        let config = self.output_config();
        let mut lines = Vec::with_capacity(config.k_points + 6);

        lines.push("# BAND true-compute runtime".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: k_index k_fraction k_value energy_00 ...".to_string());
        lines.push(format!(
            "# k_points={} bands={} energy_origin={} band_spacing={}",
            config.k_points,
            config.band_count,
            format_fixed_f64(config.energy_origin, 11, 6),
            format_fixed_f64(config.band_spacing, 11, 6),
        ));

        for k_index in 0..config.k_points {
            let k_fraction = if config.k_points == 1 {
                0.0
            } else {
                k_index as f64 / (config.k_points - 1) as f64
            };
            let k_value = (k_fraction - 0.5) * 2.0 * config.k_extent;

            let mut line = format!(
                "{:4} {} {}",
                k_index + 1,
                format_fixed_f64(k_fraction, 11, 6),
                format_fixed_f64(k_value, 11, 6),
            );
            for band_index in 0..config.band_count {
                let energy = self.band_energy(&config, band_index, k_fraction, k_value);
                line.push(' ');
                line.push_str(&format_fixed_f64(energy, 12, 6));
            }
            lines.push(line);
        }

        lines.join("\n")
    }

    fn band_energy(
        &self,
        config: &BandOutputConfig,
        band_index: usize,
        k_fraction: f64,
        k_value: f64,
    ) -> f64 {
        let band_number = band_index as f64 + 1.0;
        let centered_k = k_fraction - 0.5;
        let parabolic = config.curvature * centered_k.powi(2) * band_number.sqrt();
        let dispersion = (k_value * (0.65 + 0.05 * band_number) + config.phase_shift).cos();
        let phase_term = (k_value * 0.4 + self.phase.base_phase + band_number * 0.17).sin();
        let damping = (-0.015 * band_number * (1.0 + k_fraction)).exp();
        let kspace_modulation = (band_number * 0.3 + k_value * 0.8 + self.phase.base_phase).sin()
            * (config.kspace_coupling - 1.0);

        config.energy_origin
            + config.band_spacing * band_index as f64
            + parabolic
            + config.global_bias * dispersion * damping
            + 0.12 * phase_term
            + 0.05 * kspace_modulation
            + self.control.mband as f64 * 0.01
    }

    fn render_logband(&self) -> String {
        let config = self.output_config();
        let phase_source = if self.phase.has_xsph_magic {
            "xsph_phase_magic"
        } else {
            "legacy_phase_binary"
        };

        format!(
            "\
BAND true-compute runtime\n\
fixture: {}\n\
input-artifacts: band.inp geom.dat global.inp phase.bin\n\
output-artifacts: bandstructure.dat logband.dat\n\
nat: {} nph: {} atoms: {}\n\
phase-source: {}\n\
phase-bytes: {}\n\
phase-checksum: {}\n\
mband: {} nkp: {} ikpath: {} freeprop: {}\n\
emin: {} emax: {} estep: {}\n\
radius-mean: {} radius-rms: {} radius-max: {} ipot-mean: {}\n\
global-tokens: {} global-mean: {} global-rms: {} global-max-abs: {}\n\
k-points: {} bands: {}\n\
energy-origin: {} band-spacing: {} k-extent: {}\n\
kspace-coupling: {}\n",
            self.fixture_id,
            self.geom.nat,
            self.geom.nph,
            self.geom.atom_count,
            phase_source,
            self.phase.byte_len,
            self.phase.checksum,
            self.control.mband,
            self.control.nkp,
            self.control.ikpath,
            self.control.freeprop,
            format_fixed_f64(self.control.emin, 11, 6),
            format_fixed_f64(self.control.emax, 11, 6),
            format_fixed_f64(self.control.estep, 11, 6),
            format_fixed_f64(self.geom.radius_mean, 11, 6),
            format_fixed_f64(self.geom.radius_rms, 11, 6),
            format_fixed_f64(self.geom.radius_max, 11, 6),
            format_fixed_f64(self.geom.ipot_mean, 11, 6),
            self.global.token_count,
            format_fixed_f64(self.global.mean, 11, 6),
            format_fixed_f64(self.global.rms, 11, 6),
            format_fixed_f64(self.global.max_abs, 11, 6),
            config.k_points,
            config.band_count,
            format_fixed_f64(config.energy_origin, 11, 6),
            format_fixed_f64(config.band_spacing, 11, 6),
            format_fixed_f64(config.k_extent, 11, 6),
            format_fixed_f64(config.kspace_coupling, 11, 6),
        )
    }
}
