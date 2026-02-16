use super::PipelineExecutor;
use super::band::BandPipelineScaffold;
use super::comparator::{ArtifactComparisonResult, Comparator, ComparatorError};
use super::compton::ComptonPipelineScaffold;
use super::crpa::CrpaPipelineScaffold;
use super::debye::DebyePipelineScaffold;
use super::fms::FmsPipelineScaffold;
use super::ldos::LdosPipelineScaffold;
use super::path::PathPipelineScaffold;
use super::pot::PotPipelineScaffold;
use super::rdinp::RdinpPipelineScaffold;
use super::rixs::RixsPipelineScaffold;
use super::xsph::XsphPipelineScaffold;
use crate::domain::{FeffError, PipelineModule, PipelineRequest, PipelineResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct RegressionRunnerConfig {
    pub manifest_path: PathBuf,
    pub policy_path: PathBuf,
    pub baseline_root: PathBuf,
    pub actual_root: PathBuf,
    pub baseline_subdir: String,
    pub actual_subdir: String,
    pub report_path: PathBuf,
    pub run_rdinp: bool,
    pub run_pot: bool,
    pub run_xsph: bool,
    pub run_path: bool,
    pub run_fms: bool,
    pub run_band: bool,
    pub run_ldos: bool,
    pub run_rixs: bool,
    pub run_crpa: bool,
    pub run_compton: bool,
    pub run_debye: bool,
}

impl Default for RegressionRunnerConfig {
    fn default() -> Self {
        Self {
            manifest_path: PathBuf::from("tasks/golden-fixture-manifest.json"),
            policy_path: PathBuf::from("tasks/numeric-tolerance-policy.json"),
            baseline_root: PathBuf::from("artifacts/fortran-baselines"),
            actual_root: PathBuf::from("artifacts/fortran-baselines"),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "baseline".to_string(),
            report_path: PathBuf::from("artifacts/regression/report.json"),
            run_rdinp: false,
            run_pot: false,
            run_xsph: false,
            run_path: false,
            run_fms: false,
            run_band: false,
            run_ldos: false,
            run_rixs: false,
            run_crpa: false,
            run_compton: false,
            run_debye: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RegressionRunReport {
    pub generated_at_unix_seconds: u64,
    pub passed: bool,
    pub manifest_path: String,
    pub policy_path: String,
    pub baseline_root: String,
    pub actual_root: String,
    pub baseline_subdir: String,
    pub actual_subdir: String,
    pub fixture_count: usize,
    pub passed_fixture_count: usize,
    pub failed_fixture_count: usize,
    pub artifact_count: usize,
    pub passed_artifact_count: usize,
    pub failed_artifact_count: usize,
    pub fixtures: Vec<FixtureRegressionReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureRegressionReport {
    pub fixture_id: String,
    pub passed: bool,
    pub artifact_count: usize,
    pub passed_artifact_count: usize,
    pub failed_artifact_count: usize,
    pub artifact_pass_rate: f64,
    pub threshold: FixturePassFailThreshold,
    pub artifacts: Vec<ArtifactRegressionReport>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct FixturePassFailThreshold {
    #[serde(rename = "minimumArtifactPassRate")]
    pub minimum_artifact_pass_rate: f64,
    #[serde(rename = "maxArtifactFailures")]
    pub max_artifact_failures: usize,
}

impl Default for FixturePassFailThreshold {
    fn default() -> Self {
        Self {
            minimum_artifact_pass_rate: 1.0,
            max_artifact_failures: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactRegressionReport {
    pub artifact_path: String,
    pub baseline_path: String,
    pub actual_path: String,
    pub passed: bool,
    pub reason: Option<String>,
    pub comparison: Option<ArtifactComparisonResult>,
}

pub fn run_regression(config: &RegressionRunnerConfig) -> PipelineResult<RegressionRunReport> {
    let manifest = load_manifest(&config.manifest_path).map_err(FeffError::from)?;
    let comparator = Comparator::from_policy_path(&config.policy_path)
        .map_err(|source| FeffError::from(RegressionRunnerError::Comparator(source)))?;

    let mut fixture_reports = Vec::with_capacity(manifest.fixtures.len());
    for fixture in &manifest.fixtures {
        run_rdinp_if_enabled(config, fixture)?;
        run_pot_if_enabled(config, fixture)?;
        run_xsph_if_enabled(config, fixture)?;
        run_band_if_enabled(config, fixture)?;
        run_ldos_if_enabled(config, fixture)?;
        run_rixs_if_enabled(config, fixture)?;
        run_crpa_if_enabled(config, fixture)?;
        run_path_if_enabled(config, fixture)?;
        run_debye_if_enabled(config, fixture)?;
        run_fms_if_enabled(config, fixture)?;
        run_compton_if_enabled(config, fixture)?;
        let threshold = threshold_for_fixture(&manifest.default_comparison, fixture);
        let report = compare_fixture(config, fixture, threshold, &comparator)?;
        fixture_reports.push(report);
    }

    let fixture_count = fixture_reports.len();
    let passed_fixture_count = fixture_reports
        .iter()
        .filter(|fixture| fixture.passed)
        .count();
    let failed_fixture_count = fixture_count.saturating_sub(passed_fixture_count);

    let artifact_count = fixture_reports
        .iter()
        .map(|fixture| fixture.artifact_count)
        .sum::<usize>();
    let passed_artifact_count = fixture_reports
        .iter()
        .map(|fixture| fixture.passed_artifact_count)
        .sum::<usize>();
    let failed_artifact_count = artifact_count.saturating_sub(passed_artifact_count);
    let passed = failed_fixture_count == 0;

    let report = RegressionRunReport {
        generated_at_unix_seconds: current_unix_timestamp_seconds(),
        passed,
        manifest_path: normalize_path(&config.manifest_path),
        policy_path: normalize_path(&config.policy_path),
        baseline_root: normalize_path(&config.baseline_root),
        actual_root: normalize_path(&config.actual_root),
        baseline_subdir: config.baseline_subdir.clone(),
        actual_subdir: config.actual_subdir.clone(),
        fixture_count,
        passed_fixture_count,
        failed_fixture_count,
        artifact_count,
        passed_artifact_count,
        failed_artifact_count,
        fixtures: fixture_reports,
    };

    write_report_file(&config.report_path, &report).map_err(FeffError::from)?;
    Ok(report)
}

pub fn render_human_summary(report: &RegressionRunReport) -> String {
    let mut lines = Vec::new();
    let status = if report.passed { "PASS" } else { "FAIL" };
    lines.push(format!("Regression status: {}", status));
    lines.push(format!(
        "Fixtures: {} total ({} passed, {} failed)",
        report.fixture_count, report.passed_fixture_count, report.failed_fixture_count
    ));
    lines.push(format!(
        "Artifacts: {} total ({} passed, {} failed)",
        report.artifact_count, report.passed_artifact_count, report.failed_artifact_count
    ));

    for fixture in &report.fixtures {
        let fixture_status = if fixture.passed { "PASS" } else { "FAIL" };
        lines.push(format!(
            "Fixture {}: {} ({}/{} artifacts, pass_rate={:.4}, threshold: min_pass_rate={:.4}, max_failures={})",
            fixture.fixture_id,
            fixture_status,
            fixture.passed_artifact_count,
            fixture.artifact_count,
            fixture.artifact_pass_rate,
            fixture.threshold.minimum_artifact_pass_rate,
            fixture.threshold.max_artifact_failures
        ));

        if !fixture.passed {
            if let Some(first_failure) = fixture.artifacts.iter().find(|artifact| !artifact.passed)
            {
                let reason = first_failure
                    .reason
                    .as_deref()
                    .unwrap_or("artifact comparison failed without a reason");
                lines.push(format!(
                    "  first failure: {} ({})",
                    first_failure.artifact_path, reason
                ));
            }
        }
    }

    lines.join("\n")
}

#[derive(Debug)]
pub enum RegressionRunnerError {
    ReadManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseManifest {
        path: PathBuf,
        source: serde_json::Error,
    },
    InvalidFixtureConfig {
        fixture_id: String,
        message: String,
    },
    Comparator(ComparatorError),
    RdinpPipeline {
        fixture_id: String,
        source: FeffError,
    },
    PotPipeline {
        fixture_id: String,
        source: FeffError,
    },
    XsphPipeline {
        fixture_id: String,
        source: FeffError,
    },
    BandPipeline {
        fixture_id: String,
        source: FeffError,
    },
    LdosPipeline {
        fixture_id: String,
        source: FeffError,
    },
    RixsPipeline {
        fixture_id: String,
        source: FeffError,
    },
    CrpaPipeline {
        fixture_id: String,
        source: FeffError,
    },
    ComptonPipeline {
        fixture_id: String,
        source: FeffError,
    },
    DebyePipeline {
        fixture_id: String,
        source: FeffError,
    },
    PathPipeline {
        fixture_id: String,
        source: FeffError,
    },
    FmsPipeline {
        fixture_id: String,
        source: FeffError,
    },
    ReadDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    ReportDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    SerializeReport {
        path: PathBuf,
        source: serde_json::Error,
    },
    WriteReport {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl Display for RegressionRunnerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadManifest { path, source } => {
                write!(
                    f,
                    "failed to read manifest '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::ParseManifest { path, source } => {
                write!(
                    f,
                    "failed to parse manifest '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::InvalidFixtureConfig {
                fixture_id,
                message,
            } => write!(
                f,
                "invalid fixture configuration for '{}': {}",
                fixture_id, message
            ),
            Self::Comparator(source) => write!(f, "comparator setup failed: {}", source),
            Self::RdinpPipeline { fixture_id, source } => write!(
                f,
                "RDINP scaffold execution failed for fixture '{}': {}",
                fixture_id, source
            ),
            Self::PotPipeline { fixture_id, source } => write!(
                f,
                "POT scaffold execution failed for fixture '{}': {}",
                fixture_id, source
            ),
            Self::XsphPipeline { fixture_id, source } => write!(
                f,
                "XSPH parity execution failed for fixture '{}': {}",
                fixture_id, source
            ),
            Self::BandPipeline { fixture_id, source } => write!(
                f,
                "BAND parity execution failed for fixture '{}': {}",
                fixture_id, source
            ),
            Self::LdosPipeline { fixture_id, source } => write!(
                f,
                "LDOS parity execution failed for fixture '{}': {}",
                fixture_id, source
            ),
            Self::RixsPipeline { fixture_id, source } => write!(
                f,
                "RIXS parity execution failed for fixture '{}': {}",
                fixture_id, source
            ),
            Self::CrpaPipeline { fixture_id, source } => write!(
                f,
                "CRPA parity execution failed for fixture '{}': {}",
                fixture_id, source
            ),
            Self::ComptonPipeline { fixture_id, source } => write!(
                f,
                "COMPTON parity execution failed for fixture '{}': {}",
                fixture_id, source
            ),
            Self::DebyePipeline { fixture_id, source } => write!(
                f,
                "DEBYE parity execution failed for fixture '{}': {}",
                fixture_id, source
            ),
            Self::PathPipeline { fixture_id, source } => write!(
                f,
                "PATH scaffold execution failed for fixture '{}': {}",
                fixture_id, source
            ),
            Self::FmsPipeline { fixture_id, source } => write!(
                f,
                "FMS scaffold execution failed for fixture '{}': {}",
                fixture_id, source
            ),
            Self::ReadDirectory { path, source } => {
                write!(
                    f,
                    "failed to read directory '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::ReportDirectory { path, source } => write!(
                f,
                "failed to create report directory '{}': {}",
                path.display(),
                source
            ),
            Self::SerializeReport { path, source } => write!(
                f,
                "failed to serialize report '{}': {}",
                path.display(),
                source
            ),
            Self::WriteReport { path, source } => {
                write!(f, "failed to write report '{}': {}", path.display(), source)
            }
        }
    }
}

impl Error for RegressionRunnerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadManifest { source, .. } => Some(source),
            Self::ParseManifest { source, .. } => Some(source),
            Self::InvalidFixtureConfig { .. } => None,
            Self::Comparator(source) => Some(source),
            Self::RdinpPipeline { source, .. } => Some(source),
            Self::PotPipeline { source, .. } => Some(source),
            Self::XsphPipeline { source, .. } => Some(source),
            Self::BandPipeline { source, .. } => Some(source),
            Self::LdosPipeline { source, .. } => Some(source),
            Self::RixsPipeline { source, .. } => Some(source),
            Self::CrpaPipeline { source, .. } => Some(source),
            Self::ComptonPipeline { source, .. } => Some(source),
            Self::DebyePipeline { source, .. } => Some(source),
            Self::PathPipeline { source, .. } => Some(source),
            Self::FmsPipeline { source, .. } => Some(source),
            Self::ReadDirectory { source, .. } => Some(source),
            Self::ReportDirectory { source, .. } => Some(source),
            Self::SerializeReport { source, .. } => Some(source),
            Self::WriteReport { source, .. } => Some(source),
        }
    }
}

impl From<RegressionRunnerError> for FeffError {
    fn from(error: RegressionRunnerError) -> Self {
        let message = error.to_string();
        match error {
            RegressionRunnerError::ReadManifest { .. } => {
                FeffError::io_system("IO.REGRESSION_MANIFEST", message)
            }
            RegressionRunnerError::ParseManifest { .. } => {
                FeffError::input_validation("INPUT.REGRESSION_MANIFEST", message)
            }
            RegressionRunnerError::InvalidFixtureConfig { .. } => {
                FeffError::input_validation("INPUT.REGRESSION_MANIFEST", message)
            }
            RegressionRunnerError::Comparator(source) => source.into(),
            RegressionRunnerError::RdinpPipeline { source, .. } => source,
            RegressionRunnerError::PotPipeline { source, .. } => source,
            RegressionRunnerError::XsphPipeline { source, .. } => source,
            RegressionRunnerError::BandPipeline { source, .. } => source,
            RegressionRunnerError::LdosPipeline { source, .. } => source,
            RegressionRunnerError::RixsPipeline { source, .. } => source,
            RegressionRunnerError::CrpaPipeline { source, .. } => source,
            RegressionRunnerError::ComptonPipeline { source, .. } => source,
            RegressionRunnerError::DebyePipeline { source, .. } => source,
            RegressionRunnerError::PathPipeline { source, .. } => source,
            RegressionRunnerError::FmsPipeline { source, .. } => source,
            RegressionRunnerError::ReadDirectory { .. }
            | RegressionRunnerError::ReportDirectory { .. }
            | RegressionRunnerError::WriteReport { .. } => {
                FeffError::io_system("IO.REGRESSION_FILESYSTEM", message)
            }
            RegressionRunnerError::SerializeReport { .. } => {
                FeffError::internal("SYS.REGRESSION_REPORT", message)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct FixtureManifest {
    #[serde(rename = "defaultComparison", default)]
    default_comparison: ManifestComparison,
    #[serde(default)]
    fixtures: Vec<ManifestFixture>,
}

#[derive(Debug, Deserialize, Default)]
struct ManifestComparison {
    #[serde(rename = "passFailThreshold", default)]
    pass_fail_threshold: Option<FixturePassFailThreshold>,
}

#[derive(Debug, Deserialize)]
struct ManifestFixture {
    id: String,
    #[serde(rename = "modulesCovered", default)]
    modules_covered: Vec<String>,
    #[serde(rename = "inputDirectory", default)]
    input_directory: Option<PathBuf>,
    #[serde(rename = "entryFiles", default)]
    entry_files: Vec<String>,
    #[serde(default)]
    comparison: Option<ManifestComparison>,
}

impl ManifestFixture {
    fn covers_module(&self, module: PipelineModule) -> bool {
        self.modules_covered
            .iter()
            .any(|covered| covered.eq_ignore_ascii_case(module.as_str()))
    }

    fn resolve_rdinp_input_path(&self) -> Result<PathBuf, RegressionRunnerError> {
        let input_directory = self.input_directory.as_ref().ok_or_else(|| {
            RegressionRunnerError::InvalidFixtureConfig {
                fixture_id: self.id.clone(),
                message: "missing required inputDirectory for RDINP scaffold execution".to_string(),
            }
        })?;

        let relative_input = self
            .entry_files
            .iter()
            .find(|entry| entry.eq_ignore_ascii_case("feff.inp"))
            .cloned()
            .unwrap_or_else(|| "feff.inp".to_string());

        Ok(input_directory.join(relative_input))
    }
}

fn load_manifest(manifest_path: &Path) -> Result<FixtureManifest, RegressionRunnerError> {
    let content = fs::read_to_string(manifest_path).map_err(|source| {
        RegressionRunnerError::ReadManifest {
            path: manifest_path.to_path_buf(),
            source,
        }
    })?;
    serde_json::from_str(&content).map_err(|source| RegressionRunnerError::ParseManifest {
        path: manifest_path.to_path_buf(),
        source,
    })
}

fn threshold_for_fixture(
    default_comparison: &ManifestComparison,
    fixture: &ManifestFixture,
) -> FixturePassFailThreshold {
    fixture
        .comparison
        .as_ref()
        .and_then(|comparison| comparison.pass_fail_threshold)
        .or(default_comparison.pass_fail_threshold)
        .unwrap_or_default()
}

fn run_rdinp_if_enabled(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
) -> Result<(), RegressionRunnerError> {
    if !config.run_rdinp || !fixture.covers_module(PipelineModule::Rdinp) {
        return Ok(());
    }

    let input_path = fixture.resolve_rdinp_input_path()?;
    let output_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);
    let request = PipelineRequest::new(
        fixture.id.clone(),
        PipelineModule::Rdinp,
        input_path,
        output_dir,
    );

    RdinpPipelineScaffold.execute(&request).map_err(|source| {
        RegressionRunnerError::RdinpPipeline {
            fixture_id: fixture.id.clone(),
            source,
        }
    })?;

    Ok(())
}

fn run_pot_if_enabled(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
) -> Result<(), RegressionRunnerError> {
    if !config.run_pot || !fixture.covers_module(PipelineModule::Pot) {
        return Ok(());
    }

    let output_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);
    let request = PipelineRequest::new(
        fixture.id.clone(),
        PipelineModule::Pot,
        output_dir.join("pot.inp"),
        output_dir,
    );

    PotPipelineScaffold
        .execute(&request)
        .map_err(|source| RegressionRunnerError::PotPipeline {
            fixture_id: fixture.id.clone(),
            source,
        })?;

    Ok(())
}

fn run_xsph_if_enabled(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
) -> Result<(), RegressionRunnerError> {
    if !config.run_xsph || !fixture.covers_module(PipelineModule::Xsph) {
        return Ok(());
    }

    let output_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);
    let request = PipelineRequest::new(
        fixture.id.clone(),
        PipelineModule::Xsph,
        output_dir.join("xsph.inp"),
        output_dir,
    );

    XsphPipelineScaffold.execute(&request).map_err(|source| {
        RegressionRunnerError::XsphPipeline {
            fixture_id: fixture.id.clone(),
            source,
        }
    })?;

    Ok(())
}

fn run_band_if_enabled(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
) -> Result<(), RegressionRunnerError> {
    if !config.run_band || !fixture.covers_module(PipelineModule::Band) {
        return Ok(());
    }

    let output_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);
    let request = PipelineRequest::new(
        fixture.id.clone(),
        PipelineModule::Band,
        output_dir.join("band.inp"),
        output_dir,
    );

    BandPipelineScaffold.execute(&request).map_err(|source| {
        RegressionRunnerError::BandPipeline {
            fixture_id: fixture.id.clone(),
            source,
        }
    })?;

    Ok(())
}

fn run_ldos_if_enabled(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
) -> Result<(), RegressionRunnerError> {
    if !config.run_ldos || !fixture.covers_module(PipelineModule::Ldos) {
        return Ok(());
    }

    let output_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);
    let request = PipelineRequest::new(
        fixture.id.clone(),
        PipelineModule::Ldos,
        output_dir.join("ldos.inp"),
        output_dir,
    );

    LdosPipelineScaffold.execute(&request).map_err(|source| {
        RegressionRunnerError::LdosPipeline {
            fixture_id: fixture.id.clone(),
            source,
        }
    })?;

    Ok(())
}

fn run_rixs_if_enabled(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
) -> Result<(), RegressionRunnerError> {
    if !config.run_rixs || !fixture.covers_module(PipelineModule::Rixs) {
        return Ok(());
    }

    let output_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);
    let request = PipelineRequest::new(
        fixture.id.clone(),
        PipelineModule::Rixs,
        output_dir.join("rixs.inp"),
        output_dir,
    );

    RixsPipelineScaffold.execute(&request).map_err(|source| {
        RegressionRunnerError::RixsPipeline {
            fixture_id: fixture.id.clone(),
            source,
        }
    })?;

    Ok(())
}

fn run_crpa_if_enabled(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
) -> Result<(), RegressionRunnerError> {
    if !config.run_crpa || !fixture.covers_module(PipelineModule::Crpa) {
        return Ok(());
    }

    let output_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);
    let request = PipelineRequest::new(
        fixture.id.clone(),
        PipelineModule::Crpa,
        output_dir.join("crpa.inp"),
        output_dir,
    );

    CrpaPipelineScaffold.execute(&request).map_err(|source| {
        RegressionRunnerError::CrpaPipeline {
            fixture_id: fixture.id.clone(),
            source,
        }
    })?;

    Ok(())
}

fn run_compton_if_enabled(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
) -> Result<(), RegressionRunnerError> {
    if !config.run_compton || !fixture.covers_module(PipelineModule::Compton) {
        return Ok(());
    }

    let output_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);
    let request = PipelineRequest::new(
        fixture.id.clone(),
        PipelineModule::Compton,
        output_dir.join("compton.inp"),
        output_dir,
    );

    ComptonPipelineScaffold
        .execute(&request)
        .map_err(|source| RegressionRunnerError::ComptonPipeline {
            fixture_id: fixture.id.clone(),
            source,
        })?;

    Ok(())
}

fn run_debye_if_enabled(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
) -> Result<(), RegressionRunnerError> {
    if !config.run_debye || !fixture.covers_module(PipelineModule::Debye) {
        return Ok(());
    }

    let output_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);
    let request = PipelineRequest::new(
        fixture.id.clone(),
        PipelineModule::Debye,
        output_dir.join("ff2x.inp"),
        output_dir,
    );

    DebyePipelineScaffold.execute(&request).map_err(|source| {
        RegressionRunnerError::DebyePipeline {
            fixture_id: fixture.id.clone(),
            source,
        }
    })?;

    Ok(())
}

fn run_path_if_enabled(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
) -> Result<(), RegressionRunnerError> {
    if !config.run_path || !fixture.covers_module(PipelineModule::Path) {
        return Ok(());
    }

    let output_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);
    let request = PipelineRequest::new(
        fixture.id.clone(),
        PipelineModule::Path,
        output_dir.join("paths.inp"),
        output_dir,
    );

    PathPipelineScaffold.execute(&request).map_err(|source| {
        RegressionRunnerError::PathPipeline {
            fixture_id: fixture.id.clone(),
            source,
        }
    })?;

    Ok(())
}

fn run_fms_if_enabled(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
) -> Result<(), RegressionRunnerError> {
    if !config.run_fms || !fixture.covers_module(PipelineModule::Fms) {
        return Ok(());
    }

    let output_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);
    let request = PipelineRequest::new(
        fixture.id.clone(),
        PipelineModule::Fms,
        output_dir.join("fms.inp"),
        output_dir,
    );

    FmsPipelineScaffold
        .execute(&request)
        .map_err(|source| RegressionRunnerError::FmsPipeline {
            fixture_id: fixture.id.clone(),
            source,
        })?;

    Ok(())
}

fn compare_fixture(
    config: &RegressionRunnerConfig,
    fixture: &ManifestFixture,
    threshold: FixturePassFailThreshold,
    comparator: &Comparator,
) -> Result<FixtureRegressionReport, RegressionRunnerError> {
    let baseline_dir = config
        .baseline_root
        .join(&fixture.id)
        .join(&config.baseline_subdir);
    let actual_dir = config
        .actual_root
        .join(&fixture.id)
        .join(&config.actual_subdir);

    let baseline_files = collect_relative_files(&baseline_dir)?;
    let actual_files = collect_relative_files(&actual_dir)?;

    let baseline_paths = baseline_files.as_deref().unwrap_or(&[]);
    let actual_paths = actual_files.as_deref().unwrap_or(&[]);

    let mut all_paths = BTreeSet::new();
    for path in baseline_paths {
        all_paths.insert(path.clone());
    }
    for path in actual_paths {
        all_paths.insert(path.clone());
    }

    if all_paths.is_empty() && baseline_files.is_none() && actual_files.is_none() {
        all_paths.insert(".".to_string());
    }

    let mut artifact_reports = Vec::with_capacity(all_paths.len());
    for artifact_path in all_paths {
        let baseline_path = baseline_dir.join(&artifact_path);
        let actual_path = actual_dir.join(&artifact_path);

        let baseline_exists = baseline_paths.contains(&artifact_path);
        let actual_exists = actual_paths.contains(&artifact_path);

        if !baseline_exists {
            artifact_reports.push(ArtifactRegressionReport {
                artifact_path,
                baseline_path: normalize_path(&baseline_path),
                actual_path: normalize_path(&actual_path),
                passed: false,
                reason: Some("Missing baseline artifact".to_string()),
                comparison: None,
            });
            continue;
        }

        if !actual_exists {
            artifact_reports.push(ArtifactRegressionReport {
                artifact_path,
                baseline_path: normalize_path(&baseline_path),
                actual_path: normalize_path(&actual_path),
                passed: false,
                reason: Some("Missing actual artifact".to_string()),
                comparison: None,
            });
            continue;
        }

        match comparator.compare_artifact(&artifact_path, &baseline_path, &actual_path) {
            Ok(comparison) => {
                artifact_reports.push(ArtifactRegressionReport {
                    artifact_path,
                    baseline_path: normalize_path(&baseline_path),
                    actual_path: normalize_path(&actual_path),
                    passed: comparison.passed,
                    reason: comparison.reason.clone(),
                    comparison: Some(comparison),
                });
            }
            Err(error) => {
                artifact_reports.push(ArtifactRegressionReport {
                    artifact_path,
                    baseline_path: normalize_path(&baseline_path),
                    actual_path: normalize_path(&actual_path),
                    passed: false,
                    reason: Some(format!("Comparison error: {}", error)),
                    comparison: None,
                });
            }
        }
    }

    let artifact_count = artifact_reports.len();
    let failed_artifact_count = artifact_reports
        .iter()
        .filter(|artifact| !artifact.passed)
        .count();
    let passed_artifact_count = artifact_count.saturating_sub(failed_artifact_count);
    let artifact_pass_rate = if artifact_count == 0 {
        1.0
    } else {
        passed_artifact_count as f64 / artifact_count as f64
    };
    let passed = failed_artifact_count <= threshold.max_artifact_failures
        && artifact_pass_rate >= threshold.minimum_artifact_pass_rate;

    Ok(FixtureRegressionReport {
        fixture_id: fixture.id.clone(),
        passed,
        artifact_count,
        passed_artifact_count,
        failed_artifact_count,
        artifact_pass_rate,
        threshold,
        artifacts: artifact_reports,
    })
}

fn collect_relative_files(root: &Path) -> Result<Option<Vec<String>>, RegressionRunnerError> {
    if !root.exists() {
        return Ok(None);
    }

    let mut results = Vec::new();
    collect_relative_files_recursive(root, root, &mut results)?;
    results.sort();
    Ok(Some(results))
}

fn collect_relative_files_recursive(
    root: &Path,
    current_dir: &Path,
    results: &mut Vec<String>,
) -> Result<(), RegressionRunnerError> {
    let directory =
        fs::read_dir(current_dir).map_err(|source| RegressionRunnerError::ReadDirectory {
            path: current_dir.to_path_buf(),
            source,
        })?;

    for entry in directory {
        let entry = entry.map_err(|source| RegressionRunnerError::ReadDirectory {
            path: current_dir.to_path_buf(),
            source,
        })?;
        let entry_path = entry.path();
        let file_type =
            entry
                .file_type()
                .map_err(|source| RegressionRunnerError::ReadDirectory {
                    path: entry_path.clone(),
                    source,
                })?;

        if file_type.is_dir() {
            collect_relative_files_recursive(root, &entry_path, results)?;
            continue;
        }

        if file_type.is_file() {
            let relative_path = entry_path
                .strip_prefix(root)
                .unwrap_or(&entry_path)
                .to_string_lossy()
                .replace('\\', "/");
            results.push(relative_path);
        }
    }

    Ok(())
}

fn write_report_file(
    report_path: &Path,
    report: &RegressionRunReport,
) -> Result<(), RegressionRunnerError> {
    if let Some(parent_dir) = report_path.parent() {
        fs::create_dir_all(parent_dir).map_err(|source| {
            RegressionRunnerError::ReportDirectory {
                path: parent_dir.to_path_buf(),
                source,
            }
        })?;
    }

    let report_json = serde_json::to_string_pretty(report).map_err(|source| {
        RegressionRunnerError::SerializeReport {
            path: report_path.to_path_buf(),
            source,
        }
    })?;
    fs::write(report_path, report_json).map_err(|source| RegressionRunnerError::WriteReport {
        path: report_path.to_path_buf(),
        source,
    })
}

fn current_unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::{RegressionRunnerConfig, render_human_summary, run_regression};
    use serde_json::Value;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn run_regression_builds_report_and_applies_thresholds() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "defaultComparison": {
                "passFailThreshold": {
                  "minimumArtifactPassRate": 1.0,
                  "maxArtifactFailures": 0
                }
              },
              "fixtures": [
                { "id": "FX-PASS-001" },
                {
                  "id": "FX-FAIL-001",
                  "comparison": {
                    "passFailThreshold": {
                      "minimumArtifactPassRate": 0.75,
                      "maxArtifactFailures": 1
                    }
                  }
                }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        write_fixture_file(
            &baseline_root,
            "FX-PASS-001",
            "baseline",
            "alpha.dat",
            "same\n",
        );
        write_fixture_file(&actual_root, "FX-PASS-001", "actual", "alpha.dat", "same\n");

        write_fixture_file(&baseline_root, "FX-FAIL-001", "baseline", "a.dat", "same\n");
        write_fixture_file(&actual_root, "FX-FAIL-001", "actual", "a.dat", "same\n");
        write_fixture_file(&baseline_root, "FX-FAIL-001", "baseline", "b.dat", "left\n");
        write_fixture_file(&actual_root, "FX-FAIL-001", "actual", "b.dat", "right\n");
        write_fixture_file(&baseline_root, "FX-FAIL-001", "baseline", "c.dat", "same\n");
        write_fixture_file(&actual_root, "FX-FAIL-001", "actual", "c.dat", "same\n");
        write_fixture_file(&baseline_root, "FX-FAIL-001", "baseline", "d.dat", "same\n");
        write_fixture_file(&actual_root, "FX-FAIL-001", "actual", "d.dat", "same\n");

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root,
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path: report_path.clone(),
            run_rdinp: false,
            run_pot: false,
            run_xsph: false,
            run_path: false,
            run_fms: false,
            run_band: false,
            run_ldos: false,
            run_rixs: false,
            run_crpa: false,
            run_compton: false,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should succeed");
        assert!(report.passed);
        assert_eq!(report.fixture_count, 2);
        assert_eq!(report.passed_fixture_count, 2);
        assert_eq!(report.failed_fixture_count, 0);
        assert_eq!(report.artifact_count, 5);
        assert_eq!(report.failed_artifact_count, 1);

        let summary = render_human_summary(&report);
        assert!(summary.contains("Regression status: PASS"));
        assert!(report_path.exists());

        let report_json = fs::read_to_string(&report_path).expect("report file should be readable");
        let parsed: Value = serde_json::from_str(&report_json).expect("report should parse");
        assert_eq!(parsed["passed"], Value::Bool(true));
    }

    #[test]
    fn run_regression_marks_missing_actual_file_as_failure() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "fixtures": [
                { "id": "FX-MISSING-001" }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        write_fixture_file(
            &baseline_root,
            "FX-MISSING-001",
            "baseline",
            "log.dat",
            "baseline only\n",
        );

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root,
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: false,
            run_pot: false,
            run_xsph: false,
            run_path: false,
            run_fms: false,
            run_band: false,
            run_ldos: false,
            run_rixs: false,
            run_crpa: false,
            run_compton: false,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);
        assert_eq!(report.failed_fixture_count, 1);
        assert_eq!(report.failed_artifact_count, 1);

        let fixture = &report.fixtures[0];
        assert!(!fixture.passed);
        assert_eq!(fixture.artifacts[0].artifact_path, "log.dat");
        assert_eq!(
            fixture.artifacts[0].reason.as_deref(),
            Some("Missing actual artifact")
        );
    }

    #[test]
    fn run_regression_can_execute_rdinp_path() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");
        let input_dir = temp.path().join("fixtures/FX-RDINP-001");

        write_file(
            &input_dir.join("feff.inp"),
            "TITLE Cu\nPOTENTIALS\n0 29 Cu\nATOMS\n0.0 0.0 0.0 0 Cu\nEND\n",
        );

        let manifest_json = format!(
            "{{\n  \"fixtures\": [\n    {{\n      \"id\": \"FX-RDINP-001\",\n      \"modulesCovered\": [\"RDINP\"],\n      \"inputDirectory\": \"{}\"\n    }}\n  ]\n}}",
            input_dir.to_string_lossy().replace('\\', "\\\\")
        );
        write_file(&manifest_path, &manifest_json);

        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root: actual_root.clone(),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: true,
            run_pot: false,
            run_xsph: false,
            run_path: false,
            run_fms: false,
            run_band: false,
            run_ldos: false,
            run_rixs: false,
            run_crpa: false,
            run_compton: false,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);

        let generated = actual_root
            .join("FX-RDINP-001")
            .join("actual")
            .join("log.dat");
        assert!(generated.exists(), "RDINP output should exist");
    }

    #[test]
    fn run_regression_can_execute_pot_path() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "fixtures": [
                {
                  "id": "FX-POT-001",
                  "modulesCovered": ["POT"]
                }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        let staged_dir = actual_root.join("FX-POT-001").join("actual");
        copy_repo_fixture_file("FX-POT-001", "pot.inp", &staged_dir.join("pot.inp"));
        copy_repo_fixture_file("FX-POT-001", "geom.dat", &staged_dir.join("geom.dat"));

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root: actual_root.clone(),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: false,
            run_pot: true,
            run_xsph: false,
            run_path: false,
            run_fms: false,
            run_band: false,
            run_ldos: false,
            run_rixs: false,
            run_crpa: false,
            run_compton: false,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);

        let generated = actual_root
            .join("FX-POT-001")
            .join("actual")
            .join("log1.dat");
        assert!(generated.exists(), "POT output should exist");
    }

    #[test]
    fn run_regression_can_execute_path_scaffold() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "fixtures": [
                {
                  "id": "FX-PATH-001",
                  "modulesCovered": ["PATH"]
                }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        let staged_dir = actual_root.join("FX-PATH-001").join("actual");
        copy_repo_fixture_file("FX-PATH-001", "paths.inp", &staged_dir.join("paths.inp"));
        copy_repo_fixture_file("FX-PATH-001", "geom.dat", &staged_dir.join("geom.dat"));
        copy_repo_fixture_file("FX-PATH-001", "global.inp", &staged_dir.join("global.inp"));
        copy_repo_fixture_file("FX-PATH-001", "phase.bin", &staged_dir.join("phase.bin"));

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root: actual_root.clone(),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: false,
            run_pot: false,
            run_xsph: false,
            run_path: true,
            run_fms: false,
            run_band: false,
            run_ldos: false,
            run_rixs: false,
            run_crpa: false,
            run_compton: false,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);

        let generated = actual_root
            .join("FX-PATH-001")
            .join("actual")
            .join("log4.dat");
        assert!(generated.exists(), "PATH output should exist");
    }

    #[test]
    fn run_regression_can_execute_xsph_scaffold() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "fixtures": [
                {
                  "id": "FX-XSPH-001",
                  "modulesCovered": ["XSPH"]
                }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        let staged_dir = actual_root.join("FX-XSPH-001").join("actual");
        copy_repo_fixture_file("FX-XSPH-001", "xsph.inp", &staged_dir.join("xsph.inp"));
        copy_repo_fixture_file("FX-XSPH-001", "geom.dat", &staged_dir.join("geom.dat"));
        copy_repo_fixture_file("FX-XSPH-001", "global.inp", &staged_dir.join("global.inp"));
        copy_repo_fixture_file("FX-XSPH-001", "pot.bin", &staged_dir.join("pot.bin"));
        copy_repo_fixture_file("FX-XSPH-001", "wscrn.dat", &staged_dir.join("wscrn.dat"));

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root: actual_root.clone(),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: false,
            run_pot: false,
            run_xsph: true,
            run_path: false,
            run_fms: false,
            run_band: false,
            run_ldos: false,
            run_rixs: false,
            run_crpa: false,
            run_compton: false,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);

        let generated = actual_root
            .join("FX-XSPH-001")
            .join("actual")
            .join("log2.dat");
        assert!(generated.exists(), "XSPH output should exist");
    }

    #[test]
    fn run_regression_can_execute_fms_scaffold() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "fixtures": [
                {
                  "id": "FX-FMS-001",
                  "modulesCovered": ["FMS"]
                }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        let staged_dir = actual_root.join("FX-FMS-001").join("actual");
        copy_repo_fixture_file("FX-FMS-001", "fms.inp", &staged_dir.join("fms.inp"));
        copy_repo_fixture_file("FX-FMS-001", "geom.dat", &staged_dir.join("geom.dat"));
        copy_repo_fixture_file("FX-FMS-001", "global.inp", &staged_dir.join("global.inp"));
        copy_repo_fixture_file("FX-FMS-001", "phase.bin", &staged_dir.join("phase.bin"));

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root: actual_root.clone(),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: false,
            run_pot: false,
            run_xsph: false,
            run_path: false,
            run_fms: true,
            run_band: false,
            run_ldos: false,
            run_rixs: false,
            run_crpa: false,
            run_compton: false,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);

        let generated = actual_root
            .join("FX-FMS-001")
            .join("actual")
            .join("log3.dat");
        assert!(generated.exists(), "FMS output should exist");
    }

    #[test]
    fn run_regression_can_execute_band_scaffold() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "fixtures": [
                {
                  "id": "FX-BAND-001",
                  "modulesCovered": ["BAND"]
                }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        let staged_dir = actual_root.join("FX-BAND-001").join("actual");
        stage_repo_band_input("FX-BAND-001", &staged_dir.join("band.inp"));
        copy_repo_fixture_file("FX-BAND-001", "geom.dat", &staged_dir.join("geom.dat"));
        copy_repo_fixture_file("FX-BAND-001", "global.inp", &staged_dir.join("global.inp"));
        copy_repo_fixture_file("FX-BAND-001", "phase.bin", &staged_dir.join("phase.bin"));

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root: actual_root.clone(),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: false,
            run_pot: false,
            run_xsph: false,
            run_path: false,
            run_fms: false,
            run_band: true,
            run_ldos: false,
            run_rixs: false,
            run_crpa: false,
            run_compton: false,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);

        let output_dir = actual_root.join("FX-BAND-001").join("actual");
        let has_band_output = ["bandstructure.dat", "logband.dat", "list.dat", "log5.dat"]
            .iter()
            .any(|artifact| output_dir.join(artifact).is_file());
        assert!(has_band_output, "BAND output should exist");
    }

    #[test]
    fn run_regression_can_execute_ldos_scaffold() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "fixtures": [
                {
                  "id": "FX-LDOS-001",
                  "modulesCovered": ["LDOS"]
                }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        let staged_dir = actual_root.join("FX-LDOS-001").join("actual");
        copy_repo_fixture_file("FX-LDOS-001", "ldos.inp", &staged_dir.join("ldos.inp"));
        copy_repo_fixture_file("FX-LDOS-001", "geom.dat", &staged_dir.join("geom.dat"));
        copy_repo_fixture_file("FX-LDOS-001", "pot.bin", &staged_dir.join("pot.bin"));
        copy_repo_fixture_file(
            "FX-LDOS-001",
            "reciprocal.inp",
            &staged_dir.join("reciprocal.inp"),
        );

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root: actual_root.clone(),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: false,
            run_pot: false,
            run_xsph: false,
            run_path: false,
            run_fms: false,
            run_band: false,
            run_ldos: true,
            run_rixs: false,
            run_crpa: false,
            run_compton: false,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);

        let output_dir = actual_root.join("FX-LDOS-001").join("actual");
        let has_ldos_output = fs::read_dir(&output_dir)
            .expect("LDOS output directory should be readable")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_ascii_lowercase())
            .any(|name| {
                name == "logdos.dat" || (name.starts_with("ldos") && name.ends_with(".dat"))
            });
        assert!(has_ldos_output, "LDOS output should exist");
    }

    #[test]
    fn run_regression_can_execute_rixs_scaffold() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "fixtures": [
                {
                  "id": "FX-RIXS-001",
                  "modulesCovered": ["RIXS"]
                }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        let staged_dir = actual_root.join("FX-RIXS-001").join("actual");
        stage_repo_rixs_inputs("FX-RIXS-001", &staged_dir);

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root: actual_root.clone(),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: false,
            run_pot: false,
            run_xsph: false,
            run_path: false,
            run_fms: false,
            run_band: false,
            run_ldos: false,
            run_rixs: true,
            run_crpa: false,
            run_compton: false,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);

        let output_dir = actual_root.join("FX-RIXS-001").join("actual");
        let has_rixs_output = [
            "rixs0.dat",
            "rixs1.dat",
            "rixsET.dat",
            "rixsEE.dat",
            "rixsET-sat.dat",
            "rixsEE-sat.dat",
            "logrixs.dat",
            "referenceherfd.dat",
            "referenceherfd-sat.dat",
            "referencerixsET.dat",
        ]
        .iter()
        .any(|artifact| output_dir.join(artifact).is_file());
        assert!(has_rixs_output, "RIXS output should exist");
    }

    #[test]
    fn run_regression_can_execute_crpa_scaffold() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "fixtures": [
                {
                  "id": "FX-CRPA-001",
                  "modulesCovered": ["CRPA"]
                }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        let staged_dir = actual_root.join("FX-CRPA-001").join("actual");
        copy_repo_fixture_file("FX-CRPA-001", "crpa.inp", &staged_dir.join("crpa.inp"));
        copy_repo_fixture_file("FX-CRPA-001", "pot.inp", &staged_dir.join("pot.inp"));
        copy_repo_fixture_file("FX-CRPA-001", "geom.dat", &staged_dir.join("geom.dat"));

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root: actual_root.clone(),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: false,
            run_pot: false,
            run_xsph: false,
            run_path: false,
            run_fms: false,
            run_band: false,
            run_ldos: false,
            run_rixs: false,
            run_crpa: true,
            run_compton: false,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);

        let output_dir = actual_root.join("FX-CRPA-001").join("actual");
        let has_crpa_output = ["wscrn.dat", "logscrn.dat"]
            .iter()
            .any(|artifact| output_dir.join(artifact).is_file());
        assert!(has_crpa_output, "CRPA output should exist");
    }

    #[test]
    fn run_regression_can_execute_compton_scaffold() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "fixtures": [
                {
                  "id": "FX-COMPTON-001",
                  "modulesCovered": ["COMPTON"]
                }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        let staged_dir = actual_root.join("FX-COMPTON-001").join("actual");
        stage_repo_compton_inputs("FX-COMPTON-001", &staged_dir);

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root: actual_root.clone(),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: false,
            run_pot: false,
            run_xsph: false,
            run_path: false,
            run_fms: false,
            run_band: false,
            run_ldos: false,
            run_rixs: false,
            run_crpa: false,
            run_compton: true,
            run_debye: false,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);

        let output_dir = actual_root.join("FX-COMPTON-001").join("actual");
        let has_compton_output = ["compton.dat", "jzzp.dat", "rhozzp.dat", "logcompton.dat"]
            .iter()
            .any(|artifact| output_dir.join(artifact).is_file());
        assert!(has_compton_output, "COMPTON output should exist");
    }

    #[test]
    fn run_regression_can_execute_debye_scaffold() {
        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_root = temp.path().join("baseline-root");
        let actual_root = temp.path().join("actual-root");
        let report_path = temp.path().join("reports/report.json");
        let manifest_path = temp.path().join("manifest.json");
        let policy_path = temp.path().join("policy.json");

        write_file(
            &manifest_path,
            r#"
            {
              "fixtures": [
                {
                  "id": "FX-DEBYE-001",
                  "modulesCovered": ["DEBYE"]
                }
              ]
            }
            "#,
        );
        write_file(
            &policy_path,
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        );

        let staged_dir = actual_root.join("FX-DEBYE-001").join("actual");
        stage_repo_debye_inputs("FX-DEBYE-001", &staged_dir);

        let config = RegressionRunnerConfig {
            manifest_path,
            policy_path,
            baseline_root,
            actual_root: actual_root.clone(),
            baseline_subdir: "baseline".to_string(),
            actual_subdir: "actual".to_string(),
            report_path,
            run_rdinp: false,
            run_pot: false,
            run_xsph: false,
            run_path: false,
            run_fms: false,
            run_band: false,
            run_ldos: false,
            run_rixs: false,
            run_crpa: false,
            run_compton: false,
            run_debye: true,
        };

        let report = run_regression(&config).expect("runner should produce report");
        assert!(!report.passed);

        let output_dir = actual_root.join("FX-DEBYE-001").join("actual");
        let has_debye_output = [
            "s2_em.dat",
            "s2_rm1.dat",
            "s2_rm2.dat",
            "xmu.dat",
            "chi.dat",
            "log6.dat",
            "spring.dat",
        ]
        .iter()
        .any(|artifact| output_dir.join(artifact).is_file());
        assert!(has_debye_output, "DEBYE output should exist");
    }

    fn write_fixture_file(
        root: &Path,
        fixture_id: &str,
        subdir: &str,
        relative_path: &str,
        content: &str,
    ) {
        let path = root.join(fixture_id).join(subdir).join(relative_path);
        write_file(&path, content);
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir should be created");
        }
        fs::write(path, content).expect("file should be written");
    }

    fn copy_repo_fixture_file(fixture_id: &str, relative_path: &str, destination: &Path) {
        let source = Path::new("artifacts/fortran-baselines")
            .join(fixture_id)
            .join("baseline")
            .join(relative_path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should be created");
        }
        fs::copy(&source, destination).expect("fixture file should be copied");
    }

    fn stage_repo_band_input(fixture_id: &str, destination: &Path) {
        let source = Path::new("artifacts/fortran-baselines")
            .join(fixture_id)
            .join("baseline")
            .join("band.inp");
        if source.is_file() {
            copy_repo_fixture_file(fixture_id, "band.inp", destination);
            return;
        }

        write_file(
            destination,
            "mband : calculate bands if = 1\n   0\nemin, emax, estep : energy mesh\n      0.00000      0.00000      0.00000\nnkp : # points in k-path\n   0\nikpath : type of k-path\n  -1\nfreeprop :  empty lattice if = T\n F\n",
        );
    }

    fn stage_repo_rixs_inputs(fixture_id: &str, destination_dir: &Path) {
        stage_repo_text_input(
            fixture_id,
            "rixs.inp",
            &destination_dir.join("rixs.inp"),
            "nenergies\n3\nemin emax estep\n-10.0 10.0 0.5\n",
        );
        stage_repo_binary_input(
            fixture_id,
            "phase_1.bin",
            &destination_dir.join("phase_1.bin"),
            &[0_u8, 1_u8, 2_u8, 3_u8],
        );
        stage_repo_binary_input(
            fixture_id,
            "phase_2.bin",
            &destination_dir.join("phase_2.bin"),
            &[4_u8, 5_u8, 6_u8, 7_u8],
        );
        stage_repo_text_input(
            fixture_id,
            "wscrn_1.dat",
            &destination_dir.join("wscrn_1.dat"),
            "0.0 0.0 0.0\n",
        );
        stage_repo_text_input(
            fixture_id,
            "wscrn_2.dat",
            &destination_dir.join("wscrn_2.dat"),
            "0.0 0.0 0.0\n",
        );
        stage_repo_text_input(
            fixture_id,
            "xsect_2.dat",
            &destination_dir.join("xsect_2.dat"),
            "0.0 0.0 0.0\n",
        );
    }

    fn stage_repo_compton_inputs(fixture_id: &str, destination_dir: &Path) {
        stage_repo_text_input(
            fixture_id,
            "compton.inp",
            &destination_dir.join("compton.inp"),
            "icore: core level index\n1\nemin emax estep\n-10.0 10.0 0.5\n",
        );
        stage_repo_binary_input(
            fixture_id,
            "pot.bin",
            &destination_dir.join("pot.bin"),
            &[0_u8, 1_u8, 2_u8, 3_u8],
        );
        stage_repo_binary_input(
            fixture_id,
            "gg_slice.bin",
            &destination_dir.join("gg_slice.bin"),
            &[4_u8, 5_u8, 6_u8, 7_u8],
        );
    }

    fn stage_repo_debye_inputs(fixture_id: &str, destination_dir: &Path) {
        stage_repo_text_input(
            fixture_id,
            "ff2x.inp",
            &destination_dir.join("ff2x.inp"),
            "DEBYE PARAMETERS\n0.0 0.0 0.0\n",
        );
        stage_repo_text_input(
            fixture_id,
            "paths.dat",
            &destination_dir.join("paths.dat"),
            "PATHS PLACEHOLDER\n",
        );
        stage_repo_text_input(
            fixture_id,
            "feff.inp",
            &destination_dir.join("feff.inp"),
            "TITLE Cu\nEND\n",
        );
        stage_repo_text_input(
            fixture_id,
            "spring.inp",
            &destination_dir.join("spring.inp"),
            "0.0 0.0 0.0\n",
        );
    }

    fn stage_repo_text_input(
        fixture_id: &str,
        relative_path: &str,
        destination: &Path,
        fallback_content: &str,
    ) {
        let source = Path::new("artifacts/fortran-baselines")
            .join(fixture_id)
            .join("baseline")
            .join(relative_path);
        if source.is_file() {
            copy_repo_fixture_file(fixture_id, relative_path, destination);
            return;
        }

        write_file(destination, fallback_content);
    }

    fn stage_repo_binary_input(
        fixture_id: &str,
        relative_path: &str,
        destination: &Path,
        fallback_bytes: &[u8],
    ) {
        let source = Path::new("artifacts/fortran-baselines")
            .join(fixture_id)
            .join("baseline")
            .join(relative_path);
        if source.is_file() {
            copy_repo_fixture_file(fixture_id, relative_path, destination);
            return;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("destination directory should be created");
        }
        fs::write(destination, fallback_bytes).expect("binary input should be written");
    }
}
