use super::PipelineExecutor;
use crate::domain::{FeffError, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult};
use crate::parser::parse_input_deck;
use std::fs;
use std::path::Path;

const RDINP_REQUIRED_INPUTS: [&str; 1] = ["feff.inp"];
const RDINP_PRIMARY_OUTPUTS: [&str; 19] = [
    "geom.dat",
    "global.inp",
    "reciprocal.inp",
    "pot.inp",
    "ldos.inp",
    "xsph.inp",
    "fms.inp",
    "paths.inp",
    "genfmt.inp",
    "ff2x.inp",
    "sfconv.inp",
    "eels.inp",
    "compton.inp",
    "band.inp",
    "rixs.inp",
    "crpa.inp",
    "fullspectrum.inp",
    "dmdw.inp",
    "log.dat",
];
const RDINP_OPTIONAL_SCREEN_OUTPUT: &str = "screen.inp";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdinpPipelineInterface {
    pub required_inputs: Vec<PipelineArtifact>,
    pub expected_outputs: Vec<PipelineArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RdinpPipelineScaffold;

impl RdinpPipelineScaffold {
    pub fn contract_for_request(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<RdinpPipelineInterface> {
        if request.module != PipelineModule::Rdinp {
            return Err(FeffError::input_validation(
                "INPUT.RDINP_MODULE",
                format!(
                    "RDINP scaffold expects module RDINP, got {}",
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
                    "INPUT.RDINP_INPUT_ARTIFACT",
                    format!(
                        "RDINP scaffold expects input artifact '{}' at '{}'",
                        RDINP_REQUIRED_INPUTS[0],
                        request.input_path.display()
                    ),
                )
            })?;
        if !input_file_name.eq_ignore_ascii_case(RDINP_REQUIRED_INPUTS[0]) {
            return Err(FeffError::input_validation(
                "INPUT.RDINP_INPUT_ARTIFACT",
                format!(
                    "RDINP scaffold requires input artifact '{}' but received '{}'",
                    RDINP_REQUIRED_INPUTS[0], input_file_name
                ),
            ));
        }

        let has_screen_card = detect_screen_card(&request.input_path)?;

        Ok(RdinpPipelineInterface {
            required_inputs: artifact_list(&RDINP_REQUIRED_INPUTS),
            expected_outputs: expected_outputs_for_screen_card(has_screen_card),
        })
    }
}

impl PipelineExecutor for RdinpPipelineScaffold {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        let interface = self.contract_for_request(request)?;

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.RDINP_OUTPUT_DIRECTORY",
                format!(
                    "failed to create RDINP output directory '{}': {}",
                    request.output_dir.display(),
                    source
                ),
            )
        })?;

        for artifact in &interface.expected_outputs {
            let output_path = request.output_dir.join(&artifact.relative_path);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|source| {
                    FeffError::io_system(
                        "IO.RDINP_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create RDINP artifact directory '{}': {}",
                            parent.display(),
                            source
                        ),
                    )
                })?;
            }

            let artifact_path = artifact.relative_path.to_string_lossy();
            let content = format!(
                "# RDINP scaffold placeholder\n# fixture: {}\n# module: {}\n# artifact: {}\n",
                request.fixture_id, request.module, artifact_path
            );
            fs::write(&output_path, content).map_err(|source| {
                FeffError::io_system(
                    "IO.RDINP_OUTPUT_WRITE",
                    format!(
                        "failed to write RDINP scaffold artifact '{}': {}",
                        output_path.display(),
                        source
                    ),
                )
            })?;
        }

        Ok(interface.expected_outputs)
    }
}

fn detect_screen_card(input_path: &Path) -> PipelineResult<bool> {
    let input_source = fs::read_to_string(input_path).map_err(|source| {
        FeffError::io_system(
            "IO.RDINP_INPUT_READ",
            format!(
                "failed to read RDINP input '{}': {}",
                input_path.display(),
                source
            ),
        )
    })?;
    let deck = parse_input_deck(&input_source)?;
    Ok(deck.cards.iter().any(|card| card.keyword == "SCREEN"))
}

fn expected_outputs_for_screen_card(has_screen_card: bool) -> Vec<PipelineArtifact> {
    let mut outputs = vec![
        PipelineArtifact::new("geom.dat"),
        PipelineArtifact::new("global.inp"),
        PipelineArtifact::new("reciprocal.inp"),
        PipelineArtifact::new("pot.inp"),
        PipelineArtifact::new("ldos.inp"),
    ];
    if has_screen_card {
        outputs.push(PipelineArtifact::new(RDINP_OPTIONAL_SCREEN_OUTPUT));
    }
    outputs.extend(
        RDINP_PRIMARY_OUTPUTS[5..]
            .iter()
            .copied()
            .map(PipelineArtifact::new),
    );
    outputs
}

fn artifact_list(paths: &[&str]) -> Vec<PipelineArtifact> {
    paths.iter().copied().map(PipelineArtifact::new).collect()
}

#[cfg(test)]
mod tests {
    use super::{RdinpPipelineScaffold, expected_outputs_for_screen_card};
    use crate::domain::{FeffErrorCategory, PipelineModule, PipelineRequest};
    use crate::pipelines::PipelineExecutor;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn contract_matches_rdinp_compatibility_interfaces() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("feff.inp");
        let output_dir = temp.path().join("out");
        fs::write(
            &input_path,
            "TITLE Cu\nPOTENTIALS\n0 29 Cu\nATOMS\n0.0 0.0 0.0 0 Cu\nEND\n",
        )
        .expect("input should be written");

        let request = PipelineRequest::new(
            "FX-RDINP-001",
            PipelineModule::Rdinp,
            &input_path,
            &output_dir,
        );
        let scaffold = RdinpPipelineScaffold;
        let contract = scaffold
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(contract.required_inputs.len(), 1);
        assert_eq!(
            contract.required_inputs[0].relative_path,
            PathBuf::from("feff.inp")
        );
        assert_eq!(contract.expected_outputs.len(), 19);
        assert!(
            contract
                .expected_outputs
                .iter()
                .all(|artifact| artifact.relative_path != PathBuf::from("screen.inp"))
        );
    }

    #[test]
    fn contract_adds_screen_output_when_screen_card_is_present() {
        let outputs = expected_outputs_for_screen_card(true);
        assert_eq!(outputs.len(), 20);
        assert!(
            outputs
                .iter()
                .any(|artifact| artifact.relative_path == PathBuf::from("screen.inp"))
        );
    }

    #[test]
    fn execute_materializes_placeholder_artifacts() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("feff.inp");
        let output_dir = temp.path().join("actual");
        fs::write(
            &input_path,
            "TITLE Cu\nPOTENTIALS\n0 29 Cu\nATOMS\n0.0 0.0 0.0 0 Cu\nEND\n",
        )
        .expect("input should be written");

        let request = PipelineRequest::new(
            "FX-RDINP-001",
            PipelineModule::Rdinp,
            &input_path,
            &output_dir,
        );
        let scaffold = RdinpPipelineScaffold;
        let artifacts = scaffold
            .execute(&request)
            .expect("scaffold execution should succeed");

        assert_eq!(artifacts.len(), 19);
        let log_dat = output_dir.join("log.dat");
        assert!(log_dat.exists());
        let content = fs::read_to_string(log_dat).expect("placeholder output should be readable");
        assert!(content.contains("RDINP scaffold placeholder"));
    }

    #[test]
    fn execute_rejects_non_rdinp_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_path = temp.path().join("feff.inp");
        fs::write(
            &input_path,
            "TITLE Cu\nPOTENTIALS\n0 29 Cu\nATOMS\n0.0 0.0 0.0 0 Cu\nEND\n",
        )
        .expect("input should be written");

        let request =
            PipelineRequest::new("FX-POT-001", PipelineModule::Pot, &input_path, temp.path());
        let scaffold = RdinpPipelineScaffold;
        let error = scaffold
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.RDINP_MODULE");
    }
}
