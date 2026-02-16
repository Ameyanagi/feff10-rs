use crate::domain::FeffError;
use crate::numerics::{NumericTolerance, compare_with_policy_tolerance, format_numeric_for_policy};
use globset::{Glob, GlobMatcher};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Comparator {
    default_mode: ComparisonMode,
    match_strategy: MatchStrategy,
    numeric_parsing: NumericParsingOptions,
    categories: Vec<CompiledCategory>,
}

struct CompiledCategory {
    id: String,
    mode: ComparisonMode,
    tolerance: Option<NumericTolerance>,
    matchers: Vec<GlobMatcher>,
}

impl CompiledCategory {
    fn matches(&self, artifact_path: &str) -> bool {
        let path = Path::new(artifact_path);
        self.matchers.iter().any(|matcher| matcher.is_match(path))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonMode {
    ExactText,
    NumericTolerance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum MatchStrategy {
    #[default]
    FirstMatch,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResolvedRule {
    pub mode: ComparisonMode,
    pub category_id: Option<String>,
    pub tolerance: Option<NumericTolerance>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArtifactComparisonResult {
    pub artifact_path: String,
    pub mode: ComparisonMode,
    pub matched_category: Option<String>,
    pub passed: bool,
    pub reason: Option<String>,
    pub metrics: ArtifactComparisonMetrics,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArtifactComparisonMetrics {
    ExactText(ExactTextMetrics),
    NumericTolerance(NumericToleranceMetrics),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExactTextMetrics {
    pub baseline_bytes: usize,
    pub actual_bytes: usize,
    pub first_mismatch_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NumericToleranceMetrics {
    pub baseline_value_count: usize,
    pub actual_value_count: usize,
    pub compared_values: usize,
    pub failing_values: usize,
    pub max_abs_diff: f64,
    pub max_rel_diff: f64,
    pub tolerance: NumericTolerance,
}

#[derive(Debug, Clone)]
pub struct ArtifactPair {
    pub artifact_path: PathBuf,
    pub baseline_path: PathBuf,
    pub actual_path: PathBuf,
}

impl ArtifactPair {
    pub fn new(
        artifact_path: impl Into<PathBuf>,
        baseline_path: impl Into<PathBuf>,
        actual_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            artifact_path: artifact_path.into(),
            baseline_path: baseline_path.into(),
            actual_path: actual_path.into(),
        }
    }
}

impl Comparator {
    pub fn from_policy_path(policy_path: impl AsRef<Path>) -> Result<Self, ComparatorError> {
        let policy_path = policy_path.as_ref();
        let content =
            fs::read_to_string(policy_path).map_err(|source| ComparatorError::ReadPolicy {
                path: policy_path.to_path_buf(),
                source,
            })?;
        let raw: RawPolicy =
            serde_json::from_str(&content).map_err(|source| ComparatorError::ParsePolicy {
                path: policy_path.to_path_buf(),
                source,
            })?;
        Self::from_raw_policy(raw)
    }

    pub fn from_policy_json(policy_json: &str) -> Result<Self, ComparatorError> {
        let raw: RawPolicy =
            serde_json::from_str(policy_json).map_err(|source| ComparatorError::ParsePolicy {
                path: PathBuf::from("<inline-policy>"),
                source,
            })?;
        Self::from_raw_policy(raw)
    }

    pub fn resolve_rule_for_artifact(&self, artifact_path: impl AsRef<Path>) -> ResolvedRule {
        let artifact_path = normalize_artifact_path(artifact_path.as_ref());
        match self.match_strategy {
            MatchStrategy::FirstMatch => {
                for category in &self.categories {
                    if category.matches(&artifact_path) {
                        return ResolvedRule {
                            mode: category.mode,
                            category_id: Some(category.id.clone()),
                            tolerance: category.tolerance,
                        };
                    }
                }
            }
        }

        ResolvedRule {
            mode: self.default_mode,
            category_id: None,
            tolerance: None,
        }
    }

    pub fn compare_artifact(
        &self,
        artifact_path: impl AsRef<Path>,
        baseline_path: impl AsRef<Path>,
        actual_path: impl AsRef<Path>,
    ) -> Result<ArtifactComparisonResult, ComparatorError> {
        let artifact_path = normalize_artifact_path(artifact_path.as_ref());
        let rule = self.resolve_rule_for_artifact(&artifact_path);

        match rule.mode {
            ComparisonMode::ExactText => self.compare_exact_text(
                artifact_path,
                rule,
                baseline_path.as_ref(),
                actual_path.as_ref(),
            ),
            ComparisonMode::NumericTolerance => self.compare_numeric_tolerance(
                artifact_path,
                rule,
                baseline_path.as_ref(),
                actual_path.as_ref(),
            ),
        }
    }

    pub fn compare_artifacts(
        &self,
        artifacts: &[ArtifactPair],
    ) -> Result<Vec<ArtifactComparisonResult>, ComparatorError> {
        artifacts
            .iter()
            .map(|artifact| {
                self.compare_artifact(
                    &artifact.artifact_path,
                    &artifact.baseline_path,
                    &artifact.actual_path,
                )
            })
            .collect()
    }

    fn from_raw_policy(raw: RawPolicy) -> Result<Self, ComparatorError> {
        let mut categories = Vec::with_capacity(raw.categories.len());
        for category in raw.categories {
            let mut matchers = Vec::with_capacity(category.file_globs.len());
            for pattern in category.file_globs {
                let matcher = Glob::new(&pattern)
                    .map_err(|source| ComparatorError::InvalidGlob {
                        pattern: pattern.clone(),
                        source,
                    })?
                    .compile_matcher();
                matchers.push(matcher);
            }

            if category.mode == ComparisonMode::NumericTolerance && category.tolerance.is_none() {
                return Err(ComparatorError::InvalidPolicy(format!(
                    "category '{}' uses numeric_tolerance but does not define tolerance",
                    category.id
                )));
            }

            categories.push(CompiledCategory {
                id: category.id,
                mode: category.mode,
                tolerance: category.tolerance,
                matchers,
            });
        }

        Ok(Self {
            default_mode: raw.default_mode,
            match_strategy: raw.match_strategy,
            numeric_parsing: raw.numeric_parsing,
            categories,
        })
    }

    fn compare_exact_text(
        &self,
        artifact_path: String,
        rule: ResolvedRule,
        baseline_path: &Path,
        actual_path: &Path,
    ) -> Result<ArtifactComparisonResult, ComparatorError> {
        let baseline_bytes = read_artifact_bytes(baseline_path)?;
        let actual_bytes = read_artifact_bytes(actual_path)?;
        let mismatch = first_mismatch_offset(&baseline_bytes, &actual_bytes);
        let passed = mismatch.is_none();
        let reason = mismatch.map(|offset| {
            format!(
                "Exact-text mismatch at byte {} (baseline={} bytes, actual={} bytes).",
                offset,
                baseline_bytes.len(),
                actual_bytes.len()
            )
        });

        Ok(ArtifactComparisonResult {
            artifact_path,
            mode: rule.mode,
            matched_category: rule.category_id,
            passed,
            reason,
            metrics: ArtifactComparisonMetrics::ExactText(ExactTextMetrics {
                baseline_bytes: baseline_bytes.len(),
                actual_bytes: actual_bytes.len(),
                first_mismatch_offset: mismatch,
            }),
        })
    }

    fn compare_numeric_tolerance(
        &self,
        artifact_path: String,
        rule: ResolvedRule,
        baseline_path: &Path,
        actual_path: &Path,
    ) -> Result<ArtifactComparisonResult, ComparatorError> {
        let tolerance = rule
            .tolerance
            .ok_or_else(|| ComparatorError::MissingTolerance {
                artifact_path: artifact_path.clone(),
                category_id: rule.category_id.clone(),
            })?;
        let baseline_text = read_artifact_utf8(baseline_path)?;
        let actual_text = read_artifact_utf8(actual_path)?;

        let baseline_rows = self.parse_numeric_rows(&baseline_text).map_err(|source| {
            ComparatorError::NumericParse {
                path: baseline_path.to_path_buf(),
                source,
            }
        })?;
        let actual_rows = self.parse_numeric_rows(&actual_text).map_err(|source| {
            ComparatorError::NumericParse {
                path: actual_path.to_path_buf(),
                source,
            }
        })?;

        let baseline_value_count = baseline_rows.iter().map(Vec::len).sum::<usize>();
        let actual_value_count = actual_rows.iter().map(Vec::len).sum::<usize>();
        let line_count_mismatch = baseline_rows.len() != actual_rows.len();
        let mut first_token_count_mismatch: Option<(usize, usize, usize)> = None;

        let mut compared_values = 0usize;
        let mut failing_values = 0usize;
        let mut max_abs_diff = 0.0_f64;
        let mut max_rel_diff = 0.0_f64;
        let mut first_failure: Option<String> = None;
        let mut comparison_index = 0usize;

        for (line_index, (baseline_row, actual_row)) in
            baseline_rows.iter().zip(actual_rows.iter()).enumerate()
        {
            let row_compared = baseline_row.len().min(actual_row.len());
            let row_unmatched = baseline_row.len().max(actual_row.len()) - row_compared;
            compared_values += row_compared;
            if row_unmatched > 0 {
                failing_values += row_unmatched;
                if first_token_count_mismatch.is_none() {
                    first_token_count_mismatch =
                        Some((line_index + 1, baseline_row.len(), actual_row.len()));
                }
            }

            for (baseline_value, actual_value) in baseline_row.iter().zip(actual_row.iter()) {
                if baseline_value.is_finite() && actual_value.is_finite() {
                    let comparison =
                        compare_with_policy_tolerance(*baseline_value, *actual_value, tolerance);
                    max_abs_diff = max_abs_diff.max(comparison.abs_diff);
                    max_rel_diff = max_rel_diff.max(comparison.rel_diff);

                    if !comparison.passes {
                        failing_values += 1;
                        if first_failure.is_none() {
                            first_failure = Some(format!(
                                "index {} baseline={} actual={} abs_diff={} rel_diff={}",
                                comparison_index,
                                format_numeric_for_policy(*baseline_value),
                                format_numeric_for_policy(*actual_value),
                                format_numeric_for_policy(comparison.abs_diff),
                                format_numeric_for_policy(comparison.rel_diff)
                            ));
                        }
                    }
                    comparison_index += 1;
                    continue;
                }

                let non_finite_match = non_finite_values_match(*baseline_value, *actual_value);
                let finite_mismatch = baseline_value.is_finite() != actual_value.is_finite();
                let should_fail = finite_mismatch
                    || (self.numeric_parsing.fail_on_nan_or_inf_mismatch && !non_finite_match);

                if should_fail {
                    failing_values += 1;
                    if first_failure.is_none() {
                        first_failure = Some(format!(
                            "index {} baseline={} actual={} non-finite mismatch",
                            comparison_index,
                            format_numeric_for_policy(*baseline_value),
                            format_numeric_for_policy(*actual_value)
                        ));
                    }
                }

                comparison_index += 1;
            }
        }

        for row in baseline_rows.iter().skip(actual_rows.len()) {
            failing_values += row.len();
        }
        for row in actual_rows.iter().skip(baseline_rows.len()) {
            failing_values += row.len();
        }

        let passed = failing_values == 0;
        let reason = if passed {
            None
        } else if line_count_mismatch {
            Some(format!(
                "Numeric line count mismatch (baseline={}, actual={}).",
                baseline_rows.len(),
                actual_rows.len()
            ))
        } else if let Some((line, baseline_tokens, actual_tokens)) = first_token_count_mismatch {
            Some(format!(
                "Numeric token count mismatch at line {} (baseline={}, actual={}).",
                line, baseline_tokens, actual_tokens
            ))
        } else if baseline_value_count != actual_value_count {
            Some(format!(
                "Numeric token count mismatch (baseline={}, actual={}).",
                baseline_value_count, actual_value_count
            ))
        } else {
            Some(format!(
                "Numeric comparison found {} value(s) outside tolerance. {}",
                failing_values,
                first_failure.unwrap_or_else(|| "No failure details available.".to_string())
            ))
        };

        Ok(ArtifactComparisonResult {
            artifact_path,
            mode: rule.mode,
            matched_category: rule.category_id,
            passed,
            reason,
            metrics: ArtifactComparisonMetrics::NumericTolerance(NumericToleranceMetrics {
                baseline_value_count,
                actual_value_count,
                compared_values,
                failing_values,
                max_abs_diff,
                max_rel_diff,
                tolerance,
            }),
        })
    }

    fn parse_numeric_rows(&self, input: &str) -> Result<Vec<Vec<f64>>, NumericParseError> {
        let mut rows = Vec::new();

        for (line_index, raw_line) in input.lines().enumerate() {
            let mut line = if self.numeric_parsing.trim_whitespace {
                raw_line.trim().to_string()
            } else {
                raw_line.to_string()
            };

            if self.numeric_parsing.skip_empty_lines && line.is_empty() {
                continue;
            }

            if starts_with_comment(&line, &self.numeric_parsing.comment_prefixes) {
                continue;
            }

            if self.numeric_parsing.collapse_whitespace {
                line = line.split_whitespace().collect::<Vec<_>>().join(" ");
            }

            let mut row = Vec::new();
            for (token_index, token) in line.split_whitespace().enumerate() {
                let normalized =
                    normalize_numeric_token(token, &self.numeric_parsing.fortran_exponent_markers);
                let value = normalized.parse::<f64>().map_err(|_| NumericParseError {
                    line: line_index + 1,
                    token_index: token_index + 1,
                    token: token.to_string(),
                })?;
                row.push(value);
            }
            if !row.is_empty() {
                rows.push(row);
            }
        }

        Ok(rows)
    }
}

#[derive(Debug)]
pub enum ComparatorError {
    ReadPolicy {
        path: PathBuf,
        source: std::io::Error,
    },
    ParsePolicy {
        path: PathBuf,
        source: serde_json::Error,
    },
    InvalidPolicy(String),
    InvalidGlob {
        pattern: String,
        source: globset::Error,
    },
    ReadArtifact {
        path: PathBuf,
        source: std::io::Error,
    },
    DecodeArtifact {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },
    NumericParse {
        path: PathBuf,
        source: NumericParseError,
    },
    MissingTolerance {
        artifact_path: String,
        category_id: Option<String>,
    },
}

impl Display for ComparatorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadPolicy { path, source } => {
                write!(f, "failed to read policy '{}': {}", path.display(), source)
            }
            Self::ParsePolicy { path, source } => {
                write!(f, "failed to parse policy '{}': {}", path.display(), source)
            }
            Self::InvalidPolicy(message) => write!(f, "invalid policy: {}", message),
            Self::InvalidGlob { pattern, source } => {
                write!(f, "invalid glob pattern '{}': {}", pattern, source)
            }
            Self::ReadArtifact { path, source } => {
                write!(
                    f,
                    "failed to read artifact '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::DecodeArtifact { path, source } => {
                write!(
                    f,
                    "artifact '{}' is not valid UTF-8: {}",
                    path.display(),
                    source
                )
            }
            Self::NumericParse { path, source } => {
                write!(
                    f,
                    "failed to parse numeric content in '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::MissingTolerance {
                artifact_path,
                category_id,
            } => write!(
                f,
                "numeric_tolerance selected for artifact '{}' without tolerance (category={:?})",
                artifact_path, category_id
            ),
        }
    }
}

impl Error for ComparatorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadPolicy { source, .. } => Some(source),
            Self::ParsePolicy { source, .. } => Some(source),
            Self::InvalidGlob { source, .. } => Some(source),
            Self::ReadArtifact { source, .. } => Some(source),
            Self::DecodeArtifact { source, .. } => Some(source),
            Self::NumericParse { source, .. } => Some(source),
            Self::InvalidPolicy(_) | Self::MissingTolerance { .. } => None,
        }
    }
}

impl From<ComparatorError> for FeffError {
    fn from(error: ComparatorError) -> Self {
        let message = error.to_string();
        match error {
            ComparatorError::ReadPolicy { .. } | ComparatorError::ReadArtifact { .. } => {
                FeffError::io_system("IO.COMPARATOR_ACCESS", message)
            }
            ComparatorError::ParsePolicy { .. }
            | ComparatorError::InvalidPolicy(_)
            | ComparatorError::InvalidGlob { .. }
            | ComparatorError::MissingTolerance { .. } => {
                FeffError::input_validation("INPUT.COMPARATOR_POLICY", message)
            }
            ComparatorError::DecodeArtifact { .. } | ComparatorError::NumericParse { .. } => {
                FeffError::computation("RUN.COMPARATOR", message)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct NumericParseError {
    pub line: usize,
    pub token_index: usize,
    pub token: String,
}

impl Display for NumericParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "line {}, token {} ('{}') is not a valid number",
            self.line, self.token_index, self.token
        )
    }
}

impl Error for NumericParseError {}

#[derive(Debug, Deserialize)]
struct RawPolicy {
    #[serde(rename = "defaultMode")]
    default_mode: ComparisonMode,
    #[serde(rename = "matchStrategy", default)]
    match_strategy: MatchStrategy,
    #[serde(rename = "numericParsing", default)]
    numeric_parsing: NumericParsingOptions,
    #[serde(default)]
    categories: Vec<RawPolicyCategory>,
}

#[derive(Debug, Deserialize)]
struct RawPolicyCategory {
    id: String,
    mode: ComparisonMode,
    #[serde(rename = "fileGlobs", default)]
    file_globs: Vec<String>,
    tolerance: Option<NumericTolerance>,
}

#[derive(Debug, Clone, Deserialize)]
struct NumericParsingOptions {
    #[serde(rename = "trimWhitespace", default = "default_true")]
    trim_whitespace: bool,
    #[serde(rename = "collapseWhitespace", default = "default_true")]
    collapse_whitespace: bool,
    #[serde(rename = "skipEmptyLines", default = "default_true")]
    skip_empty_lines: bool,
    #[serde(rename = "commentPrefixes", default)]
    comment_prefixes: Vec<String>,
    #[serde(rename = "fortranExponentMarkers", default = "default_fortran_markers")]
    fortran_exponent_markers: Vec<String>,
    #[serde(rename = "failOnNaNOrInfMismatch", default = "default_true")]
    fail_on_nan_or_inf_mismatch: bool,
}

impl Default for NumericParsingOptions {
    fn default() -> Self {
        Self {
            trim_whitespace: true,
            collapse_whitespace: true,
            skip_empty_lines: true,
            comment_prefixes: Vec::new(),
            fortran_exponent_markers: default_fortran_markers(),
            fail_on_nan_or_inf_mismatch: true,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_fortran_markers() -> Vec<String> {
    vec!["D".to_string(), "d".to_string()]
}

fn starts_with_comment(line: &str, comment_prefixes: &[String]) -> bool {
    let trimmed = line.trim_start();
    comment_prefixes
        .iter()
        .filter(|prefix| !prefix.is_empty())
        .any(|prefix| trimmed.starts_with(prefix))
}

fn normalize_numeric_token(token: &str, fortran_markers: &[String]) -> String {
    let mut normalized = token.to_string();
    for marker in fortran_markers {
        if marker == "D" {
            normalized = normalized.replace('D', "E");
        } else if marker == "d" {
            normalized = normalized.replace('d', "e");
        }
    }
    normalized
}

fn normalize_artifact_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    normalized
}

fn first_mismatch_offset(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right.iter())
        .position(|(left_byte, right_byte)| left_byte != right_byte)
        .or_else(|| (left.len() != right.len()).then_some(left.len().min(right.len())))
}

fn non_finite_values_match(left: f64, right: f64) -> bool {
    if left.is_nan() || right.is_nan() {
        return left.is_nan() && right.is_nan();
    }

    if left.is_infinite() || right.is_infinite() {
        return left.is_infinite() && right.is_infinite() && left.signum() == right.signum();
    }

    true
}

fn read_artifact_bytes(path: &Path) -> Result<Vec<u8>, ComparatorError> {
    fs::read(path).map_err(|source| ComparatorError::ReadArtifact {
        path: path.to_path_buf(),
        source,
    })
}

fn read_artifact_utf8(path: &Path) -> Result<String, ComparatorError> {
    let bytes = read_artifact_bytes(path)?;
    String::from_utf8(bytes).map_err(|source| ComparatorError::DecodeArtifact {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactComparisonMetrics, ArtifactPair, Comparator, ComparatorError, ComparisonMode,
        NumericTolerance,
    };
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn loads_policy_from_file_and_resolves_mode() {
        let temp = TempDir::new().expect("tempdir should be created");
        let policy_path = write_file(
            &temp,
            "policy.json",
            r##"
            {
              "defaultMode": "exact_text",
              "numericParsing": {
                "trimWhitespace": true,
                "collapseWhitespace": true,
                "skipEmptyLines": true,
                "commentPrefixes": ["#", "!"],
                "fortranExponentMarkers": ["D", "d"],
                "failOnNaNOrInfMismatch": true
              },
              "categories": [
                {
                  "id": "spectra",
                  "mode": "numeric_tolerance",
                  "fileGlobs": ["**/xmu.dat"],
                  "tolerance": {
                    "absTol": 1e-8,
                    "relTol": 1e-6,
                    "relativeFloor": 1e-12
                  }
                }
              ]
            }
            "##,
        );

        let comparator = Comparator::from_policy_path(&policy_path).expect("policy should parse");
        let rule = comparator.resolve_rule_for_artifact("fixture/baseline/xmu.dat");

        assert_eq!(rule.mode, ComparisonMode::NumericTolerance);
        assert_eq!(rule.category_id.as_deref(), Some("spectra"));
        assert_eq!(
            rule.tolerance,
            Some(NumericTolerance {
                abs_tol: 1e-8,
                rel_tol: 1e-6,
                relative_floor: 1e-12,
            })
        );
    }

    #[test]
    fn resolve_rule_uses_first_matching_category_order() {
        let comparator = Comparator::from_policy_json(
            r#"
            {
              "defaultMode": "exact_text",
              "categories": [
                {
                  "id": "all_dat_as_exact",
                  "mode": "exact_text",
                  "fileGlobs": ["**/*.dat"]
                },
                {
                  "id": "spectra",
                  "mode": "numeric_tolerance",
                  "fileGlobs": ["**/xmu.dat"],
                  "tolerance": {
                    "absTol": 1e-8,
                    "relTol": 1e-6,
                    "relativeFloor": 1e-12
                  }
                }
              ]
            }
            "#,
        )
        .expect("policy should parse");

        let rule = comparator.resolve_rule_for_artifact("out/xmu.dat");
        assert_eq!(rule.mode, ComparisonMode::ExactText);
        assert_eq!(rule.category_id.as_deref(), Some("all_dat_as_exact"));
    }

    #[test]
    fn rejects_unknown_match_strategy() {
        let result = Comparator::from_policy_json(
            r#"
            {
              "defaultMode": "exact_text",
              "matchStrategy": "last_match"
            }
            "#,
        );

        assert!(matches!(result, Err(ComparatorError::ParsePolicy { .. })));
    }

    #[test]
    fn exact_text_comparison_reports_structured_failure() {
        let comparator = Comparator::from_policy_json(
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        )
        .expect("policy should parse");

        let temp = TempDir::new().expect("tempdir should be created");
        let baseline = write_file(&temp, "baseline/log.dat", "alpha\nbeta\n");
        let actual = write_file(&temp, "actual/log.dat", "alpha\nbeto\n");

        let result = comparator
            .compare_artifact("log.dat", &baseline, &actual)
            .expect("comparison should succeed");

        assert!(!result.passed);
        assert_eq!(result.mode, ComparisonMode::ExactText);
        assert_eq!(result.matched_category, None);
        assert!(
            result
                .reason
                .as_deref()
                .expect("failure should have reason")
                .contains("Exact-text mismatch")
        );

        match result.metrics {
            ArtifactComparisonMetrics::ExactText(metrics) => {
                assert_eq!(metrics.baseline_bytes, 11);
                assert_eq!(metrics.actual_bytes, 11);
                assert_eq!(metrics.first_mismatch_offset, Some(9));
            }
            _ => panic!("expected exact-text metrics"),
        }
    }

    #[test]
    fn numeric_tolerance_accepts_fortran_d_exponents() {
        let comparator = Comparator::from_policy_json(
            r##"
            {
              "defaultMode": "exact_text",
              "numericParsing": {
                "trimWhitespace": true,
                "collapseWhitespace": true,
                "skipEmptyLines": true,
                "commentPrefixes": ["#", "!"],
                "fortranExponentMarkers": ["D", "d"],
                "failOnNaNOrInfMismatch": true
              },
              "categories": [
                {
                  "id": "spectra",
                  "mode": "numeric_tolerance",
                  "fileGlobs": ["**/xmu.dat"],
                  "tolerance": {
                    "absTol": 1e-8,
                    "relTol": 1e-6,
                    "relativeFloor": 1e-12
                  }
                }
              ]
            }
            "##,
        )
        .expect("policy should parse");

        let temp = TempDir::new().expect("tempdir should be created");
        let baseline = write_file(&temp, "baseline/xmu.dat", "1.000000D+00 2.0\n3.0");
        let actual = write_file(&temp, "actual/xmu.dat", "1.0000008E+00 2.0\n3.0");

        let result = comparator
            .compare_artifact("xmu.dat", &baseline, &actual)
            .expect("comparison should succeed");

        assert!(result.passed);
        assert_eq!(result.mode, ComparisonMode::NumericTolerance);
        assert_eq!(result.matched_category.as_deref(), Some("spectra"));

        match result.metrics {
            ArtifactComparisonMetrics::NumericTolerance(metrics) => {
                assert_eq!(metrics.baseline_value_count, 3);
                assert_eq!(metrics.actual_value_count, 3);
                assert_eq!(metrics.compared_values, 3);
                assert_eq!(metrics.failing_values, 0);
                assert!(metrics.max_abs_diff > 0.0);
            }
            _ => panic!("expected numeric-tolerance metrics"),
        }
    }

    #[test]
    fn numeric_tolerance_reports_failing_values() {
        let comparator = Comparator::from_policy_json(
            r#"
            {
              "defaultMode": "exact_text",
              "categories": [
                {
                  "id": "spectra",
                  "mode": "numeric_tolerance",
                  "fileGlobs": ["**/xmu.dat"],
                  "tolerance": {
                    "absTol": 1e-8,
                    "relTol": 1e-6,
                    "relativeFloor": 1e-12
                  }
                }
              ]
            }
            "#,
        )
        .expect("policy should parse");

        let temp = TempDir::new().expect("tempdir should be created");
        let baseline = write_file(&temp, "baseline/xmu.dat", "1.0\n2.0\n3.0\n");
        let actual = write_file(&temp, "actual/xmu.dat", "1.01\n2.0\n3.0\n");

        let result = comparator
            .compare_artifact("xmu.dat", &baseline, &actual)
            .expect("comparison should succeed");

        assert!(!result.passed);
        assert!(
            result
                .reason
                .as_deref()
                .expect("failure should have reason")
                .contains("outside tolerance")
        );

        match result.metrics {
            ArtifactComparisonMetrics::NumericTolerance(metrics) => {
                assert_eq!(metrics.compared_values, 3);
                assert_eq!(metrics.failing_values, 1);
                assert!(metrics.max_abs_diff >= 0.01);
                assert!(metrics.max_rel_diff >= 0.01);
            }
            _ => panic!("expected numeric-tolerance metrics"),
        }
    }

    #[test]
    fn numeric_tolerance_fails_on_non_finite_mismatch() {
        let comparator = Comparator::from_policy_json(
            r#"
            {
              "defaultMode": "exact_text",
              "numericParsing": {
                "failOnNaNOrInfMismatch": true
              },
              "categories": [
                {
                  "id": "spectra",
                  "mode": "numeric_tolerance",
                  "fileGlobs": ["**/xmu.dat"],
                  "tolerance": {
                    "absTol": 1e-8,
                    "relTol": 1e-6,
                    "relativeFloor": 1e-12
                  }
                }
              ]
            }
            "#,
        )
        .expect("policy should parse");

        let temp = TempDir::new().expect("tempdir should be created");
        let baseline = write_file(&temp, "baseline/xmu.dat", "NaN inf -inf\n");
        let actual = write_file(&temp, "actual/xmu.dat", "NaN inf inf\n");

        let result = comparator
            .compare_artifact("xmu.dat", &baseline, &actual)
            .expect("comparison should succeed");

        assert!(!result.passed);
        assert_eq!(result.matched_category.as_deref(), Some("spectra"));
        assert!(
            result
                .reason
                .as_deref()
                .expect("failure should have reason")
                .contains("non-finite mismatch")
        );

        match result.metrics {
            ArtifactComparisonMetrics::NumericTolerance(metrics) => {
                assert_eq!(metrics.compared_values, 3);
                assert_eq!(metrics.failing_values, 1);
            }
            _ => panic!("expected numeric-tolerance metrics"),
        }
    }

    #[test]
    fn numeric_tolerance_uses_first_matching_category_tolerance() {
        let comparator = Comparator::from_policy_json(
            r#"
            {
              "defaultMode": "exact_text",
              "categories": [
                {
                  "id": "strict_first",
                  "mode": "numeric_tolerance",
                  "fileGlobs": ["**/*.dat"],
                  "tolerance": {
                    "absTol": 1e-9,
                    "relTol": 1e-9,
                    "relativeFloor": 1e-12
                  }
                },
                {
                  "id": "loose_second",
                  "mode": "numeric_tolerance",
                  "fileGlobs": ["**/xmu.dat"],
                  "tolerance": {
                    "absTol": 1e-1,
                    "relTol": 1e-1,
                    "relativeFloor": 1e-12
                  }
                }
              ]
            }
            "#,
        )
        .expect("policy should parse");

        let temp = TempDir::new().expect("tempdir should be created");
        let baseline = write_file(&temp, "baseline/xmu.dat", "1.0\n");
        let actual = write_file(&temp, "actual/xmu.dat", "1.01\n");

        let result = comparator
            .compare_artifact("xmu.dat", &baseline, &actual)
            .expect("comparison should succeed");

        assert!(!result.passed);
        assert_eq!(result.matched_category.as_deref(), Some("strict_first"));

        match result.metrics {
            ArtifactComparisonMetrics::NumericTolerance(metrics) => {
                assert_eq!(metrics.tolerance.abs_tol, 1e-9);
                assert_eq!(metrics.tolerance.rel_tol, 1e-9);
            }
            _ => panic!("expected numeric-tolerance metrics"),
        }
    }

    #[test]
    fn numeric_tolerance_reports_line_count_mismatch() {
        let comparator = Comparator::from_policy_json(
            r#"
            {
              "defaultMode": "exact_text",
              "categories": [
                {
                  "id": "spectra",
                  "mode": "numeric_tolerance",
                  "fileGlobs": ["**/xmu.dat"],
                  "tolerance": {
                    "absTol": 1e-8,
                    "relTol": 1e-6,
                    "relativeFloor": 1e-12
                  }
                }
              ]
            }
            "#,
        )
        .expect("policy should parse");

        let temp = TempDir::new().expect("tempdir should be created");
        let baseline = write_file(&temp, "baseline/xmu.dat", "1.0\n2.0\n3.0\n");
        let actual = write_file(&temp, "actual/xmu.dat", "1.0 2.0\n3.0\n");

        let result = comparator
            .compare_artifact("xmu.dat", &baseline, &actual)
            .expect("comparison should succeed");

        assert!(!result.passed);
        assert!(
            result
                .reason
                .as_deref()
                .expect("failure should have reason")
                .contains("line count mismatch")
        );

        match result.metrics {
            ArtifactComparisonMetrics::NumericTolerance(metrics) => {
                assert_eq!(metrics.baseline_value_count, 3);
                assert_eq!(metrics.actual_value_count, 3);
                assert!(metrics.failing_values > 0);
            }
            _ => panic!("expected numeric-tolerance metrics"),
        }
    }

    #[test]
    fn compare_artifacts_returns_result_per_artifact() {
        let comparator = Comparator::from_policy_json(
            r#"
            {
              "defaultMode": "exact_text"
            }
            "#,
        )
        .expect("policy should parse");

        let temp = TempDir::new().expect("tempdir should be created");
        let baseline_pass = write_file(&temp, "baseline/a.dat", "same");
        let actual_pass = write_file(&temp, "actual/a.dat", "same");
        let baseline_fail = write_file(&temp, "baseline/b.dat", "left");
        let actual_fail = write_file(&temp, "actual/b.dat", "right");

        let artifacts = vec![
            ArtifactPair::new("a.dat", baseline_pass, actual_pass),
            ArtifactPair::new("b.dat", baseline_fail, actual_fail),
        ];

        let results = comparator
            .compare_artifacts(&artifacts)
            .expect("batch comparison should succeed");

        assert_eq!(results.len(), 2);
        assert!(results[0].passed);
        assert!(!results[1].passed);
        assert_eq!(results[0].artifact_path, "a.dat");
        assert_eq!(results[1].artifact_path, "b.dat");
    }

    fn write_file(temp_dir: &TempDir, relative_path: &str, content: &str) -> PathBuf {
        let path = temp_dir.path().join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir should be created");
        }
        fs::write(&path, content).expect("file should be written");
        path
    }
}
