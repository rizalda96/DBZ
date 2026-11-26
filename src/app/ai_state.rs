//! The AI chat as the panel shows it, and the bookkeeping of one turn: a user message,
//! then AI replies and tool results until the AI answers. No egui here.

use std::time::Instant;

use crate::ai::{parse_reply, segments, AiError, Parsed};
use crate::domain::{AiTool, ChatMsg, DbKind, Role, Segment};

/// Tool calls the AI may make while answering one message.
pub(super) const MAX_ROUNDS: u32 = 12;
const THINKING: &str = "Thinking…";

/// The connection and database a conversation works on; fixed by its first message.
#[derive(Clone, Debug)]
pub struct AiTarget {
    pub cid: u64,
    pub db: String,
    pub schema: String,
    pub kind: DbKind,
    /// e.g. "shop @ Local PG (PostgreSQL 16.2)".
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChatItem {
    User(String),
    Assistant(Vec<Segment>),
    /// A tool the AI used: what it did, and what came back (cut) or why it failed.
    Tool { label: String, detail: String, ok: bool },
    Error(String),
}

/// What the AI is doing right now, for the loading indicator.
pub struct AiBusy {
    pub label: String,
    pub started: Instant,
}

/// What the app does after an AI reply.
#[derive(Debug, PartialEq)]
pub(super) enum Step {
    Done,
    Run(AiTool),
    /// The tool call was malformed; the error is in the history, ask again.
    Retry,
    LimitReached,
    /// The reply belongs to a turn that was stopped or cleared.
    Stale,
}

#[derive(Default)]
pub struct AiChat {
    /// The panel is shown.
    pub open: bool,
    /// What the user is typing.
    pub input: String,
    pub target: Option<AiTarget>,
    pub items: Vec<ChatItem>,
    pub busy: Option<AiBusy>,
    history: Vec<ChatMsg>,
    turn: u64,
    rounds: u32,
}

impl AiChat {
    /// Adds the user's message and starts a new turn; returns its number.
    pub(super) fn begin_turn(&mut self, text: String) -> u64 {
        self.turn += 1;
        self.rounds = 0;
        self.items.push(ChatItem::User(text.clone()));
        self.history.push(ChatMsg { role: Role::User, text });
        self.set_busy(THINKING);
        self.turn
    }

    /// Takes the AI's reply for `turn` and says what to do next.
    pub(super) fn accept(&mut self, turn: u64, result: Result<String, AiError>) -> Step {
        if turn != self.turn || self.busy.is_none() {
            return Step::Stale;
        }
        let reply = match result {
            Ok(r) => r,
            Err(e) => {
                self.fail(e.to_string());
                return Step::Done;
            }
        };
        self.history.push(ChatMsg { role: Role::Assistant, text: reply.clone() });
        match parse_reply(&reply) {
            Parsed::Answer(text) => {
                self.say(&text);
                self.busy = None;
                Step::Done
            }
            Parsed::Call { text, tool } => self.call(&text, tool),
            Parsed::Invalid { text, error } => self.invalid(&text, error),
        }
    }

    fn call(&mut self, text: &str, tool: AiTool) -> Step {
        self.say(text);
        if !self.count_round() {
            return Step::LimitReached;
        }
        self.set_busy(&tool.label());
        Step::Run(tool)
    }

    fn invalid(&mut self, text: &str, error: String) -> Step {
        self.say(text);
        if !self.count_round() {
            return Step::LimitReached;
        }
        self.history.push(ChatMsg { role: Role::ToolResult, text: format!("invalid tool call: {}", error) });
        self.items.push(ChatItem::Tool { label: "Invalid tool call".into(), detail: error, ok: false });
        self.set_busy(THINKING);
        Step::Retry
    }

    /// One more tool call in this turn; false (and the turn ended) past the limit.
    fn count_round(&mut self) -> bool {
        self.rounds += 1;
        if self.rounds <= MAX_ROUNDS {
            return true;
        }
        self.fail(format!("Stopped after {} tool calls", MAX_ROUNDS));
        false
    }

    /// Records a tool's outcome for the AI and the panel; the AI is asked next.
    pub(super) fn tool_result(&mut self, label: String, result: Result<String, String>) {
        let (ok, text) = match result {
            Ok(t) => (true, t),
            Err(e) => (false, format!("ERROR: {}", e)),
        };
        self.items.push(ChatItem::Tool { label, detail: text.clone(), ok });
        self.history.push(ChatMsg { role: Role::ToolResult, text });
        self.set_busy(THINKING);
    }

    /// Ends the running turn; a reply that still arrives for it is dropped.
    pub(super) fn stop(&mut self) {
        self.turn += 1;
        self.busy = None;
        self.items.push(ChatItem::Error("Stopped".into()));
    }

    /// Starts over: no messages, no target.
    pub(super) fn reset(&mut self) {
        self.turn += 1;
        self.rounds = 0;
        self.busy = None;
        self.target = None;
        self.items.clear();
        self.history.clear();
    }

    pub(super) fn fail(&mut self, msg: String) {
        self.items.push(ChatItem::Error(msg));
        self.busy = None;
    }

    pub(super) fn history(&self) -> &[ChatMsg] {
        &self.history
    }

    pub(super) fn turn(&self) -> u64 {
        self.turn
    }

    fn say(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.items.push(ChatItem::Assistant(segments(text)));
        }
    }

    fn set_busy(&mut self, label: &str) {
        self.busy = Some(AiBusy { label: label.to_string(), started: Instant::now() });
    }
}

#[cfg(test)]
mod tests;
