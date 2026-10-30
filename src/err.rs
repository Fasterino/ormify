use std::fmt::{Debug, Display};

use serde::{Deserialize, Serialize};

use crate::EntityId;

/// Errors returned by schema construction, query execution, and conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    TypeMismatch,
    EmptyResponse,
    EntityNotInitialized(EntityId),
    NoField(String),
    Build(String),
    Database(String),
    SerializationError(String),
    InvalidJoin(String),
    InvalidSchema(String),
}

impl Error {
    pub fn make_build_error<I: Into<String>>(error_text: I) -> Self {
        Self::Build(error_text.into())
    }
    pub fn make_database_error<I: Into<String>>(error_text: I) -> Self {
        Self::Database(error_text.into())
    }
    pub fn make_serialization_error<I: Into<String>>(error_text: I) -> Self {
        Self::SerializationError(error_text.into())
    }

    pub fn make_invalid_join_error<I: Into<String>>(error_text: I) -> Self {
        Self::InvalidJoin(error_text.into())
    }

    pub fn make_invalid_schema_error<I: Into<String>>(error_text: I) -> Self {
        Self::InvalidSchema(error_text.into())
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self, f)
    }
}

/// Result type used throughout the ORM API.
pub type Result<T = ()> = std::result::Result<T, Error>;
