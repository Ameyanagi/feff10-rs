use super::parser::{
    CrpaControlInput, GeomCrpaInput, PotCrpaInput, ScreenWscrnInput, parse_crpa_source,
    parse_geom_source, parse_pot_source, parse_wscrn_source,
};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct CrpaModel {
    fixture_id: String,
    control: CrpaControlInput,
    pot: PotCrpaInput,
    geom: GeomCrpaInput,
    screen_wscrn: Option<ScreenWscrnInput>,
    screen_log_present: bool,
}

#[derive(Debug, Clone, Copy)]
struct CrpaOutputConfig {
    radial_points: usize,
    radius_min: f64,
    radius_max: f64,
    screening_level: f64,
    screening_slope: f64,
    decay_rate: f64,
    coupling_gain: f64,
    hubbard_scale: f64,
}

impl CrpaModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        crpa_source: &str,
        pot_source: &str,
        geom_source: &str,
        screen_wscrn_source: Option<&str>,
        screen_log_source: Option<&str>,
    ) -> ComputeResult<Self> {
        let screen_log_present = screen_log_source
            .map(|source| {
                source
                    .to_ascii_lowercase()
                    .contains("screen true-compute runtime")
            })
            .unwrap_or(false);

        let screen_wscrn = if screen_log_present {
            screen_wscrn_source
                .map(|source| parse_wscrn_source(fixture_id, source))
                .transpose()?
        } else {
            None
        };

        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_crpa_source(fixture_id, crpa_source)?,
            pot: parse_pot_source(fixture_id, pot_source)?,
            geom: parse_geom_source(fixture_id, geom_source)?,
            screen_wscrn,
            screen_log_present,
        })
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
    ) -> ComputeResult<()> {
        match artifact_name {
            "wscrn.dat" => {
                write_text_artifact(output_path, &self.render_wscrn()).map_err(|source| {
                    FeffError::io_system(
                        "IO.CRPA_OUTPUT_WRITE",
                        format!(
                            "failed to write CRPA artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "logscrn.dat" => {
                write_text_artifact(output_path, &self.render_log()).map_err(|source| {
                    FeffError::io_system(
                        "IO.CRPA_OUTPUT_WRITE",
                        format!(
                            "failed to write CRPA artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            other => Err(FeffError::internal(
                "SYS.CRPA_OUTPUT_CONTRACT",
                format!("unsupported CRPA output artifact '{}'", other),
            )),
        }
    }

    fn output_config(&self) -> CrpaOutputConfig {
        let atom_count = self.geom.atoms.len() as f64;
        let ipot_mean = self
            .geom
            .atoms
            .iter()
            .map(|atom| atom.ipot as f64)
            .sum::<f64>()
            / atom_count;

        let base_radial_points = ((self.geom.nat.max(1) as f64).sqrt() * 20.0
            + (self.control.l_crpa.max(1) as f64 * 8.0)
            + (self.pot.lmaxsc_max.max(1) as f64 * 4.0))
            .round() as usize;
        let base_radial_points = base_radial_points.clamp(64, 2048);

        let radial_points = self
            .screen_wscrn
            .as_ref()
            .map(|screen| screen.radial_rows.len().clamp(64, 4096))
            .unwrap_or(base_radial_points);

        let base_radius_min = (self.control.rcut * 1.0e-4).max(1.0e-5);
        let base_radius_max = (self.control.rcut
            + self.geom.radius_max * 0.4
            + self.pot.rfms1.abs() * 0.25
            + self.geom.radius_mean * 0.1)
            .max(base_radius_min + 1.0e-3);

        let radius_min = self
            .screen_wscrn
            .as_ref()
            .map(|screen| screen.radius_min.max(1.0e-5))
            .unwrap_or(base_radius_min);
        let radius_max = self
            .screen_wscrn
            .as_ref()
            .map(|screen| {
                (screen.radius_max + self.control.rcut * 0.05)
                    .max(base_radius_max * 0.5)
                    .max(radius_min + 1.0e-3)
            })
            .unwrap_or(base_radius_max)
            .max(radius_min + 1.0e-3);

        let base_screening_level = (self.pot.mean_folp.max(0.05) * 0.32
            + 0.012 * self.pot.gamach.abs()
            + 0.004 * ipot_mean.abs()
            + 0.001 * self.geom.nph as f64)
            .max(1.0e-5);
        let base_screening_slope =
            ((self.geom.radius_rms + self.pot.mean_xion.abs() + atom_count.sqrt() * 0.01)
                * 0.002
                * self.control.l_crpa.max(1) as f64)
                .max(1.0e-6);
        let base_decay_rate = 1.0 / (self.control.rcut + self.geom.radius_mean + 1.0);

        let screening_level = self
            .screen_wscrn
            .as_ref()
            .map(|screen| {
                (0.70 * screen.screen_mean.abs()
                    + 0.25 * screen.charge_mean.abs()
                    + 0.05 * base_screening_level)
                    .max(1.0e-6)
            })
            .unwrap_or(base_screening_level);
        let screening_slope = self
            .screen_wscrn
            .as_ref()
            .map(|screen| (0.35 * screen.delta_mean + 0.65 * base_screening_slope).max(1.0e-6))
            .unwrap_or(base_screening_slope);
        let decay_rate = self
            .screen_wscrn
            .as_ref()
            .map(|screen| {
                (1.0 / (self.control.rcut + screen.radius_max + self.geom.radius_mean + 1.0))
                    .max(1.0e-6)
            })
            .unwrap_or(base_decay_rate);

        let coupling_gain = self
            .screen_wscrn
            .as_ref()
            .map(|screen| {
                (1.0 + 0.02 * self.control.l_crpa.max(1) as f64
                    + 0.01 * self.pot.lmaxsc_max.max(1) as f64
                    + 0.002 * screen.delta_mean)
                    .clamp(1.0, 2.5)
            })
            .unwrap_or(1.0);

        let hubbard_scale = self
            .screen_wscrn
            .as_ref()
            .map(|screen| {
                (0.08 + 0.01 * self.control.l_crpa.max(1) as f64 + 0.003 * screen.delta_mean)
                    .clamp(0.02, 0.5)
            })
            .unwrap_or(0.0);

        CrpaOutputConfig {
            radial_points,
            radius_min,
            radius_max,
            screening_level,
            screening_slope,
            decay_rate,
            coupling_gain,
            hubbard_scale,
        }
    }

    fn render_wscrn(&self) -> String {
        let config = self.output_config();
        let radius_ratio = (config.radius_max / config.radius_min).max(1.0 + 1.0e-9);
        let mut lines = Vec::with_capacity(config.radial_points);

        for index in 0..config.radial_points {
            let t = if config.radial_points == 1 {
                0.0
            } else {
                index as f64 / (config.radial_points - 1) as f64
            };
            let mut radius = config.radius_min * radius_ratio.powf(t);
            let screen_row = self.screen_wscrn.as_ref().and_then(|screen| {
                let row_count = screen.radial_rows.len();
                if row_count == 0 {
                    return None;
                }
                let source_index = if row_count == 1 {
                    0
                } else {
                    ((row_count - 1) as f64 * t).round() as usize
                }
                .min(row_count - 1);
                Some(screen.radial_rows[source_index])
            });
            if let Some(row) = screen_row {
                radius = row.radius.clamp(config.radius_min, config.radius_max);
            }
            let attenuation = (-radius * config.decay_rate).exp();
            let (screening, hubbard_u) = if let Some(row) = screen_row {
                let blended_screen = 0.60 * row.screened + 0.40 * row.charge;
                let charge_delta = (row.charge - row.screened).abs();
                let screening = (config.screening_level
                    + config.coupling_gain * blended_screen.abs() * 0.10
                    + config.screening_slope * t.powf(1.3)
                    + 0.0015 * attenuation)
                    .max(1.0e-12);
                let hubbard_u = (charge_delta * config.hubbard_scale * (1.0 + 0.20 * (1.0 - t))
                    + 1.0e-8)
                    .max(0.0);
                (screening, hubbard_u)
            } else {
                (
                    config.screening_level
                        + config.screening_slope * t.powf(1.4)
                        + 0.0015 * attenuation,
                    0.0_f64,
                )
            };

            lines.push(format!(
                "{:>16} {:>16} {:>16}",
                format_scientific_f64(radius),
                format_scientific_f64(screening),
                format_scientific_f64(hubbard_u)
            ));
        }

        lines.join("\n")
    }

    fn render_log(&self) -> String {
        let screen_wscrn_status = if self.screen_wscrn.is_some() {
            "present"
        } else {
            "absent"
        };
        let screen_log_status = if self.screen_log_present {
            "present"
        } else {
            "absent"
        };
        let screen_wscrn_rows = self
            .screen_wscrn
            .as_ref()
            .map(|screen| screen.radial_rows.len())
            .unwrap_or(0);

        format!(
            "\
CRPA true-compute runtime\n\
fixture: {}\n\
title: {}\n\
rcut: {}\n\
l_crpa: {}\n\
nat: {} nph: {} atoms: {}\n\
gamach: {}\n\
rfms1: {}\n\
optional_screen_log: {}\n\
optional_screen_wscrn: {}\n\
screen_wscrn_rows: {}\n\
",
            self.fixture_id,
            self.pot.title,
            format_fixed_f64(self.control.rcut, 10, 5),
            self.control.l_crpa,
            self.geom.nat,
            self.geom.nph,
            self.geom.atoms.len(),
            format_fixed_f64(self.pot.gamach, 10, 5),
            format_fixed_f64(self.pot.rfms1, 10, 5),
            screen_log_status,
            screen_wscrn_status,
            screen_wscrn_rows,
        )
    }
}

fn format_scientific_f64(value: f64) -> String {
    format!("{value:.10E}")
}
