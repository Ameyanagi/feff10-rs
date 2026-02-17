mod commands;
mod dispatch;
mod helpers;

use clap::Parser;
use feff_core::domain::FeffError;
use dispatch::{module_command_spec, command_alias_from_program_name};

pub fn run_from_env() -> i32 {
    let mut args = std::env::args();
    let program_name = args.next().unwrap_or_else(|| "feff10-rs".to_string());
    let remaining: Vec<String> = args.collect();

    match run_with_program_name(&program_name, remaining) {
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
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    let full_args = std::iter::once("feff10-rs".to_string())
        .chain(args)
        .collect::<Vec<_>>();
    parse_and_dispatch(full_args)
}

fn run_with_program_name(program_name: &str, args: Vec<String>) -> Result<i32, CliError> {
    if let Some(alias_command) = command_alias_from_program_name(program_name) {
        let full_args = std::iter::once("feff10-rs".to_string())
            .chain(std::iter::once(alias_command.to_string()))
            .chain(args)
            .collect::<Vec<_>>();
        return parse_and_dispatch(full_args);
    }

    let full_args = std::iter::once("feff10-rs".to_string())
        .chain(args)
        .collect::<Vec<_>>();
    parse_and_dispatch(full_args)
}

fn parse_and_dispatch(args: Vec<String>) -> Result<i32, CliError> {
    match Cli::try_parse_from(&args) {
        Ok(cli) => dispatch_parsed(cli.command),
        Err(err) => match err.kind() {
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                print!("{}", err);
                Ok(0)
            }
            _ => Err(CliError::Usage(err.to_string())),
        },
    }
}

#[derive(Parser)]
#[command(name = "feff10-rs", about = "FEFF10 Rust compute engine")]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(clap::Subcommand)]
enum CliCommand {
    /// Run fixture regression comparisons
    Regression(commands::RegressionArgs),
    /// Run validation-only dual-run oracle capture and comparison
    Oracle(commands::OracleArgs),
    /// Run serial FEFF compatibility chain in current directory
    Feff,
    /// Run MPI-compatible FEFF entrypoint (serial fallback in v1)
    Feffmpi {
        /// Number of MPI processes
        #[arg(value_name = "nprocs")]
        nprocs: usize,
    },
    /// Run RDINP module in current directory
    Rdinp,
    /// Run POT module in current directory
    Pot,
    /// Run XSPH module in current directory
    Xsph,
    /// Run PATH module in current directory
    Path,
    /// Run FMS module in current directory
    Fms,
    /// Run BAND module in current directory
    Band,
    /// Run LDOS module in current directory
    Ldos,
    /// Run RIXS module in current directory
    Rixs,
    /// Run CRPA module in current directory
    Crpa,
    /// Run COMPTON module in current directory
    Compton,
    /// Run DEBYE module (ff2x) in current directory
    #[command(name = "ff2x")]
    Ff2x,
    /// Run DMDW module in current directory
    Dmdw,
    /// Run SCREEN module in current directory
    Screen,
    /// Run SELF module (sfconv) in current directory
    #[command(name = "sfconv")]
    Sfconv,
    /// Run EELS module in current directory
    Eels,
    /// Run FULLSPECTRUM module in current directory
    Fullspectrum,
}

fn dispatch_parsed(command: CliCommand) -> Result<i32, CliError> {
    match command {
        CliCommand::Regression(args) => commands::run_regression_command(args),
        CliCommand::Oracle(args) => commands::run_oracle_command(args),
        CliCommand::Feff => commands::run_feff_command(),
        CliCommand::Feffmpi { nprocs } => commands::run_feffmpi_command(nprocs),
        CliCommand::Rdinp => dispatch_module("rdinp"),
        CliCommand::Pot => dispatch_module("pot"),
        CliCommand::Xsph => dispatch_module("xsph"),
        CliCommand::Path => dispatch_module("path"),
        CliCommand::Fms => dispatch_module("fms"),
        CliCommand::Band => dispatch_module("band"),
        CliCommand::Ldos => dispatch_module("ldos"),
        CliCommand::Rixs => dispatch_module("rixs"),
        CliCommand::Crpa => dispatch_module("crpa"),
        CliCommand::Compton => dispatch_module("compton"),
        CliCommand::Ff2x => dispatch_module("ff2x"),
        CliCommand::Dmdw => dispatch_module("dmdw"),
        CliCommand::Screen => dispatch_module("screen"),
        CliCommand::Sfconv => dispatch_module("sfconv"),
        CliCommand::Eels => dispatch_module("eels"),
        CliCommand::Fullspectrum => dispatch_module("fullspectrum"),
    }
}

fn dispatch_module(command_name: &str) -> Result<i32, CliError> {
    let spec = module_command_spec(command_name)
        .expect("module command should be registered in MODULE_COMMANDS");
    commands::run_module_command(spec)
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Compute(FeffError),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl CliError {
    fn as_feff_error(&self) -> FeffError {
        match self {
            Self::Usage(message) => FeffError::input_validation("INPUT.CLI_USAGE", message.clone()),
            Self::Compute(error) => error.clone(),
            Self::Internal(error) => FeffError::io_system("IO.CLI", format!("{error:#}")),
        }
    }
}
