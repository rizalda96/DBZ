//! The AI assistant's brain, without any UI or database access: the tool protocol,
//! the system prompt, and the [`AiBackend`] strategies (Claude CLI, OpenAI-style URL).
//! [`backend_for`] is the single place that picks one.

mod backend;
mod claude_cli;
mod images;
mod openai;
mod prompt;
mod protocol;
#[cfg(test)]
mod tests;

pub use backend::{AiBackend, AiError};
pub use prompt::{system_prompt, PromptContext};
pub use protocol::{clip, format_objects, format_sets, format_table, parse_reply, segments, Parsed, MAX_ROWS};

use std::path::PathBuf;

use crate::domain::{AiBackendKind, AiConfig};
use claude_cli::ClaudeCli;
use openai::OpenAiCompat;

/// The backend `cfg` chooses; `work_dir` is the CLI's own empty working folder.
pub fn backend_for(cfg: &AiConfig, work_dir: PathBuf) -> Box<dyn AiBackend> {
    match cfg.backend {
        AiBackendKind::ClaudeCli => Box::new(ClaudeCli { path: cfg.cli_path.trim().to_string(), model: cfg.cli_model.clone(), work_dir }),
        AiBackendKind::OpenAiCompat => Box::new(OpenAiCompat { base_url: cfg.base_url.clone(), api_key: cfg.api_key.clone(), model: cfg.model.clone() }),
    }
}
