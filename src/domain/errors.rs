use std::error::Error;
use std::fmt::{Display, Formatter};

pub type FeffResult<T> = Result<T, FeffError>;
pub type ParserResult<T> = FeffResult<T>;
pub type PipelineResult<T> = FeffResult<T>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeffErrorCategory {
    Success,
    InputValidationError,
    IoSystemError,
    ComputationError,
    InternalError,
}

impl FeffErrorCategory {
    pub const fn compatibility_placeholder(self) -> CompatibilityExitPlaceholder {
        match self {
            Self::Success => CompatibilityExitPlaceholder {
                exit_code: 0,
                rust_category: "Success",
                legacy_class: "SUCCESS",
            },
            Self::InputValidationError => CompatibilityExitPlaceholder {
                exit_code: 2,
                rust_category: "InputValidationError",
                legacy_class: "INPUT_FATAL",
            },
            Self::IoSystemError => CompatibilityExitPlaceholder {
                exit_code: 3,
                rust_category: "IoSystemError",
                legacy_class: "IO_FATAL",
            },
            Self::ComputationError => CompatibilityExitPlaceholder {
                exit_code: 4,
                rust_category: "ComputationError",
                legacy_class: "RUN_FATAL",
            },
            Self::InternalError => CompatibilityExitPlaceholder {
                exit_code: 5,
                rust_category: "InternalError",
                legacy_class: "SYS_FATAL",
            },
        }
    }

    pub const fn exit_code(self) -> i32 {
        self.compatibility_placeholder().exit_code
    }

    pub const fn rust_category(self) -> &'static str {
        self.compatibility_placeholder().rust_category
    }

    pub const fn legacy_class(self) -> &'static str {
        self.compatibility_placeholder().legacy_class
    }

    pub const fn is_fatal(self) -> bool {
        !matches!(self, Self::Success)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompatibilityExitPlaceholder {
    pub exit_code: i32,
    pub rust_category: &'static str,
    pub legacy_class: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeffError {
    category: FeffErrorCategory,
    placeholder: &'static str,
    message: String,
}

impl FeffError {
    pub fn new(
        category: FeffErrorCategory,
        placeholder: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            placeholder,
            message: message.into(),
        }
    }

    pub fn input_validation(placeholder: &'static str, message: impl Into<String>) -> Self {
        Self::new(
            FeffErrorCategory::InputValidationError,
            placeholder,
            message,
        )
    }

    pub fn io_system(placeholder: &'static str, message: impl Into<String>) -> Self {
        Self::new(FeffErrorCategory::IoSystemError, placeholder, message)
    }

    pub fn computation(placeholder: &'static str, message: impl Into<String>) -> Self {
        Self::new(FeffErrorCategory::ComputationError, placeholder, message)
    }

    pub fn internal(placeholder: &'static str, message: impl Into<String>) -> Self {
        Self::new(FeffErrorCategory::InternalError, placeholder, message)
    }

    pub const fn category(&self) -> FeffErrorCategory {
        self.category
    }

    pub const fn placeholder(&self) -> &'static str {
        self.placeholder
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn exit_code(&self) -> i32 {
        self.category.exit_code()
    }

    pub const fn compatibility_placeholder(&self) -> CompatibilityExitPlaceholder {
        self.category.compatibility_placeholder()
    }

    pub fn diagnostic_line(&self) -> String {
        let severity = if self.category.is_fatal() {
            "ERROR"
        } else {
            "INFO"
        };
        format!("{}: [{}] {}", severity, self.placeholder, self.message)
    }

    pub fn fatal_exit_line(&self) -> Option<String> {
        self.category
            .is_fatal()
            .then(|| format!("FATAL EXIT CODE: {}", self.exit_code()))
    }
}

impl Display for FeffError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} [{}] {}",
            self.category.rust_category(),
            self.placeholder,
            self.message
        )
    }
}

impl Error for FeffError {}

#[cfg(test)]
mod tests {
    use super::{FeffError, FeffErrorCategory};

    #[test]
    fn d4_exit_mapping_is_stable() {
        let cases = [
            (FeffErrorCategory::Success, 0, "Success", "SUCCESS"),
            (
                FeffErrorCategory::InputValidationError,
                2,
                "InputValidationError",
                "INPUT_FATAL",
            ),
            (
                FeffErrorCategory::IoSystemError,
                3,
                "IoSystemError",
                "IO_FATAL",
            ),
            (
                FeffErrorCategory::ComputationError,
                4,
                "ComputationError",
                "RUN_FATAL",
            ),
            (
                FeffErrorCategory::InternalError,
                5,
                "InternalError",
                "SYS_FATAL",
            ),
        ];

        for (category, exit_code, rust_category, legacy_class) in cases {
            let placeholder = category.compatibility_placeholder();
            assert_eq!(placeholder.exit_code, exit_code);
            assert_eq!(placeholder.rust_category, rust_category);
            assert_eq!(placeholder.legacy_class, legacy_class);
        }
    }

    #[test]
    fn fatal_error_renders_compatibility_lines() {
        let error =
            FeffError::input_validation("INPUT.INVALID_CARD", "invalid card 'BAD!' at line 3");

        assert_eq!(error.exit_code(), 2);
        assert_eq!(
            error.diagnostic_line(),
            "ERROR: [INPUT.INVALID_CARD] invalid card 'BAD!' at line 3"
        );
        assert_eq!(
            error.fatal_exit_line().as_deref(),
            Some("FATAL EXIT CODE: 2")
        );
    }
}
