//! Errors from drivers and file tasks. `Display` shows only the message, which is what
//! the user sees in alerts.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum DbError {
    /// Could not reach or log in to the server.
    Connect(String),
    /// The server rejected a statement.
    Query(String),
    /// Reading or writing a file failed.
    Io(String),
    /// The input cannot be used, e.g. an empty import file.
    Invalid(String),
}

impl DbError {
    pub fn message(&self) -> &str {
        match self {
            DbError::Connect(m) | DbError::Query(m) | DbError::Io(m) | DbError::Invalid(m) => m,
        }
    }

    /// Same kind of error with extra context in front of the message.
    pub fn context(self, prefix: &str) -> Self {
        let msg = format!("{}{}", prefix, self.message());
        match self {
            DbError::Connect(_) => DbError::Connect(msg),
            DbError::Query(_) => DbError::Query(msg),
            DbError::Io(_) => DbError::Io(msg),
            DbError::Invalid(_) => DbError::Invalid(msg),
        }
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for DbError {}
