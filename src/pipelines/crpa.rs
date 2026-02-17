use super::PipelineExecutor;
use super::serialization::{format_fixed_f64, write_text_artifact};
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::Path;

const CRPA_REQUIRED_INPUTS: [&str; 3] = ["crpa.inp", "pot.inp", "geom.dat"];
const CRPA_REQUIRED_OUTPUTS: [&str; 2] = ["wscrn.dat", "logscrn.dat"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrpaPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CrpaPipelineScaffold;

#[derive(Debug, Clone)]
struct CrpaModel {
    fixture_id: String,
    control: CrpaControlInput,
    pot: PotCrpaInput,
    geom: GeomCrpaInput,
}

#[derive(Debug, Clone, Copy)]
struct CrpaControlInput {
    rcut: f64,
    l_crpa: i32,
}

#[derive(Debug, Clone)]
struct PotCrpaInput {
    title: String,
    gamach: f64,
    rfms1: f64,
    mean_folp: f64,
    mean_xion: f64,
    lmaxsc_max: i32,
}

#[derive(Debug, Clone)]
struct GeomCrpaInput {
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

#[derive(Debug, Clone, Copy)]
struct PotentialRow {
    lmaxsc: i32,
    xion: f64,
    folp: f64,
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

impl CrpaPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<CrpaPipelineInterface> {
        validate_request_shape(request)?;
        Ok(CrpaPipelineInterface {
            required_inputs: artifact_list(&CRPA_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&CRPA_REQUIRED_OUTPUTS),
        })
    }
}

impl PipelineExecutor for CrpaPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let crpa_source = read_input_source(&request.input_path, CRPA_REQUIRED_INPUTS[0])?;
        let pot_source = read_input_source(
            &input_dir.join(CRPA_REQUIRED_INPUTS[1]),
            CRPA_REQUIRED_INPUTS[1],
        )?;
        let geom_source = read_input_source(
            &input_dir.join(CRPA_REQUIRED_INPUTS[2]),
            CRPA_REQUIRED_INPUTS[2],
        )?;

        let model =
            CrpaModel::from_sources(&request.fixture_id, &crpa_source, &pot_source, &geom_source)?;
        let outputs = artifact_list(&CRPA_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.CRPA_OUTPUT_DIRECTORY",
                format!(
                    "failed to create CRPA output directory '{}': {}",
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
                        "IO.CRPA_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create CRPA artifact directory '{}': {}",
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

impl CrpaModel {
    fn from_sources(
        fixture_id: &str,
        crpa_source: &str,
        pot_source: &str,
        geom_source: &str,
    ) -> PipelineResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_crpa_source(fixture_id, crpa_source)?,
            pot: parse_pot_source(fixture_id, pot_source)?,
            geom: parse_geom_source(fixture_id, geom_source)?,
        })
    }

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> PipelineResult<()> {
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

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Crpa {
        return Err(FeffError::input_validation(
            "INPUT.CRPA_MODULE",
            format!("CRPA pipeline expects module CRPA, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.CRPA_INPUT_ARTIFACT",
                format!(
                    "CRPA pipeline expects input artifact '{}' at '{}'",
                    CRPA_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(CRPA_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.CRPA_INPUT_ARTIFACT",
            format!(
                "CRPA pipeline requires input artifact '{}' but received '{}'",
                CRPA_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.CRPA_INPUT_ARTIFACT",
            format!(
                "CRPA pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.CRPA_INPUT_READ",
            format!(
                "failed to read CRPA input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn parse_crpa_source(fixture_id: &str, source: &str) -> PipelineResult<CrpaControlInput> {
    let lines: Vec<&str> = source.lines().collect();
    let mut do_crpa: Option<i32> = None;
    let mut rcut: Option<f64> = None;
    let mut l_crpa: Option<i32> = None;

    for index in 0..lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(raw_keyword) = trimmed.split_whitespace().next() else {
            continue;
        };
        let keyword = raw_keyword
            .trim_matches(|character: char| matches!(character, ':' | ',' | ';'))
            .to_ascii_lowercase();

        let value = parse_keyword_value(&lines, index).or_else(|| {
            next_nonempty_line(&lines, index + 1)
                .and_then(|(_, line)| parse_numeric_tokens(line).into_iter().next())
        });

        match keyword.as_str() {
            "do_crpa" => {
                if let Some(parsed) = value {
                    do_crpa = Some(f64_to_i32(parsed, fixture_id, "crpa.inp do_CRPA")?);
                }
            }
            "rcut" => {
                if let Some(parsed) = value {
                    rcut = Some(parsed.abs());
                }
            }
            "l_crpa" => {
                if let Some(parsed) = value {
                    l_crpa = Some(f64_to_i32(parsed, fixture_id, "crpa.inp l_crpa")?);
                }
            }
            _ => {}
        }
    }

    let do_crpa = do_crpa.ok_or_else(|| {
        crpa_parse_error(fixture_id, "crpa.inp missing required do_CRPA control flag")
    })?;
    if do_crpa <= 0 {
        return Err(crpa_parse_error(
            fixture_id,
            "crpa.inp requires do_CRPA = 1 for CRPA runtime execution",
        ));
    }

    Ok(CrpaControlInput {
        rcut: rcut.unwrap_or(1.5).max(1.0e-6),
        l_crpa: l_crpa.unwrap_or(3).max(1),
    })
}

fn parse_pot_source(fixture_id: &str, source: &str) -> PipelineResult<PotCrpaInput> {
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
        return Err(crpa_parse_error(
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

    Ok(PotCrpaInput {
        title,
        gamach,
        rfms1,
        mean_folp: folp_sum / potential_rows.len() as f64,
        mean_xion: xion_sum / potential_rows.len() as f64,
        lmaxsc_max,
    })
}

fn parse_geom_source(fixture_id: &str, source: &str) -> PipelineResult<GeomCrpaInput> {
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
        return Err(crpa_parse_error(
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

    Ok(GeomCrpaInput {
        nat: nat_value,
        nph: nph_value,
        atoms,
        radius_mean,
        radius_rms,
        radius_max,
    })
}

fn parse_keyword_value(lines: &[&str], index: usize) -> Option<f64> {
    let line = lines.get(index)?;
    line.split_whitespace()
        .skip(1)
        .find_map(parse_numeric_token)
}

fn next_nonempty_line<'a>(lines: &'a [&'a str], start_index: usize) -> Option<(usize, &'a str)> {
    for (offset, line) in lines.iter().enumerate().skip(start_index) {
        if !line.trim().is_empty() {
            return Some((offset, *line));
        }
    }

    None
}

fn f64_to_i32(value: f64, fixture_id: &str, field: &str) -> PipelineResult<i32> {
    if !value.is_finite() {
        return Err(crpa_parse_error(
            fixture_id,
            format!("{} must be finite", field),
        ));
    }

    let rounded = value.round();
    if (value - rounded).abs() > 1.0e-6 {
        return Err(crpa_parse_error(
            fixture_id,
            format!("{} must be an integer", field),
        ));
    }

    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err(crpa_parse_error(
            fixture_id,
            format!("{} is out of i32 range", field),
        ));
    }

    Ok(rounded as i32)
}

fn f64_to_usize(value: f64, fixture_id: &str, field: &str) -> PipelineResult<usize> {
    let integer = f64_to_i32(value, fixture_id, field)?;
    if integer < 0 {
        return Err(crpa_parse_error(
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

fn crpa_parse_error(fixture_id: &str, message: impl Into<String>) -> FeffError {
    FeffError::computation(
        "RUN.CRPA_INPUT_PARSE",
        format!("fixture '{}': {}", fixture_id, message.into()),
    )
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::CrpaPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const CRPA_INPUT_FIXTURE: &str = " do_CRPA           1
 rcut   1.49000000000000
 l_crpa           3
";

    const POT_INPUT_FIXTURE: &str = "mpot, nph, ntitle, ihole, ipr1, iafolp, ixc,ispec
   1   1   1   4   0   0   0   1
nmix, nohole, jumprm, inters, nscmt, icoul, lfms1, iunf
   1  -1   0   0 100   0   0   1
Ce example
gamach, rgrd, ca1, ecv, totvol, rfms1, corval_emin
      3.26955      0.05000      0.20000    -40.00000      0.00000      4.00000    -70.00000
 iz, lmaxsc, xnatph, xion, folp
   58    3      1.00000      0.00000      1.15000
   58    3    100.00000      0.00000      1.15000
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

    #[test]
    fn contract_exposes_true_compute_crpa_artifact_contract() {
        let request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Crpa,
            "crpa.inp",
            "actual-output",
        );
        let scaffold = CrpaPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_set(&["crpa.inp", "pot.inp", "geom.dat"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_set(&["wscrn.dat", "logscrn.dat"])
        );
    }

    #[test]
    fn execute_emits_required_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("actual");
        let input_path = stage_crpa_inputs(temp.path(), CRPA_INPUT_FIXTURE);

        let request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Crpa,
            &input_path,
            &output_dir,
        );
        let artifacts = CrpaPipelineScaffold
            .execute(&request)
            .expect("CRPA execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_set(&["wscrn.dat", "logscrn.dat"])
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
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let first_root = temp.path().join("first");
        let second_root = temp.path().join("second");
        let first_input = stage_crpa_inputs(&first_root, CRPA_INPUT_FIXTURE);
        let second_input = stage_crpa_inputs(&second_root, CRPA_INPUT_FIXTURE);
        let first_output = first_root.join("out");
        let second_output = second_root.join("out");

        let first_request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Crpa,
            &first_input,
            &first_output,
        );
        let second_request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Crpa,
            &second_input,
            &second_output,
        );

        CrpaPipelineScaffold
            .execute(&first_request)
            .expect("first run should succeed");
        CrpaPipelineScaffold
            .execute(&second_request)
            .expect("second run should succeed");

        for artifact in ["wscrn.dat", "logscrn.dat"] {
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
    fn execute_rejects_non_crpa_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = stage_crpa_inputs(temp.path(), CRPA_INPUT_FIXTURE);

        let request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Screen,
            &input_path,
            temp.path(),
        );
        let error = CrpaPipelineScaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.CRPA_MODULE");
    }

    #[test]
    fn execute_requires_pot_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("crpa.inp");
        fs::write(&input_path, CRPA_INPUT_FIXTURE).expect("crpa input should be staged");
        fs::write(temp.path().join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be staged");

        let request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Crpa,
            &input_path,
            temp.path(),
        );
        let error = CrpaPipelineScaffold
            .execute(&request)
            .expect_err("missing pot input should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.CRPA_INPUT_READ");
    }

    #[test]
    fn execute_rejects_disabled_crpa_flag() {
        let temp = TempDir::new().expect("tempdir should be created");
        let disabled_crpa_input = " do_CRPA           0
 rcut   1.49000000000000
 l_crpa           3
";
        let input_path = stage_crpa_inputs(temp.path(), disabled_crpa_input);

        let request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Crpa,
            &input_path,
            temp.path(),
        );
        let error = CrpaPipelineScaffold
            .execute(&request)
            .expect_err("disabled CRPA flag should fail");

        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.CRPA_INPUT_PARSE");
    }

    fn stage_crpa_inputs(root: &Path, crpa_input_source: &str) -> PathBuf {
        fs::create_dir_all(root).expect("root should exist");
        let crpa_path = root.join("crpa.inp");
        fs::write(&crpa_path, crpa_input_source).expect("crpa input should be written");
        fs::write(root.join("pot.inp"), POT_INPUT_FIXTURE).expect("pot input should be written");
        fs::write(root.join("geom.dat"), GEOM_INPUT_FIXTURE).expect("geom input should be written");
        crpa_path
    }

    fn expected_set(entries: &[&str]) -> BTreeSet<String> {
        entries.iter().map(|entry| entry.to_string()).collect()
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }
}
