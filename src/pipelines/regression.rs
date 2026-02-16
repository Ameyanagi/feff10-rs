use super::comparator::{ArtifactComparisonResult, Comparator, ComparatorError};
use crate::domain::{FeffError, PipelineResult};
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
    Comparator(ComparatorError),
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
            Self::Comparator(source) => write!(f, "comparator setup failed: {}", source),
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
            Self::Comparator(source) => Some(source),
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
            RegressionRunnerError::Comparator(source) => source.into(),
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
    #[serde(default)]
    comparison: Option<ManifestComparison>,
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
}
