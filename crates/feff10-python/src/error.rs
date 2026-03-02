use feff10::error::{Error, ParseError, PipelineError};
use pyo3::create_exception;
use pyo3::prelude::*;

create_exception!(_feff10, FeffError, pyo3::exceptions::PyException);
create_exception!(_feff10, FeffIOError, FeffError);
create_exception!(_feff10, FeffParseError, FeffError);
create_exception!(_feff10, FeffPipelineError, FeffError);
create_exception!(_feff10, FeffConfigError, FeffError);

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("FeffError", m.py().get_type::<FeffError>())?;
    m.add("FeffIOError", m.py().get_type::<FeffIOError>())?;
    m.add("FeffParseError", m.py().get_type::<FeffParseError>())?;
    m.add("FeffPipelineError", m.py().get_type::<FeffPipelineError>())?;
    m.add("FeffConfigError", m.py().get_type::<FeffConfigError>())?;
    Ok(())
}

/// Convert feff10::error::Error to a PyErr with the appropriate exception type.
pub fn to_pyerr(err: Error) -> PyErr {
    match err {
        Error::Io(e) => FeffIOError::new_err(e.to_string()),
        Error::Parse(ParseError { line, message }) => {
            FeffParseError::new_err(format!("line {line}: {message}"))
        }
        Error::Pipeline(PipelineError {
            stage,
            exit_code,
            stderr,
            feff_error,
        }) => {
            let mut msg = format!("stage '{stage}' failed");
            if let Some(code) = exit_code {
                msg.push_str(&format!(" (exit code: {code})"));
            }
            if !stderr.is_empty() {
                msg.push_str(&format!(": {stderr}"));
            }
            if let Some(ref fe) = feff_error {
                msg.push_str(&format!("\n.feff.error: {fe}"));
            }
            FeffPipelineError::new_err(msg)
        }
        Error::Config(msg) => FeffConfigError::new_err(msg),
    }
}
