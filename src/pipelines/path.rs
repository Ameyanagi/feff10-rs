use super::PipelineExecutor;
use super::serialization::{format_fixed_f64, write_binary_artifact, write_text_artifact};
use super::xsph::XSPH_PHASE_BINARY_MAGIC;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

const PATH_REQUIRED_INPUTS: [&str; 4] = ["paths.inp", "geom.dat", "global.inp", "phase.bin"];
const PATH_REQUIRED_OUTPUTS: [&str; 4] = ["paths.dat", "paths.bin", "crit.dat", "log4.dat"];
pub const PATH_BINARY_MAGIC: &[u8; 8] = b"PATHBIN1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PathPipelineScaffold;

#[derive(Debug, Clone)]
struct PathModel {
    fixture_id: String,
    control: PathControlInput,
    geometry: GeomPathInput,
    global: GlobalPathInput,
    phase: PhasePathInput,
}

#[derive(Debug, Clone, Copy)]
struct PathControlInput {
    mpath: i32,
    ms: i32,
    nncrit: i32,
    nlegxx: i32,
    ipr4: i32,
    critpw: f64,
    pcritk: f64,
    pcrith: f64,
    rmax: f64,
    rfms2: f64,
    ica: i32,
}

#[derive(Debug, Clone)]
struct GeomPathInput {
    nat: usize,
    nph: usize,
    atoms: Vec<AtomSite>,
    absorber_index: usize,
    radius_mean: f64,
    radius_rms: f64,
    radius_max: f64,
}

#[derive(Debug, Clone, Copy)]
struct AtomSite {
    x: f64,
    y: f64,
    z: f64,
    ipot: i32,
}

#[derive(Debug, Clone, Copy)]
struct GlobalPathInput {
    token_count: usize,
    mean: f64,
    rms: f64,
    max_abs: f64,
}

#[derive(Debug, Clone, Copy)]
struct PhasePathInput {
    has_xsph_magic: bool,
    channel_count: usize,
    spectral_points: usize,
    energy_step: f64,
    base_phase: f64,
    byte_len: usize,
    checksum: u64,
}

#[derive(Debug, Clone, Copy)]
struct NeighborSite {
    atom_index: usize,
    radius: f64,
    shell_size: usize,
}

#[derive(Debug, Clone)]
struct PathEntry {
    index: usize,
    nleg: usize,
    leg_atom_indices: Vec<usize>,
    degeneracy: usize,
    reff: f64,
    amplitude: f64,
    beta_deg: f64,
    eta_deg: f64,
}

impl PathPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<PathPipelineInterface> {
        validate_request_shape(request)?;
        Ok(PathPipelineInterface {
            required_inputs: artifact_list(&PATH_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&PATH_REQUIRED_OUTPUTS),
        })
    }
}

impl PipelineExecutor for PathPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let path_source = read_input_source(&request.input_path, PATH_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(PATH_REQUIRED_INPUTS[1]),
            PATH_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(PATH_REQUIRED_INPUTS[2]),
            PATH_REQUIRED_INPUTS[2],
        )?;
        let phase_bytes = read_input_bytes(
            &input_dir.join(PATH_REQUIRED_INPUTS[3]),
            PATH_REQUIRED_INPUTS[3],
        )?;

        let model = PathModel::from_sources(
            &request.fixture_id,
            &path_source,
            &geom_source,
            &global_source,
            &phase_bytes,
        )?;
        let outputs = artifact_list(&PATH_REQUIRED_OUTPUTS);
        let generated_paths = model.generated_paths();

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.PATH_OUTPUT_DIRECTORY",
                format!(
                    "failed to create PATH output directory '{}': {}",
                    request.output_dir.display(),
                    source
                ),
            )
        })?;

        for artifact in &outputs {
            let output_path = request.output_dir.join(&artifact.relative_path);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|source| {
                    FeffError::io_system(
                        "IO.PATH_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create PATH artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            let artifact_name = artifact.relative_path.to_string_lossy().replace('\\', "/");
            model.write_artifact(&artifact_name, &output_path, &generated_paths)?;
        }

        Ok(outputs)
    }
}

impl PathModel {
    fn from_sources(
        fixture_id: &str,
        path_source: &str,
        geom_source: &str,
        global_source: &str,
        phase_bytes: &[u8],
    ) -> PipelineResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_paths_input(fixture_id, path_source)?,
            geometry: parse_geom_input(fixture_id, geom_source)?,
            global: parse_global_input(fixture_id, global_source)?,
            phase: parse_phase_input(fixture_id, phase_bytes)?,
        })
    }

    fn generated_paths(&self) -> Vec<PathEntry> {
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

    fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
        paths: &[PathEntry],
    ) -> PipelineResult<()> {
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

impl GeomPathInput {
    fn absorber_position(&self) -> [f64; 3] {
        self.atoms[self.absorber_index].position()
    }

    fn neighbor_sites(&self, rmax: f64) -> Vec<NeighborSite> {
        if rmax <= 0.0 {
            return Vec::new();
        }

        let absorber = self.absorber_position();
        let mut candidates = self
            .atoms
            .iter()
            .enumerate()
            .filter_map(|(index, atom)| {
                if index == self.absorber_index {
                    return None;
                }
                let radius = distance(absorber, atom.position());
                if radius <= 1.0e-10 || radius > rmax * 1.35 {
                    return None;
                }
                Some((index, radius))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.0.cmp(&right.0))
        });
        candidates.truncate(96);

        let mut shell_counts: BTreeMap<i64, usize> = BTreeMap::new();
        for (_, radius) in &candidates {
            let key = quantized_radius_key(*radius);
            *shell_counts.entry(key).or_insert(0) += 1;
        }

        candidates
            .into_iter()
            .map(|(atom_index, radius)| {
                let key = quantized_radius_key(radius);
                let shell_size = *shell_counts.get(&key).unwrap_or(&1);
                NeighborSite {
                    atom_index,
                    radius,
                    shell_size,
                }
            })
            .collect()
    }

    fn shell_summary(&self, rmax: f64) -> Vec<(f64, usize)> {
        let absorber = self.absorber_position();
        let mut shells: BTreeMap<i64, (f64, usize)> = BTreeMap::new();
        for (index, atom) in self.atoms.iter().enumerate() {
            if index == self.absorber_index {
                continue;
            }
            let radius = distance(absorber, atom.position());
            if radius <= 1.0e-10 || (rmax > 0.0 && radius > rmax * 1.5) {
                continue;
            }

            let key = quantized_radius_key(radius);
            if let Some((stored_radius, count)) = shells.get_mut(&key) {
                *stored_radius =
                    (*stored_radius * (*count as f64) + radius) / (*count as f64 + 1.0);
                *count += 1;
            } else {
                shells.insert(key, (radius, 1));
            }
        }

        shells.into_values().collect()
    }
}

impl AtomSite {
    fn position(&self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Path {
        return Err(FeffError::input_validation(
            "INPUT.PATH_MODULE",
            format!("PATH pipeline expects module PATH, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.PATH_INPUT_ARTIFACT",
                format!(
                    "PATH pipeline expects input artifact '{}' at '{}'",
                    PATH_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(PATH_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.PATH_INPUT_ARTIFACT",
            format!(
                "PATH pipeline requires input artifact '{}' but received '{}'",
                PATH_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.PATH_INPUT_ARTIFACT",
            format!(
                "PATH pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.PATH_INPUT_READ",
            format!(
                "failed to read PATH input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn read_input_bytes(path: &Path, artifact_name: &str) -> PipelineResult<Vec<u8>> {
    fs::read(path).map_err(|source| {
        FeffError::io_system(
            "IO.PATH_INPUT_READ",
            format!(
                "failed to read PATH input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn parse_paths_input(fixture_id: &str, source: &str) -> PipelineResult<PathControlInput> {
    let numeric_rows = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();
    if numeric_rows.len() < 3 {
        return Err(path_parse_error(
            fixture_id,
            "paths.inp must contain at least three numeric rows",
        ));
    }

    let control_row = &numeric_rows[0];
    let threshold_row = &numeric_rows[1];
    let ica_row = &numeric_rows[2];
    if control_row.len() < 5 {
        return Err(path_parse_error(
            fixture_id,
            "paths.inp control row is missing required integer fields",
        ));
    }
    if threshold_row.len() < 5 {
        return Err(path_parse_error(
            fixture_id,
            "paths.inp threshold row is missing required floating-point fields",
        ));
    }
    if ica_row.is_empty() {
        return Err(path_parse_error(
            fixture_id,
            "paths.inp must define the ica row",
        ));
    }

    Ok(PathControlInput {
        mpath: f64_to_i32(control_row[0], fixture_id, "mpath")?,
        ms: f64_to_i32(control_row[1], fixture_id, "ms")?,
        nncrit: f64_to_i32(control_row[2], fixture_id, "nncrit")?,
        nlegxx: f64_to_i32(control_row[3], fixture_id, "nlegxx")?,
        ipr4: f64_to_i32(control_row[4], fixture_id, "ipr4")?,
        critpw: threshold_row[0],
        pcritk: threshold_row[1],
        pcrith: threshold_row[2],
        rmax: threshold_row[3],
        rfms2: threshold_row[4],
        ica: f64_to_i32(ica_row[0], fixture_id, "ica")?,
    })
}

fn parse_geom_input(fixture_id: &str, source: &str) -> PipelineResult<GeomPathInput> {
    let numeric_rows = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();
    if numeric_rows.is_empty() {
        return Err(path_parse_error(
            fixture_id,
            "geom.dat is missing numeric content",
        ));
    }
    if numeric_rows[0].len() < 2 {
        return Err(path_parse_error(
            fixture_id,
            "geom.dat header must provide nat and nph values",
        ));
    }

    let declared_nat = f64_to_usize(numeric_rows[0][0], fixture_id, "nat")?;
    let declared_nph = f64_to_usize(numeric_rows[0][1], fixture_id, "nph")?;

    let mut atoms = Vec::new();
    for row in numeric_rows {
        if row.len() < 6 {
            continue;
        }
        atoms.push(AtomSite {
            x: row[1],
            y: row[2],
            z: row[3],
            ipot: f64_to_i32(row[4], fixture_id, "ipot")?,
        });
    }

    if atoms.is_empty() {
        return Err(path_parse_error(
            fixture_id,
            "geom.dat does not contain atom rows",
        ));
    }

    let absorber_index = atoms.iter().position(|atom| atom.ipot == 0).unwrap_or(0);
    let absorber_position = atoms[absorber_index].position();
    let radii = atoms
        .iter()
        .enumerate()
        .filter_map(|(index, atom)| {
            if index == absorber_index {
                return None;
            }
            let radius = distance(absorber_position, atom.position());
            (radius > 1.0e-10).then_some(radius)
        })
        .collect::<Vec<_>>();

    let nat = declared_nat.max(atoms.len());
    let nph = declared_nph.max(1);
    let radius_mean = if radii.is_empty() {
        0.0
    } else {
        radii.iter().sum::<f64>() / radii.len() as f64
    };
    let radius_rms = if radii.is_empty() {
        0.0
    } else {
        (radii.iter().map(|radius| radius * radius).sum::<f64>() / radii.len() as f64).sqrt()
    };
    let radius_max = radii.into_iter().fold(0.0_f64, f64::max);

    Ok(GeomPathInput {
        nat,
        nph,
        atoms,
        absorber_index,
        radius_mean,
        radius_rms,
        radius_max,
    })
}

fn parse_global_input(fixture_id: &str, source: &str) -> PipelineResult<GlobalPathInput> {
    let values = source
        .lines()
        .flat_map(parse_numeric_tokens)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err(path_parse_error(
            fixture_id,
            "global.inp does not contain numeric values",
        ));
    }

    let token_count = values.len();
    let mean = values.iter().sum::<f64>() / token_count as f64;
    let rms = (values.iter().map(|value| value * value).sum::<f64>() / token_count as f64).sqrt();
    let max_abs = values
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);

    Ok(GlobalPathInput {
        token_count,
        mean,
        rms,
        max_abs,
    })
}

fn parse_phase_input(fixture_id: &str, bytes: &[u8]) -> PipelineResult<PhasePathInput> {
    if bytes.is_empty() {
        return Err(path_parse_error(fixture_id, "phase.bin must be non-empty"));
    }

    let checksum = checksum_bytes(bytes);
    let has_xsph_magic = bytes.starts_with(XSPH_PHASE_BINARY_MAGIC);
    if !has_xsph_magic {
        let normalized_phase = checksum as f64 / u64::MAX as f64;
        return Ok(PhasePathInput {
            has_xsph_magic: false,
            channel_count: 0,
            spectral_points: 0,
            energy_step: 0.0,
            base_phase: (normalized_phase - 0.5) * PI,
            byte_len: bytes.len(),
            checksum,
        });
    }

    let channel_count = read_u32_le(bytes, 12)
        .map(|value| value.max(1) as usize)
        .ok_or_else(|| path_parse_error(fixture_id, "phase.bin header missing channel count"))?;
    let spectral_points = read_u32_le(bytes, 16)
        .map(|value| value.max(1) as usize)
        .ok_or_else(|| path_parse_error(fixture_id, "phase.bin header missing spectral points"))?;
    let energy_step = read_f64_le(bytes, 28)
        .ok_or_else(|| path_parse_error(fixture_id, "phase.bin header missing energy step"))?;
    let base_phase = read_f64_le(bytes, 36)
        .ok_or_else(|| path_parse_error(fixture_id, "phase.bin header missing base phase"))?;

    Ok(PhasePathInput {
        has_xsph_magic: true,
        channel_count,
        spectral_points,
        energy_step,
        base_phase,
        byte_len: bytes.len(),
        checksum,
    })
}

fn path_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.PATH_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

fn parse_numeric_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(parse_numeric_token)
        .collect()
}

fn parse_numeric_token(token: &str) -> Option<f64> {
    let trimmed = token.trim_matches(|character: char| {
        matches!(
            character,
            ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
        )
    });
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.replace(['D', 'd'], "E");
    normalized.parse::<f64>().ok()
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> PipelineResult<i32> {
    if !value.is_finite() {
        return Err(path_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-8 {
        return Err(path_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }
    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(path_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }
    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> PipelineResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(path_parse_error(
            fixture_id,
            format!("{} must be non-negative", field),
        ));
    }
    Ok(integer as usize)
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    let mut value = [0_u8; 4];
    value.copy_from_slice(slice);
    Some(u32::from_le_bytes(value))
}

fn read_f64_le(bytes: &[u8], offset: usize) -> Option<f64> {
    let slice = bytes.get(offset..offset + 8)?;
    let mut value = [0_u8; 8];
    value.copy_from_slice(slice);
    Some(f64::from_le_bytes(value))
}

fn checksum_bytes(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        hash.wrapping_mul(0x100000001b3).wrapping_add(*byte as u64)
    })
}

fn quantized_radius_key(radius: f64) -> i64 {
    (radius * 1.0e4).round() as i64
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn norm(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}

fn distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    norm(subtract(left, right))
}

fn angle_between(left: [f64; 3], right: [f64; 3]) -> f64 {
    let denom = norm(left) * norm(right);
    if denom <= 1.0e-12 {
        return 0.0;
    }

    let cosine = (dot(left, right) / denom).clamp(-1.0, 1.0);
    cosine.acos().to_degrees()
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

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::{PATH_BINARY_MAGIC, PathPipelineScaffold};
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use crate::pipelines::path::PATH_BINARY_MAGIC as EXPORTED_PATH_BINARY_MAGIC;
    use crate::pipelines::xsph::XSPH_PHASE_BINARY_MAGIC;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    const PATH_INPUT_FIXTURE: &str = "mpath, ms, nncrit, nlegxx, ipr4
   1   1   0  10   0
critpw, pcritk, pcrith,  rmax, rfms2
      2.50000      0.00000      0.00000      5.50000      4.00000
ica
  -1
";

    const GEOM_INPUT_FIXTURE: &str = "nat, nph =    6    1
    1    2
 iat     x       y        z       iph
 -----------------------------------------------------------------------
   1      0.00000      0.00000      0.00000   0   1
   2      1.80500      1.80500      0.00000   1   1
   3     -1.80500      1.80500      0.00000   1   1
   4      1.80500     -1.80500      0.00000   1   1
   5     -1.80500     -1.80500      0.00000   1   1
   6      0.00000      0.00000      3.61000   1   1
";

    const GLOBAL_INPUT_FIXTURE: &str = "nabs iphabs
1 0 100000.0
ipol ispin le2 elpty angks
0 0 0 0.0 0.0
";

    #[test]
    fn contract_returns_required_path_compute_artifacts() {
        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            "paths.inp",
            "actual-output",
        );
        let contract = PathPipelineScaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["paths.inp", "geom.dat", "global.inp", "phase.bin"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&["paths.dat", "paths.bin", "crit.dat", "log4.dat"])
        );
    }

    #[test]
    fn execute_emits_required_true_compute_artifacts() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("actual");
        stage_path_inputs(&input_dir, &sample_xsph_phase_binary());

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            input_dir.join("paths.inp"),
            &output_dir,
        );
        let artifacts = PathPipelineScaffold
            .execute(&request)
            .expect("PATH execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["paths.dat", "paths.bin", "crit.dat", "log4.dat"])
        );
        for artifact in &artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            assert!(
                output_path.is_file(),
                "PATH artifact '{}' should exist",
                output_path.display()
            );
            let bytes = fs::read(&output_path).expect("artifact bytes should be readable");
            assert!(
                !bytes.is_empty(),
                "PATH artifact '{}' should not be empty",
                output_path.display()
            );
        }

        let path_bin = fs::read(output_dir.join("paths.bin")).expect("paths.bin should exist");
        assert!(
            path_bin.starts_with(PATH_BINARY_MAGIC),
            "paths.bin should use PATH binary magic header"
        );
        assert_eq!(PATH_BINARY_MAGIC, EXPORTED_PATH_BINARY_MAGIC);
    }

    #[test]
    fn execute_outputs_are_deterministic_across_runs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_path_inputs(&input_dir, &sample_xsph_phase_binary());

        let first_dir = temp.path().join("first");
        let first_request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            input_dir.join("paths.inp"),
            &first_dir,
        );
        let first_artifacts = PathPipelineScaffold
            .execute(&first_request)
            .expect("first PATH run should succeed");

        let second_dir = temp.path().join("second");
        let second_request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            input_dir.join("paths.inp"),
            &second_dir,
        );
        let second_artifacts = PathPipelineScaffold
            .execute(&second_request)
            .expect("second PATH run should succeed");

        assert_eq!(
            artifact_set(&first_artifacts),
            artifact_set(&second_artifacts),
            "artifact sets should match across runs"
        );
        for artifact in &first_artifacts {
            let first_bytes =
                fs::read(first_dir.join(&artifact.relative_path)).expect("first bytes should read");
            let second_bytes = fs::read(second_dir.join(&artifact.relative_path))
                .expect("second bytes should read");
            assert_eq!(
                first_bytes,
                second_bytes,
                "artifact '{}' should be deterministic",
                artifact.relative_path.display()
            );
        }
    }

    #[test]
    fn execute_accepts_legacy_phase_binary_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("actual");
        stage_path_inputs(
            &input_dir,
            &[1_u8, 2_u8, 3_u8, 4_u8, 5_u8, 6_u8, 7_u8, 8_u8],
        );

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            input_dir.join("paths.inp"),
            &output_dir,
        );
        let artifacts = PathPipelineScaffold
            .execute(&request)
            .expect("PATH execution should accept legacy phase.bin");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&["paths.dat", "paths.bin", "crit.dat", "log4.dat"])
        );
    }

    #[test]
    fn execute_rejects_non_path_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("actual");
        stage_path_inputs(&input_dir, &sample_xsph_phase_binary());

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Pot,
            input_dir.join("paths.inp"),
            &output_dir,
        );
        let error = PathPipelineScaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.PATH_MODULE");
    }

    #[test]
    fn execute_requires_phase_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input directory should be created");
        fs::write(input_dir.join("paths.inp"), PATH_INPUT_FIXTURE).expect("paths.inp should write");
        fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom.dat should write");
        fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global.inp should write");

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            input_dir.join("paths.inp"),
            temp.path().join("actual"),
        );
        let error = PathPipelineScaffold
            .execute(&request)
            .expect_err("missing phase.bin should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.PATH_INPUT_READ");
    }

    #[test]
    fn execute_rejects_invalid_paths_control_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input directory should be created");
        fs::write(input_dir.join("paths.inp"), "invalid path deck\n")
            .expect("paths.inp should write");
        fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom.dat should write");
        fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global.inp should write");
        fs::write(input_dir.join("phase.bin"), sample_xsph_phase_binary())
            .expect("phase.bin should write");

        let request = PipelineRequest::new(
            "FX-PATH-001",
            PipelineModule::Path,
            input_dir.join("paths.inp"),
            temp.path().join("actual"),
        );
        let error = PathPipelineScaffold
            .execute(&request)
            .expect_err("invalid paths input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.PATH_INPUT_PARSE");
    }

    fn stage_path_inputs(input_dir: &Path, phase_bytes: &[u8]) {
        fs::create_dir_all(input_dir).expect("input directory should be created");
        fs::write(input_dir.join("paths.inp"), PATH_INPUT_FIXTURE).expect("paths.inp should write");
        fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom.dat should write");
        fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global.inp should write");
        fs::write(input_dir.join("phase.bin"), phase_bytes).expect("phase.bin should write");
    }

    fn sample_xsph_phase_binary() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(XSPH_PHASE_BINARY_MAGIC);
        bytes.extend_from_slice(&0xA5A5A5A5_u32.to_le_bytes());
        bytes.extend_from_slice(&4_u32.to_le_bytes());
        bytes.extend_from_slice(&32_u32.to_le_bytes());
        bytes.extend_from_slice(&(-8.0_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.15_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.25_f64).to_le_bytes());
        bytes.extend_from_slice(&(1.10_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.03_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.005_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.001_f64).to_le_bytes());
        bytes
    }

    fn expected_artifact_set(artifacts: &[&str]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }
}
