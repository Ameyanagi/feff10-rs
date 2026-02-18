use super::CliError;
use super::dispatch::ModuleCommandSpec;
use anyhow::Context;
use feff_core::domain::{ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult, FeffError};
use feff_core::modules::execute_runtime_module;
use feff_core::modules::regression::RegressionRunnerConfig;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) const MANIFEST_RELATIVE_PATH: &str = "tasks/golden-fixture-manifest.json";

#[derive(Debug, Deserialize, Clone)]
pub(super) struct CliManifest {
    pub(super) fixtures: Vec<CliManifestFixture>,
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct CliManifestFixture {
    pub(super) id: String,
    #[serde(rename = "fixtureType", default)]
    pub(super) fixture_type: String,
    #[serde(rename = "modulesCovered", default)]
    pub(super) modules_covered: Vec<String>,
    #[serde(rename = "inputDirectory", default)]
    pub(super) input_directory: String,
}

impl CliManifestFixture {
    pub(super) fn covers_module(&self, module: ComputeModule) -> bool {
        self.modules_covered
            .iter()
            .any(|covered| covered.eq_ignore_ascii_case(module.as_str()))
    }

    pub(super) fn is_workflow(&self) -> bool {
        self.fixture_type.eq_ignore_ascii_case("workflow") || self.modules_covered.len() > 1
    }
}

#[derive(Debug, Clone)]
pub(super) struct CliContext {
    pub(super) working_dir: PathBuf,
    pub(super) workspace_root: PathBuf,
    pub(super) manifest: CliManifest,
}

#[derive(Debug, Clone)]
pub(super) struct OracleCommandConfig {
    pub(super) regression: RegressionRunnerConfig,
    pub(super) capture_mode: OracleCaptureMode,
    pub(super) allow_missing_entry_files: bool,
}

#[derive(Debug, Clone)]
pub(super) enum OracleCaptureMode {
    Runner(String),
    BinDir(PathBuf),
}

pub(super) fn load_cli_context() -> Result<CliContext, CliError> {
    let working_dir = current_working_dir().map_err(CliError::Compute)?;
    let workspace_root = find_workspace_root(&working_dir).ok_or_else(|| {
        CliError::Compute(FeffError::input_validation(
            "INPUT.CLI_WORKSPACE",
            format!(
                "failed to locate workspace root from '{}'; expected to find '{}'",
                working_dir.display(),
                MANIFEST_RELATIVE_PATH
            ),
        ))
    })?;
    let manifest = load_cli_manifest(&workspace_root)?;
    Ok(CliContext {
        working_dir,
        workspace_root,
        manifest,
    })
}

pub(super) fn load_cli_context_if_available(
    working_dir: &Path,
) -> Result<Option<CliContext>, CliError> {
    let Some(workspace_root) = find_workspace_root(working_dir) else {
        return Ok(None);
    };
    let manifest = load_cli_manifest(&workspace_root)?;
    Ok(Some(CliContext {
        working_dir: working_dir.to_path_buf(),
        workspace_root,
        manifest,
    }))
}

pub(super) fn current_working_dir() -> ComputeResult<PathBuf> {
    std::env::current_dir().map_err(|source| {
        FeffError::io_system(
            "IO.CLI_CURRENT_DIR",
            format!("failed to read current working directory: {}", source),
        )
    })
}

pub(super) fn default_fixture_for_module(module: ComputeModule) -> &'static str {
    match module {
        ComputeModule::Rdinp => "FX-RDINP-001",
        ComputeModule::Pot => "FX-POT-001",
        ComputeModule::Xsph => "FX-XSPH-001",
        ComputeModule::Path => "FX-PATH-001",
        ComputeModule::Fms => "FX-FMS-001",
        ComputeModule::Band => "FX-BAND-001",
        ComputeModule::Ldos => "FX-LDOS-001",
        ComputeModule::Rixs => "FX-RIXS-001",
        ComputeModule::Crpa => "FX-CRPA-001",
        ComputeModule::Compton => "FX-COMPTON-001",
        ComputeModule::Debye => "FX-DEBYE-001",
        ComputeModule::Dmdw => "FX-DMDW-001",
        ComputeModule::Screen => "FX-SCREEN-001",
        ComputeModule::SelfEnergy => "FX-SELF-001",
        ComputeModule::Eels => "FX-EELS-001",
        ComputeModule::FullSpectrum => "FX-FULLSPECTRUM-001",
    }
}

pub(super) fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        let manifest = candidate.join(MANIFEST_RELATIVE_PATH);
        if manifest.is_file() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

pub(super) fn load_cli_manifest(workspace_root: &Path) -> Result<CliManifest, CliError> {
    let path = workspace_root.join(MANIFEST_RELATIVE_PATH);
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read CLI manifest '{}'", path.display()))?;
    serde_json::from_str::<CliManifest>(&content)
        .with_context(|| format!("failed to parse CLI manifest '{}'", path.display()))
        .map_err(CliError::from)
}

pub(super) fn select_serial_fixture(context: &CliContext) -> ComputeResult<CliManifestFixture> {
    let mut candidates = fixtures_covering_module(&context.manifest, ComputeModule::Rdinp)
        .into_iter()
        .filter(|fixture| fixture.is_workflow())
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        return Err(FeffError::input_validation(
            "INPUT.CLI_WORKFLOW_FIXTURE",
            "no workflow fixtures covering RDINP were found in the manifest",
        ));
    }

    if let Some(matched) = select_by_input_directory(context, &candidates) {
        return Ok(matched.clone());
    }

    candidates.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(candidates[0].clone())
}

pub(super) fn modules_for_serial_fixture(fixture: &CliManifestFixture) -> Vec<ComputeModule> {
    use super::dispatch::{SERIAL_CHAIN_ORDER, parse_compute_module};

    let covered = fixture
        .modules_covered
        .iter()
        .filter_map(|module| parse_compute_module(module))
        .collect::<Vec<_>>();

    SERIAL_CHAIN_ORDER
        .iter()
        .copied()
        .filter(|module| covered.contains(module))
        .collect()
}

pub(super) fn select_fixture_for_module(
    context: &CliContext,
    spec: ModuleCommandSpec,
    prefer_workflow: bool,
) -> ComputeResult<String> {
    let mut candidates = fixtures_covering_module(&context.manifest, spec.module);
    if candidates.is_empty() {
        return Err(FeffError::input_validation(
            "INPUT.CLI_FIXTURE_LOOKUP",
            format!(
                "no fixtures in '{}' cover module {}",
                MANIFEST_RELATIVE_PATH, spec.module
            ),
        ));
    }

    if spec.module == ComputeModule::Rdinp {
        if let Some(matched) = select_by_input_directory(context, &candidates) {
            return Ok(matched.id.clone());
        }
        if prefer_workflow {
            if let Some(workflow) = choose_single_fixture_by_type(&candidates, true) {
                return Ok(workflow.id.clone());
            }
        } else if let Some(module_fixture) = choose_single_fixture_by_type(&candidates, false) {
            return Ok(module_fixture.id.clone());
        }
        candidates.sort_by(|a, b| a.id.cmp(&b.id));
        return Ok(candidates[0].id.clone());
    }

    let mut passing = Vec::new();
    let mut failure_messages = Vec::new();
    for fixture in &candidates {
        match probe_module_for_fixture(context, spec, &fixture.id) {
            Ok(()) => passing.push(*fixture),
            Err(error) => failure_messages.push(format!("{} => {}", fixture.id, error)),
        }
    }

    if passing.is_empty() {
        let detail = if failure_messages.is_empty() {
            "no candidate fixtures matched module input contracts".to_string()
        } else {
            failure_messages.join("; ")
        };
        return Err(FeffError::input_validation(
            "INPUT.CLI_FIXTURE_LOOKUP",
            format!(
                "unable to resolve fixture for module {} in '{}': {}",
                spec.module,
                context.working_dir.display(),
                detail
            ),
        ));
    }

    if passing.len() == 1 {
        return Ok(passing[0].id.clone());
    }

    if let Some(matched) = select_by_input_directory(context, &passing) {
        return Ok(matched.id.clone());
    }

    if prefer_workflow {
        if let Some(workflow) = choose_single_fixture_by_type(&passing, true) {
            return Ok(workflow.id.clone());
        }
    } else if let Some(module_fixture) = choose_single_fixture_by_type(&passing, false) {
        return Ok(module_fixture.id.clone());
    }

    passing.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(passing[0].id.clone())
}

fn fixtures_covering_module(
    manifest: &CliManifest,
    module: ComputeModule,
) -> Vec<&CliManifestFixture> {
    manifest
        .fixtures
        .iter()
        .filter(|fixture| fixture.covers_module(module))
        .collect()
}

fn choose_single_fixture_by_type<'a>(
    candidates: &'a [&'a CliManifestFixture],
    workflow: bool,
) -> Option<&'a CliManifestFixture> {
    let mut filtered = candidates
        .iter()
        .copied()
        .filter(|fixture| fixture.is_workflow() == workflow)
        .collect::<Vec<_>>();
    if filtered.len() == 1 {
        return Some(filtered.remove(0));
    }
    None
}

fn select_by_input_directory<'a>(
    context: &CliContext,
    candidates: &'a [&'a CliManifestFixture],
) -> Option<&'a CliManifestFixture> {
    let mut matches = Vec::new();
    for fixture in candidates {
        if fixture.input_directory.is_empty() {
            continue;
        }

        let candidate_path = context.workspace_root.join(&fixture.input_directory);
        if candidate_path == context.working_dir {
            matches.push(*fixture);
            continue;
        }

        let canonical_candidate = fs::canonicalize(&candidate_path).ok();
        let canonical_working = fs::canonicalize(&context.working_dir).ok();
        if canonical_candidate.is_some() && canonical_candidate == canonical_working {
            matches.push(*fixture);
        }
    }

    if matches.len() == 1 {
        Some(matches[0])
    } else {
        None
    }
}

fn probe_module_for_fixture(
    context: &CliContext,
    spec: ModuleCommandSpec,
    fixture_id: &str,
) -> ComputeResult<()> {
    let probe_output_dir = probe_directory_for(spec.module, fixture_id)?;
    let request = ComputeRequest::new(
        fixture_id.to_string(),
        spec.module,
        context.working_dir.join(spec.input_artifact),
        &probe_output_dir,
    );

    let result = execute_runtime_module(spec.module, &request).map(|_| ());

    let _ = fs::remove_dir_all(&probe_output_dir);
    result
}

fn probe_directory_for(module: ComputeModule, fixture_id: &str) -> ComputeResult<PathBuf> {
    let mut path = std::env::temp_dir();
    let unix_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|source| {
            FeffError::internal(
                "SYS.CLI_TIME",
                format!(
                    "failed to read system time for probe directory naming: {}",
                    source
                ),
            )
        })?
        .as_nanos();
    path.push(format!(
        "feff10-rs-cli-probe-{}-{}-{}-{}",
        module.as_str().to_ascii_lowercase(),
        fixture_id,
        std::process::id(),
        unix_nanos
    ));
    fs::create_dir_all(&path).map_err(|source| {
        FeffError::io_system(
            "IO.CLI_PROBE_DIR",
            format!(
                "failed to create module probe directory '{}': {}",
                path.display(),
                source
            ),
        )
    })?;
    Ok(path)
}

pub(super) fn execute_module_with_fixture(
    working_dir: &Path,
    spec: ModuleCommandSpec,
    fixture_id: &str,
) -> ComputeResult<Vec<ComputeArtifact>> {
    let request = ComputeRequest::new(
        fixture_id.to_string(),
        spec.module,
        working_dir.join(spec.input_artifact),
        working_dir,
    );
    execute_runtime_module(spec.module, &request)
}

pub(super) fn resolve_regression_paths(
    mut config: RegressionRunnerConfig,
    working_dir: &Path,
) -> RegressionRunnerConfig {
    config.manifest_path = resolve_cli_path(working_dir, &config.manifest_path);
    config.policy_path = resolve_cli_path(working_dir, &config.policy_path);
    config.baseline_root = resolve_cli_path(working_dir, &config.baseline_root);
    config.actual_root = resolve_cli_path(working_dir, &config.actual_root);
    config.report_path = resolve_cli_path(working_dir, &config.report_path);
    config
}

pub(super) fn resolve_cli_path(working_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(path)
    }
}

pub(super) fn run_oracle_capture(
    workspace_root: &Path,
    config: &OracleCommandConfig,
) -> ComputeResult<()> {
    let capture_script = workspace_root.join("scripts/fortran/capture-baselines.sh");
    if !capture_script.is_file() {
        return Err(FeffError::io_system(
            "IO.ORACLE_CAPTURE_SCRIPT",
            format!(
                "oracle capture script was not found at '{}'",
                capture_script.display()
            ),
        ));
    }

    let mut command = Command::new(&capture_script);
    command
        .current_dir(workspace_root)
        .arg("--manifest")
        .arg(&config.regression.manifest_path)
        .arg("--output-root")
        .arg(&config.regression.baseline_root)
        .arg("--all-fixtures");

    if config.allow_missing_entry_files {
        command.arg("--allow-missing-entry-files");
    }

    match &config.capture_mode {
        OracleCaptureMode::Runner(runner) => {
            command.arg("--runner").arg(runner);
        }
        OracleCaptureMode::BinDir(path) => {
            command.arg("--bin-dir").arg(path);
        }
    }

    let status = command.status().map_err(|source| {
        FeffError::io_system(
            "IO.ORACLE_CAPTURE_EXEC",
            format!(
                "failed to execute oracle capture command '{}': {}",
                capture_script.display(),
                source
            ),
        )
    })?;

    if status.success() {
        return Ok(());
    }

    let status_text = status.code().map_or_else(
        || "terminated by signal".to_string(),
        |code| format!("exit code {}", code),
    );
    Err(FeffError::computation(
        "RUN.ORACLE_CAPTURE",
        format!("oracle capture step failed with {}", status_text),
    ))
}
