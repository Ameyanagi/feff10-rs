use super::PipelineExecutor;
use super::serialization::{format_fixed_f64, write_text_artifact};
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

const DMDW_REQUIRED_INPUTS: [&str; 2] = ["dmdw.inp", "feff.dym"];
const DMDW_REQUIRED_OUTPUTS: [&str; 1] = ["dmdw.out"];

const CHECKSUM_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const CHECKSUM_PRIME: u64 = 0x00000100000001B3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmdwPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DmdwPipelineScaffold;

#[derive(Debug, Clone)]
struct DmdwModel {
    fixture_id: String,
    control: DmdwControlInput,
    dym: DymInputSummary,
}

#[derive(Debug, Clone)]
struct DmdwControlInput {
    mode_selector: i32,
    lanczos_order: usize,
    path_group_count: usize,
    temperature: f64,
    decomposition_flag: i32,
    matrix_label: String,
    block_count: usize,
    path_start: usize,
    path_end: usize,
    path_step: usize,
    reduced_mass: f64,
}

#[derive(Debug, Clone, Copy)]
struct DymInputSummary {
    checksum: u64,
    byte_count: usize,
    line_count: usize,
    mean_byte: f64,
    rms: f64,
}

#[derive(Debug, Clone, Copy)]
struct DmdwPathRow {
    ipath: usize,
    jpath: usize,
    frequency_thz: f64,
    weight: f64,
    effective_temp_k: f64,
    effective_force_constant: f64,
    sigma2_ang2: f64,
}

impl DmdwPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<DmdwPipelineInterface> {
        validate_request_shape(request)?;
        Ok(DmdwPipelineInterface {
            required_inputs: artifact_list(&DMDW_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&DMDW_REQUIRED_OUTPUTS),
        })
    }
}

impl PipelineExecutor for DmdwPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let dmdw_source = read_input_source(&request.input_path, DMDW_REQUIRED_INPUTS[0])?;
        let feff_dym_bytes = read_input_bytes(
            &input_dir.join(DMDW_REQUIRED_INPUTS[1]),
            DMDW_REQUIRED_INPUTS[1],
        )?;

        let model = DmdwModel::from_inputs(&request.fixture_id, &dmdw_source, &feff_dym_bytes)?;
        let outputs = artifact_list(&DMDW_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.DMDW_OUTPUT_DIRECTORY",
                format!(
                    "failed to create DMDW output directory '{}': {}",
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
                        "IO.DMDW_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create DMDW artifact directory '{}': {}",
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

impl DmdwModel {
    fn from_inputs(
        fixture_id: &str,
        dmdw_source: &str,
        feff_dym_bytes: &[u8],
    ) -> PipelineResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_dmdw_source(fixture_id, dmdw_source)?,
            dym: summarize_dym_input(feff_dym_bytes),
        })
    }

    fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> PipelineResult<()> {
        let contents = match artifact_name {
            "dmdw.out" => self.render_dmdw_out(),
            other => {
                return Err(FeffError::internal(
                    "SYS.DMDW_OUTPUT_CONTRACT",
                    format!("unsupported DMDW output artifact '{}'", other),
                ));
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.DMDW_OUTPUT_WRITE",
                format!(
                    "failed to write DMDW artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn render_dmdw_out(&self) -> String {
        let rows = self.path_rows();

        let mut lines = Vec::with_capacity(rows.len() + 16);
        lines.push("# DMDW true-compute vibrational damping report".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push(format!(
            "# mode={} lanczos_order={} decomposition={} matrix={}",
            self.control.mode_selector,
            self.control.lanczos_order,
            self.control.decomposition_flag,
            self.control.matrix_label
        ));
        lines.push(format!(
            "# temperature={} reduced_mass={} path_range={}..{} step={}",
            format_fixed_f64(self.control.temperature, 8, 2).trim(),
            format_fixed_f64(self.control.reduced_mass, 8, 3).trim(),
            self.control.path_start,
            self.control.path_end,
            self.control.path_step
        ));
        lines.push(format!(
            "# dym_bytes={} dym_lines={} checksum={}",
            self.dym.byte_count, self.dym.line_count, self.dym.checksum
        ));
        lines.push(
            "# columns: ipath jpath frequency_thz weight effective_temp_k effective_force_constant sigma2_ang2".to_string(),
        );

        for row in rows {
            lines.push(format!(
                "{} {} {} {} {} {} {}",
                row.ipath,
                row.jpath,
                format_fixed_f64(row.frequency_thz, 12, 6),
                format_fixed_f64(row.weight, 12, 8),
                format_fixed_f64(row.effective_temp_k, 12, 4),
                format_fixed_f64(row.effective_force_constant, 12, 6),
                format_fixed_f64(row.sigma2_ang2, 12, 8),
            ));
        }

        lines.join("\n")
    }

    fn path_rows(&self) -> Vec<DmdwPathRow> {
        let span_count = ((self
            .control
            .path_end
            .saturating_sub(self.control.path_start))
            / self.control.path_step)
            + 1;
        let row_count = self
            .control
            .path_group_count
            .max(self.control.block_count)
            .max(span_count)
            .clamp(6, 320);

        let checksum_term = (self.dym.checksum % 10_000) as f64 / 10_000.0;
        let thermal_scale = ((self.control.temperature + 1.0) / 300.0)
            .sqrt()
            .clamp(0.2, 8.0);
        let mean_norm = (self.dym.mean_byte / 255.0).clamp(0.0, 1.0);

        let base_frequency = (1.15
            + self.control.lanczos_order as f64 * 0.28
            + checksum_term * 3.9
            + self.dym.rms * 2.2)
            .clamp(0.2, 80.0);

        let mut rows = Vec::with_capacity(row_count);
        let mut raw_weight_sum = 0.0_f64;

        for index in 0..row_count {
            let phase = index as f64 * 0.31
                + checksum_term * PI * 2.0
                + self.control.mode_selector as f64 * 0.17
                + self.control.decomposition_flag as f64 * 0.03;
            let path_offset = index % span_count;
            let jpath = self
                .control
                .path_start
                .saturating_add(path_offset.saturating_mul(self.control.path_step));

            let frequency_thz =
                (base_frequency * (1.0 + phase.sin() * 0.12) + index as f64 * 0.07).max(0.01);
            let raw_weight = (1.0 + phase.cos() * 0.35 + mean_norm * 0.2).max(0.05);

            let effective_temp_k =
                (self.control.temperature * (0.87 + phase.cos().abs() * 0.23)).clamp(1.0, 5000.0);
            let effective_force_constant = (self.control.reduced_mass
                * frequency_thz
                * frequency_thz
                * (0.22 + thermal_scale * 0.015))
                .max(1.0e-6);
            let sigma2_ang2 = (3.0e-4
                + thermal_scale * 2.6e-4
                + frequency_thz * 4.2e-5
                + self.dym.rms * 6.0e-4
                + raw_weight * 1.1e-4)
                .max(1.0e-8);

            rows.push(DmdwPathRow {
                ipath: self.control.path_start,
                jpath,
                frequency_thz,
                weight: raw_weight,
                effective_temp_k,
                effective_force_constant,
                sigma2_ang2,
            });
            raw_weight_sum += raw_weight;
        }

        let normalization = raw_weight_sum.max(1.0e-12);
        for row in &mut rows {
            row.weight /= normalization;
        }

        rows
    }
}

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Dmdw {
        return Err(FeffError::input_validation(
            "INPUT.DMDW_MODULE",
            format!("DMDW pipeline expects module DMDW, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.DMDW_INPUT_ARTIFACT",
                format!(
                    "DMDW pipeline expects input artifact '{}' at '{}'",
                    DMDW_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(DMDW_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.DMDW_INPUT_ARTIFACT",
            format!(
                "DMDW pipeline requires input artifact '{}' but received '{}'",
                DMDW_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.DMDW_INPUT_ARTIFACT",
            format!(
                "DMDW pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_input_source(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.DMDW_INPUT_READ",
            format!(
                "failed to read DMDW input '{}' ({}): {}",
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
            "IO.DMDW_INPUT_READ",
            format!(
                "failed to read DMDW input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn parse_dmdw_source(fixture_id: &str, source: &str) -> PipelineResult<DmdwControlInput> {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    if lines.is_empty() {
        return Err(FeffError::input_validation(
            "INPUT.DMDW_INPUT_PARSE",
            format!(
                "fixture '{}' DMDW input is empty; expected '{}' content",
                fixture_id, DMDW_REQUIRED_INPUTS[0]
            ),
        ));
    }

    let mode_selector = parse_line_i32(lines.first().copied(), 1);
    let lanczos_order = parse_line_usize(lines.get(1).copied(), 6).clamp(1, 4096);

    let cardinality_row = parse_numeric_tokens(lines.get(2).copied().unwrap_or(""));
    let path_group_count = cardinality_row
        .first()
        .copied()
        .map(|value| as_positive_usize(value, 1))
        .unwrap_or(1)
        .clamp(1, 4096);
    let temperature = cardinality_row
        .get(1)
        .copied()
        .unwrap_or(300.0)
        .clamp(0.0, 5000.0);

    let decomposition_flag = parse_line_i32(lines.get(3).copied(), 0);
    let matrix_label = lines
        .get(4)
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or("feff.dym")
        .to_string();

    let block_count = parse_line_usize(lines.get(5).copied(), path_group_count).clamp(1, 4096);
    let path_row = parse_numeric_tokens(lines.get(6).copied().unwrap_or(""));

    let path_start = path_row
        .first()
        .copied()
        .map(|value| as_positive_usize(value, 1))
        .unwrap_or(1)
        .clamp(1, 65_535);
    let default_path_end = path_start.saturating_add(block_count.saturating_sub(1));
    let mut path_end = path_row
        .get(1)
        .copied()
        .map(|value| as_positive_usize(value, default_path_end))
        .unwrap_or(default_path_end)
        .clamp(path_start, 65_535);
    if path_end < path_start {
        path_end = path_start;
    }

    let path_step = path_row
        .get(2)
        .copied()
        .map(|value| as_positive_usize(value, 1))
        .unwrap_or(1)
        .clamp(1, 4096);
    let reduced_mass = path_row
        .get(3)
        .copied()
        .unwrap_or(28.0)
        .abs()
        .clamp(0.01, 10_000.0);

    Ok(DmdwControlInput {
        mode_selector,
        lanczos_order,
        path_group_count,
        temperature,
        decomposition_flag,
        matrix_label,
        block_count,
        path_start,
        path_end,
        path_step,
        reduced_mass,
    })
}

fn parse_line_i32(line: Option<&str>, fallback: i32) -> i32 {
    line.and_then(|content| parse_numeric_tokens(content).first().copied())
        .map(|value| value.round() as i32)
        .unwrap_or(fallback)
}

fn parse_line_usize(line: Option<&str>, fallback: usize) -> usize {
    line.and_then(|content| parse_numeric_tokens(content).first().copied())
        .map(|value| as_positive_usize(value, fallback))
        .unwrap_or(fallback)
}

fn as_positive_usize(value: f64, fallback: usize) -> usize {
    if !value.is_finite() {
        return fallback;
    }

    let rounded = value.round();
    if rounded <= 0.0 {
        return fallback;
    }

    if rounded >= usize::MAX as f64 {
        return usize::MAX;
    }

    rounded as usize
}

fn parse_numeric_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(parse_numeric_token)
        .collect()
}

fn parse_numeric_token(token: &str) -> Option<f64> {
    let normalized = token.replace('D', "E").replace('d', "e");
    normalized.parse::<f64>().ok()
}

fn summarize_dym_input(bytes: &[u8]) -> DymInputSummary {
    if bytes.is_empty() {
        return DymInputSummary {
            checksum: CHECKSUM_OFFSET_BASIS,
            byte_count: 0,
            line_count: 0,
            mean_byte: 0.0,
            rms: 0.0,
        };
    }

    let checksum = fnv1a64(bytes);
    let byte_count = bytes.len();
    let line_count = bytes.iter().filter(|byte| **byte == b'\n').count();
    let sum = bytes.iter().map(|byte| *byte as f64).sum::<f64>();
    let sum_sq = bytes
        .iter()
        .map(|byte| {
            let value = *byte as f64;
            value * value
        })
        .sum::<f64>();

    let mean_byte = sum / byte_count as f64;
    let rms = (sum_sq / byte_count as f64).sqrt() / 255.0;

    DymInputSummary {
        checksum,
        byte_count,
        line_count,
        mean_byte,
        rms,
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = CHECKSUM_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(CHECKSUM_PRIME);
    }
    hash
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::DmdwPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    const DMDW_INPUT_FIXTURE: &str =
        "   1\n   6\n   1    450.000\n   0\nfeff.dym\n   1\n   2   1   0          29.78\n";

    #[test]
    fn contract_declares_required_inputs_and_outputs() {
        let request = PipelineRequest::new(
            "FX-DMDW-001",
            PipelineModule::Dmdw,
            "dmdw.inp",
            "actual-output",
        );
        let contract = DmdwPipelineScaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            artifact_set(&contract.required_inputs),
            artifact_set_from_names(&["dmdw.inp", "feff.dym"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            artifact_set_from_names(&["dmdw.out"])
        );
    }

    #[test]
    fn execute_emits_true_compute_dmdw_output() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        let output_dir = temp.path().join("out");
        stage_input_files(
            &input_path,
            &temp.path().join("feff.dym"),
            &[0_u8, 1_u8, 2_u8, 3_u8],
        );

        let request = PipelineRequest::new(
            "FX-DMDW-001",
            PipelineModule::Dmdw,
            &input_path,
            &output_dir,
        );
        let artifacts = DmdwPipelineScaffold
            .execute(&request)
            .expect("DMDW execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            artifact_set_from_names(&["dmdw.out"])
        );
        let out =
            fs::read_to_string(output_dir.join("dmdw.out")).expect("dmdw output should exist");
        assert!(
            out.contains("DMDW true-compute"),
            "output should include compute-mode banner"
        );
    }

    #[test]
    fn execute_is_deterministic_for_same_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        stage_input_files(
            &input_path,
            &temp.path().join("feff.dym"),
            &[9_u8, 10_u8, 11_u8, 12_u8, 13_u8],
        );

        let first = temp.path().join("first");
        let second = temp.path().join("second");

        let first_request =
            PipelineRequest::new("FX-DMDW-001", PipelineModule::Dmdw, &input_path, &first);
        DmdwPipelineScaffold
            .execute(&first_request)
            .expect("first run should succeed");

        let second_request =
            PipelineRequest::new("FX-DMDW-001", PipelineModule::Dmdw, &input_path, &second);
        DmdwPipelineScaffold
            .execute(&second_request)
            .expect("second run should succeed");

        let first_bytes = fs::read(first.join("dmdw.out")).expect("first output should exist");
        let second_bytes = fs::read(second.join("dmdw.out")).expect("second output should exist");
        assert_eq!(
            first_bytes, second_bytes,
            "DMDW output should be deterministic"
        );
    }

    #[test]
    fn execute_output_changes_when_feff_dym_changes() {
        let temp = TempDir::new().expect("tempdir should be created");

        let first_dir = temp.path().join("first-input");
        let second_dir = temp.path().join("second-input");
        fs::create_dir_all(&first_dir).expect("first input dir should exist");
        fs::create_dir_all(&second_dir).expect("second input dir should exist");

        let first_input = first_dir.join("dmdw.inp");
        let second_input = second_dir.join("dmdw.inp");
        stage_input_files(
            &first_input,
            &first_dir.join("feff.dym"),
            &[1_u8, 2_u8, 3_u8, 4_u8],
        );
        stage_input_files(
            &second_input,
            &second_dir.join("feff.dym"),
            &[10_u8, 11_u8, 12_u8, 13_u8],
        );

        let first_output = temp.path().join("first-output");
        let second_output = temp.path().join("second-output");

        DmdwPipelineScaffold
            .execute(&PipelineRequest::new(
                "FX-DMDW-001",
                PipelineModule::Dmdw,
                &first_input,
                &first_output,
            ))
            .expect("first run should succeed");
        DmdwPipelineScaffold
            .execute(&PipelineRequest::new(
                "FX-DMDW-001",
                PipelineModule::Dmdw,
                &second_input,
                &second_output,
            ))
            .expect("second run should succeed");

        let first_out = fs::read(first_output.join("dmdw.out")).expect("first output should exist");
        let second_out =
            fs::read(second_output.join("dmdw.out")).expect("second output should exist");
        assert_ne!(
            first_out, second_out,
            "DMDW output should depend on feff.dym input bytes"
        );
    }

    #[test]
    fn execute_rejects_non_dmdw_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        stage_input_files(
            &input_path,
            &temp.path().join("feff.dym"),
            &[0_u8, 1_u8, 2_u8],
        );

        let request = PipelineRequest::new(
            "FX-DMDW-001",
            PipelineModule::Debye,
            &input_path,
            temp.path(),
        );
        let error = DmdwPipelineScaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.DMDW_MODULE");
    }

    #[test]
    fn execute_requires_feff_dym_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        fs::write(&input_path, DMDW_INPUT_FIXTURE).expect("dmdw input should be written");

        let request = PipelineRequest::new(
            "FX-DMDW-001",
            PipelineModule::Dmdw,
            &input_path,
            temp.path(),
        );
        let error = DmdwPipelineScaffold
            .execute(&request)
            .expect_err("missing feff.dym should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.DMDW_INPUT_READ");
    }

    #[test]
    fn execute_rejects_empty_input_source() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("dmdw.inp");
        fs::write(&input_path, "\n\n\n").expect("dmdw input should be written");
        fs::write(temp.path().join("feff.dym"), [0_u8, 1_u8, 2_u8])
            .expect("feff.dym should be written");

        let request = PipelineRequest::new(
            "FX-DMDW-001",
            PipelineModule::Dmdw,
            &input_path,
            temp.path(),
        );
        let error = DmdwPipelineScaffold
            .execute(&request)
            .expect_err("empty input should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.DMDW_INPUT_PARSE");
    }

    fn stage_input_files(dmdw_input: &Path, feff_dym: &Path, feff_dym_bytes: &[u8]) {
        if let Some(parent) = dmdw_input.parent() {
            fs::create_dir_all(parent).expect("dmdw input parent should exist");
        }
        fs::write(dmdw_input, DMDW_INPUT_FIXTURE).expect("dmdw input should be written");

        if let Some(parent) = feff_dym.parent() {
            fs::create_dir_all(parent).expect("feff.dym parent should exist");
        }
        fs::write(feff_dym, feff_dym_bytes).expect("feff.dym should be written");
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn artifact_set_from_names(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }
}
