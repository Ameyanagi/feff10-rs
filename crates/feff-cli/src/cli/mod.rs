mod commands;
mod dispatch;
mod helpers;

use feff_core::domain::FeffError;
use dispatch::module_command_spec;
use helpers::usage_text;
use std::error::Error;
use std::fmt::{Display, Formatter};

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
    if let Some(alias_command) = dispatch::command_alias_from_program_name(program_name) {
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
        return commands::run_module_command(spec, args);
    }

    match command {
        "regression" => commands::run_regression_command(args),
        "oracle" => commands::run_oracle_command(args),
        "feff" => commands::run_feff_command(args),
        "feffmpi" => commands::run_feffmpi_command(args),
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
