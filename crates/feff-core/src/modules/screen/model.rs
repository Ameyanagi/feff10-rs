use super::parser::{
    GeomScreenInput, LdosScreenInput, PotScreenInput, ScreenOverrideInput, format_scientific_f64,
    parse_geom_source, parse_ldos_source, parse_pot_source, parse_screen_override_source,
};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::helpers::mkgtr_workflow_coupling;
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct ScreenModel {
    fixture_id: String,
    pot: PotScreenInput,
    geom: GeomScreenInput,
    ldos: LdosScreenInput,
    screen_override: Option<ScreenOverrideInput>,
}

#[derive(Debug, Clone, Copy)]
struct ScreenOutputConfig {
    radial_points: usize,
    radius_min: f64,
    radius_max: f64,
    screen_level: f64,
    screen_amplitude: f64,
    charge_delta: f64,
    decay_rate: f64,
    energy_bias: f64,
    maxl: i32,
    ner: i32,
    nei: i32,
    rfms: f64,
    mkgtr_coupling: f64,
}

impl ScreenModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        pot_source: &str,
        geom_source: &str,
        ldos_source: &str,
        screen_source: Option<&str>,
    ) -> ComputeResult<Self> {
        let pot = parse_pot_source(fixture_id, pot_source)?;
        let geom = parse_geom_source(fixture_id, geom_source)?;
        let ldos = parse_ldos_source(fixture_id, ldos_source)?;
        let screen_override = match screen_source {
            Some(source) => Some(parse_screen_override_source(fixture_id, source)?),
            None => None,
        };

        Ok(Self {
            fixture_id: fixture_id.to_string(),
            pot,
            geom,
            ldos,
            screen_override,
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
                        "IO.SCREEN_OUTPUT_WRITE",
                        format!(
                            "failed to write SCREEN artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "logscreen.dat" => {
                write_text_artifact(output_path, &self.render_log()).map_err(|source| {
                    FeffError::io_system(
                        "IO.SCREEN_OUTPUT_WRITE",
                        format!(
                            "failed to write SCREEN artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            other => Err(FeffError::internal(
                "SYS.SCREEN_OUTPUT_CONTRACT",
                format!("unsupported SCREEN output artifact '{}'", other),
            )),
        }
    }

    fn output_config(&self) -> ScreenOutputConfig {
        let override_input = self.screen_override.as_ref();

        let ner = override_input
            .and_then(|input| input.ner)
            .unwrap_or((self.ldos.neldos / 2).max(8));
        let nei = override_input
            .and_then(|input| input.nei)
            .unwrap_or((self.ldos.neldos / 10).max(4));
        let radial_points = ((ner.max(1) as usize * 4) + nei.max(1) as usize).clamp(24, 512);

        let maxl = override_input
            .and_then(|input| input.maxl)
            .unwrap_or(self.ldos.lmaxph_max.max(self.pot.lmaxsc_max).max(1));
        let rfms = override_input
            .and_then(|input| input.rfms)
            .unwrap_or(self.ldos.rfms2.max(self.pot.rfms1))
            .max(0.5);
        let radius_min = (self.geom.radius_mean.max(0.1) * 1.0e-3).max(1.0e-4);
        let radius_max = (rfms + self.geom.radius_max * 0.15).max(radius_min + 1.0e-3);

        let effective_emin = override_input
            .and_then(|input| input.emin)
            .unwrap_or(self.ldos.emin);
        let effective_emax = override_input
            .and_then(|input| input.emax)
            .unwrap_or(self.ldos.emax);
        let energy_span = (effective_emax - effective_emin).abs().max(1.0);

        let eimax = override_input
            .and_then(|input| input.eimax)
            .unwrap_or(self.ldos.eimag.abs());
        let ermin = override_input
            .and_then(|input| input.ermin)
            .unwrap_or(1.0e-3)
            .abs()
            .max(1.0e-6);
        let lfxc = override_input.and_then(|input| input.lfxc).unwrap_or(0);
        let ipot_mean = self
            .geom
            .atoms
            .iter()
            .map(|atom| atom.ipot as f64)
            .sum::<f64>()
            / self.geom.atoms.len() as f64;
        let mkgtr_coupling = mkgtr_workflow_coupling(maxl, ner, nei, false);

        let screen_level = (self.pot.mean_folp.max(0.05) + 0.02 * self.pot.gamach.abs())
            * (1.0 + 0.015 * maxl as f64 + 0.005 * lfxc as f64 + 0.01 * ipot_mean.abs())
            * (0.9 + 0.2 * mkgtr_coupling);
        let screen_amplitude = (self.geom.radius_rms + self.ldos.rgrd.abs() + ermin)
            * (1.0 + 0.01 * ner as f64 + 0.02 * eimax.abs());
        let screen_amplitude = screen_amplitude * (0.85 + 0.15 * mkgtr_coupling.sqrt());
        let charge_delta = (self.pot.mean_xion.abs() + self.ldos.toler1 + self.ldos.toler2 + 0.1)
            * (1.0 + 0.005 * (self.geom.nat as f64).sqrt())
            * (1.0 + 0.05 * (mkgtr_coupling - 1.0));
        let decay_rate = 1.0 / (rfms + self.geom.radius_mean + 1.0);
        let energy_bias = energy_span / (self.ldos.neldos.max(1) as f64 * 25.0);

        ScreenOutputConfig {
            radial_points,
            radius_min,
            radius_max,
            screen_level,
            screen_amplitude,
            charge_delta,
            decay_rate,
            energy_bias,
            maxl,
            ner,
            nei,
            rfms,
            mkgtr_coupling,
        }
    }

    fn render_wscrn(&self) -> String {
        let config = self.output_config();
        let irrh = self
            .screen_override
            .as_ref()
            .and_then(|input| input.irrh)
            .unwrap_or(1)
            .max(0) as f64;
        let iend = self
            .screen_override
            .as_ref()
            .and_then(|input| input.iend)
            .unwrap_or(0)
            .max(0) as f64;
        let response_power = (1.0 + 0.10 * irrh + 0.05 * iend).max(0.25);

        let mut lines = Vec::with_capacity(config.radial_points + 1);
        lines.push("# r       w_scrn(r)      v_ch(r)".to_string());
        for index in 0..config.radial_points {
            let t = if config.radial_points == 1 {
                0.0
            } else {
                index as f64 / (config.radial_points - 1) as f64
            };
            let radius = config.radius_min + (config.radius_max - config.radius_min) * t;
            let attenuation = (-radius * config.decay_rate).exp();
            let w_scrn = config.screen_level + config.screen_amplitude * attenuation;
            let v_ch = w_scrn
                + config.charge_delta * (1.0 - t).powf(response_power)
                + config.energy_bias * t;

            lines.push(format!(
                "{:>16} {:>16} {:>16}",
                format_scientific_f64(radius),
                format_scientific_f64(w_scrn),
                format_scientific_f64(v_ch)
            ));
        }

        lines.join("\n")
    }

    fn render_log(&self) -> String {
        let config = self.output_config();
        let has_override = if self.screen_override.is_some() {
            "present"
        } else {
            "absent"
        };
        let nrptx0 = self
            .screen_override
            .as_ref()
            .and_then(|input| input.nrptx0)
            .unwrap_or(config.radial_points as i32);

        format!(
            "\
SCREEN true-compute runtime\n\
fixture: {}\n\
title: {}\n\
nat: {} nph: {} atoms: {}\n\
neldos: {}\n\
radial_points: {}\n\
rfms: {}\n\
ner: {} nei: {} maxl: {}\n\
mkgtr_coupling: {}\n\
optional_screen_inp: {}\n\
nrptx0: {}\n\
",
            self.fixture_id,
            self.pot.title,
            self.geom.nat,
            self.geom.nph,
            self.geom.atoms.len(),
            self.ldos.neldos,
            config.radial_points,
            format_fixed_f64(config.rfms, 10, 5),
            config.ner,
            config.nei,
            config.maxl,
            format_fixed_f64(config.mkgtr_coupling, 10, 6),
            has_override,
            nrptx0,
        )
    }
}
