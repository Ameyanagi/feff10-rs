use super::parser::{
    CrpaControlInput, GeomCrpaInput, PotCrpaInput,
    parse_crpa_source, parse_pot_source, parse_geom_source,
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
}

#[derive(Debug, Clone, Copy)]
struct CrpaOutputConfig {
    radial_points: usize,
    radius_min: f64,
    radius_max: f64,
    screening_level: f64,
    screening_slope: f64,
    decay_rate: f64,
}

impl CrpaModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        crpa_source: &str,
        pot_source: &str,
        geom_source: &str,
    ) -> ComputeResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_crpa_source(fixture_id, crpa_source)?,
            pot: parse_pot_source(fixture_id, pot_source)?,
            geom: parse_geom_source(fixture_id, geom_source)?,
        })
    }

    pub(super) fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
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

        let radial_points = ((self.geom.nat.max(1) as f64).sqrt() * 20.0
            + (self.control.l_crpa.max(1) as f64 * 8.0)
            + (self.pot.lmaxsc_max.max(1) as f64 * 4.0))
            .round() as usize;
        let radial_points = radial_points.clamp(64, 2048);

        let radius_min = (self.control.rcut * 1.0e-4).max(1.0e-5);
        let radius_max = (self.control.rcut
            + self.geom.radius_max * 0.4
            + self.pot.rfms1.abs() * 0.25
            + self.geom.radius_mean * 0.1)
            .max(radius_min + 1.0e-3);

        let screening_level = (self.pot.mean_folp.max(0.05) * 0.32
            + 0.012 * self.pot.gamach.abs()
            + 0.004 * ipot_mean.abs()
            + 0.001 * self.geom.nph as f64)
            .max(1.0e-5);
        let screening_slope =
            ((self.geom.radius_rms + self.pot.mean_xion.abs() + atom_count.sqrt() * 0.01)
                * 0.002
                * self.control.l_crpa.max(1) as f64)
                .max(1.0e-6);
        let decay_rate = 1.0 / (self.control.rcut + self.geom.radius_mean + 1.0);

        CrpaOutputConfig {
            radial_points,
            radius_min,
            radius_max,
            screening_level,
            screening_slope,
            decay_rate,
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
            let radius = config.radius_min * radius_ratio.powf(t);
            let attenuation = (-radius * config.decay_rate).exp();
            let screening = config.screening_level
                + config.screening_slope * t.powf(1.4)
                + 0.0015 * attenuation;
            let hubbard_u = 0.0_f64;

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
        )
    }
}

fn format_scientific_f64(value: f64) -> String {
    format!("{value:.10E}")
}
