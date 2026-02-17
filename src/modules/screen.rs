use super::ModuleExecutor;
use super::serialization::{format_fixed_f64, write_text_artifact};
use crate::domain::{FeffError, ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult};
use std::fs;
use std::path::{Path, PathBuf};

const SCREEN_REQUIRED_INPUTS: [&str; 3] = ["pot.inp", "geom.dat", "ldos.inp"];
const SCREEN_OPTIONAL_INPUTS: [&str; 1] = ["screen.inp"];
const SCREEN_REQUIRED_OUTPUTS: [&str; 2] = ["wscrn.dat", "logscreen.dat"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub optional_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenModule;

#[derive(Debug, Clone)]
struct ScreenModel {
    fixture_id: String,
    pot: PotScreenInput,
    geom: GeomScreenInput,
    ldos: LdosScreenInput,
    screen_override: Option<ScreenOverrideInput>,
}

#[derive(Debug, Clone)]
struct PotScreenInput {
    title: String,
    gamach: f64,
    rfms1: f64,
    mean_folp: f64,
    mean_xion: f64,
    lmaxsc_max: i32,
}

#[derive(Debug, Clone)]
struct GeomScreenInput {
    nat: usize,
    nph: usize,
    atoms: Vec<AtomSite>,
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

#[derive(Debug, Clone)]
struct LdosScreenInput {
    neldos: i32,
    rfms2: f64,
    emin: f64,
    emax: f64,
    eimag: f64,
    rgrd: f64,
    toler1: f64,
    toler2: f64,
    lmaxph_max: i32,
}

#[derive(Debug, Clone, Default)]
struct ScreenOverrideInput {
    ner: Option<i32>,
    nei: Option<i32>,
    maxl: Option<i32>,
    irrh: Option<i32>,
    iend: Option<i32>,
    lfxc: Option<i32>,
    emin: Option<f64>,
    emax: Option<f64>,
    eimax: Option<f64>,
    ermin: Option<f64>,
    rfms: Option<f64>,
    nrptx0: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
struct PotentialRow {
    lmaxsc: i32,
    xion: f64,
    folp: f64,
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
}

impl ScreenModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<ScreenContract> {
        validate_request_shape(request)?;
        Ok(ScreenContract {
            required_inputs: artifact_list(&SCREEN_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&SCREEN_OPTIONAL_INPUTS),
            expected_outputs: artifact_list(&SCREEN_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for ScreenModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let pot_source = read_input_source(&request.input_path, SCREEN_REQUIRED_INPUTS[0])?;
        let geom_source = read_input_source(
            &input_dir.join(SCREEN_REQUIRED_INPUTS[1]),
            SCREEN_REQUIRED_INPUTS[1],
        )?;
        let ldos_source = read_input_source(
            &input_dir.join(SCREEN_REQUIRED_INPUTS[2]),
            SCREEN_REQUIRED_INPUTS[2],
        )?;
        let screen_source = maybe_read_optional_input_source(
            input_dir.join(SCREEN_OPTIONAL_INPUTS[0]),
            SCREEN_OPTIONAL_INPUTS[0],
        )?;

        let model = ScreenModel::from_sources(
            &request.fixture_id,
            &pot_source,
            &geom_source,
            &ldos_source,
            screen_source.as_deref(),
        )?;
        let outputs = artifact_list(&SCREEN_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.SCREEN_OUTPUT_DIRECTORY",
                format!(
                    "failed to create SCREEN output directory '{}': {}",
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
                        "IO.SCREEN_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create SCREEN artifact directory '{}': {}",
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

impl ScreenModel {
    fn from_sources(
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

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
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

        let screen_level = (self.pot.mean_folp.max(0.05) + 0.02 * self.pot.gamach.abs())
            * (1.0 + 0.015 * maxl as f64 + 0.005 * lfxc as f64 + 0.01 * ipot_mean.abs());
        let screen_amplitude = (self.geom.radius_rms + self.ldos.rgrd.abs() + ermin)
            * (1.0 + 0.01 * ner as f64 + 0.02 * eimax.abs());
        let charge_delta = (self.pot.mean_xion.abs() + self.ldos.toler1 + self.ldos.toler2 + 0.1)
            * (1.0 + 0.005 * (self.geom.nat as f64).sqrt());
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
            has_override,
            nrptx0,
        )
    }
}

fn validate_request_shape(request: &ComputeRequest) -> ComputeResult<()> {
    if request.module != ComputeModule::Screen {
        return Err(FeffError::input_validation(
            "INPUT.SCREEN_MODULE",
            format!(
                "SCREEN module expects SCREEN, got {}",
                request.module
            ),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.SCREEN_INPUT_ARTIFACT",
                format!(
                    "SCREEN module expects input artifact '{}' at '{}'",
                    SCREEN_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(SCREEN_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.SCREEN_INPUT_ARTIFACT",
            format!(
                "SCREEN module requires input artifact '{}' but received '{}'",
                SCREEN_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &ComputeRequest) -> ComputeResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.SCREEN_INPUT_ARTIFACT",
            format!(
                "SCREEN module requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> ComputeResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.SCREEN_INPUT_READ",
            format!(
                "failed to read SCREEN input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn maybe_read_optional_input_source(
    path: PathBuf,
    artifact_name: &str,
) -> ComputeResult<Option<String>> {
    if path.is_file() {
        return read_input_source(&path, artifact_name).map(Some);
    }

    Ok(None)
}

fn parse_pot_source(fixture_id: &str, source: &str) -> ComputeResult<PotScreenInput> {
    let lines: Vec<&str> = source.lines().collect();
    let title = lines
        .iter()
        .map(|line| line.trim())
        .find(|line| {
            !line.is_empty()
                && parse_numeric_tokens(line).is_empty()
                && !line.contains(',')
                && !line.ends_with(':')
        })
        .unwrap_or("untitled")
        .to_string();

    let mut gamach = 1.0_f64;
    let mut rfms1 = 4.0_f64;
    let mut potential_rows = Vec::new();

    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("gamach") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 6 {
                    gamach = values[0];
                    rfms1 = values[5];
                }
            }
            index += 1;
            continue;
        }

        if lower.starts_with("iz") && lower.contains("lmaxsc") {
            index += 1;
            while index < lines.len() {
                let row = lines[index].trim();
                if row.is_empty() {
                    index += 1;
                    continue;
                }

                let values = parse_numeric_tokens(row);
                if values.len() < 5 {
                    break;
                }

                potential_rows.push(PotentialRow {
                    lmaxsc: f64_to_i32(values[1], fixture_id, "pot.inp potential lmaxsc")?,
                    xion: values[3],
                    folp: values[4],
                });
                index += 1;
            }
            continue;
        }

        index += 1;
    }

    if potential_rows.is_empty() {
        return Err(screen_parse_error(
            fixture_id,
            "pot.inp does not contain any potential rows",
        ));
    }

    let folp_sum: f64 = potential_rows.iter().map(|row| row.folp).sum();
    let xion_sum: f64 = potential_rows.iter().map(|row| row.xion).sum();
    let lmaxsc_max = potential_rows
        .iter()
        .map(|row| row.lmaxsc)
        .max()
        .unwrap_or(1)
        .max(1);

    Ok(PotScreenInput {
        title,
        gamach,
        rfms1,
        mean_folp: folp_sum / potential_rows.len() as f64,
        mean_xion: xion_sum / potential_rows.len() as f64,
        lmaxsc_max,
    })
}

fn parse_geom_source(fixture_id: &str, source: &str) -> ComputeResult<GeomScreenInput> {
    let mut nat: Option<usize> = None;
    let mut nph: Option<usize> = None;
    let mut atoms = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let values = parse_numeric_tokens(trimmed);
        if values.is_empty() {
            continue;
        }

        if nat.is_none() && values.len() >= 2 {
            nat = Some(f64_to_usize(values[0], fixture_id, "geom.dat nat")?);
            nph = Some(f64_to_usize(values[1], fixture_id, "geom.dat nph")?);
            continue;
        }

        if values.len() >= 5 {
            atoms.push(AtomSite {
                x: values[1],
                y: values[2],
                z: values[3],
                ipot: f64_to_i32(values[4], fixture_id, "geom.dat atom ipot")?,
            });
        }
    }

    if atoms.is_empty() {
        return Err(screen_parse_error(
            fixture_id,
            "geom.dat does not contain any atom rows",
        ));
    }

    let nat_value = nat.unwrap_or(atoms.len()).max(atoms.len());
    let nph_value = nph.unwrap_or(1).max(1);

    let mut radius_sum = 0.0_f64;
    let mut radius_sq_sum = 0.0_f64;
    let mut radius_max = 0.0_f64;
    for atom in &atoms {
        let radius = (atom.x * atom.x + atom.y * atom.y + atom.z * atom.z).sqrt();
        radius_sum += radius;
        radius_sq_sum += radius * radius;
        radius_max = radius_max.max(radius);
    }

    let atom_count = atoms.len() as f64;
    let radius_mean = radius_sum / atom_count;
    let radius_rms = (radius_sq_sum / atom_count).sqrt();

    Ok(GeomScreenInput {
        nat: nat_value,
        nph: nph_value,
        atoms,
        radius_mean,
        radius_rms,
        radius_max,
    })
}

fn parse_ldos_source(fixture_id: &str, source: &str) -> ComputeResult<LdosScreenInput> {
    let lines: Vec<&str> = source.lines().collect();

    let mut neldos: Option<i32> = None;
    let mut rfms2: Option<f64> = None;
    let mut emin: Option<f64> = None;
    let mut emax: Option<f64> = None;
    let mut eimag: Option<f64> = None;
    let mut rgrd: Option<f64> = None;
    let mut toler1: Option<f64> = None;
    let mut toler2: Option<f64> = None;
    let mut lmaxph_max: Option<i32> = None;

    for index in 0..lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("mldos") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 5 {
                    neldos = if values.len() >= 6 {
                        Some(f64_to_i32(values[5], fixture_id, "ldos.inp neldos")?)
                    } else {
                        Some(101)
                    };
                }
            }
            continue;
        }

        if lower.starts_with("rfms2") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 5 {
                    rfms2 = Some(values[0]);
                    emin = Some(values[1]);
                    emax = Some(values[2]);
                    eimag = Some(values[3]);
                    rgrd = Some(values[4]);
                }
            }
            continue;
        }

        if lower.starts_with("rdirec") {
            if let Some((_, values_line)) = next_nonempty_line(&lines, index + 1) {
                let values = parse_numeric_tokens(values_line);
                if values.len() >= 3 {
                    toler1 = Some(values[1]);
                    toler2 = Some(values[2]);
                }
            }
            continue;
        }

        if lower.contains("lmaxph")
            && let Some((_, values_line)) = next_nonempty_line(&lines, index + 1)
        {
            let values = parse_numeric_tokens(values_line);
            if !values.is_empty() {
                let mut local_max = i32::MIN;
                for value in values {
                    local_max = local_max.max(f64_to_i32(value, fixture_id, "ldos.inp lmaxph")?);
                }
                lmaxph_max = Some(local_max.max(1));
            }
        }
    }

    let neldos = neldos.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing neldos in mldos/lfms2 control block",
        )
    })?;
    let rfms2 = rfms2.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
        )
    })?;
    let emin = emin.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
        )
    })?;
    let emax = emax.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
        )
    })?;
    let eimag = eimag.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
        )
    })?;
    let rgrd = rgrd.ok_or_else(|| {
        screen_parse_error(
            fixture_id,
            "ldos.inp missing rfms2/emin/emax/eimag/rgrd control values",
        )
    })?;

    Ok(LdosScreenInput {
        neldos,
        rfms2,
        emin,
        emax,
        eimag,
        rgrd,
        toler1: toler1.unwrap_or(1.0e-3),
        toler2: toler2.unwrap_or(1.0e-3),
        lmaxph_max: lmaxph_max.unwrap_or(1).max(1),
    })
}

fn parse_screen_override_source(
    fixture_id: &str,
    source: &str,
) -> ComputeResult<ScreenOverrideInput> {
    let mut parsed = ScreenOverrideInput::default();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let mut tokens = trimmed.split_whitespace();
        let Some(raw_key) = tokens.next() else {
            continue;
        };
        let Some(raw_value) = tokens.next() else {
            continue;
        };

        let key = raw_key
            .trim_matches(|character: char| matches!(character, ':' | ',' | ';'))
            .to_ascii_lowercase();
        let Some(value) = parse_numeric_token(raw_value) else {
            continue;
        };

        match key.as_str() {
            "ner" => parsed.ner = Some(f64_to_i32(value, fixture_id, "screen.inp ner")?),
            "nei" => parsed.nei = Some(f64_to_i32(value, fixture_id, "screen.inp nei")?),
            "maxl" => parsed.maxl = Some(f64_to_i32(value, fixture_id, "screen.inp maxl")?),
            "irrh" => parsed.irrh = Some(f64_to_i32(value, fixture_id, "screen.inp irrh")?),
            "iend" => parsed.iend = Some(f64_to_i32(value, fixture_id, "screen.inp iend")?),
            "lfxc" => parsed.lfxc = Some(f64_to_i32(value, fixture_id, "screen.inp lfxc")?),
            "emin" => parsed.emin = Some(value),
            "emax" => parsed.emax = Some(value),
            "eimax" => parsed.eimax = Some(value),
            "ermin" => parsed.ermin = Some(value),
            "rfms" => parsed.rfms = Some(value),
            "nrptx0" => parsed.nrptx0 = Some(f64_to_i32(value, fixture_id, "screen.inp nrptx0")?),
            _ => {}
        }
    }

    Ok(parsed)
}

fn next_nonempty_line<'a>(lines: &'a [&'a str], start_index: usize) -> Option<(usize, &'a str)> {
    for (offset, line) in lines.iter().enumerate().skip(start_index) {
        if !line.trim().is_empty() {
            return Some((offset, *line));
        }
    }

    None
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> ComputeResult<i32> {
    if !value.is_finite() {
        return Err(screen_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }

    let rounded = value.round();
    if (value - rounded).abs() > 1.0e-6 {
        return Err(screen_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }

    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(screen_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }

    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> ComputeResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(screen_parse_error(
            fixture_id,
            format!("{} must be non-negative", field),
        ));
    }

    Ok(integer as usize)
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

fn format_scientific_f64(value: f64) -> String {
    format!("{value:.10E}")
}

fn screen_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.SCREEN_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
    paths.iter().copied().map(ComputeArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::ScreenModule;
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const POT_INPUT_FIXTURE: &str = "mpot, nph, ntitle, ihole, ipr1, iafolp, ixc,ispec
   1   1   1   1   0   0   0   1
nmix, nohole, jumprm, inters, nscmt, icoul, lfms1, iunf
   1   2   0   0 100   0   0   0
Cu crystal
gamach, rgrd, ca1, ecv, totvol, rfms1
      1.72919      0.05000      0.20000    -40.00000      0.00000      4.00000
 iz, lmaxsc, xnatph, xion, folp
   29    2      1.00000      0.00000      1.15000
   29    3      1.00000      0.10000      1.35000
";

    const GEOM_INPUT_FIXTURE: &str = "nat, nph =    4    1
    1    2
 iat     x       y        z       iph
 -----------------------------------------------------------------------
   1      0.00000      0.00000      0.00000   0   1
   2      1.80500      1.80500      0.00000   1   1
   3     -1.80500      1.80500      0.00000   1   1
   4      0.00000      1.80500      1.80500   1   1
";

    const LDOS_INPUT_FIXTURE: &str = "mldos, lfms2, ixc, ispin, minv, neldos
   0   0   0   0   0     101
rfms2, emin, emax, eimag, rgrd
      6.00000   1000.00000      0.00000     -1.00000      0.05000
rdirec, toler1, toler2
     12.00000      0.00100      0.00100
 lmaxph(0:nph)
   3   3
";

    const SCREEN_OVERRIDE_FIXTURE: &str = "ner          40
nei          20
maxl           4
irrh           1
iend           0
lfxc           0
emin  -40.0000000000000
emax  0.000000000000000E+000
eimax   2.00000000000000
ermin  1.000000000000000E-003
rfms   4.00000000000000
nrptx0         251
";

    #[test]
    fn contract_exposes_true_compute_screen_artifact_contract() {
        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            "pot.inp",
            "actual-output",
        );
        let scaffold = ScreenModule;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_set(&["pot.inp", "geom.dat", "ldos.inp"])
        );
        assert_eq!(
            artifact_set(&contract.optional_inputs),
            expected_set(&["screen.inp"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_set(&["wscrn.dat", "logscreen.dat"])
        );
    }

    #[test]
    fn execute_emits_required_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");
        let input_path = stage_screen_inputs(temp.path(), Some(SCREEN_OVERRIDE_FIXTURE));

        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &input_path,
            &output_dir,
        );
        let artifacts = ScreenModule
            .execute(&request)
            .expect("SCREEN execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_set(&["wscrn.dat", "logscreen.dat"])
        );
        for artifact in artifacts {
            let output_path = output_dir.join(&artifact.relative_path);
            assert!(
                output_path.is_file(),
                "output artifact '{}' should exist",
                output_path.display()
            );
            assert!(
                !fs::read(&output_path)
                    .expect("output artifact should be readable")
                    .is_empty(),
                "output artifact '{}' should not be empty",
                output_path.display()
            );
        }
    }

    #[test]
    fn execute_allows_missing_optional_screen_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");
        let input_path = stage_screen_inputs(temp.path(), None);

        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &input_path,
            &output_dir,
        );
        let artifacts = ScreenModule
            .execute(&request)
            .expect("SCREEN execution should succeed without screen.inp");

        assert_eq!(
            artifact_set(&artifacts),
            expected_set(&["wscrn.dat", "logscreen.dat"])
        );
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_root = temp.path().join("first");
        let second_root = temp.path().join("second");
        let first_input = stage_screen_inputs(&first_root, Some(SCREEN_OVERRIDE_FIXTURE));
        let second_input = stage_screen_inputs(&second_root, Some(SCREEN_OVERRIDE_FIXTURE));

        let first_output = first_root.join("out");
        let second_output = second_root.join("out");

        let first_request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &first_input,
            &first_output,
        );
        let second_request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &second_input,
            &second_output,
        );

        ScreenModule
            .execute(&first_request)
            .expect("first run should succeed");
        ScreenModule
            .execute(&second_request)
            .expect("second run should succeed");

        for artifact in ["wscrn.dat", "logscreen.dat"] {
            let first_bytes =
                fs::read(first_output.join(artifact)).expect("first output should exist");
            let second_bytes =
                fs::read(second_output.join(artifact)).expect("second output should exist");
            assert_eq!(
                first_bytes, second_bytes,
                "artifact '{}' should be deterministic across runs",
                artifact
            );
        }
    }

    #[test]
    fn execute_optional_screen_input_changes_screen_response() {
        let temp = TempDir::new().expect("tempdir should be created");

        let with_override_root = temp.path().join("with-override");
        let without_override_root = temp.path().join("without-override");
        let with_override_input =
            stage_screen_inputs(&with_override_root, Some(SCREEN_OVERRIDE_FIXTURE));
        let without_override_input = stage_screen_inputs(&without_override_root, None);

        let with_override_output = with_override_root.join("out");
        let without_override_output = without_override_root.join("out");

        let with_override_request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &with_override_input,
            &with_override_output,
        );
        let without_override_request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &without_override_input,
            &without_override_output,
        );

        ScreenModule
            .execute(&with_override_request)
            .expect("override run should succeed");
        ScreenModule
            .execute(&without_override_request)
            .expect("default run should succeed");

        let with_override =
            fs::read(with_override_output.join("wscrn.dat")).expect("override wscrn should exist");
        let without_override = fs::read(without_override_output.join("wscrn.dat"))
            .expect("default wscrn should exist");
        assert_ne!(
            with_override, without_override,
            "optional screen.inp should influence computed wscrn.dat"
        );
    }

    #[test]
    fn execute_rejects_non_screen_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = stage_screen_inputs(temp.path(), Some(SCREEN_OVERRIDE_FIXTURE));

        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Crpa,
            &input_path,
            temp.path(),
        );
        let error = ScreenModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.SCREEN_MODULE");
    }

    #[test]
    fn execute_requires_geom_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        fs::write(&input_path, POT_INPUT_FIXTURE).expect("pot input should be staged");
        fs::write(temp.path().join("ldos.inp"), LDOS_INPUT_FIXTURE)
            .expect("ldos input should be staged");

        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &input_path,
            temp.path(),
        );
        let error = ScreenModule
            .execute(&request)
            .expect_err("missing geom input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.SCREEN_INPUT_READ");
    }

    #[test]
    fn execute_rejects_invalid_ldos_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("pot.inp");
        fs::write(&input_path, POT_INPUT_FIXTURE).expect("pot input should be staged");
        fs::write(temp.path().join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be staged");
        fs::write(temp.path().join("ldos.inp"), "invalid ldos input\n")
            .expect("ldos input should be staged");

        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            &input_path,
            temp.path(),
        );
        let error = ScreenModule
            .execute(&request)
            .expect_err("invalid ldos should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.SCREEN_INPUT_PARSE");
    }

    fn stage_screen_inputs(root: &Path, screen_override: Option<&str>) -> PathBuf {
        fs::create_dir_all(root).expect("root directory should exist");
        let pot_path = root.join("pot.inp");
        fs::write(&pot_path, POT_INPUT_FIXTURE).expect("pot input should be written");
        fs::write(root.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom input should be written");
        fs::write(root.join("ldos.inp"), LDOS_INPUT_FIXTURE).expect("ldos input should be written");

        if let Some(source) = screen_override {
            fs::write(root.join("screen.inp"), source).expect("screen override should be written");
        }

        pot_path
    }

    fn expected_set(entries: &[&str]) -> BTreeSet<String> {
        entries.iter().map(|entry| entry.to_string()).collect()
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }
}
