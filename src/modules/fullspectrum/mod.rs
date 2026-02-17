mod model;
mod parser;

use super::ModuleExecutor;
use crate::domain::{ComputeArtifact, ComputeRequest, ComputeResult, FeffError};
use std::fs;

use model::FullSpectrumModel;
use parser::{
    artifact_list, input_parent_dir, maybe_read_optional_input_source, read_input_source,
    validate_request_shape,
};

pub(crate) const FULLSPECTRUM_REQUIRED_INPUTS: [&str; 2] = ["fullspectrum.inp", "xmu.dat"];
pub(crate) const FULLSPECTRUM_OPTIONAL_INPUTS: [&str; 2] = ["prexmu.dat", "referencexmu.dat"];
pub(crate) const FULLSPECTRUM_REQUIRED_OUTPUTS: [&str; 7] = [
    "xmu.dat",
    "osc_str.dat",
    "eps.dat",
    "drude.dat",
    "background.dat",
    "fine_st.dat",
    "logfullspectrum.dat",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullSpectrumContract {
    pub required_inputs: Vec<ComputeArtifact>,
    pub optional_inputs: Vec<ComputeArtifact>,
    pub expected_outputs: Vec<ComputeArtifact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FullSpectrumModule;

impl FullSpectrumModule {
    pub fn contract_for_request(
        &self,
        request: &ComputeRequest,
    ) -> ComputeResult<FullSpectrumContract> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let fullspectrum_source =
            read_input_source(&request.input_path, FULLSPECTRUM_REQUIRED_INPUTS[0])?;
        let xmu_source = read_input_source(
            &input_dir.join(FULLSPECTRUM_REQUIRED_INPUTS[1]),
            FULLSPECTRUM_REQUIRED_INPUTS[1],
        )?;
        let prexmu_source = maybe_read_optional_input_source(
            input_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[0]),
            FULLSPECTRUM_OPTIONAL_INPUTS[0],
        )?;
        let referencexmu_source = maybe_read_optional_input_source(
            input_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[1]),
            FULLSPECTRUM_OPTIONAL_INPUTS[1],
        )?;

        let _model = FullSpectrumModel::from_sources(
            &request.fixture_id,
            &fullspectrum_source,
            &xmu_source,
            prexmu_source.as_deref(),
            referencexmu_source.as_deref(),
        )?;

        Ok(FullSpectrumContract {
            required_inputs: artifact_list(&FULLSPECTRUM_REQUIRED_INPUTS),
            optional_inputs: artifact_list(&FULLSPECTRUM_OPTIONAL_INPUTS),
            expected_outputs: artifact_list(&FULLSPECTRUM_REQUIRED_OUTPUTS),
        })
    }
}

impl ModuleExecutor for FullSpectrumModule {
    fn execute(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        validate_request_shape(request)?;
        let input_dir = input_parent_dir(request)?;

        let fullspectrum_source =
            read_input_source(&request.input_path, FULLSPECTRUM_REQUIRED_INPUTS[0])?;
        let xmu_source = read_input_source(
            &input_dir.join(FULLSPECTRUM_REQUIRED_INPUTS[1]),
            FULLSPECTRUM_REQUIRED_INPUTS[1],
        )?;
        let prexmu_source = maybe_read_optional_input_source(
            input_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[0]),
            FULLSPECTRUM_OPTIONAL_INPUTS[0],
        )?;
        let referencexmu_source = maybe_read_optional_input_source(
            input_dir.join(FULLSPECTRUM_OPTIONAL_INPUTS[1]),
            FULLSPECTRUM_OPTIONAL_INPUTS[1],
        )?;

        let model = FullSpectrumModel::from_sources(
            &request.fixture_id,
            &fullspectrum_source,
            &xmu_source,
            prexmu_source.as_deref(),
            referencexmu_source.as_deref(),
        )?;
        let outputs = artifact_list(&FULLSPECTRUM_REQUIRED_OUTPUTS);

        fs::create_dir_all(&request.output_dir).map_err(|source| {
            FeffError::io_system(
                "IO.FULLSPECTRUM_OUTPUT_DIRECTORY",
                format!(
                    "failed to create FULLSPECTRUM output directory '{}': {}",
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
                        "IO.FULLSPECTRUM_OUTPUT_DIRECTORY",
                        format!(
                            "failed to create FULLSPECTRUM artifact directory '{}': {}",
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

#[cfg(test)]
mod tests {
    use super::FullSpectrumModule;
    use crate::domain::{FeffErrorCategory, ComputeArtifact, ComputeModule, ComputeRequest};
    use crate::modules::ModuleExecutor;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const FULLSPECTRUM_INPUT_DEFAULT: &str = "\
 mFullSpectrum
           0
";

    const FULLSPECTRUM_INPUT_WITH_CONTROLS: &str = "\
 mFullSpectrum
           1
 broadening drude
     0.45000     1.25000
 oscillator epsilon_shift
     1.10000     0.25000
";

    const XMU_INPUT: &str = "\
# omega e k mu mu0 chi
8956.1761 -40.0000 -2.9103 9.162321E-02 9.102713E-02 5.960831E-04
8956.6084 -39.5677 -2.8908 7.595159E-02 7.534298E-02 6.086083E-04
8957.0407 -39.1354 -2.8711 6.248403E-02 6.186194E-02 6.220848E-04
8957.4730 -38.7031 -2.8512 5.166095E-02 5.102360E-02 6.373535E-04
";

    const PREXMU_INPUT: &str = "\
-1.4699723600E+00 -5.2212753390E-04 1.1530407310E-05
-1.4540857260E+00 -5.1175235060E-04 9.5436958570E-06
-1.4381990910E+00 -5.0195981330E-04 7.8360530260E-06
";

    const REFERENCE_XMU_INPUT: &str = "\
# omega e k mu mu0 chi
8956.1761 -40.0000 -2.9103 9.162321E-02 9.102713E-02 5.960831E-04
8956.6084 -39.5677 -2.8908 7.595159E-02 7.534298E-02 6.086083E-04
8957.0407 -39.1354 -2.8711 6.248403E-02 6.186194E-02 6.220848E-04
";

    #[test]
    fn contract_exposes_required_inputs_and_true_compute_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(
            temp.path().join("fullspectrum.inp"),
            FULLSPECTRUM_INPUT_WITH_CONTROLS,
        );
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("fullspectrum.inp"),
            temp.path().join("out"),
        );
        let contract = FullSpectrumModule
            .contract_for_request(&request)
            .expect("contract should build");

        assert_eq!(
            contract.required_inputs,
            artifact_list(&["fullspectrum.inp", "xmu.dat"])
        );
        assert_eq!(
            contract.optional_inputs,
            artifact_list(&["prexmu.dat", "referencexmu.dat"])
        );
        assert_eq!(artifact_set(&contract.expected_outputs), expected_set());
    }

    #[test]
    fn execute_writes_true_compute_fullspectrum_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(
            temp.path().join("fullspectrum.inp"),
            FULLSPECTRUM_INPUT_WITH_CONTROLS,
        );
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("fullspectrum.inp"),
            temp.path().join("out"),
        );
        let artifacts = FullSpectrumModule
            .execute(&request)
            .expect("execution should succeed");

        assert_eq!(artifact_set(&artifacts), expected_set());
        assert!(temp.path().join("out/xmu.dat").is_file());
        assert!(temp.path().join("out/osc_str.dat").is_file());
        assert!(temp.path().join("out/eps.dat").is_file());
        assert!(temp.path().join("out/drude.dat").is_file());
        assert!(temp.path().join("out/background.dat").is_file());
        assert!(temp.path().join("out/fine_st.dat").is_file());
        assert!(temp.path().join("out/logfullspectrum.dat").is_file());
    }

    #[test]
    fn execute_optional_component_inputs_influence_outputs() {
        let temp = TempDir::new().expect("tempdir should be created");

        stage_text(
            temp.path().join("with-optional/fullspectrum.inp"),
            FULLSPECTRUM_INPUT_DEFAULT,
        );
        stage_text(temp.path().join("with-optional/xmu.dat"), XMU_INPUT);
        stage_text(temp.path().join("with-optional/prexmu.dat"), PREXMU_INPUT);
        stage_text(
            temp.path().join("with-optional/referencexmu.dat"),
            REFERENCE_XMU_INPUT,
        );

        stage_text(
            temp.path().join("without-optional/fullspectrum.inp"),
            FULLSPECTRUM_INPUT_DEFAULT,
        );
        stage_text(temp.path().join("without-optional/xmu.dat"), XMU_INPUT);

        let with_optional_request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("with-optional/fullspectrum.inp"),
            temp.path().join("out-with"),
        );
        let without_optional_request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("without-optional/fullspectrum.inp"),
            temp.path().join("out-without"),
        );

        let with_optional = FullSpectrumModule
            .execute(&with_optional_request)
            .expect("execution with optional inputs should succeed");
        let without_optional = FullSpectrumModule
            .execute(&without_optional_request)
            .expect("execution without optional inputs should succeed");

        assert_eq!(artifact_set(&with_optional), expected_set());
        assert_eq!(artifact_set(&without_optional), expected_set());

        let with_xmu = fs::read(with_optional_request.output_dir.join("xmu.dat"))
            .expect("xmu output should be readable");
        let without_xmu = fs::read(without_optional_request.output_dir.join("xmu.dat"))
            .expect("xmu output should be readable");

        assert_ne!(
            with_xmu, without_xmu,
            "optional FULLSPECTRUM component inputs should influence xmu.dat"
        );
    }

    #[test]
    fn execute_is_deterministic_for_identical_inputs() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(
            temp.path().join("shared/fullspectrum.inp"),
            FULLSPECTRUM_INPUT_WITH_CONTROLS,
        );
        stage_text(temp.path().join("shared/xmu.dat"), XMU_INPUT);
        stage_text(temp.path().join("shared/prexmu.dat"), PREXMU_INPUT);
        stage_text(
            temp.path().join("shared/referencexmu.dat"),
            REFERENCE_XMU_INPUT,
        );

        let first_request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("shared/fullspectrum.inp"),
            temp.path().join("out-first"),
        );
        let second_request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("shared/fullspectrum.inp"),
            temp.path().join("out-second"),
        );

        let first = FullSpectrumModule
            .execute(&first_request)
            .expect("first execution should succeed");
        let second = FullSpectrumModule
            .execute(&second_request)
            .expect("second execution should succeed");

        assert_eq!(artifact_set(&first), artifact_set(&second));
        for artifact in first {
            let first_bytes = fs::read(first_request.output_dir.join(&artifact.relative_path))
                .expect("first output should be readable");
            let second_bytes = fs::read(second_request.output_dir.join(&artifact.relative_path))
                .expect("second output should be readable");
            assert_eq!(
                first_bytes,
                second_bytes,
                "artifact '{}' should be deterministic",
                artifact.relative_path.display()
            );
        }
    }

    #[test]
    fn execute_rejects_non_fullspectrum_module_requests() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(
            temp.path().join("fullspectrum.inp"),
            FULLSPECTRUM_INPUT_DEFAULT,
        );
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::Eels,
            temp.path().join("fullspectrum.inp"),
            temp.path().join("out"),
        );
        let error = FullSpectrumModule
            .execute(&request)
            .expect_err("module mismatch should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.FULLSPECTRUM_MODULE");
    }

    #[test]
    fn execute_requires_xmu_input_in_same_directory() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(
            temp.path().join("fullspectrum.inp"),
            FULLSPECTRUM_INPUT_DEFAULT,
        );

        let request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("fullspectrum.inp"),
            temp.path().join("out"),
        );
        let error = FullSpectrumModule
            .execute(&request)
            .expect_err("missing xmu should fail");

        assert_eq!(error.category(), FeffErrorCategory::IoSystemError);
        assert_eq!(error.placeholder(), "IO.FULLSPECTRUM_INPUT_READ");
    }

    #[test]
    fn execute_rejects_unparseable_fullspectrum_control_input() {
        let temp = TempDir::new().expect("tempdir should be created");
        stage_text(temp.path().join("fullspectrum.inp"), "mFullSpectrum\n");
        stage_text(temp.path().join("xmu.dat"), XMU_INPUT);

        let request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            temp.path().join("fullspectrum.inp"),
            temp.path().join("out"),
        );
        let error = FullSpectrumModule
            .execute(&request)
            .expect_err("invalid controls should fail");

        assert_eq!(error.category(), FeffErrorCategory::InputValidationError);
        assert_eq!(error.placeholder(), "INPUT.FULLSPECTRUM_PARSE");
    }

    fn artifact_list(paths: &[&str]) -> Vec<ComputeArtifact> {
        paths.iter().copied().map(ComputeArtifact::new).collect()
    }

    fn expected_set() -> BTreeSet<String> {
        BTreeSet::from([
            "xmu.dat".to_string(),
            "osc_str.dat".to_string(),
            "eps.dat".to_string(),
            "drude.dat".to_string(),
            "background.dat".to_string(),
            "fine_st.dat".to_string(),
            "logfullspectrum.dat".to_string(),
        ])
    }

    fn artifact_set(artifacts: &[ComputeArtifact]) -> BTreeSet<String> {
        artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn stage_text(destination: PathBuf, contents: &str) {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should exist");
        }
        fs::write(destination, contents).expect("text input should be written");
    }
}
