use std::fmt;

use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeErrorKind {
    BadRequest,
    NotFound,
    Forbidden,
    Conflict,
    ValidationFailed,
    Unavailable,
    Internal,
}

impl RuntimeErrorKind {
    pub fn code(self) -> &'static str {
        match self {
            RuntimeErrorKind::BadRequest => "bad_request",
            RuntimeErrorKind::NotFound => "not_found",
            RuntimeErrorKind::Forbidden => "forbidden",
            RuntimeErrorKind::Conflict => "conflict",
            RuntimeErrorKind::ValidationFailed => "validation_failed",
            RuntimeErrorKind::Unavailable => "unavailable",
            RuntimeErrorKind::Internal => "internal_error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeDomainError {
    pub kind: RuntimeErrorKind,
    pub message: String,
    pub details: Value,
}

impl RuntimeDomainError {
    pub fn new(kind: RuntimeErrorKind, message: impl Into<String>, details: Value) -> Self {
        Self {
            kind,
            message: message.into(),
            details,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(RuntimeErrorKind::BadRequest, message, json!({}))
    }

    pub fn not_found(entity: &'static str, id: impl ToString) -> Self {
        let id = id.to_string();
        Self::new(
            RuntimeErrorKind::NotFound,
            format!("{entity} not found: {id}"),
            json!({"entity": entity, "id": id}),
        )
    }

    pub fn forbidden(message: impl Into<String>, details: Value) -> Self {
        Self::new(RuntimeErrorKind::Forbidden, message, details)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(RuntimeErrorKind::Conflict, message, json!({}))
    }

    pub fn validation_failed(message: impl Into<String>) -> Self {
        Self::new(RuntimeErrorKind::ValidationFailed, message, json!({}))
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(RuntimeErrorKind::Unavailable, message, json!({}))
    }

    pub fn internal_safe() -> Self {
        Self::new(RuntimeErrorKind::Internal, "unexpected server error", json!({}))
    }
}

impl fmt::Display for RuntimeDomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RuntimeDomainError {}

pub type DomainResult<T> = Result<T, RuntimeDomainError>;
