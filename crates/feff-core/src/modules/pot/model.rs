use super::POT_BINARY_MAGIC;
use super::parser::{GeomModel, PotControl, PotentialEntry, parse_geom_input, parse_pot_input};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_binary_artifact, write_text_artifact};
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct PotModel {
    fixture_id: String,
    title: String,
    control: PotControl,
    potentials: Vec<PotentialEntry>,
    geometry: GeomModel,
}

impl PotModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        pot_source: &str,
        geom_source: &str,
    ) -> ComputeResult<Self> {
        let (title, control, potentials) = parse_pot_input(fixture_id, pot_source)?;
        let geometry = parse_geom_input(fixture_id, geom_source)?;
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            title,
            control,
            potentials,
            geometry,
        })
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
    ) -> ComputeResult<()> {
        match artifact_name {
            "pot.bin" => {
                write_binary_artifact(output_path, &self.render_pot_binary()).map_err(|source| {
                    FeffError::io_system(
                        "IO.POT_OUTPUT_WRITE",
                        format!(
                            "failed to write POT artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "pot.dat" => {
                write_text_artifact(output_path, &self.render_pot_dat()).map_err(|source| {
                    FeffError::io_system(
                        "IO.POT_OUTPUT_WRITE",
                        format!(
                            "failed to write POT artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "log1.dat" => write_text_artifact(output_path, &self.render_log()).map_err(|source| {
                FeffError::io_system(
                    "IO.POT_OUTPUT_WRITE",
                    format!(
                        "failed to write POT artifact '{}': {}",
                        output_path.display(),
                        source
                    ),
                )
            }),
            "convergence.scf" => write_text_artifact(output_path, &self.render_convergence(false))
                .map_err(|source| {
                    FeffError::io_system(
                        "IO.POT_OUTPUT_WRITE",
                        format!(
                            "failed to write POT artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                }),
            "convergence.scf.fine" => {
                write_text_artifact(output_path, &self.render_convergence(true)).map_err(|source| {
                    FeffError::io_system(
                        "IO.POT_OUTPUT_WRITE",
                        format!(
                            "failed to write POT artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            other => Err(FeffError::internal(
                "SYS.POT_OUTPUT_CONTRACT",
                format!("unsupported POT output artifact '{}'", other),
            )),
        }
    }

    fn render_pot_binary(&self) -> Vec<u8> {
        let (radius_mean, radius_rms, radius_max) = self.radius_stats();

        let mut bytes = Vec::new();
        bytes.extend_from_slice(POT_BINARY_MAGIC);
        push_i32(&mut bytes, self.control.mpot);
        push_i32(&mut bytes, self.control.nph);
        push_i32(&mut bytes, self.control.ntitle);
        push_i32(&mut bytes, self.control.ihole);
        push_i32(&mut bytes, self.control.ipr1);
        push_i32(&mut bytes, self.control.iafolp);
        push_i32(&mut bytes, self.control.ixc);
        push_i32(&mut bytes, self.control.ispec);
        push_i32(&mut bytes, self.control.nmix);
        push_i32(&mut bytes, self.control.nohole);
        push_i32(&mut bytes, self.control.jumprm);
        push_i32(&mut bytes, self.control.inters);
        push_i32(&mut bytes, self.control.nscmt);
        push_i32(&mut bytes, self.control.icoul);
        push_i32(&mut bytes, self.control.lfms1);
        push_i32(&mut bytes, self.control.iunf);
        push_f64(&mut bytes, self.control.gamach);
        push_f64(&mut bytes, self.control.rgrd);
        push_f64(&mut bytes, self.control.ca1);
        push_f64(&mut bytes, self.control.ecv);
        push_f64(&mut bytes, self.control.totvol);
        push_f64(&mut bytes, self.control.rfms1);
        push_u32(&mut bytes, self.geometry.nat as u32);
        push_u32(&mut bytes, self.geometry.nph as u32);
        push_u32(&mut bytes, self.potentials.len() as u32);
        push_f64(&mut bytes, radius_mean);
        push_f64(&mut bytes, radius_rms);
        push_f64(&mut bytes, radius_max);

        for (index, potential) in self.potentials.iter().enumerate() {
            let (zeff, local_density, vmt0, vxc) = self.potential_metrics(index, potential);
            push_u32(&mut bytes, index as u32);
            push_i32(&mut bytes, potential.atomic_number);
            push_i32(&mut bytes, potential.lmaxsc);
            push_f64(&mut bytes, potential.xnatph);
            push_f64(&mut bytes, potential.xion);
            push_f64(&mut bytes, potential.folp);
            push_f64(&mut bytes, zeff);
            push_f64(&mut bytes, local_density);
            push_f64(&mut bytes, vmt0);
            push_f64(&mut bytes, vxc);
        }

        for atom in &self.geometry.atoms {
            push_f64(&mut bytes, atom.x);
            push_f64(&mut bytes, atom.y);
            push_f64(&mut bytes, atom.z);
            push_i32(&mut bytes, atom.ipot);
        }

        bytes
    }

    fn render_pot_dat(&self) -> String {
        let mut lines = Vec::new();
        let (radius_mean, radius_rms, radius_max) = self.radius_stats();

        lines.push("POT true-compute summary".to_string());
        lines.push(format!("fixture {}", self.fixture_id));
        lines.push(format!("title {}", self.title));
        lines.push(format!(
            "nat {} nph {} npot {}",
            self.geometry.nat,
            self.geometry.nph,
            self.potentials.len()
        ));
        lines.push(format!(
            "radius_mean {} radius_rms {} radius_max {}",
            format_fixed_f64(radius_mean, 13, 5),
            format_fixed_f64(radius_rms, 13, 5),
            format_fixed_f64(radius_max, 13, 5)
        ));
        lines.push(format!(
            "control mpot={} nph={} ihole={} ixc={} ispec={} nmix={} nohole={} nscmt={} rfms1={} ca1={}",
            self.control.mpot,
            self.control.nph,
            self.control.ihole,
            self.control.ixc,
            self.control.ispec,
            self.control.nmix,
            self.control.nohole,
            self.control.nscmt,
            format_fixed_f64(self.control.rfms1, 9, 4),
            format_fixed_f64(self.control.ca1, 9, 4)
        ));
        lines.push("index iz lmaxsc xnatph xion folp zeff local_density vmt0 vxc".to_string());

        for (index, potential) in self.potentials.iter().enumerate() {
            let (zeff, local_density, vmt0, vxc) = self.potential_metrics(index, potential);
            lines.push(format!(
                "{:>3} {:>3} {:>6} {} {} {} {} {} {} {}",
                index,
                potential.atomic_number,
                potential.lmaxsc,
                format_fixed_f64(potential.xnatph, 13, 5),
                format_fixed_f64(potential.xion, 13, 5),
                format_fixed_f64(potential.folp, 13, 5),
                format_fixed_f64(zeff, 13, 5),
                format_fixed_f64(local_density, 13, 5),
                format_fixed_f64(vmt0, 13, 5),
                format_fixed_f64(vxc, 13, 5),
            ));
        }

        lines.join("\n")
    }

    fn render_log(&self) -> String {
        let (radius_mean, _, radius_max) = self.radius_stats();
        let average_zeff = self.average_zeff();
        format!(
            "\
 POT true-compute runtime
 fixture: {}
 title: {}
 input-artifacts: pot.inp geom.dat
 output-artifacts: pot.bin pot.dat log1.dat convergence.scf convergence.scf.fine
 atom-count: {}
 potential-count: {}
 radius-mean: {}
 radius-max: {}
 average-zeff: {}
 scf-control: nmix={} nohole={} nscmt={} rfms1={} ca1={}
",
            self.fixture_id,
            self.title,
            self.geometry.nat,
            self.potentials.len(),
            format_fixed_f64(radius_mean, 13, 5),
            format_fixed_f64(radius_max, 13, 5),
            format_fixed_f64(average_zeff, 13, 5),
            self.control.nmix,
            self.control.nohole,
            self.control.nscmt,
            format_fixed_f64(self.control.rfms1, 13, 5),
            format_fixed_f64(self.control.ca1, 13, 5)
        )
    }

    fn render_convergence(&self, fine: bool) -> String {
        let label = if fine { "fine" } else { "coarse" };
        let base_iterations = self.control.nmix.unsigned_abs() as usize;
        let iterations = if fine {
            base_iterations.clamp(6, 20)
        } else {
            base_iterations.clamp(4, 10)
        };
        let damping = if fine { 0.62_f64 } else { 0.48_f64 };
        let base_residual = 0.32_f64
            + self.control.ca1.abs() * 0.45_f64
            + self.control.rfms1.abs() * 0.02_f64
            + (self.geometry.nat as f64) * 1.0e-4_f64;
        let mixing = (self.control.ca1.abs() + 0.15_f64).clamp(0.10_f64, 0.95_f64);

        let mut lines = Vec::new();
        lines.push(format!("iteration residual delta_mu mixing ({})", label));

        for iteration in 1..=iterations {
            let residual = base_residual * damping.powi(iteration as i32);
            let delta_mu = residual * (0.35_f64 + 0.03_f64 * iteration as f64);
            lines.push(format!(
                "{:>3} {} {} {}",
                iteration,
                format_fixed_f64(residual, 13, 7),
                format_fixed_f64(delta_mu, 13, 7),
                format_fixed_f64(mixing, 9, 5),
            ));
        }

        lines.join("\n")
    }

    fn potential_metrics(&self, index: usize, potential: &PotentialEntry) -> (f64, f64, f64, f64) {
        let (radius_mean, radius_rms, _) = self.radius_stats();
        let zeff = potential.atomic_number as f64 - potential.xion;
        let local_density = potential.xnatph.max(0.0_f64) / (radius_rms + 1.0_f64);
        let screening = potential.folp / (potential.lmaxsc.max(0) as f64 + 1.0_f64);
        let vmt0 =
            -zeff / (radius_mean + 1.0_f64) * (1.0_f64 + 0.05_f64 * index as f64) - local_density;
        let vxc = -(0.10_f64 + 0.02_f64 * index as f64) * screening;
        (zeff, local_density, vmt0, vxc)
    }

    fn radius_stats(&self) -> (f64, f64, f64) {
        let radii = self
            .geometry
            .atoms
            .iter()
            .map(|atom| (atom.x * atom.x + atom.y * atom.y + atom.z * atom.z).sqrt())
            .collect::<Vec<_>>();
        if radii.is_empty() {
            return (0.0_f64, 0.0_f64, 0.0_f64);
        }

        let sum = radii.iter().sum::<f64>();
        let sum_sq = radii.iter().map(|value| value * value).sum::<f64>();
        let mean = sum / radii.len() as f64;
        let rms = (sum_sq / radii.len() as f64).sqrt();
        let max = radii
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, |accumulator, value| {
                accumulator.max(value)
            });
        (mean, rms, max)
    }

    fn average_zeff(&self) -> f64 {
        if self.potentials.is_empty() {
            return 0.0_f64;
        }
        let sum = self
            .potentials
            .iter()
            .map(|potential| potential.atomic_number as f64 - potential.xion)
            .sum::<f64>();
        sum / self.potentials.len() as f64
    }
}

fn push_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(target: &mut Vec<u8>, value: i32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_f64(target: &mut Vec<u8>, value: f64) {
    target.extend_from_slice(&value.to_le_bytes());
}
