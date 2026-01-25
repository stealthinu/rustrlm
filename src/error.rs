use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplError {
    #[error("parse error: {0}")]
    ParseError(String),

    #[error("forbidden syntax: {0}")]
    ForbiddenSyntax(String),

    #[error("forbidden name: {0}")]
    ForbiddenName(String),

    #[error("name error: {0}")]
    NameError(String),

    #[error("type error: {0}")]
    TypeError(String),

    #[error("value error: {0}")]
    ValueError(String),

    #[error("resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("runtime error: {0}")]
    RuntimeError(String),

    #[error("SystemExit")]
    SystemExit,
}
