use mcp_schema::ValidationReport;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ValidationFailed,
    ApiNotFound,
    PermissionDenied,
    ConsentRequired,
    VmFailed,
    RenderFailed,
    Timeout,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ValidationFailed => "validation_failed",
            Self::ApiNotFound => "api_not_found",
            Self::PermissionDenied => "permission_denied",
            Self::ConsentRequired => "consent_required",
            Self::VmFailed => "vm_failed",
            Self::RenderFailed => "render_failed",
            Self::Timeout => "timeout",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Error)]
pub enum DockCoreError {
    #[error("{code}: {message}")]
    Core { code: ErrorCode, message: String },

    #[error("{code}: {message}")]
    Validation {
        code: ErrorCode,
        message: String,
        report: ValidationReport,
    },
}

impl DockCoreError {
    pub fn core(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Core {
            code,
            message: message.into(),
        }
    }

    pub fn validation(message: impl Into<String>, report: ValidationReport) -> Self {
        Self::Validation {
            code: ErrorCode::ValidationFailed,
            message: message.into(),
            report,
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Core { code, .. } | Self::Validation { code, .. } => *code,
        }
    }
}
