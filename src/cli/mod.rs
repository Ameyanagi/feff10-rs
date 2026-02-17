use crate::domain::{FeffError, ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult};
use crate::modules::regression::{RegressionRunnerConfig, render_human_summary, run_regression};
use crate::modules::{
    execute_runtime_module, runtime_compute_engine_available, runtime_engine_unavailable_error,
};
use serde::Deserialize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const MANIFEST_RELATIVE_PATH: &str = "tasks/golden-fixture-manifest.json";

#[derive(Debug, Clone, Copy)]
struct ModuleCommandSpec {
    command: &'static str,
    module: ComputeModule,
    input_artifact: &'static str,
}

const MODULE_COMMANDS: [ModuleCommandSpec; 16] = [
    ModuleCommandSpec {
        command: "rdinp",
        module: ComputeModule::Rdinp,
        input_artifact: "feff.inp",
    },
    ModuleCommandSpec {
        command: "pot",
        module: ComputeModule::Pot,
        input_artifact: "pot.inp",
    },
    ModuleCommandSpec {
        command: "xsph",
        module: ComputeModule::Xsph,
        input_artifact: "xsph.inp",
    },
    ModuleCommandSpec {
        command: "path",
        module: ComputeModule::Path,
        input_artifact: "paths.inp",
    },
    ModuleCommandSpec {
        command: "fms",
        module: ComputeModule::Fms,
        input_artifact: "fms.inp",
    },
    ModuleCommandSpec {
        command: "band",
        module: ComputeModule::Band,
        input_artifact: "band.inp",
    },
    ModuleCommandSpec {
        command: "ldos",
        module: ComputeModule::Ldos,
        input_artifact: "ldos.inp",
    },
    ModuleCommandSpec {
        command: "rixs",
        module: ComputeModule::Rixs,
        input_artifact: "rixs.inp",
    },
    ModuleCommandSpec {
        command: "crpa",
        module: ComputeModule::Crpa,
        input_artifact: "crpa.inp",
    },
    ModuleCommandSpec {
        command: "compton",
        module: ComputeModule::Compton,
        input_artifact: "compton.inp",
    },
    ModuleCommandSpec {
        command: "ff2x",
        module: ComputeModule::Debye,
        input_artifact: "ff2x.inp",
    },
    ModuleCommandSpec {
        command: "dmdw",
        module: ComputeModule::Dmdw,
        input_artifact: "dmdw.inp",
    },
    ModuleCommandSpec {
        command: "screen",
        module: ComputeModule::Screen,
        input_artifact: "pot.inp",
    },
    ModuleCommandSpec {
        command: "sfconv",
        module: ComputeModule::SelfEnergy,
        input_artifact: "sfconv.inp",
    },
    ModuleCommandSpec {
        command: "eels",
        module: ComputeModule::Eels,
        input_artifact: "eels.inp",
    },
    ModuleCommandSpec {
        command: "fullspectrum",
        module: ComputeModule::FullSpectrum,
        input_artifact: "fullspectrum.inp",
    },
];

const SERIAL_CHAIN_ORDER: [ComputeModule; 16] = [
    ComputeModule::Rdinp,
    ComputeModule::Pot,
    ComputeModule::Screen,
    ComputeModule::SelfEnergy,
    ComputeModule::Eels,
    ComputeModule::Xsph,
    ComputeModule::Band,
    ComputeModule::Ldos,
    ComputeModule::Rixs,
    ComputeModule::Crpa,
    ComputeModule::Path,
    ComputeModule::Debye,
    ComputeModule::Dmdw,
    ComputeModule::Fms,
    ComputeModule::Compton,
    ComputeModule::FullSpectrum,
];

#[derive(Debug, Deserialize, Clone)]
struct CliManifest {
    fixtures: Vec<CliManifestFixture>,
}

#[derive(Debug, Deserialize, Clone)]
struct CliManifestFixture {
    id: String,
    #[serde(rename = "fixtureType", default)]
    fixture_type: String,
    #[serde(rename = "modulesCovered", default)]
    modules_covered: Vec<String>,
    #[serde(rename = "inputDirectory", default)]
    input_directory: String,
}

impl CliManifestFixture {
    fn covers_module(&self, module: ComputeModule) -> bool {
        self.modules_covered
            .iter()
            .any(|covered| covered.eq_ignore_ascii_case(module.as_str()))
    }

    fn is_workflow(&self) -> bool {
        self.fixture_type.eq_ignore_ascii_case("workflow") || self.modules_covered.len() > 1
    }
}

#[derive(Debug, Clone)]
struct CliContext {
    working_dir: PathBuf,
    workspace_root: PathBuf,
    manifest: CliManifest,
}

#[derive(Debug, Clone)]
struct OracleCommandConfig {
    regression: RegressionRunnerConfig,
    capture_mode: OracleCaptureMode,
    allow_missing_entry_files: bool,
}

#[derive(Debug, Clone)]
enum OracleCaptureMode {
    Runner(String),
    BinDir(PathBuf),
}

pub fn run_from_env() -> i32 {
    let mut args = std::env::args();
    let program_name = args.next().unwrap_or_else(|| "feff10-rs".to_string());
    match run_with_program_name(&program_name, args) {
        Ok(code) => code,
        Err(error) => {
            let compatibility_error = error.as_feff_error();
            eprintln!("{}", compatibility_error.diagnostic_line());
            if let Some(summary_line) = compatibility_error.fatal_exit_line() {
                eprintln!("{}", summary_line);
            }
            compatibility_error.exit_code()
        }
    }
}

pub fn run<I, S>(args: I) -> Result<i32, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    run_with_program_name("feff10-rs", args)
}

fn run_with_program_name<I, S>(program_name: &str, args: I) -> Result<i32, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args: Vec<String> = args.into_iter().map(Into::into).collect();
    if let Some(alias_command) = command_alias_from_program_name(program_name) {
        return dispatch_command(alias_command, args);
    }

    if args.is_empty() {
        return Err(CliError::Usage(usage_text().to_string()));
    }

    let command = args.remove(0);
    dispatch_command(&command, args)
}

fn dispatch_command(command: &str, args: Vec<String>) -> Result<i32, CliError> {
    if let Some(spec) = module_command_spec(command) {
        return run_module_command(spec, args);
    }

    match command {
        "regression" => run_regression_command(args),
        "oracle" => run_oracle_command(args),
        "feff" => run_feff_command(args),
        "feffmpi" => run_feffmpi_command(args),
        "help" | "--help" | "-h" => {
            println!("{}", usage_text());
            Ok(0)
        }
        other => Err(CliError::Usage(format!(
            "Unknown command '{}'.\n{}",
            other,
            usage_text()
        ))),
    }
}

fn run_regression_command(args: Vec<String>) -> Result<i32, CliError> {
    if help_requested(&args) {
        println!("{}", regression_usage_text());
        return Ok(0);
    }

    let config = parse_regression_args(args)?;
    let report = run_regression(&config).map_err(CliError::Compute)?;
    println!("{}", render_human_summary(&report));
    println!("JSON report: {}", config.report_path.display());

    if report.passed { Ok(0) } else { Ok(1) }
}

fn run_oracle_command(args: Vec<String>) -> Result<i32, CliError> {
    if help_requested(&args) {
        println!("{}", oracle_usage_text());
        return Ok(0);
    }

    let mut config = parse_oracle_args(args)?;
    let working_dir = std::env::current_dir().map_err(|source| {
        CliError::Compute(FeffError::io_system(
            "IO.CLI_CURRENT_DIR",
            format!("failed to read current working directory: {}", source),
        ))
    })?;
    config.regression = resolve_regression_paths(config.regression, &working_dir);

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

    println!("Running Fortran oracle capture...");
    run_oracle_capture(&workspace_root, &config).map_err(CliError::Compute)?;

    println!("Running Rust-vs-Fortran regression comparison...");
    let report = run_regression(&config.regression).map_err(CliError::Compute)?;
    println!("{}", render_human_summary(&report));
    println!("JSON report: {}", config.regression.report_path.display());

    if report.passed { Ok(0) } else { Ok(1) }
}

fn run_feff_command(args: Vec<String>) -> Result<i32, CliError> {
    if help_requested(&args) {
        println!("{}", feff_usage_text());
        return Ok(0);
    }
    if !args.is_empty() {
        return Err(CliError::Usage(format!(
            "Command 'feff' does not accept positional arguments.\n{}",
            feff_usage_text()
        )));
    }

    let context = load_cli_context()?;
    let fixture = select_serial_fixture(&context).map_err(CliError::Compute)?;
    let modules = modules_for_serial_fixture(&fixture);
    if modules.is_empty() {
        return Err(CliError::Compute(FeffError::input_validation(
            "INPUT.CLI_FIXTURE_MODULES",
            format!(
                "fixture '{}' does not provide any serial modules for 'feff'",
                fixture.id
            ),
        )));
    }

    if let Some(module) = modules
        .iter()
        .copied()
        .find(|module| !runtime_compute_engine_available(*module))
    {
        return Err(CliError::Compute(runtime_engine_unavailable_error(module)));
    }

    for module in modules {
        if let Some(spec) = module_command_for_module(module) {
            println!("Running {}...", spec.module);
            execute_module_with_fixture(&context.working_dir, spec, &fixture.id)
                .map_err(CliError::Compute)?;
        }
    }
    println!("Completed serial workflow for fixture '{}'.", fixture.id);
    Ok(0)
}

fn run_feffmpi_command(args: Vec<String>) -> Result<i32, CliError> {
    if help_requested(&args) {
        println!("{}", feffmpi_usage_text());
        return Ok(0);
    }

    if args.len() != 1 {
        return Err(CliError::Usage(format!(
            "Command 'feffmpi' requires exactly one argument: <nprocs>.\n{}",
            feffmpi_usage_text()
        )));
    }

    let process_count = args[0].parse::<usize>().map_err(|_| {
        CliError::Usage(format!(
            "Invalid process count '{}'; expected a positive integer.\n{}",
            args[0],
            feffmpi_usage_text()
        ))
    })?;
    if process_count == 0 {
        return Err(CliError::Usage(format!(
            "Invalid process count '0'; expected a positive integer.\n{}",
            feffmpi_usage_text()
        )));
    }

    if process_count > 1 {
        eprintln!(
            "WARNING: [RUN.MPI_DEFERRED] MPI parity is deferred for Rust v1; executing serial compatibility chain instead (requested nprocs={}).",
            process_count
        );
    }

    run_feff_command(Vec::new())
}

fn run_module_command(spec: ModuleCommandSpec, args: Vec<String>) -> Result<i32, CliError> {
    if help_requested(&args) {
        println!("{}", module_usage_text(spec));
        return Ok(0);
    }
    if !args.is_empty() {
        return Err(CliError::Usage(format!(
            "Command '{}' does not accept positional arguments.\n{}",
            spec.command,
            module_usage_text(spec)
        )));
    }
    if !runtime_compute_engine_available(spec.module) {
        return Err(CliError::Compute(runtime_engine_unavailable_error(
            spec.module,
        )));
    }

    let working_dir = current_working_dir().map_err(CliError::Compute)?;
    let fixture_id = if let Some(context) = load_cli_context_if_available(&working_dir)? {
        select_fixture_for_module(&context, spec, false).map_err(CliError::Compute)?
    } else {
        default_fixture_for_module(spec.module).to_string()
    };
    println!("Running {}...", spec.module);
    let artifacts =
        execute_module_with_fixture(&working_dir, spec, &fixture_id).map_err(CliError::Compute)?;
    println!(
        "{} completed for fixture '{}' ({} artifacts).",
        spec.module,
        fixture_id,
        artifacts.len()
    );
    Ok(0)
}

fn load_cli_context() -> Result<CliContext, CliError> {
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

fn load_cli_context_if_available(working_dir: &Path) -> Result<Option<CliContext>, CliError> {
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

fn current_working_dir() -> ComputeResult<PathBuf> {
    std::env::current_dir().map_err(|source| {
        FeffError::io_system(
            "IO.CLI_CURRENT_DIR",
            format!("failed to read current working directory: {}", source),
        )
    })
}

fn default_fixture_for_module(module: ComputeModule) -> &'static str {
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

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        let manifest = candidate.join(MANIFEST_RELATIVE_PATH);
        if manifest.is_file() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

fn load_cli_manifest(workspace_root: &Path) -> Result<CliManifest, CliError> {
    let path = workspace_root.join(MANIFEST_RELATIVE_PATH);
    let content = fs::read_to_string(&path).map_err(|source| {
        CliError::Compute(FeffError::io_system(
            "IO.CLI_MANIFEST_READ",
            format!(
                "failed to read CLI manifest '{}': {}",
                path.display(),
                source
            ),
        ))
    })?;
    serde_json::from_str::<CliManifest>(&content).map_err(|source| {
        CliError::Compute(FeffError::input_validation(
            "INPUT.CLI_MANIFEST_PARSE",
            format!(
                "failed to parse CLI manifest '{}': {}",
                path.display(),
                source
            ),
        ))
    })
}

fn select_serial_fixture(context: &CliContext) -> ComputeResult<CliManifestFixture> {
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

fn modules_for_serial_fixture(fixture: &CliManifestFixture) -> Vec<ComputeModule> {
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

fn select_fixture_for_module(
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

    let result = execute_module_for_spec(spec, &request).map(|_| ());

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

fn execute_module_with_fixture(
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
    execute_module_for_spec(spec, &request)
}

fn execute_module_for_spec(
    spec: ModuleCommandSpec,
    request: &ComputeRequest,
) -> ComputeResult<Vec<ComputeArtifact>> {
    execute_runtime_module(spec.module, request)
}

fn parse_compute_module(token: &str) -> Option<ComputeModule> {
    match token.to_ascii_uppercase().as_str() {
        "RDINP" => Some(ComputeModule::Rdinp),
        "POT" => Some(ComputeModule::Pot),
        "PATH" => Some(ComputeModule::Path),
        "FMS" => Some(ComputeModule::Fms),
        "XSPH" => Some(ComputeModule::Xsph),
        "BAND" => Some(ComputeModule::Band),
        "LDOS" => Some(ComputeModule::Ldos),
        "RIXS" => Some(ComputeModule::Rixs),
        "CRPA" => Some(ComputeModule::Crpa),
        "COMPTON" => Some(ComputeModule::Compton),
        "DEBYE" => Some(ComputeModule::Debye),
        "DMDW" => Some(ComputeModule::Dmdw),
        "SCREEN" => Some(ComputeModule::Screen),
        "SELF" => Some(ComputeModule::SelfEnergy),
        "EELS" => Some(ComputeModule::Eels),
        "FULLSPECTRUM" => Some(ComputeModule::FullSpectrum),
        _ => None,
    }
}

fn module_command_spec(command: &str) -> Option<ModuleCommandSpec> {
    MODULE_COMMANDS
        .iter()
        .copied()
        .find(|spec| spec.command == command)
}

fn module_command_for_module(module: ComputeModule) -> Option<ModuleCommandSpec> {
    MODULE_COMMANDS
        .iter()
        .copied()
        .find(|spec| spec.module == module)
}

fn command_alias_from_program_name(program_name: &str) -> Option<&'static str> {
    let executable_name = Path::new(program_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program_name);
    let normalized = executable_name
        .strip_suffix(".exe")
        .unwrap_or(executable_name);

    if normalized == "feff10-rs" {
        return None;
    }

    if normalized == "feff" || normalized == "feffmpi" {
        return Some(if normalized == "feff" {
            "feff"
        } else {
            "feffmpi"
        });
    }

    module_command_spec(normalized).map(|spec| spec.command)
}

fn parse_regression_args(args: Vec<String>) -> Result<RegressionRunnerConfig, CliError> {
    let mut config = RegressionRunnerConfig::default();
    let mut index = 0;
    while index < args.len() {
        let option = &args[index];
        let next_index = index + 1;

        match option.as_str() {
            "--manifest" => {
                config.manifest_path = PathBuf::from(value_for_option(&args, next_index, option)?);
                index += 2;
            }
            "--policy" => {
                config.policy_path = PathBuf::from(value_for_option(&args, next_index, option)?);
                index += 2;
            }
            "--baseline-root" => {
                config.baseline_root = PathBuf::from(value_for_option(&args, next_index, option)?);
                index += 2;
            }
            "--actual-root" => {
                config.actual_root = PathBuf::from(value_for_option(&args, next_index, option)?);
                index += 2;
            }
            "--baseline-subdir" => {
                config.baseline_subdir = value_for_option(&args, next_index, option)?.to_string();
                index += 2;
            }
            "--actual-subdir" => {
                config.actual_subdir = value_for_option(&args, next_index, option)?.to_string();
                index += 2;
            }
            "--report" => {
                config.report_path = PathBuf::from(value_for_option(&args, next_index, option)?);
                index += 2;
            }
            _ => {
                if apply_regression_run_flag(&mut config, option) {
                    index += 1;
                } else {
                    return Err(CliError::Usage(format!(
                        "Unknown option '{}'.\n{}",
                        option,
                        regression_usage_text()
                    )));
                }
            }
        }
    }

    Ok(config)
}

fn parse_oracle_args(args: Vec<String>) -> Result<OracleCommandConfig, CliError> {
    let mut regression = RegressionRunnerConfig {
        baseline_root: PathBuf::from("artifacts/fortran-oracle-capture"),
        actual_root: PathBuf::from("artifacts/oracle-actual"),
        baseline_subdir: "outputs".to_string(),
        actual_subdir: "actual".to_string(),
        report_path: PathBuf::from("artifacts/regression/oracle-report.json"),
        ..RegressionRunnerConfig::default()
    };

    let mut capture_runner = None;
    let mut capture_bin_dir = None;
    let mut allow_missing_entry_files = false;

    let mut index = 0;
    while index < args.len() {
        let option = &args[index];
        let next_index = index + 1;

        match option.as_str() {
            "--manifest" => {
                regression.manifest_path =
                    PathBuf::from(value_for_oracle_option(&args, next_index, option)?);
                index += 2;
            }
            "--policy" => {
                regression.policy_path =
                    PathBuf::from(value_for_oracle_option(&args, next_index, option)?);
                index += 2;
            }
            "--oracle-root" => {
                regression.baseline_root =
                    PathBuf::from(value_for_oracle_option(&args, next_index, option)?);
                index += 2;
            }
            "--actual-root" => {
                regression.actual_root =
                    PathBuf::from(value_for_oracle_option(&args, next_index, option)?);
                index += 2;
            }
            "--oracle-subdir" => {
                regression.baseline_subdir =
                    value_for_oracle_option(&args, next_index, option)?.to_string();
                index += 2;
            }
            "--actual-subdir" => {
                regression.actual_subdir =
                    value_for_oracle_option(&args, next_index, option)?.to_string();
                index += 2;
            }
            "--report" => {
                regression.report_path =
                    PathBuf::from(value_for_oracle_option(&args, next_index, option)?);
                index += 2;
            }
            "--capture-runner" => {
                capture_runner =
                    Some(value_for_oracle_option(&args, next_index, option)?.to_string());
                index += 2;
            }
            "--capture-bin-dir" => {
                capture_bin_dir = Some(PathBuf::from(value_for_oracle_option(
                    &args, next_index, option,
                )?));
                index += 2;
            }
            "--capture-allow-missing-entry-files" => {
                allow_missing_entry_files = true;
                index += 1;
            }
            _ => {
                if apply_regression_run_flag(&mut regression, option) {
                    index += 1;
                } else {
                    return Err(CliError::Usage(format!(
                        "Unknown option '{}'.\n{}",
                        option,
                        oracle_usage_text()
                    )));
                }
            }
        }
    }

    let capture_mode = match (capture_runner, capture_bin_dir) {
        (Some(_), Some(_)) => {
            return Err(CliError::Usage(format!(
                "Use either '--capture-runner' or '--capture-bin-dir', not both.\n{}",
                oracle_usage_text()
            )));
        }
        (Some(command), None) => OracleCaptureMode::Runner(command),
        (None, Some(path)) => OracleCaptureMode::BinDir(path),
        (None, None) => {
            return Err(CliError::Usage(format!(
                "Missing required oracle capture mode ('--capture-runner' or '--capture-bin-dir').\n{}",
                oracle_usage_text()
            )));
        }
    };

    Ok(OracleCommandConfig {
        regression,
        capture_mode,
        allow_missing_entry_files,
    })
}

fn apply_regression_run_flag(config: &mut RegressionRunnerConfig, option: &str) -> bool {
    match option {
        "--run-rdinp" | "--run-rdinp-placeholder" => config.run_rdinp = true,
        "--run-pot" | "--run-pot-placeholder" => config.run_pot = true,
        "--run-xsph" | "--run-xsph-placeholder" => config.run_xsph = true,
        "--run-path" | "--run-path-placeholder" => config.run_path = true,
        "--run-fms" | "--run-fms-placeholder" => config.run_fms = true,
        "--run-band" | "--run-band-placeholder" => config.run_band = true,
        "--run-ldos" | "--run-ldos-placeholder" => config.run_ldos = true,
        "--run-rixs" | "--run-rixs-placeholder" => config.run_rixs = true,
        "--run-crpa" | "--run-crpa-placeholder" => config.run_crpa = true,
        "--run-compton" | "--run-compton-placeholder" => config.run_compton = true,
        "--run-debye" | "--run-debye-placeholder" => config.run_debye = true,
        "--run-dmdw" | "--run-dmdw-placeholder" => config.run_dmdw = true,
        "--run-fullspectrum" | "--run-fullspectrum-placeholder" => config.run_full_spectrum = true,
        "--run-screen" | "--run-screen-placeholder" => config.run_screen = true,
        "--run-self" | "--run-self-placeholder" => config.run_self = true,
        "--run-eels" | "--run-eels-placeholder" => config.run_eels = true,
        _ => return false,
    }
    true
}

fn resolve_regression_paths(
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

fn resolve_cli_path(working_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(path)
    }
}

fn run_oracle_capture(workspace_root: &Path, config: &OracleCommandConfig) -> ComputeResult<()> {
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

fn value_for_option<'a>(
    args: &'a [String],
    value_index: usize,
    option: &str,
) -> Result<&'a str, CliError> {
    args.get(value_index)
        .map(|value| value.as_str())
        .ok_or_else(|| {
            CliError::Usage(format!(
                "Missing value for option '{}'.\n{}",
                option,
                regression_usage_text()
            ))
        })
}

fn value_for_oracle_option<'a>(
    args: &'a [String],
    value_index: usize,
    option: &str,
) -> Result<&'a str, CliError> {
    args.get(value_index)
        .map(|value| value.as_str())
        .ok_or_else(|| {
            CliError::Usage(format!(
                "Missing value for option '{}'.\n{}",
                option,
                oracle_usage_text()
            ))
        })
}

fn help_requested(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--help" || arg == "-h")
}

fn usage_text() -> &'static str {
    "Usage:\n  feff10-rs <command> [options]\n  feff10-rs help\n\nCommands:\n  regression       Run fixture regression comparisons\n  oracle           Run validation-only dual-run oracle capture and comparison\n  feff             Run serial FEFF compatibility chain in current directory\n  feffmpi <nprocs> Run MPI-compatible FEFF entrypoint (serial fallback in v1)\n  rdinp            Run RDINP module in current directory\n  pot              Run POT module in current directory\n  xsph             Run XSPH module in current directory\n  path             Run PATH module in current directory\n  fms              Run FMS module in current directory\n  band             Run BAND module in current directory\n  ldos             Run LDOS module in current directory\n  rixs             Run RIXS module in current directory\n  crpa             Run CRPA module in current directory\n  compton          Run COMPTON module in current directory\n  ff2x             Run DEBYE module (`ff2x`) in current directory\n  dmdw             Run DMDW module in current directory\n  screen           Run SCREEN module in current directory\n  sfconv           Run SELF module (`sfconv`) in current directory\n  eels             Run EELS module in current directory\n  fullspectrum     Run FULLSPECTRUM module in current directory\n\nRun `feff10-rs regression --help` for regression options.\nRun `feff10-rs oracle --help` for oracle options.\nRun `feff10-rs <module> --help` for module command usage."
}

fn regression_usage_text() -> &'static str {
    "Usage:\n  feff10-rs regression [options]\n\nOptions:\n  --manifest <path>         Fixture manifest path (default: tasks/golden-fixture-manifest.json)\n  --policy <path>           Numeric tolerance policy path (default: tasks/numeric-tolerance-policy.json)\n  --baseline-root <path>    Baseline snapshot root (default: artifacts/fortran-baselines)\n  --actual-root <path>      Actual output root (default: artifacts/fortran-baselines)\n  --baseline-subdir <name>  Baseline subdirectory per fixture (default: baseline)\n  --actual-subdir <name>    Actual subdirectory per fixture (default: baseline)\n  --report <path>           JSON report output path (default: artifacts/regression/report.json)\n  --run-rdinp              Run RDINP module before fixture comparisons\n  --run-pot                Run POT module before fixture comparisons\n  --run-screen             Run SCREEN module before fixture comparisons\n  --run-self               Run SELF module before fixture comparisons\n  --run-eels               Run EELS module before fixture comparisons\n  --run-fullspectrum       Run FULLSPECTRUM module before fixture comparisons\n  --run-xsph               Run XSPH module before fixture comparisons\n  --run-path               Run PATH module before fixture comparisons\n  --run-fms                Run FMS module before fixture comparisons\n  --run-band               Run BAND module before fixture comparisons\n  --run-ldos               Run LDOS module before fixture comparisons\n  --run-rixs               Run RIXS module before fixture comparisons\n  --run-crpa               Run CRPA module before fixture comparisons\n  --run-compton            Run COMPTON module before fixture comparisons\n  --run-debye              Run DEBYE module before fixture comparisons\n  --run-dmdw               Run DMDW module before fixture comparisons"
}

fn oracle_usage_text() -> &'static str {
    "Usage:\n  feff10-rs oracle [options]\n\nOptions:\n  --manifest <path>         Fixture manifest path for capture and comparison (default: tasks/golden-fixture-manifest.json)\n  --policy <path>           Numeric tolerance policy path (default: tasks/numeric-tolerance-policy.json)\n  --oracle-root <path>      Fortran capture output root used as regression baseline (default: artifacts/fortran-oracle-capture)\n  --oracle-subdir <name>    Oracle subdirectory per fixture (default: outputs)\n  --actual-root <path>      Rust actual output root (default: artifacts/oracle-actual)\n  --actual-subdir <name>    Rust actual subdirectory per fixture (default: actual)\n  --report <path>           JSON report output path (default: artifacts/regression/oracle-report.json)\n  --capture-runner <cmd>    Runner command passed to scripts/fortran/capture-baselines.sh\n  --capture-bin-dir <path>  Fortran module binary directory passed to scripts/fortran/capture-baselines.sh\n  --capture-allow-missing-entry-files\n                           Continue capture when manifest entry files are missing and record metadata\n  --run-rdinp              Run RDINP module before fixture comparisons\n  --run-pot                Run POT module before fixture comparisons\n  --run-screen             Run SCREEN module before fixture comparisons\n  --run-self               Run SELF module before fixture comparisons\n  --run-eels               Run EELS module before fixture comparisons\n  --run-fullspectrum       Run FULLSPECTRUM module before fixture comparisons\n  --run-xsph               Run XSPH module before fixture comparisons\n  --run-path               Run PATH module before fixture comparisons\n  --run-fms                Run FMS module before fixture comparisons\n  --run-band               Run BAND module before fixture comparisons\n  --run-ldos               Run LDOS module before fixture comparisons\n  --run-rixs               Run RIXS module before fixture comparisons\n  --run-crpa               Run CRPA module before fixture comparisons\n  --run-compton            Run COMPTON module before fixture comparisons\n  --run-debye              Run DEBYE module before fixture comparisons\n  --run-dmdw               Run DMDW module before fixture comparisons\n\nThe oracle command is validation-only and must not be used as a production runtime path."
}

fn feff_usage_text() -> &'static str {
    "Usage:\n  feff10-rs feff\n\nRuns the serial FEFF compatibility chain in the current working directory. No positional arguments are accepted."
}

fn feffmpi_usage_text() -> &'static str {
    "Usage:\n  feff10-rs feffmpi <nprocs>\n\nRuns the MPI-compatible FEFF entrypoint.\n`<nprocs>` must be a positive integer."
}

fn module_usage_text(spec: ModuleCommandSpec) -> String {
    format!(
        "Usage:\n  feff10-rs {}\n\nRuns module {} in the current working directory.\nRequired entry artifact: '{}'.",
        spec.command, spec.module, spec.input_artifact
    )
}

#[derive(Debug)]
pub enum CliError {
    Usage(String),
    Compute(FeffError),
}

impl CliError {
    fn as_feff_error(&self) -> FeffError {
        match self {
            Self::Usage(message) => FeffError::input_validation("INPUT.CLI_USAGE", message.clone()),
            Self::Compute(error) => error.clone(),
        }
    }
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => f.write_str(message),
            Self::Compute(source) => write!(f, "{}", source),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Usage(_) => None,
            Self::Compute(source) => Some(source),
        }
    }
}
