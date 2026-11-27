//! The AI assistant's data: its settings, the conversation and the tools it may call.

use serde::{Deserialize, Serialize};

/// Where the assistant's answers come from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiBackendKind {
    /// The Claude command line on this computer.
    #[default]
    ClaudeCli,
    /// An OpenAI-style `/chat/completions` URL (e.g. 9router).
    OpenAiCompat,
}

impl AiBackendKind {
    pub const ALL: [AiBackendKind; 2] = [AiBackendKind::ClaudeCli, AiBackendKind::OpenAiCompat];

    pub fn label(self) -> &'static str {
        match self {
            AiBackendKind::ClaudeCli => "Claude CLI",
            AiBackendKind::OpenAiCompat => "OpenAI-compatible URL",
        }
    }
}

/// AI settings, saved in `settings.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub backend: AiBackendKind,
    /// The `claude` executable; a bare name is looked up on PATH.
    pub cli_path: String,
    /// Empty = the CLI's default model.
    pub cli_model: String,
    /// Up to `/v1`, e.g. `http://localhost:20128/v1`.
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        AiConfig {
            backend: AiBackendKind::ClaudeCli,
            cli_path: "claude".into(),
            cli_model: String::new(),
            base_url: "http://localhost:20128/v1".into(),
            api_key: String::new(),
            model: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    /// What DBZ answers to a tool call.
    ToolResult,
}

/// An image attached to a user message.
#[derive(Clone, Debug, PartialEq)]
pub struct AiImage {
    /// As the chat shows it, e.g. "pasted-1.png".
    pub name: String,
    /// `image/png`, `image/jpeg`, `image/gif` or `image/webp`.
    pub media_type: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatMsg {
    pub role: Role,
    pub text: String,
    /// Only on user messages.
    pub images: Vec<AiImage>,
}

impl ChatMsg {
    /// A message without images.
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        ChatMsg { role, text: text.into(), images: vec![] }
    }
}

/// Tools that read the database on the AI's own connection.
#[derive(Clone, Debug, PartialEq)]
pub enum DbRead {
    ListObjects,
    DescribeTable { table: String },
    GetDdl { name: String, is_view: bool },
    Select { sql: String },
    Explain { sql: String },
}

impl DbRead {
    /// What the loading indicator says while it runs.
    pub fn label(&self) -> String {
        match self {
            DbRead::ListObjects => "Listing objects…".into(),
            DbRead::DescribeTable { table } => format!("Reading columns of `{}`…", table),
            DbRead::GetDdl { name, .. } => format!("Reading DDL of `{}`…", name),
            DbRead::Select { .. } => "Running SELECT…".into(),
            DbRead::Explain { .. } => "Running EXPLAIN…".into(),
        }
    }
}

/// Everything the AI can ask DBZ to do.
#[derive(Clone, Debug, PartialEq)]
pub enum AiTool {
    Db(DbRead),
    /// The SQL in the active editor tab on the same connection.
    ActiveEditor,
    /// A change for the user to confirm; it opens in an editor and never runs by itself.
    ProposeChange { title: String, sql: String, view: Option<String> },
}

impl AiTool {
    pub fn label(&self) -> String {
        match self {
            AiTool::Db(read) => read.label(),
            AiTool::ActiveEditor => "Reading the active editor…".into(),
            AiTool::ProposeChange { .. } => "Waiting for your confirmation".into(),
        }
    }
}

/// A piece of an answer as the chat shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Text(String),
    Sql(String),
}
