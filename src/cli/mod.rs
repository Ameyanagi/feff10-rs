use crate::domain::FeffError;
use crate::pipelines::regression::{RegressionRunnerConfig, render_human_summary, run_regression};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

pub fn run_from_env() -> i32 {
    match run(std::env::args().skip(1)) {
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
    let mut args: Vec<String> = args.into_iter().map(Into::into).collect();
    if args.is_empty() {
        return Err(CliError::Usage(usage_text().to_string()));
    }

    let command = args.remove(0);
    match command.as_str() {
        "regression" => run_regression_command(args),
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
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", regression_usage_text());
        return Ok(0);
    }

    let config = parse_regression_args(args)?;
    let report = run_regression(&config).map_err(CliError::Pipeline)?;
    println!("{}", render_human_summary(&report));
    println!("JSON report: {}", config.report_path.display());

    if report.passed { Ok(0) } else { Ok(1) }
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
            "--run-rdinp" | "--run-rdinp-placeholder" => {
                config.run_rdinp = true;
                index += 1;
            }
            "--run-pot" | "--run-pot-placeholder" => {
                config.run_pot = true;
                index += 1;
            }
            "--run-xsph" | "--run-xsph-placeholder" => {
                config.run_xsph = true;
                index += 1;
            }
            "--run-path" | "--run-path-placeholder" => {
                config.run_path = true;
                index += 1;
            }
            "--run-fms" | "--run-fms-placeholder" => {
                config.run_fms = true;
                index += 1;
            }
            "--run-band" | "--run-band-placeholder" => {
                config.run_band = true;
                index += 1;
            }
            "--run-ldos" | "--run-ldos-placeholder" => {
                config.run_ldos = true;
                index += 1;
            }
            "--run-rixs" | "--run-rixs-placeholder" => {
                config.run_rixs = true;
                index += 1;
            }
            "--run-crpa" | "--run-crpa-placeholder" => {
                config.run_crpa = true;
                index += 1;
            }
            "--run-compton" | "--run-compton-placeholder" => {
                config.run_compton = true;
                index += 1;
            }
            _ => {
                return Err(CliError::Usage(format!(
                    "Unknown option '{}'.\n{}",
                    option,
                    regression_usage_text()
                )));
            }
        }
    }

    Ok(config)
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

fn usage_text() -> &'static str {
    "Usage:\n  feff10-rs regression [options]\n  feff10-rs help\n\nRun `feff10-rs regression --help` for command options."
}

fn regression_usage_text() -> &'static str {
    "Usage:\n  feff10-rs regression [options]\n\nOptions:\n  --manifest <path>         Fixture manifest path (default: tasks/golden-fixture-manifest.json)\n  --policy <path>           Numeric tolerance policy path (default: tasks/numeric-tolerance-policy.json)\n  --baseline-root <path>    Baseline snapshot root (default: artifacts/fortran-baselines)\n  --actual-root <path>      Actual output root (default: artifacts/fortran-baselines)\n  --baseline-subdir <name>  Baseline subdirectory per fixture (default: baseline)\n  --actual-subdir <name>    Actual subdirectory per fixture (default: baseline)\n  --report <path>           JSON report output path (default: artifacts/regression/report.json)\n  --run-rdinp              Run RDINP pipeline before fixture comparisons\n  --run-pot                Run POT pipeline before fixture comparisons\n  --run-xsph               Run XSPH pipeline before fixture comparisons\n  --run-path               Run PATH pipeline before fixture comparisons\n  --run-fms                Run FMS pipeline before fixture comparisons\n  --run-band               Run BAND pipeline before fixture comparisons\n  --run-ldos               Run LDOS pipeline before fixture comparisons\n  --run-rixs               Run RIXS pipeline before fixture comparisons\n  --run-crpa               Run CRPA pipeline before fixture comparisons\n  --run-compton            Run COMPTON pipeline before fixture comparisons"
}

#[derive(Debug)]
pub enum CliError {
    Usage(String),
    Pipeline(FeffError),
}

impl CliError {
    fn as_feff_error(&self) -> FeffError {
        match self {
            Self::Usage(message) => FeffError::input_validation("INPUT.CLI_USAGE", message.clone()),
            Self::Pipeline(error) => error.clone(),
        }
    }
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => f.write_str(message),
            Self::Pipeline(source) => write!(f, "{}", source),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Usage(_) => None,
            Self::Pipeline(source) => Some(source),
        }
    }
}
