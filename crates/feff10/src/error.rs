use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Parse(ParseError),
    Pipeline(PipelineError),
    Config(String),
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

#[derive(Debug)]
pub struct PipelineError {
    pub stage: String,
    pub exit_code: Option<i32>,
    pub stderr: String,
    pub feff_error: Option<String>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Parse(e) => write!(f, "Parse error at line {}: {}", e.line, e.message),
            Error::Pipeline(e) => {
                write!(f, "Pipeline error in stage '{}': {}", e.stage, e.stderr)?;
                if let Some(code) = e.exit_code {
                    write!(f, " (exit code: {code})")?;
                }
                if let Some(ref feff_err) = e.feff_error {
                    write!(f, "\n.feff.error: {feff_err}")?;
                }
                Ok(())
            }
            Error::Config(msg) => write!(f, "Config error: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
