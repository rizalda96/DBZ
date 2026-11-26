//! The `AiBackend` strategy: sends a conversation somewhere and gets the next reply.

use std::fmt;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::domain::ChatMsg;

/// How long one reply may take before it is given up on.
pub const AI_TIMEOUT: Duration = Duration::from_secs(180);

pub trait AiBackend: Send {
    /// The assistant's next message after `msgs`. Returns early with
    /// [`AiError::Cancelled`] once `cancel` is set, where the backend can stop.
    fn complete(&self, system: &str, msgs: &[ChatMsg], cancel: &AtomicBool) -> Result<String, AiError>;
}

#[derive(Debug, PartialEq)]
pub enum AiError {
    /// A setting is missing or can't work; says which.
    NotConfigured(String),
    /// The CLI could not start or ended with an error.
    Spawn(String),
    Http(String),
    Timeout,
    BadResponse(String),
    Cancelled,
}

impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiError::NotConfigured(m) => write!(f, "AI is not set up: {}", m),
            AiError::Spawn(m) => write!(f, "Claude CLI failed: {}", m),
            AiError::Http(m) => write!(f, "AI request failed: {}", m),
            AiError::Timeout => write!(f, "The AI took longer than {} seconds and was stopped", AI_TIMEOUT.as_secs()),
            AiError::BadResponse(m) => write!(f, "Unexpected AI response: {}", m),
            AiError::Cancelled => write!(f, "Stopped"),
        }
    }
}

/// At most `max` characters of `s`, for error messages.
pub(super) fn snippet(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    format!("{}…", s.chars().take(max).collect::<String>())
}
