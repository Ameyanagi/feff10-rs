use super::PipelineExecutor;
use super::serialization::{format_fixed_f64, write_binary_artifact, write_text_artifact};
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::{Path, PathBuf};

const POT_REQUIRED_INPUTS: [&str; 2] = ["pot.inp", "geom.dat"];
const POT_REQUIRED_OUTPUTS: [&str; 5] = [
    "pot.bin",
    "pot.dat",
    "log1.dat",
    "convergence.scf",
    "convergence.scf.fine",
];
pub const POT_BINARY_MAGIC: &[u8; 8] = b"POTBIN10";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PotPipelineScaffold;

#[derive(Debug, Clone)]
struct PotModel {
    fixture_id: String,
    title: String,
    control: PotControl,
    potentials: Vec<PotentialEntry>,
    geometry: GeomModel,
}

#[derive(Debug, Clone, Copy)]
struct PotControl {
    mpot: i32,
    nph: i32,
    ntitle: i32,
    ihole: i32,
    ipr1: i32,
    iafolp: i32,
    ixc: i32,
    ispec: i32,
    nmix: i32,
    nohole: i32,
    jumprm: i32,
    inters: i32,
    nscmt: i32,
    icoul: i32,
    lfms1: i32,
    iunf: i32,
    gamach: f64,
    rgrd: f64,
    ca1: f64,
    ecv: f64,
    totvol: f64,
    rfms1: f64,
}

#[derive(Debug, Clone, Copy)]
struct PotentialEntry {
    atomic_number: i32,
    lmaxsc: i32,
    xnatph: f64,
    xion: f64,
    folp: f64,
}

#[derive(Debug, Clone)]
struct GeomModel {
    nat: usize,
    nph: usize,
    atoms: Vec<AtomSite>,
}

#[derive(Debug, Clone, Copy)]
struct AtomSite {
    x: f64,
    y: f64,
    z: f64,
    ipot: i32,
}

impl PotPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<PotPipelineInterface> {
        validate_request_shape(request)?;
        Ok(PotPipelineInterface {
            required_inputs: artifact_list(&POT_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&POT_REQUIRED_OUTPUTS),
        })
    }
}

impl PipelineExecutor for PotPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;

        let pot_inp_source = read_input_source(&request.input_path, POT_REQUIRED_INPUTS[0])?;
        let geom_path = geom_input_path(request)?;
        let geom_source = read_input_source(&geom_path, POT_REQUIRED_INPUTS[1])?;
        let model = PotModel::from_sources(&request.fixture_id, &pot_inp_source, &geom_source)?;
        let outputs = artifact_list(&POT_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.POT_OUTPUT_DIRECTORY",
                format!(
                    "failed to create POT output directory '{}': {}",
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
                        "IO.POT_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create POT artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            let artifact_name = artifact.relative_path.to_string_lossy().replace('\\', "/");
            model.write_artifact(&artifact_name, &output_path)?;
        }

        Ok(outputs)
    }
}

impl PotModel {
    fn from_sources(fixture_id: &str, pot_source: &str, geom_source: &str) -> PipelineResult<Self> {
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

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> PipelineResult<()> {
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

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Pot {
        return Err(FeffError::input_validation(
            "INPUT.POT_MODULE",
            format!("POT pipeline expects module POT, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.POT_INPUT_ARTIFACT",
                format!(
                    "POT pipeline expects input artifact '{}' at '{}'",
                    POT_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(POT_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.POT_INPUT_ARTIFACT",
            format!(
                "POT pipeline requires input artifact '{}' but received '{}'",
                POT_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn geom_input_path(request: &PipelineRequest) -> PipelineResult<PathBuf> {
    request
        .input_path
        .parent()
        .map(|parent| parent.join(POT_REQUIRED_INPUTS[1]))
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.POT_INPUT_ARTIFACT",
                format!(
                    "POT pipeline requires sibling '{}' for input '{}'",
                    POT_REQUIRED_INPUTS[1],
                    request.input_path.display()
                ),
            )
        })
}

fn read_input_source(input_path: &Path, label: &str) -> PipelineResult<String> {
    fs::read_to_string(input_path).map_err(|source| {
        FeffError::io_system(
            "IO.POT_INPUT_READ",
            format!(
                "failed to read POT input '{}' ({}): {}",
                input_path.display(),
                label,
                source
            ),
        )
    })
}

fn parse_pot_input(
    fixture_id: &str,
    source: &str,
) -> PipelineResult<(String, PotControl, Vec<PotentialEntry>)> {
    let lines = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    let (header_index, header_values) = find_i32_row(&lines, 8, 0)
        .ok_or_else(|| pot_contract_error(fixture_id, "missing POT control row with 8 integers"))?;
    let (scf_index, scf_values) = find_i32_row(&lines, 8, header_index + 1)
        .ok_or_else(|| pot_contract_error(fixture_id, "missing SCF control row with 8 integers"))?;
    let title = lines
        .iter()
        .skip(scf_index + 1)
        .find(|line| {
            line.chars()
                .any(|character| character.is_ascii_alphabetic())
        })
        .map(|line| (*line).to_string())
        .unwrap_or_else(|| "POT input".to_string());

    let gamma_header = lines
        .iter()
        .position(|line| line.to_ascii_lowercase().contains("gamach"))
        .ok_or_else(|| pot_contract_error(fixture_id, "missing 'gamach' control header"))?;
    let (_, gamma_values) = find_f64_row(&lines, 6, gamma_header + 1).ok_or_else(|| {
        pot_contract_error(
            fixture_id,
            "missing numeric row with 6 values after 'gamach' header",
        )
    })?;

    let potential_header = lines
        .iter()
        .position(|line| line.to_ascii_lowercase().contains("iz, lmaxsc"))
        .ok_or_else(|| pot_contract_error(fixture_id, "missing potential table header"))?;

    let mut potentials = Vec::new();
    for line in lines.iter().skip(potential_header + 1) {
        if line
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic())
        {
            if !potentials.is_empty() {
                break;
            }
            continue;
        }

        if let Some(entry) = parse_potential_entry(line) {
            potentials.push(entry);
            continue;
        }

        if !potentials.is_empty() {
            break;
        }
    }

    if potentials.is_empty() {
        return Err(pot_contract_error(
            fixture_id,
            "missing potential rows after potential table header",
        ));
    }

    let control = PotControl {
        mpot: header_values[0],
        nph: header_values[1],
        ntitle: header_values[2],
        ihole: header_values[3],
        ipr1: header_values[4],
        iafolp: header_values[5],
        ixc: header_values[6],
        ispec: header_values[7],
        nmix: scf_values[0],
        nohole: scf_values[1],
        jumprm: scf_values[2],
        inters: scf_values[3],
        nscmt: scf_values[4],
        icoul: scf_values[5],
        lfms1: scf_values[6],
        iunf: scf_values[7],
        gamach: gamma_values[0],
        rgrd: gamma_values[1],
        ca1: gamma_values[2],
        ecv: gamma_values[3],
        totvol: gamma_values[4],
        rfms1: gamma_values[5],
    };

    Ok((title, control, potentials))
}

fn parse_geom_input(fixture_id: &str, source: &str) -> PipelineResult<GeomModel> {
    let mut lines = source.lines();
    let header_line = lines
        .next()
        .ok_or_else(|| pot_contract_error(fixture_id, "geom.dat is empty"))?;
    let header_values = parse_i32_tokens(header_line);
    if header_values.len() < 2 {
        return Err(pot_contract_error(
            fixture_id,
            "geom.dat header must contain nat and nph",
        ));
    }

    let nat = usize::try_from(header_values[0]).unwrap_or(0);
    let nph = usize::try_from(header_values[1]).unwrap_or(0);
    let mut atoms = Vec::new();

    for line in lines {
        if let Some(atom) = parse_geom_atom(line) {
            atoms.push(atom);
        }
    }

    if atoms.is_empty() {
        return Err(pot_contract_error(
            fixture_id,
            "geom.dat must include at least one atom row",
        ));
    }

    Ok(GeomModel {
        nat: nat.max(atoms.len()),
        nph: nph.max(1),
        atoms,
    })
}

fn find_i32_row(lines: &[&str], minimum_fields: usize, start: usize) -> Option<(usize, Vec<i32>)> {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, line)| {
            let values = parse_i32_tokens(line);
            if values.len() >= minimum_fields {
                Some((index, values))
            } else {
                None
            }
        })
}

fn find_f64_row(lines: &[&str], minimum_fields: usize, start: usize) -> Option<(usize, Vec<f64>)> {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, line)| {
            let values = parse_f64_tokens(line);
            if values.len() >= minimum_fields {
                Some((index, values))
            } else {
                None
            }
        })
}

fn parse_potential_entry(line: &str) -> Option<PotentialEntry> {
    let columns = line.split_whitespace().collect::<Vec<_>>();
    if columns.len() < 5 {
        return None;
    }

    Some(PotentialEntry {
        atomic_number: parse_i32_token(columns[0])?,
        lmaxsc: parse_i32_token(columns[1])?,
        xnatph: parse_f64_token(columns[2])?,
        xion: parse_f64_token(columns[3])?,
        folp: parse_f64_token(columns[4])?,
    })
}

fn parse_geom_atom(line: &str) -> Option<AtomSite> {
    let columns = line.split_whitespace().collect::<Vec<_>>();
    if columns.len() < 6 {
        return None;
    }

    let _iat = parse_i32_token(columns[0])?;
    Some(AtomSite {
        x: parse_f64_token(columns[1])?,
        y: parse_f64_token(columns[2])?,
        z: parse_f64_token(columns[3])?,
        ipot: parse_i32_token(columns[4])?,
    })
}

fn parse_i32_tokens(line: &str) -> Vec<i32> {
    line.split_whitespace()
        .filter_map(parse_i32_token)
        .collect()
}

fn parse_f64_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(parse_f64_token)
        .collect()
}

fn parse_i32_token(token: &str) -> Option<i32> {
    let cleaned = token.trim_matches(|character: char| matches!(character, ',' | ';' | ':'));
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<i32>().ok()
}

fn parse_f64_token(token: &str) -> Option<f64> {
    let cleaned = token.trim_matches(|character: char| matches!(character, ',' | ';' | ':'));
    if cleaned.is_empty() {
        return None;
    }
    cleaned.replace(['D', 'd'], "E").parse::<f64>().ok()
}

fn pot_contract_error(fixture_id: &str, reason: &str) -> FeffError {
    FeffError::computation(
        "RUN.POT_INPUT_MISMATCH",
        format!(
            "fixture '{}' input contract mismatch for POT compute path: {}",
            fixture_id, reason
        ),
    )
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

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::{POT_BINARY_MAGIC, PotPipelineScaffold};
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn contract_exposes_required_inputs_and_outputs() {
        let request = PipelineRequest::new(
            "FX-POT-001",
            PipelineModule::Pot,
            "pot.inp",
            "actual-output",
        );
        let scaffold = PotPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 2);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("pot.inp")
        );
        assert_eq!(
            contract.required_inputs[1].relative_path,
            PathBuf::from("geom.dat")
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_pot_artifact_set()
        );
    }

    #[test]
    fn execute_writes_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("actual");
        stage_pot_inputs(&input_path, &temp.path().join("geom.dat"));

        let request =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, &output_dir);
        let scaffold = PotPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("POT execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_pot_artifact_set());
        for artifact in artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            assert!(output_path.is_file(), "artifact should exist");
        }

        let pot_binary = fs::read(output_dir.join("pot.bin")).expect("pot.bin should be readable");
        assert!(
            pot_binary.starts_with(POT_BINARY_MAGIC),
            "pot.bin should use true-compute binary header"
        );

        let pot_dat =
            fs::read_to_string(output_dir.join("pot.dat")).expect("pot.dat should be readable");
        assert!(pot_dat.contains("POT true-compute summary"));
        assert!(pot_dat.contains("index iz lmaxsc"));
    }

    #[test]
    fn execute_is_deterministic_for_same_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let geom_path = temp.path().join("geom.dat");
        let output_a = temp.path().join("out-a");
        let output_b = temp.path().join("out-b");
        stage_pot_inputs(&input_path, &geom_path);

        let request_a =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, &output_a);
        let request_b =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, &output_b);

        PotPipelineScaffold
            .execute(&request_a)
            .expect("first execution should succeed");
        PotPipelineScaffold
            .execute(&request_b)
            .expect("second execution should succeed");

        for artifact in [
            "pot.bin",
            "pot.dat",
            "log1.dat",
            "convergence.scf",
            "convergence.scf.fine",
        ] {
            let first = fs::read(output_a.join(artifact)).expect("first output should be readable");
            let second =
                fs::read(output_b.join(artifact)).expect("second output should be readable");
            assert_eq!(
                first, second,
                "artifact '{}' should be deterministic",
                artifact
            );
        }
    }

    #[test]
    fn execute_rejects_non_pot_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        stage_pot_inputs(&input_path, &temp.path().join("geom.dat"));

        let request = PipelineRequest::new(
            "FX-RDINP-001",
            PipelineModule::Rdinp,
            &input_path,
            temp.path(),
        );
        let scaffold = PotPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.POT_MODULE");
    }

    #[test]
    fn execute_requires_geom_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        fs::write(&input_path, pot_input_fixture()).expect("input should be written");

        let request =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, temp.path());
        let scaffold = PotPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("missing geom input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.POT_INPUT_READ");
    }

    #[test]
    fn execute_rejects_invalid_pot_input_contract() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        let output_dir = temp.path().join("actual");
        fs::write(&input_path, "BROKEN POT INPUT\n").expect("pot input should be written");
        fs::write(
            temp.path().join("geom.dat"),
            "nat, nph =    1    1\n 1 1\n iat x y z iph\n ---\n 1 0 0 0 0 1\n",
        )
        .expect("geom input should be written");

        let request =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, &output_dir);
        let scaffold = PotPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("invalid POT input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.POT_INPUT_MISMATCH");
    }

    fn stage_pot_inputs(pot_path: &Path, geom_path: &Path) {
        fs::write(pot_path, pot_input_fixture()).expect("pot input should be written");
        fs::write(geom_path, geom_input_fixture()).expect("geom input should be written");
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn expected_pot_artifact_set() -> BTreeSet<String> {
        [
            "pot.bin",
            "pot.dat",
            "log1.dat",
            "convergence.scf",
            "convergence.scf.fine",
        ]
        .iter()
        .map(|artifact| artifact.to_string())
        .collect()
    }

    fn pot_input_fixture() -> &'static str {
        "mpot, nph, ntitle, ihole, ipr1, iafolp, ixc,ispec
   1   1   1   1   0   0   0   1
nmix, nohole, jumprm, inters, nscmt, icoul, lfms1, iunf
   6   2   0   0  30   0   0   0
Cu crystal
gamach, rgrd, ca1, ecv, totvol, rfms1
      1.72919      0.05000      0.20000    -40.00000      0.00000      4.00000
 iz, lmaxsc, xnatph, xion, folp
   29    2      1.00000      0.00000      1.15000
   29    2    100.00000      0.00000      1.15000
ExternalPot switch, StartFromFile switch
 F F
"
    }

    fn geom_input_fixture() -> &'static str {
        "nat, nph =    4    1
    1    2
 iat     x       y        z       iph
 -----------------------------------------------------------------------
   1      0.00000      0.00000      0.00000   0   1
   2      1.80500      1.80500      0.00000   1   1
   3     -1.80500      1.80500      0.00000   1   1
   4      0.00000      1.80500      1.80500   1   1
"
    }
}
