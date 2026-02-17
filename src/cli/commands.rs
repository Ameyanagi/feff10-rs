use super::CliError;
use super::dispatch::{ModuleCommandSpec, module_command_for_module};
use super::helpers::*;
use crate::domain::FeffError;
use crate::modules::regression::{RegressionRunnerConfig, render_human_summary, run_regression};
use crate::modules::{runtime_compute_engine_available, runtime_engine_unavailable_error};
use std::path::PathBuf;

pub(super) fn run_regression_command(args: Vec<String>) -> Result<i32, CliError> {
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

pub(super) fn run_oracle_command(args: Vec<String>) -> Result<i32, CliError> {
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

pub(super) fn run_feff_command(args: Vec<String>) -> Result<i32, CliError> {
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

pub(super) fn run_feffmpi_command(args: Vec<String>) -> Result<i32, CliError> {
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

pub(super) fn run_module_command(spec: ModuleCommandSpec, args: Vec<String>) -> Result<i32, CliError> {
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
