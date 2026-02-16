use feff10_rs::regression::{RegressionRunnerConfig, render_human_summary, run_regression};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

fn main() {
    let exit_code = match run_cli() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("Error: {}", error);
            2
        }
    };
    std::process::exit(exit_code);
}

fn run_cli() -> Result<i32, CliError> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
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
    let report = run_regression(&config).map_err(CliError::Regression)?;
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
    "Usage:
  feff10-rs regression [options]
  feff10-rs help

Run `feff10-rs regression --help` for command options."
}

fn regression_usage_text() -> &'static str {
    "Usage:
  feff10-rs regression [options]

Options:
  --manifest <path>        Fixture manifest path (default: tasks/golden-fixture-manifest.json)
  --policy <path>          Numeric tolerance policy path (default: tasks/numeric-tolerance-policy.json)
  --baseline-root <path>   Baseline snapshot root (default: artifacts/fortran-baselines)
  --actual-root <path>     Actual output root (default: artifacts/fortran-baselines)
  --baseline-subdir <name> Baseline subdirectory per fixture (default: baseline)
  --actual-subdir <name>   Actual subdirectory per fixture (default: baseline)
  --report <path>          JSON report output path (default: artifacts/regression/report.json)"
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Regression(feff10_rs::regression::RegressionRunnerError),
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => f.write_str(message),
            Self::Regression(source) => write!(f, "{}", source),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Usage(_) => None,
            Self::Regression(source) => Some(source),
        }
    }
}
