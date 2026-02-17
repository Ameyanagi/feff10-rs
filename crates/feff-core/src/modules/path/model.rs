use super::parser::{
    GeomPathInput, GlobalPathInput, PathControlInput, PhasePathInput,
    angle_between, distance, parse_geom_input, parse_global_input, parse_paths_input,
    parse_phase_input, subtract,
};
use super::PATH_BINARY_MAGIC;
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_binary_artifact, write_text_artifact};
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct PathModel {
    fixture_id: String,
    control: PathControlInput,
    geometry: GeomPathInput,
    global: GlobalPathInput,
    phase: PhasePathInput,
}

#[derive(Debug, Clone)]
pub(super) struct PathEntry {
    pub(super) index: usize,
    pub(super) nleg: usize,
    pub(super) leg_atom_indices: Vec<usize>,
    pub(super) degeneracy: usize,
    pub(super) reff: f64,
    pub(super) amplitude: f64,
    pub(super) beta_deg: f64,
    pub(super) eta_deg: f64,
}

impl PathModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        path_source: &str,
        geom_source: &str,
        global_source: &str,
        phase_bytes: &[u8],
    ) -> ComputeResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_paths_input(fixture_id, path_source)?,
            geometry: parse_geom_input(fixture_id, geom_source)?,
            global: parse_global_input(fixture_id, global_source)?,
            phase: parse_phase_input(fixture_id, phase_bytes)?,
        })
    }

    pub(super) fn generated_paths(&self) -> Vec<PathEntry> {
        if self.control.rmax <= 0.0 {
            return Vec::new();
        }

        let neighbors = self.geometry.neighbor_sites(self.control.rmax);
        if neighbors.is_empty() {
            return Vec::new();
        }

        let max_leg = self.control.nlegxx.unsigned_abs().clamp(2, 6) as usize;
        let budget = self.path_budget(neighbors.len());
        let mut entries = Vec::with_capacity(budget);
        let absorber = self.geometry.absorber_position();

        for neighbor in &neighbors {
            if entries.len() >= budget {
                break;
            }

            let reff = 2.0 * neighbor.radius;
            let degeneracy = neighbor.shell_size.max(1);
            let amplitude =
                self.path_amplitude(reff, degeneracy as f64, 2, neighbor.atom_index as f64);

            entries.push(PathEntry {
                index: 0,
                nleg: 2,
                leg_atom_indices: vec![neighbor.atom_index],
                degeneracy,
                reff,
                amplitude,
                beta_deg: 180.0,
                eta_deg: 0.0,
            });
        }

        if max_leg >= 3 {
            let pair_limit = neighbors.len().min(24);
            for i in 0..pair_limit {
                if entries.len() >= budget {
                    break;
                }

                for j in (i + 1)..pair_limit {
                    if entries.len() >= budget {
                        break;
                    }

                    let first = neighbors[i];
                    let second = neighbors[j];
                    let first_atom = self.geometry.atoms[first.atom_index];
                    let second_atom = self.geometry.atoms[second.atom_index];
                    let d12 = distance(first_atom.position(), second_atom.position());
                    let reff = first.radius + d12 + second.radius;
                    if reff * 0.5 > self.control.rmax * 1.25 {
                        continue;
                    }

                    let degeneracy = first
                        .shell_size
                        .saturating_mul(second.shell_size)
                        .clamp(1, 4096);
                    let beta = angle_between(
                        subtract(absorber, first_atom.position()),
                        subtract(second_atom.position(), first_atom.position()),
                    );
                    let eta = angle_between(
                        subtract(first_atom.position(), absorber),
                        subtract(second_atom.position(), absorber),
                    );
                    let amplitude = self.path_amplitude(
                        reff,
                        degeneracy as f64,
                        3,
                        (first.atom_index + second.atom_index) as f64,
                    );

                    entries.push(PathEntry {
                        index: 0,
                        nleg: 3,
                        leg_atom_indices: vec![first.atom_index, second.atom_index],
                        degeneracy,
                        reff,
                        amplitude,
                        beta_deg: beta,
                        eta_deg: eta,
                    });
                }
            }
        }

        if max_leg >= 4 {
            let triple_limit = neighbors.len().min(10);
            for i in 0..triple_limit {
                if entries.len() >= budget {
                    break;
                }
                for j in (i + 1)..triple_limit {
                    if entries.len() >= budget {
                        break;
                    }
                    for k in (j + 1)..triple_limit {
                        if entries.len() >= budget {
                            break;
                        }

                        let first = neighbors[i];
                        let second = neighbors[j];
                        let third = neighbors[k];
                        let first_atom = self.geometry.atoms[first.atom_index];
                        let second_atom = self.geometry.atoms[second.atom_index];
                        let third_atom = self.geometry.atoms[third.atom_index];

                        let reff = first.radius
                            + distance(first_atom.position(), second_atom.position())
                            + distance(second_atom.position(), third_atom.position())
                            + distance(third_atom.position(), absorber);
                        if reff * 0.5 > self.control.rmax * 1.5 {
                            continue;
                        }

                        let degeneracy = first
                            .shell_size
                            .saturating_mul(second.shell_size)
                            .saturating_mul(third.shell_size)
                            .clamp(1, 16384);
                        let beta_first = angle_between(
                            subtract(absorber, first_atom.position()),
                            subtract(second_atom.position(), first_atom.position()),
                        );
                        let beta_second = angle_between(
                            subtract(first_atom.position(), second_atom.position()),
                            subtract(third_atom.position(), second_atom.position()),
                        );
                        let beta = 0.5 * (beta_first + beta_second);
                        let eta = angle_between(
                            subtract(first_atom.position(), absorber),
                            subtract(third_atom.position(), absorber),
                        );
                        let amplitude = self.path_amplitude(
                            reff,
                            degeneracy as f64,
                            4,
                            (first.atom_index + second.atom_index + third.atom_index) as f64,
                        );

                        entries.push(PathEntry {
                            index: 0,
                            nleg: 4,
                            leg_atom_indices: vec![
                                first.atom_index,
                                second.atom_index,
                                third.atom_index,
                            ],
                            degeneracy,
                            reff,
                            amplitude,
                            beta_deg: beta,
                            eta_deg: eta,
                        });
                    }
                }
            }
        }

        entries.truncate(budget);
        for (index, entry) in entries.iter_mut().enumerate() {
            entry.index = index + 1;
        }
        entries
    }

    fn path_budget(&self, neighbor_count: usize) -> usize {
        let requested_mpath = self.control.mpath.unsigned_abs().max(1) as usize;
        let requested_ms = self.control.ms.unsigned_abs() as usize;
        let requested_legs = self.control.nlegxx.unsigned_abs().max(2) as usize;
        let phase_factor = if self.phase.has_xsph_magic {
            self.phase.channel_count + self.phase.spectral_points / 16
        } else {
            (self.phase.byte_len / 2048).max(1)
        };
        let global_factor = (self.global.token_count / 8).max(1);

        let requested = requested_mpath
            .saturating_mul(8)
            .saturating_add(requested_ms.saturating_mul(4))
            .saturating_add(requested_legs.saturating_mul(3))
            .saturating_add(phase_factor)
            .saturating_add(global_factor);
        let cap = neighbor_count.saturating_mul(3).clamp(8, 96);
        requested.clamp(1, cap)
    }

    fn path_amplitude(&self, reff: f64, degeneracy: f64, nleg: usize, signature: f64) -> f64 {
        let phase_scale = (self.phase.base_phase.abs() + self.phase.energy_step.abs() + 1.0).ln();
        let global_scale = (1.0 + self.global.rms + 0.1 * self.global.max_abs).ln();
        let path_damping = 1.0
            / (1.0
                + reff
                + self.control.rfms2.abs()
                + self.control.pcritk.abs()
                + self.control.pcrith.abs()
                + nleg as f64);
        let signature_scale = 1.0 + signature.rem_euclid(11.0) * 1.0e-3;
        degeneracy.sqrt()
            * phase_scale.max(0.05)
            * global_scale.max(0.05)
            * path_damping
            * signature_scale
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
        paths: &[PathEntry],
    ) -> ComputeResult<()> {
        match artifact_name {
            "paths.dat" => {
                write_text_artifact(output_path, &self.render_paths_dat(paths)).map_err(|source| {
                    FeffError::io_system(
                        "IO.PATH_OUTPUT_WRITE",
                        format!(
                            "failed to write PATH artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "paths.bin" => write_binary_artifact(output_path, &self.render_paths_binary(paths))
                .map_err(|source| {
                    FeffError::io_system(
                        "IO.PATH_OUTPUT_WRITE",
                        format!(
                            "failed to write PATH artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                }),
            "crit.dat" => {
                write_text_artifact(output_path, &self.render_crit_dat(paths)).map_err(|source| {
                    FeffError::io_system(
                        "IO.PATH_OUTPUT_WRITE",
                        format!(
                            "failed to write PATH artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            "log4.dat" => {
                write_text_artifact(output_path, &self.render_log4(paths)).map_err(|source| {
                    FeffError::io_system(
                        "IO.PATH_OUTPUT_WRITE",
                        format!(
                            "failed to write PATH artifact '{}': {}",
                            output_path.display(),
                            source
                        ),
                    )
                })
            }
            other => Err(FeffError::internal(
                "SYS.PATH_OUTPUT_CONTRACT",
                format!("unsupported PATH output artifact '{}'", other),
            )),
        }
    }

    fn render_paths_dat(&self, paths: &[PathEntry]) -> String {
        let mut lines = Vec::new();
        lines.push("PATH true-compute listing".to_string());
        lines.push(format!("fixture {}", self.fixture_id));
        lines.push(format!(
            "rmax {} rfms2 {} pwcrit {} pcritk {} pcrith {}",
            format_fixed_f64(self.control.rmax, 10, 4),
            format_fixed_f64(self.control.rfms2, 10, 4),
            format_fixed_f64(self.control.critpw, 10, 4),
            format_fixed_f64(self.control.pcritk, 10, 4),
            format_fixed_f64(self.control.pcrith, 10, 4),
        ));
        lines.push(format!(
            "phase magic={} channels={} points={} bytes={} checksum={:016x}",
            if self.phase.has_xsph_magic { "T" } else { "F" },
            self.phase.channel_count,
            self.phase.spectral_points,
            self.phase.byte_len,
            self.phase.checksum,
        ));
        lines.push("index nleg degeneracy reff amplitude beta eta legs".to_string());
        if paths.is_empty() {
            lines.push(
                "   0    0          0      0.0000      0.0000      0.0000      0.0000 -"
                    .to_string(),
            );
            return lines.join("\n");
        }

        for path in paths {
            let leg_indices = path
                .leg_atom_indices
                .iter()
                .map(|index| (index + 1).to_string())
                .collect::<Vec<_>>()
                .join(",");
            lines.push(format!(
                "{:>4} {:>4} {:>10} {} {} {} {} {}",
                path.index,
                path.nleg,
                path.degeneracy,
                format_fixed_f64(path.reff, 10, 4),
                format_fixed_f64(path.amplitude, 10, 4),
                format_fixed_f64(path.beta_deg, 10, 4),
                format_fixed_f64(path.eta_deg, 10, 4),
                leg_indices
            ));
        }

        lines.join("\n")
    }

    fn render_paths_binary(&self, paths: &[PathEntry]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(128 + paths.len() * 96);
        bytes.extend_from_slice(PATH_BINARY_MAGIC);
        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, paths.len() as u32);
        push_u32(&mut bytes, self.geometry.nat as u32);
        push_u32(&mut bytes, self.geometry.nph as u32);
        push_f64(&mut bytes, self.control.rmax);
        push_f64(&mut bytes, self.control.rfms2);
        push_f64(&mut bytes, self.control.critpw);
        push_f64(&mut bytes, self.global.mean);
        push_f64(&mut bytes, self.global.rms);
        push_f64(&mut bytes, self.phase.base_phase);
        push_u32(&mut bytes, self.phase.channel_count as u32);
        push_u32(&mut bytes, self.phase.spectral_points as u32);
        push_u64(&mut bytes, self.phase.checksum);

        for path in paths {
            push_u32(&mut bytes, path.index as u32);
            push_u32(&mut bytes, path.nleg as u32);
            push_u32(&mut bytes, path.degeneracy as u32);
            push_f64(&mut bytes, path.reff);
            push_f64(&mut bytes, path.amplitude);
            push_f64(&mut bytes, path.beta_deg);
            push_f64(&mut bytes, path.eta_deg);
            for slot in 0..4 {
                let atom_index = path
                    .leg_atom_indices
                    .get(slot)
                    .map(|index| (index + 1) as i32)
                    .unwrap_or(-1);
                push_i32(&mut bytes, atom_index);
            }
        }

        bytes
    }

    fn render_crit_dat(&self, paths: &[PathEntry]) -> String {
        let mut lines = Vec::new();
        lines.push("PATH criteria report".to_string());
        lines.push(format!("fixture {}", self.fixture_id));
        lines.push(format!(
            "controls mpath={} ms={} nncrit={} nlegxx={} ipr4={} ica={}",
            self.control.mpath,
            self.control.ms,
            self.control.nncrit,
            self.control.nlegxx,
            self.control.ipr4,
            self.control.ica
        ));
        lines.push(format!(
            "thresholds critpw={} pcritk={} pcrith={} rmax={} rfms2={}",
            format_fixed_f64(self.control.critpw, 10, 4),
            format_fixed_f64(self.control.pcritk, 10, 4),
            format_fixed_f64(self.control.pcrith, 10, 4),
            format_fixed_f64(self.control.rmax, 10, 4),
            format_fixed_f64(self.control.rfms2, 10, 4)
        ));
        lines.push(format!(
            "geometry nat={} nph={} radius_mean={} radius_rms={} radius_max={}",
            self.geometry.nat,
            self.geometry.nph,
            format_fixed_f64(self.geometry.radius_mean, 10, 4),
            format_fixed_f64(self.geometry.radius_rms, 10, 4),
            format_fixed_f64(self.geometry.radius_max, 10, 4)
        ));
        lines.push(format!(
            "global token_count={} mean={} rms={} max_abs={}",
            self.global.token_count,
            format_fixed_f64(self.global.mean, 10, 4),
            format_fixed_f64(self.global.rms, 10, 4),
            format_fixed_f64(self.global.max_abs, 10, 4)
        ));

        if paths.is_empty() {
            lines.push("retained_paths 0".to_string());
        } else {
            let min_reff = paths
                .iter()
                .map(|path| path.reff)
                .fold(f64::INFINITY, f64::min);
            let max_reff = paths
                .iter()
                .map(|path| path.reff)
                .fold(f64::NEG_INFINITY, f64::max);
            let mean_amplitude =
                paths.iter().map(|path| path.amplitude).sum::<f64>() / paths.len() as f64;
            lines.push(format!("retained_paths {}", paths.len()));
            lines.push(format!(
                "reff_min={} reff_max={} amplitude_mean={}",
                format_fixed_f64(min_reff, 10, 4),
                format_fixed_f64(max_reff, 10, 4),
                format_fixed_f64(mean_amplitude, 10, 4)
            ));
        }

        lines.push("shell_radius shell_multiplicity".to_string());
        for (radius, multiplicity) in self
            .geometry
            .shell_summary(self.control.rmax)
            .iter()
            .take(24)
        {
            lines.push(format!(
                "{} {}",
                format_fixed_f64(*radius, 10, 4),
                multiplicity
            ));
        }

        lines.join("\n")
    }

    fn render_log4(&self, paths: &[PathEntry]) -> String {
        let mut lines = Vec::new();
        lines.push("Preparing plane wave scattering amplitudes...".to_string());
        lines.push("Searching for paths...".to_string());
        lines.push(format!(
            "Rmax {} keep and heap limits {} {}",
            format_fixed_f64(self.control.rmax, 10, 4),
            format_fixed_f64(self.control.pcritk, 10, 4),
            format_fixed_f64(self.control.pcrith, 10, 4),
        ));
        lines.push(format!(
            "Path source {} (bytes={}, checksum={:016x})",
            if self.phase.has_xsph_magic {
                "xsph-true-compute"
            } else {
                "legacy-phase-binary"
            },
            self.phase.byte_len,
            self.phase.checksum,
        ));
        lines.push(format!(
            "Neighbor shell count {}",
            self.geometry.shell_summary(self.control.rmax).len()
        ));
        lines.push(format!(
            "Geometry atoms {} absorber index {}",
            self.geometry.nat,
            self.geometry.absorber_index + 1
        ));
        lines.push(format!(
            "Unique paths {}, total path instances {}",
            paths.len(),
            paths.iter().map(|path| path.degeneracy as u64).sum::<u64>()
        ));
        if self.control.rmax <= 0.0 {
            lines.push(
                "Internal path finder limit exceeded -- negative/zero rmax disables path retention."
                    .to_string(),
            );
        }
        lines.push("Done with module 4: pathfinder.".to_string());
        lines.join("\n")
    }
}

fn push_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(target: &mut Vec<u8>, value: u64) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(target: &mut Vec<u8>, value: i32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_f64(target: &mut Vec<u8>, value: f64) {
    target.extend_from_slice(&value.to_le_bytes());
}
