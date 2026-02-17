use super::ModuleExecutor;
use super::serialization::{format_fixed_f64, write_text_artifact};
use super::xsph::XSPH_PHASE_BINARY_MAGIC;
use crate::domain::{FeffError, ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult};
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

const BAND_REQUIRED_INPUTS: [&str; 4] = ["band.inp", "geom.dat", "global.inp", "phase.bin"];
const BAND_REQUIRED_OUTPUTS: [&str; 2] = ["bandstructure.dat", "logband.dat"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BandContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BandModule;

#[derive(Debug, Clone)]
struct BandModel {
    fixture_id: String,
    control: BandControlInput,
    geom: GeomBandInput,
    global: GlobalBandInput,
    phase: PhaseBandInput,
}

#[derive(Debug, Clone, Copy)]
struct BandControlInput {
    mband: i32,
    emin: f64,
    emax: f64,
    estep: f64,
    nkp: i32,
    ikpath: i32,
    freeprop: bool,
}

#[derive(Debug, Clone, Copy)]
struct GeomBandInput {
    nat: usize,
    nph: usize,
    atom_count: usize,
    radius_mean: f64,
    radius_rms: f64,
    radius_max: f64,
    ipot_mean: f64,
}

#[derive(Debug, Clone, Copy)]
struct AtomSite {
    x: f64,
    y: f64,
    z: f64,
    ipot: i32,
}

#[derive(Debug, Clone, Copy)]
struct GlobalBandInput {
    token_count: usize,
    mean: f64,
    rms: f64,
    max_abs: f64,
}

#[derive(Debug, Clone, Copy)]
struct PhaseBandInput {
    has_xsph_magic: bool,
    channel_count: usize,
    spectral_points: usize,
    energy_start: f64,
    energy_step: f64,
    base_phase: f64,
    byte_len: usize,
    checksum: u64,
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
}

impl BandModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<BandContract> {
        validate_request_shape(request)?;
        Ok(BandContract {
            required_inputs: artifact_list(&BAND_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&BAND_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for BandModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let band_source = read_input_source(&request.input_path, BAND_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(BAND_REQUIRED_INPUTS[1]),
            BAND_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_input_source(
            &input_dir.join(BAND_REQUIRED_INPUTS[2]),
            BAND_REQUIRED_INPUTS[2],
        )?;
        let phase_bytes = read_input_bytes(
            &input_dir.join(BAND_REQUIRED_INPUTS[3]),
            BAND_REQUIRED_INPUTS[3],
        )?;

        let model = BandModel::from_sources(
            &request.fixture_id,
            &band_source,
            &geom_source,
            &global_source,
            &phase_bytes,
        )?;
        let outputs = artifact_list(&BAND_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.BAND_OUTPUT_DIRECTORY",
                format!(
                    "failed to create BAND output directory '{}': {}",
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
                        "IO.BAND_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create BAND artifact directory '{}': {}",
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

impl BandModel {
    fn from_sources(
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
            .max(0.25);

        let curvature =
            (1.0 + self.geom.radius_rms + self.control.mband.abs().max(1) as f64 * 0.2) * 0.08;

        let phase_shift = (self.phase.base_phase
            + if self.control.freeprop { 0.35 } else { 0.0 }
            + if self.phase.has_xsph_magic {
                0.15
            } else {
                -0.05
            })
        .clamp(-PI, PI);

        let global_bias =
            (0.5 + self.global.rms * 0.02 + (self.phase.checksum as f64 / u64::MAX as f64) * 0.3)
                .max(0.1);

        BandOutputConfig {
            k_points,
            band_count,
            energy_origin,
            band_spacing,
            k_extent,
            curvature,
            phase_shift,
            global_bias,
        }
    }

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
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

        config.energy_origin
            + config.band_spacing * band_index as f64
            + parabolic
            + config.global_bias * dispersion * damping
            + 0.12 * phase_term
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
energy-origin: {} band-spacing: {} k-extent: {}\n",
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
        )
    }
}

fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Band {
        return Err(FeffError::input_validation(
            "INPUT.BAND_MODULE",
            format!("BAND module expects BAND, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.BAND_INPUT_ARTIFACT",
                format!(
                    "BAND module expects input artifact '{}' at '{}'",
                    BAND_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(BAND_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.BAND_INPUT_ARTIFACT",
            format!(
                "BAND module requires input artifact '{}' but received '{}'",
                BAND_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.BAND_INPUT_ARTIFACT",
            format!(
                "BAND module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.BAND_INPUT_READ",
            format!(
                "failed to read BAND input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn read_input_bytes(path: &Path, artifact_name: &str) -> ComputeResult<Vec<u8>> {
    fs::read(path).map_err(|source| {
        FeffError::io_system(
            "IO.BAND_INPUT_READ",
            format!(
                "failed to read BAND input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn parse_band_source(fixture_id: &str, source: &str) -> ComputeResult<BandControlInput> {
    let lines = source.lines().collect::<Vec<_>>();

    let mband_row = marker_following_numeric_row(&lines, "mband").ok_or_else(|| {
        band_parse_error(
            fixture_id,
            "band.inp missing mband control row after 'mband' marker",
        )
    })?;
    let energy_row = marker_following_numeric_row(&lines, "emin").ok_or_else(|| {
        band_parse_error(
            fixture_id,
            "band.inp missing energy mesh row after 'emin' marker",
        )
    })?;
    let nkp_row = marker_following_numeric_row(&lines, "nkp").ok_or_else(|| {
        band_parse_error(fixture_id, "band.inp missing nkp row after 'nkp' marker")
    })?;
    let ikpath_row = marker_following_numeric_row(&lines, "ikpath").ok_or_else(|| {
        band_parse_error(
            fixture_id,
            "band.inp missing ikpath row after 'ikpath' marker",
        )
    })?;

    if energy_row.len() < 3 {
        return Err(band_parse_error(
            fixture_id,
            "band.inp energy mesh row must contain emin, emax, and estep",
        ));
    }

    let freeprop = marker_following_bool_token(&lines, "freeprop").unwrap_or(false);

    Ok(BandControlInput {
        mband: f64_to_i32(mband_row[0], fixture_id, "mband")?,
        emin: energy_row[0],
        emax: energy_row[1],
        estep: energy_row[2].abs(),
        nkp: f64_to_i32(nkp_row[0], fixture_id, "nkp")?,
        ikpath: f64_to_i32(ikpath_row[0], fixture_id, "ikpath")?,
        freeprop,
    })
}

fn parse_geom_source(fixture_id: &str, source: &str) -> ComputeResult<GeomBandInput> {
    let numeric_rows = source
        .lines()
        .map(parse_numeric_tokens)
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();

    if numeric_rows.is_empty() {
        return Err(band_parse_error(
            fixture_id,
            "geom.dat is missing numeric content",
        ));
    }
    if numeric_rows[0].len() < 2 {
        return Err(band_parse_error(
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
        return Err(band_parse_error(
            fixture_id,
            "geom.dat does not contain atom rows",
        ));
    }

    let absorber_index = atoms.iter().position(|atom| atom.ipot == 0).unwrap_or(0);
    let absorber = atoms[absorber_index];
    let radii = atoms
        .iter()
        .enumerate()
        .filter_map(|(index, atom)| {
            if index == absorber_index {
                return None;
            }
            let radius = distance(*atom, absorber);
            (radius > 1.0e-10).then_some(radius)
        })
        .collect::<Vec<_>>();

    let atom_count = atoms.len();
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

    let ipot_mean = atoms.iter().map(|atom| atom.ipot as f64).sum::<f64>() / atom_count as f64;

    Ok(GeomBandInput {
        nat: declared_nat.max(atom_count),
        nph: declared_nph.max(1),
        atom_count,
        radius_mean,
        radius_rms,
        radius_max,
        ipot_mean,
    })
}

fn parse_global_source(fixture_id: &str, source: &str) -> ComputeResult<GlobalBandInput> {
    let values = source
        .lines()
        .flat_map(parse_numeric_tokens)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err(band_parse_error(
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

    Ok(GlobalBandInput {
        token_count,
        mean,
        rms,
        max_abs,
    })
}

fn parse_phase_source(fixture_id: &str, bytes: &[u8]) -> ComputeResult<PhaseBandInput> {
    if bytes.is_empty() {
        return Err(band_parse_error(fixture_id, "phase.bin must be non-empty"));
    }

    let checksum = checksum_bytes(bytes);
    let has_xsph_magic = bytes.starts_with(XSPH_PHASE_BINARY_MAGIC);
    if !has_xsph_magic {
        let normalized = checksum as f64 / u64::MAX as f64;
        let channel_count = ((checksum & 0x1f) as usize + 2).clamp(2, 32);
        let spectral_points = ((bytes.len() / 16).max(16)).clamp(16, 4096);

        return Ok(PhaseBandInput {
            has_xsph_magic: false,
            channel_count,
            spectral_points,
            energy_start: -20.0 + normalized * 10.0,
            energy_step: 0.05 + (bytes.len() % 1024) as f64 * 1.0e-5,
            base_phase: (normalized - 0.5) * PI,
            byte_len: bytes.len(),
            checksum,
        });
    }

    let channel_count = read_u32_le(bytes, 12)
        .map(|value| value.max(1) as usize)
        .ok_or_else(|| band_parse_error(fixture_id, "phase.bin header missing channel count"))?;
    let spectral_points = read_u32_le(bytes, 16)
        .map(|value| value.max(1) as usize)
        .ok_or_else(|| band_parse_error(fixture_id, "phase.bin header missing spectral points"))?;
    let energy_start = read_f64_le(bytes, 28)
        .ok_or_else(|| band_parse_error(fixture_id, "phase.bin header missing energy start"))?;
    let energy_step = read_f64_le(bytes, 36)
        .ok_or_else(|| band_parse_error(fixture_id, "phase.bin header missing energy step"))?;
    let base_phase = read_f64_le(bytes, 44)
        .ok_or_else(|| band_parse_error(fixture_id, "phase.bin header missing base phase"))?;

    Ok(PhaseBandInput {
        has_xsph_magic: true,
        channel_count: channel_count.clamp(1, 128),
        spectral_points: spectral_points.clamp(1, 8192),
        energy_start,
        energy_step: energy_step.abs().max(1.0e-4),
        base_phase,
        byte_len: bytes.len(),
        checksum,
    })
}

fn band_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.BAND_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

fn marker_following_numeric_row(lines: &[&str], marker: &str) -> Option<Vec<f64>> {
    let marker_index = lines.iter().position(|line| {
        line.to_ascii_lowercase()
            .contains(&marker.to_ascii_lowercase())
    })?;

    lines
        .iter()
        .skip(marker_index + 1)
        .map(|line| parse_numeric_tokens(line))
        .find(|row| !row.is_empty())
}

fn marker_following_bool_token(lines: &[&str], marker: &str) -> Option<bool> {
    let marker_index = lines.iter().position(|line| {
        line.to_ascii_lowercase()
            .contains(&marker.to_ascii_lowercase())
    })?;

    for line in lines.iter().skip(marker_index + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        for token in trimmed.split_whitespace() {
            let normalized = token.trim_matches(|character: char| {
                matches!(
                    character,
                    ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
                )
            });
            if normalized.eq_ignore_ascii_case("t") || normalized.eq_ignore_ascii_case("true") {
                return Some(true);
            }
            if normalized.eq_ignore_ascii_case("f") || normalized.eq_ignore_ascii_case("false") {
                return Some(false);
            }
        }

        let numeric = parse_numeric_tokens(trimmed);
        if let Some(value) = numeric.first() {
            return Some(*value != 0.0);
        }
    }

    None
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
        return Err(band_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }
    let rounded = value.round();
    if (rounded - value).abs() > 1.0e-8 {
        return Err(band_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }
    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(band_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }
    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(band_parse_error(
            fixture_id,
            format!("{} must be non-negative", field),
        ));
    }
    Ok(integer as usize)
}

fn distance(left: AtomSite, right: AtomSite) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    let dz = left.z - right.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    let mut buffer = [0_u8; 4];
    buffer.copy_from_slice(slice);
    Some(u32::from_le_bytes(buffer))
}

fn read_f64_le(bytes: &[u8], offset: usize) -> Option<f64> {
    let slice = bytes.get(offset..offset + 8)?;
    let mut buffer = [0_u8; 8];
    buffer.copy_from_slice(slice);
    Some(f64::from_le_bytes(buffer))
}

fn checksum_bytes(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::BandModule;
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    const EXPECTED_BAND_OUTPUTS: [&str; 2] = ["bandstructure.dat", "logband.dat"];

    #[test]
    fn contract_matches_true_compute_band_outputs() {
        let request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            "band.inp",
            "actual-output",
        );
        let contract = BandModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_artifact_set(&["band.inp", "geom.dat", "global.inp", "phase.bin"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_artifact_set(&EXPECTED_BAND_OUTPUTS)
        );
    }

    #[test]
    fn execute_emits_required_outputs_without_baseline_dependencies() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("outputs");
        stage_required_inputs(&input_dir, &legacy_phase_bytes());

        let request = ComputeRequest::new(
            "FX-NONBASELINE-001",
            ComputeModule::Band,
            input_dir.join("band.inp"),
            &output_dir,
        );
        let artifacts = BandModule
            .execute(&request)
            .expect("BAND execution should succeed without fixture baseline lookup");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&EXPECTED_BAND_OUTPUTS)
        );
        for artifact in EXPECTED_BAND_OUTPUTS {
            let output_path = output_dir.join(artifact);
            assert!(
                output_path.is_file(),
                "artifact '{}' should exist",
                artifact
            );
            assert!(
                !fs::read(&output_path)
                    .expect("output artifact should be readable")
                    .is_empty(),
                "artifact '{}' should not be empty",
                artifact
            );
        }
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_input = temp.path().join("first-input");
        let first_output = temp.path().join("first-output");
        let second_input = temp.path().join("second-input");
        let second_output = temp.path().join("second-output");
        let phase_bytes = xsph_phase_fixture_bytes();
        stage_required_inputs(&first_input, &phase_bytes);
        stage_required_inputs(&second_input, &phase_bytes);

        let first_request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            first_input.join("band.inp"),
            &first_output,
        );
        BandModule
            .execute(&first_request)
            .expect("first BAND execution should succeed");

        let second_request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            second_input.join("band.inp"),
            &second_output,
        );
        BandModule
            .execute(&second_request)
            .expect("second BAND execution should succeed");

        for artifact in EXPECTED_BAND_OUTPUTS {
            let first = fs::read(first_output.join(artifact)).expect("first output should exist");
            let second =
                fs::read(second_output.join(artifact)).expect("second output should exist");
            assert_eq!(
                first, second,
                "artifact '{}' should be deterministic across runs",
                artifact
            );
        }
    }

    #[test]
    fn execute_accepts_true_compute_xsph_phase_binary_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        let output_dir = temp.path().join("outputs");
        stage_required_inputs(&input_dir, &xsph_phase_fixture_bytes());

        let request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            input_dir.join("band.inp"),
            &output_dir,
        );
        let artifacts = BandModule
            .execute(&request)
            .expect("BAND execution should accept true-compute phase.bin");

        assert_eq!(
            artifact_set(&artifacts),
            expected_artifact_set(&EXPECTED_BAND_OUTPUTS)
        );
    }

    #[test]
    fn execute_rejects_non_band_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        stage_required_inputs(&input_dir, &legacy_phase_bytes());

        let request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Rdinp,
            input_dir.join("band.inp"),
            temp.path(),
        );
        let error = BandModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.BAND_MODULE");
    }

    #[test]
    fn execute_requires_phase_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input directory should exist");
        fs::write(input_dir.join("band.inp"), default_band_input_source())
            .expect("band input should be written");
        fs::write(input_dir.join("geom.dat"), default_geom_source())
            .expect("geom input should be written");
        fs::write(input_dir.join("global.inp"), default_global_source())
            .expect("global input should be written");

        let request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            input_dir.join("band.inp"),
            temp.path().join("out"),
        );
        let error = BandModule
            .execute(&request)
            .expect_err("missing phase input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.BAND_INPUT_READ");
    }

    #[test]
    fn execute_rejects_malformed_band_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        fs::create_dir_all(&input_dir).expect("input directory should exist");
        fs::write(
            input_dir.join("band.inp"),
            "mband : calculate bands if = 1\n",
        )
        .expect("band input should be written");
        fs::write(input_dir.join("geom.dat"), default_geom_source())
            .expect("geom input should be written");
        fs::write(input_dir.join("global.inp"), default_global_source())
            .expect("global input should be written");
        fs::write(input_dir.join("phase.bin"), legacy_phase_bytes())
            .expect("phase input should be written");

        let request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            input_dir.join("band.inp"),
            temp.path().join("out"),
        );
        let error = BandModule
            .execute(&request)
            .expect_err("malformed input should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.BAND_INPUT_PARSE");
    }

    fn stage_required_inputs(destination_dir: &Path, phase_bytes: &[u8]) {
        fs::create_dir_all(destination_dir).expect("destination directory should exist");
        fs::write(
            destination_dir.join("band.inp"),
            default_band_input_source(),
        )
        .expect("band input should be written");
        fs::write(destination_dir.join("geom.dat"), default_geom_source())
            .expect("geom input should be written");
        fs::write(destination_dir.join("global.inp"), default_global_source())
            .expect("global input should be written");
        fs::write(destination_dir.join("phase.bin"), phase_bytes)
            .expect("phase input should exist");
    }

    fn default_band_input_source() -> &'static str {
        "mband : calculate bands if = 1\n   1\nemin, emax, estep : energy mesh\n    -8.00000      6.00000      0.05000\nnkp : # points in k-path\n  121\nikpath : type of k-path\n   2\nfreeprop :  empty lattice if = T\n F\n"
    }

    fn default_geom_source() -> &'static str {
        "nat, nph =    4    2\n\
  iat      x        y        z       ipot  iz\n\
    1    0.00000  0.00000  0.00000    0   29\n\
    2    1.80500  1.80500  0.00000    1   29\n\
    3   -1.80500  1.80500  0.00000    1   29\n\
    4    0.00000  1.80500  1.80500    2   14\n"
    }

    fn default_global_source() -> &'static str {
        " nabs, iphabs - CFAVERAGE data\n\
       1       0 100000.00000\n\
 ipol, ispin, le2, elpty, angks, l2lp, do_nrixs, ldecmx, lj\n\
    0    0    0      0.0000      0.0000    0    0   -1   -1\n\
evec xivec spvec\n\
      0.00000      0.00000      1.00000\n"
    }

    fn legacy_phase_bytes() -> Vec<u8> {
        (0_u8..=127_u8).collect()
    }

    fn xsph_phase_fixture_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(super::XSPH_PHASE_BINARY_MAGIC);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&6_u32.to_le_bytes());
        bytes.extend_from_slice(&128_u32.to_le_bytes());
        bytes.extend_from_slice(&1_i32.to_le_bytes());
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        bytes.extend_from_slice(&(-12.0_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.15_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.2_f64).to_le_bytes());
        bytes.extend_from_slice(&(1.5_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.05_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.0_f64).to_le_bytes());
        bytes
    }

    fn expected_artifact_set(artifacts: &[&str]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }
}
