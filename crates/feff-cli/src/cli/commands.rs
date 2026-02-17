use super::CliError;
use super::dispatch::{ModuleCommandSpec, module_command_for_module};
use super::helpers::*;
use feff_core::domain::FeffError;
use feff_core::modules::regression::{RegressionRunnerConfig, render_human_summary, run_regression};
use feff_core::modules::{runtime_compute_engine_available, runtime_engine_unavailable_error};
use std::path::PathBuf;

#[derive(clap::Args)]
pub(super) struct RegressionArgs {
    /// Fixture manifest path
    #[arg(long, default_value = "tasks/golden-fixture-manifest.json")]
    manifest: PathBuf,

    /// Numeric tolerance policy path
    #[arg(long, default_value = "tasks/numeric-tolerance-policy.json")]
    policy: PathBuf,

    /// Baseline snapshot root
    #[arg(long, default_value = "artifacts/fortran-baselines")]
    baseline_root: PathBuf,

    /// Actual output root
    #[arg(long, default_value = "artifacts/fortran-baselines")]
    actual_root: PathBuf,

    /// Baseline subdirectory per fixture
    #[arg(long, default_value = "baseline")]
    baseline_subdir: String,

    /// Actual subdirectory per fixture
    #[arg(long, default_value = "baseline")]
    actual_subdir: String,

    /// JSON report output path
    #[arg(long, default_value = "artifacts/regression/report.json")]
    report: PathBuf,

    #[command(flatten)]
    run: RunModuleFlags,
}

#[derive(clap::Args)]
#[command(group(clap::ArgGroup::new("capture").required(true).args(["capture_runner", "capture_bin_dir"])))]
pub(super) struct OracleArgs {
    /// Fixture manifest path
    #[arg(long, default_value = "tasks/golden-fixture-manifest.json")]
    manifest: PathBuf,

    /// Numeric tolerance policy path
    #[arg(long, default_value = "tasks/numeric-tolerance-policy.json")]
    policy: PathBuf,

    /// Fortran capture output root used as regression baseline
    #[arg(long, default_value = "artifacts/fortran-oracle-capture")]
    oracle_root: PathBuf,

    /// Oracle subdirectory per fixture
    #[arg(long, default_value = "outputs")]
    oracle_subdir: String,

    /// Rust actual output root
    #[arg(long, default_value = "artifacts/oracle-actual")]
    actual_root: PathBuf,

    /// Rust actual subdirectory per fixture
    #[arg(long, default_value = "actual")]
    actual_subdir: String,

    /// JSON report output path
    #[arg(long, default_value = "artifacts/regression/oracle-report.json")]
    report: PathBuf,

    /// Runner command passed to capture script
    #[arg(long)]
    capture_runner: Option<String>,

    /// Fortran module binary directory passed to capture script
    #[arg(long)]
    capture_bin_dir: Option<PathBuf>,

    /// Continue capture when manifest entry files are missing
    #[arg(long)]
    capture_allow_missing_entry_files: bool,

    #[command(flatten)]
    run: RunModuleFlags,
}

#[derive(clap::Args, Default)]
pub(super) struct RunModuleFlags {
    /// Run RDINP module before comparisons
    #[arg(long, alias = "run-rdinp-placeholder")]
    run_rdinp: bool,

    /// Run POT module before comparisons
    #[arg(long, alias = "run-pot-placeholder")]
    run_pot: bool,

    /// Run XSPH module before comparisons
    #[arg(long, alias = "run-xsph-placeholder")]
    run_xsph: bool,

    /// Run PATH module before comparisons
    #[arg(long, alias = "run-path-placeholder")]
    run_path: bool,

    /// Run FMS module before comparisons
    #[arg(long, alias = "run-fms-placeholder")]
    run_fms: bool,

    /// Run BAND module before comparisons
    #[arg(long, alias = "run-band-placeholder")]
    run_band: bool,

    /// Run LDOS module before comparisons
    #[arg(long, alias = "run-ldos-placeholder")]
    run_ldos: bool,

    /// Run RIXS module before comparisons
    #[arg(long, alias = "run-rixs-placeholder")]
    run_rixs: bool,

    /// Run CRPA module before comparisons
    #[arg(long, alias = "run-crpa-placeholder")]
    run_crpa: bool,

    /// Run COMPTON module before comparisons
    #[arg(long, alias = "run-compton-placeholder")]
    run_compton: bool,

    /// Run DEBYE module before comparisons
    #[arg(long, alias = "run-debye-placeholder")]
    run_debye: bool,

    /// Run DMDW module before comparisons
    #[arg(long, alias = "run-dmdw-placeholder")]
    run_dmdw: bool,

    /// Run SCREEN module before comparisons
    #[arg(long, alias = "run-screen-placeholder")]
    run_screen: bool,

    /// Run SELF module before comparisons
    #[arg(long, alias = "run-self-placeholder")]
    run_self: bool,

    /// Run EELS module before comparisons
    #[arg(long, alias = "run-eels-placeholder")]
    run_eels: bool,

    /// Run FULLSPECTRUM module before comparisons
    #[arg(long, alias = "run-fullspectrum-placeholder")]
    run_fullspectrum: bool,
}

impl RegressionArgs {
    fn into_config(self) -> RegressionRunnerConfig {
        RegressionRunnerConfig {
            manifest_path: self.manifest,
            policy_path: self.policy,
            baseline_root: self.baseline_root,
            actual_root: self.actual_root,
            baseline_subdir: self.baseline_subdir,
            actual_subdir: self.actual_subdir,
            report_path: self.report,
            run_rdinp: self.run.run_rdinp,
            run_pot: self.run.run_pot,
            run_xsph: self.run.run_xsph,
            run_path: self.run.run_path,
            run_fms: self.run.run_fms,
            run_band: self.run.run_band,
            run_ldos: self.run.run_ldos,
            run_rixs: self.run.run_rixs,
            run_crpa: self.run.run_crpa,
            run_compton: self.run.run_compton,
            run_debye: self.run.run_debye,
            run_dmdw: self.run.run_dmdw,
            run_screen: self.run.run_screen,
            run_self: self.run.run_self,
            run_eels: self.run.run_eels,
            run_full_spectrum: self.run.run_fullspectrum,
        }
    }
}

impl OracleArgs {
    fn into_config(self) -> OracleCommandConfig {
        let capture_mode = if let Some(runner) = self.capture_runner {
            OracleCaptureMode::Runner(runner)
        } else if let Some(path) = self.capture_bin_dir {
            OracleCaptureMode::BinDir(path)
        } else {
            unreachable!("clap requires one of --capture-runner or --capture-bin-dir")
        };

        let regression = RegressionRunnerConfig {
            manifest_path: self.manifest,
            policy_path: self.policy,
            baseline_root: self.oracle_root,
            actual_root: self.actual_root,
            baseline_subdir: self.oracle_subdir,
            actual_subdir: self.actual_subdir,
            report_path: self.report,
            run_rdinp: self.run.run_rdinp,
            run_pot: self.run.run_pot,
            run_xsph: self.run.run_xsph,
            run_path: self.run.run_path,
            run_fms: self.run.run_fms,
            run_band: self.run.run_band,
            run_ldos: self.run.run_ldos,
            run_rixs: self.run.run_rixs,
            run_crpa: self.run.run_crpa,
            run_compton: self.run.run_compton,
            run_debye: self.run.run_debye,
            run_dmdw: self.run.run_dmdw,
            run_screen: self.run.run_screen,
            run_self: self.run.run_self,
            run_eels: self.run.run_eels,
            run_full_spectrum: self.run.run_fullspectrum,
        };

        OracleCommandConfig {
            regression,
            capture_mode,
            allow_missing_entry_files: self.capture_allow_missing_entry_files,
        }
    }
}

pub(super) fn run_regression_command(args: RegressionArgs) -> Result<i32, CliError> {
    let config = args.into_config();
    let report = run_regression(&config).map_err(CliError::Compute)?;
    println!("{}", render_human_summary(&report));
    println!("JSON report: {}", config.report_path.display());

    if report.passed { Ok(0) } else { Ok(1) }
}

pub(super) fn run_oracle_command(args: OracleArgs) -> Result<i32, CliError> {
    let mut config = args.into_config();
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

pub(super) fn run_feff_command() -> Result<i32, CliError> {
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

pub(super) fn run_feffmpi_command(process_count: usize) -> Result<i32, CliError> {
    if process_count == 0 {
        return Err(CliError::Usage(
            "Invalid process count '0'; expected a positive integer.".to_string(),
        ));
    }

    if process_count > 1 {
        eprintln!(
            "WARNING: [RUN.MPI_DEFERRED] MPI parity is deferred for Rust v1; executing serial compatibility chain instead (requested nprocs={}).",
            process_count
        );
    }

    run_feff_command()
}

pub(super) fn run_module_command(spec: ModuleCommandSpec) -> Result<i32, CliError> {
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
