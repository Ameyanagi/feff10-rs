use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use std::fs;
use std::path::Path;

const FMS_REQUIRED_INPUTS: [&str; 4] = ["fms.inp", "geom.dat", "global.inp", "phase.bin"];
const FMS_EXPECTED_OUTPUTS: [&str; 2] = ["gg.bin", "log3.dat"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmsPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FmsPipelineScaffold;

impl FmsPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<FmsPipelineInterface> {
        validate_request_shape(request)?;
        Ok(FmsPipelineInterface {
            required_inputs: artifact_list(&FMS_REQUIRED_INPUTS),
            expected_outputs: artifact_list(&FMS_EXPECTED_OUTPUTS),
        })
    }
}

impl PipelineExecutor for FmsPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        let contract = self.contract_for_request(request)?;
        let input_dir = input_parent_dir(request)?;

        let fms_source = read_text_input(&request.input_path, FMS_REQUIRED_INPUTS[0])?;
        let geom_source = read_text_input(
            &input_dir.join(FMS_REQUIRED_INPUTS[1]),
            FMS_REQUIRED_INPUTS[1],
        )?;
        let global_source = read_text_input(
            &input_dir.join(FMS_REQUIRED_INPUTS[2]),
            FMS_REQUIRED_INPUTS[2],
        )?;
        let phase_bytes = read_binary_input(
            &input_dir.join(FMS_REQUIRED_INPUTS[3]),
            FMS_REQUIRED_INPUTS[3],
        )?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.FMS_OUTPUT_DIRECTORY",
                format!(
                    "failed to create FMS output directory '{}': {}",
                    request.output_dir.display(),
                    source
                ),
            )
        })?;

        let gg_bin_path = request.output_dir.join(FMS_EXPECTED_OUTPUTS[0]);
        fs::write(
            &gg_bin_path,
            build_gg_bin_placeholder(
                &request.fixture_id,
                &fms_source,
                &geom_source,
                &global_source,
                &phase_bytes,
            ),
        )
        .map_err(|source| {
            FeffError::io_system(
                "IO.FMS_OUTPUT_WRITE",
                format!(
                    "failed to write FMS placeholder artifact '{}': {}",
                    gg_bin_path.display(),
                    source
                ),
            )
        })?;

        let log3_path = request.output_dir.join(FMS_EXPECTED_OUTPUTS[1]);
        fs::write(
            &log3_path,
            build_log3_placeholder(
                &request.fixture_id,
                &fms_source,
                &geom_source,
                &global_source,
                &phase_bytes,
            ),
        )
        .map_err(|source| {
            FeffError::io_system(
                "IO.FMS_OUTPUT_WRITE",
                format!(
                    "failed to write FMS placeholder artifact '{}': {}",
                    log3_path.display(),
                    source
                ),
            )
        })?;

        Ok(contract.expected_outputs)
    }
}

fn validate_request_shape(request: &PipelineRequest) -> PipelineResult<()> {
    if request.module != PipelineModule::Fms {
        return Err(FeffError::input_validation(
            "INPUT.FMS_MODULE",
            format!("FMS pipeline expects module FMS, got {}", request.module),
        ));
    }

    let input_file_name = request
        .input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FeffError::input_validation(
                "INPUT.FMS_INPUT_ARTIFACT",
                format!(
                    "FMS pipeline expects input artifact '{}' at '{}'",
                    FMS_REQUIRED_INPUTS[0],
                    request.input_path.display()
                ),
            )
        })?;

    if !input_file_name.eq_ignore_ascii_case(FMS_REQUIRED_INPUTS[0]) {
        return Err(FeffError::input_validation(
            "INPUT.FMS_INPUT_ARTIFACT",
            format!(
                "FMS pipeline requires input artifact '{}' but received '{}'",
                FMS_REQUIRED_INPUTS[0], input_file_name
            ),
        ));
    }

    Ok(())
}

fn input_parent_dir(request: &PipelineRequest) -> PipelineResult<&Path> {
    request.input_path.parent().ok_or_else(|| {
        FeffError::input_validation(
            "INPUT.FMS_INPUT_ARTIFACT",
            format!(
                "FMS pipeline requires sibling inputs next to '{}'",
                request.input_path.display()
            ),
        )
    })
}

fn read_text_input(path: &Path, artifact_name: &str) -> PipelineResult<String> {
    fs::read_to_string(path).map_err(|source| {
        FeffError::io_system(
            "IO.FMS_INPUT_READ",
            format!(
                "failed to read FMS input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn read_binary_input(path: &Path, artifact_name: &str) -> PipelineResult<Vec<u8>> {
    fs::read(path).map_err(|source| {
        FeffError::io_system(
            "IO.FMS_INPUT_READ",
            format!(
                "failed to read FMS input '{}' ({}): {}",
                path.display(),
                artifact_name,
                source
            ),
        )
    })
}

fn build_gg_bin_placeholder(
    fixture_id: &str,
    fms_source: &str,
    geom_source: &str,
    global_source: &str,
    phase_bytes: &[u8],
) -> Vec<u8> {
    format!(
        "FMS_SCAFFOLD\nfixture={}\nfms_checksum={:016x}\ngeom_checksum={:016x}\nglobal_checksum={:016x}\nphase_checksum={:016x}\nphase_bytes={}\n",
        fixture_id,
        rolling_checksum(fms_source.as_bytes()),
        rolling_checksum(geom_source.as_bytes()),
        rolling_checksum(global_source.as_bytes()),
        rolling_checksum(phase_bytes),
        phase_bytes.len()
    )
    .into_bytes()
}

fn build_log3_placeholder(
    fixture_id: &str,
    fms_source: &str,
    geom_source: &str,
    global_source: &str,
    phase_bytes: &[u8],
) -> String {
    format!(
        "FMS scaffold placeholder execution\nfixture: {}\nrequired_inputs: {}, {}, {}, {}\nfms_lines: {}\ngeom_lines: {}\nglobal_lines: {}\nphase_bytes: {}\n",
        fixture_id,
        FMS_REQUIRED_INPUTS[0],
        FMS_REQUIRED_INPUTS[1],
        FMS_REQUIRED_INPUTS[2],
        FMS_REQUIRED_INPUTS[3],
        non_empty_line_count(fms_source),
        non_empty_line_count(geom_source),
        non_empty_line_count(global_source),
        phase_bytes.len()
    )
}

fn rolling_checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0u64, |state, byte| {
        state
            .wrapping_mul(1_099_511_628_211)
            .wrapping_add((*byte as u64) + 0x9E37)
    })
}

fn non_empty_line_count(content: &str) -> usize {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::FmsPipelineScaffold;
    use crate::domain::{FeffErrorCategory, PipelineArtifact, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn contract_matches_compatibility_matrix_interfaces() {
        let request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Fms,
            "fixtures/FX-FMS-001/fms.inp",
            "out",
        );

        let contract = FmsPipelineScaffold
            .contract_for_request(&request)
            .expect("contract should resolve");
        assert_eq!(
            artifact_set(&contract.required_inputs),
            expected_set(&["fms.inp", "geom.dat", "global.inp", "phase.bin"])
        );
        assert_eq!(
            artifact_set(&contract.expected_outputs),
            expected_set(&["gg.bin", "log3.dat"])
        );
    }

    #[test]
    fn execute_writes_placeholder_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        let fixture_dir = temp.path().join("fixture");
        let output_dir = temp.path().join("out");

        fs::create_dir_all(&fixture_dir).expect("fixture dir should exist");
        fs::write(fixture_dir.join("fms.inp"), "FMS 7.5\nNLEG 8\nRCLUST 8.0\n")
            .expect("fms input should be written");
        fs::write(fixture_dir.join("geom.dat"), "nat, nph =     1    1\n")
            .expect("geom input should be written");
        fs::write(
            fixture_dir.join("global.inp"),
            "nabs, iphabs\n1 0 100000.00000\n",
        )
        .expect("global input should be written");
        fs::write(fixture_dir.join("phase.bin"), [0xFA_u8, 0xCE_u8, 0x10_u8])
            .expect("phase input should be written");

        let request = PipelineRequest::new(
            "FX-FMS-TEST-001",
            PipelineModule::Fms,
            fixture_dir.join("fms.inp"),
            &output_dir,
        );

        let artifacts = FmsPipelineScaffold
            .execute(&request)
            .expect("execution should succeed");

        assert_eq!(
            artifact_set(&artifacts),
            expected_set(&["gg.bin", "log3.dat"])
        );
        assert!(output_dir.join("gg.bin").is_file());
        assert!(output_dir.join("log3.dat").is_file());

        let log3_source =
            fs::read_to_string(output_dir.join("log3.dat")).expect("log3 output should be text");
        assert!(log3_source.contains("fixture: FX-FMS-TEST-001"));
        assert!(log3_source.contains("required_inputs: fms.inp, geom.dat, global.inp, phase.bin"));
    }

    #[test]
    fn execute_requires_phase_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        let fixture_dir = temp.path().join("fixture");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should exist");
        fs::write(fixture_dir.join("fms.inp"), "FMS 8.0\n").expect("fms input");
        fs::write(fixture_dir.join("geom.dat"), "nat, nph = 1 1\n").expect("geom input");
        fs::write(fixture_dir.join("global.inp"), "nabs, iphabs\n").expect("global input");

        let request = PipelineRequest::new(
            "FX-FMS-TEST-002",
            PipelineModule::Fms,
            fixture_dir.join("fms.inp"),
            temp.path().join("out"),
        );

        let error = FmsPipelineScaffold
            .execute(&request)
            .expect_err("execution should fail when phase.bin is missing");
        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.FMS_INPUT_READ");
    }

    #[test]
    fn contract_rejects_non_fms_module() {
        let request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Path,
            "fixtures/FX-FMS-001/fms.inp",
            "out",
        );

        let error = FmsPipelineScaffold
            .contract_for_request(&request)
            .expect_err("contract should reject wrong module");
        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.FMS_MODULE");
    }

    fn artifact_set(artifacts: &[PipelineArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn expected_set(artifacts: &[&str]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.to_string())
            .collect()
    }
}
