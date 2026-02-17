use super::PATH_REQUIRED_INPUTS;
use crate::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use crate::modules::xsph::XSPH_PHASE_BINARY_MAGIC;
use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub(super) struct PathControlInput {
    pub(super) mpath: i32,
    pub(super) ms: i32,
    pub(super) nncrit: i32,
    pub(super) nlegxx: i32,
    pub(super) ipr4: i32,
    pub(super) critpw: f64,
    pub(super) pcritk: f64,
    pub(super) pcrith: f64,
    pub(super) rmax: f64,
    pub(super) rfms2: f64,
    pub(super) ica: i32,
}

#[derive(Debug, Clone)]
pub(super) struct GeomPathInput {
    pub(super) nat: usize,
    pub(super) nph: usize,
    pub(super) atoms: Vec<AtomSite>,
    pub(super) absorber_index: usize,
    pub(super) radius_mean: f64,
    pub(super) radius_rms: f64,
    pub(super) radius_max: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct AtomSite {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) z: f64,
    pub(super) ipot: i32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GlobalPathInput {
    pub(super) token_count: usize,
    pub(super) mean: f64,
    pub(super) rms: f64,
    pub(super) max_abs: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PhasePathInput {
    pub(super) has_xsph_magic: bool,
    pub(super) channel_count: usize,
    pub(super) spectral_points: usize,
    pub(super) energy_step: f64,
    pub(super) base_phase: f64,
    pub(super) byte_len: usize,
    pub(super) checksum: u64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NeighborSite {
    pub(super) atom_index: usize,
    pub(super) radius: f64,
    pub(super) shell_size: usize,
}

impl GeomPathInput {
    pub(super) fn absorber_position(&self) -> [f64; 3] {
        self.atoms[self.absorber_index].position()
    }

    pub(super) fn neighbor_sites(&self, rmax: f64) -> Vec<NeighborSite> {
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

    pub(super) fn shell_summary(&self, rmax: f64) -> Vec<(f64, usize)> {
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
    pub(super) fn position(&self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

pub(super) fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Path {
        return Err(FeffError::input_validation(
            "INPUT.PATH_MODULE",
            format!("PATH module expects PATH, got {}", request.module),
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
                    "PATH module expects input artifact '{}' at '{}'",
                    PATH_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(PATH_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.PATH_INPUT_ARTIFACT",
            format!(
                "PATH module requires input artifact '{}' but received '{}'",
                PATH_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

pub(super) fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.PATH_INPUT_ARTIFACT",
            format!(
                "PATH module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

pub(super) fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
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

pub(super) fn read_input_bytes(path: &Path, artifact_name: &str) -> ComputeResult<Vec<u8>> {
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

pub(super) fn parse_paths_input(fixture_id: &str, source: &str) -> ComputeResult<PathControlInput> {
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

pub(super) fn parse_geom_input(fixture_id: &str, source: &str) -> ComputeResult<GeomPathInput> {
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

pub(super) fn parse_global_input(fixture_id: &str, source: &str) -> ComputeResult<GlobalPathInput> {
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

pub(super) fn parse_phase_input(fixture_id: &str, bytes: &[u8]) -> ComputeResult<PhasePathInput> {
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

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
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

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
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

pub(super) fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

pub(super) fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

pub(super) fn norm(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}

pub(super) fn distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    norm(subtract(left, right))
}

pub(super) fn angle_between(left: [f64; 3], right: [f64; 3]) -> f64 {
    let denom = norm(left) * norm(right);
    if denom <= 1.0e-12 {
        return 0.0;
    }

    let cosine = (dot(left, right) / denom).clamp(-1.0, 1.0);
    cosine.acos().to_degrees()
}

pub(super) fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}
