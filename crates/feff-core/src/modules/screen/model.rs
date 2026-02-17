use super::parser::{
    GeomScreenInput, LdosScreenInput, PotBinaryScreenInput, PotScreenInput, ScreenOverrideInput,
    format_scientific_f64, parse_geom_source, parse_ldos_source, parse_pot_binary_source,
    parse_pot_source, parse_screen_override_source,
};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use std::f64::consts::PI;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct ScreenModel {
    fixture_id: String,
    pot: PotScreenInput,
    pot_binary: Option<PotBinaryScreenInput>,
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
    pot_potential_count: usize,
    pot_density: f64,
    pot_charge_scale: f64,
}

impl ScreenModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        pot_source: &str,
        geom_source: &str,
        ldos_source: &str,
        screen_source: Option<&str>,
        pot_binary_source: Option<&[u8]>,
    ) -> ComputeResult<Self> {
        let pot = parse_pot_source(fixture_id, pot_source)?;
        let pot_binary = pot_binary_source
            .map(|bytes| parse_pot_binary_source(fixture_id, bytes))
            .transpose()?;
        let geom = parse_geom_source(fixture_id, geom_source)?;
        let ldos = parse_ldos_source(fixture_id, ldos_source)?;
        let screen_override = match screen_source {
            Some(source) => Some(parse_screen_override_source(fixture_id, source)?),
            None => None,
        };

        Ok(Self {
            fixture_id: fixture_id.to_string(),
            pot,
            pot_binary,
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
        let pot_binary = self.pot_binary.as_ref();

        let pot_npot = pot_binary.map(|pot| pot.npot.max(1)).unwrap_or(1);
        let pot_nph = pot_binary
            .map(|pot| pot.nph.max(1))
            .unwrap_or(self.geom.nph.max(1));
        let atom_scale = pot_binary
            .map(|pot| pot.nat.max(1))
            .unwrap_or(self.geom.nat.max(1));
        let pot_lmaxsc_max = pot_binary
            .map(|pot| pot.lmaxsc_max.max(1))
            .unwrap_or(self.pot.lmaxsc_max.max(1));
        let pot_rfms = pot_binary
            .map(|pot| pot.rfms1.max(0.1))
            .unwrap_or(self.pot.rfms1.max(0.1));
        let pot_radius_mean = pot_binary
            .map(|pot| pot.radius_mean.max(1.0e-6))
            .unwrap_or(self.geom.radius_mean.max(1.0e-6));
        let pot_radius_rms = pot_binary
            .map(|pot| pot.radius_rms.max(1.0e-6))
            .unwrap_or(self.geom.radius_rms.max(1.0e-6));
        let pot_radius_max = pot_binary
            .map(|pot| pot.radius_max.max(1.0e-6))
            .unwrap_or(self.geom.radius_max.max(1.0e-6));
        let pot_gamach = pot_binary
            .map(|pot| pot.gamach.abs())
            .unwrap_or(self.pot.gamach.abs());

        let ner = override_input
            .and_then(|input| input.ner)
            .unwrap_or((self.ldos.neldos / 2).max(8));
        let nei = override_input
            .and_then(|input| input.nei)
            .unwrap_or((self.ldos.neldos / 10).max(4));
        let radial_points =
            ((ner.max(1) as usize * 4) + nei.max(1) as usize + pot_npot * 3).clamp(24, 512);

        let maxl = override_input
            .and_then(|input| input.maxl)
            .unwrap_or(self.ldos.lmaxph_max.max(pot_lmaxsc_max).max(1));
        let rfms = override_input
            .and_then(|input| input.rfms)
            .unwrap_or(self.ldos.rfms2.max(pot_rfms))
            .max(0.5);
        let radius_min = (pot_radius_mean.max(0.1) * 1.0e-3).max(1.0e-4);
        let radius_max =
            (rfms + pot_radius_max * 0.2 + self.geom.radius_max * 0.1).max(radius_min + 1.0e-3);

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

        let pot_density = pot_binary
            .map(|pot| pot.mean_local_density.abs())
            .unwrap_or_else(|| {
                (self.pot.mean_folp.abs() + self.pot.mean_xion.abs() + 1.0e-6)
                    / (self.geom.radius_rms + 1.0)
            })
            .max(1.0e-12);
        let pot_charge_scale = pot_binary
            .map(|pot| pot.mean_zeff.abs())
            .unwrap_or(self.pot.mean_xion.abs() + 1.0)
            .max(1.0e-6);
        let pot_vmt0 = pot_binary
            .map(|pot| pot.mean_vmt0.abs())
            .unwrap_or((self.pot.mean_folp + self.pot.mean_xion).abs());
        let pot_vxc = pot_binary
            .map(|pot| pot.mean_vxc.abs() + 1.0e-6)
            .unwrap_or(self.pot.mean_folp.abs() * 0.1 + 1.0e-6);
        let screening_wavevector = (4.0 * PI * pot_density).powf(1.0 / 3.0);

        let screen_level =
            (0.09 * screening_wavevector + 0.012 * pot_gamach + 0.006 * pot_vxc + 0.004 * pot_vmt0)
                * (1.0 + 0.010 * maxl as f64 + 0.004 * lfxc as f64 + 0.006 * ipot_mean.abs());
        let screen_level = screen_level.max(1.0e-6);
        let screen_amplitude = (self.geom.radius_rms
            + pot_radius_rms * 0.25
            + self.ldos.rgrd.abs()
            + ermin)
            * (1.0 + 0.006 * ner as f64 + 0.008 * eimax.abs() + 0.002 * (pot_npot as f64).sqrt());
        let charge_delta =
            (pot_charge_scale + self.ldos.toler1 + self.ldos.toler2 + pot_density.sqrt())
                * (1.0 + 0.003 * (atom_scale as f64).sqrt() + 0.002 * (pot_nph as f64).sqrt());
        let decay_rate = (screening_wavevector / (1.0 + rfms + pot_radius_mean)).max(1.0e-4);
        let energy_bias = energy_span / (self.ldos.neldos.max(1) as f64 * (20.0 + pot_npot as f64));

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
            pot_potential_count: pot_npot,
            pot_density,
            pot_charge_scale,
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
        let pot_source = if self.pot_binary.is_some() {
            "pot.bin"
        } else {
            "pot.inp-fallback"
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
pot_source: {}\n\
pot_potentials: {}\n\
pot_density: {}\n\
pot_charge_scale: {}\n\
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
            pot_source,
            config.pot_potential_count,
            format_fixed_f64(config.pot_density, 13, 7),
            format_fixed_f64(config.pot_charge_scale, 13, 7),
            has_override,
            nrptx0,
        )
    }
}
