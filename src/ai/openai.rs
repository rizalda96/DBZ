//! Backend for OpenAI-style `/chat/completions` URLs, such as 9router.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde_json::{json, Value};

use super::backend::{snippet, AiBackend, AiError, AI_TIMEOUT};
use crate::domain::{ChatMsg, Role};

pub struct OpenAiCompat {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl AiBackend for OpenAiCompat {
    /// A blocking request; `cancel` is not checked, the caller drops a stopped reply.
    fn complete(&self, system: &str, msgs: &[ChatMsg], _cancel: &AtomicBool) -> Result<String, AiError> {
        let base = self.base_url.trim().trim_end_matches('/');
        if base.is_empty() || self.model.trim().is_empty() {
            return Err(AiError::NotConfigured("set the URL and the model in AI Settings".into()));
        }
        let tls = native_tls::TlsConnector::new().map_err(|e| AiError::Http(e.to_string()))?;
        let agent = ureq::AgentBuilder::new().tls_connector(Arc::new(tls)).timeout(AI_TIMEOUT).build();
        // ureq is built without its `json` feature, so the body goes as a string.
        let mut req = agent.post(&format!("{}/chat/completions", base)).set("Content-Type", "application/json");
        if !self.api_key.trim().is_empty() {
            req = req.set("Authorization", &format!("Bearer {}", self.api_key.trim()));
        }
        let body = match req.send_string(&request_body(system, msgs, self.model.trim()).to_string()) {
            Ok(r) => r.into_string().map_err(|e| AiError::Http(e.to_string()))?,
            Err(ureq::Error::Status(code, r)) => return Err(status_error(code, r)),
            // The transport error names the URL and the cause, never the headers.
            Err(e) => return Err(AiError::Http(e.to_string())),
        };
        parse_response(&body)
    }
}

fn status_error(code: u16, r: ureq::Response) -> AiError {
    let body = r.into_string().unwrap_or_default();
    let msg = serde_json::from_str::<Value>(&body).ok().and_then(|v| v["error"]["message"].as_str().map(str::to_string)).unwrap_or(body);
    AiError::Http(format!("HTTP {}: {}", code, snippet(&msg, 500)))
}

/// The request: the system prompt first; tool results go as user messages, since
/// this protocol doesn't use the API's own tool calls.
pub(super) fn request_body(system: &str, msgs: &[ChatMsg], model: &str) -> Value {
    let mut messages = vec![json!({"role": "system", "content": system})];
    messages.extend(msgs.iter().map(|m| match m.role {
        Role::User => json!({"role": "user", "content": m.text}),
        Role::Assistant => json!({"role": "assistant", "content": m.text}),
        Role::ToolResult => json!({"role": "user", "content": format!("[tool result]\n{}", m.text)}),
    }));
    json!({"model": model, "messages": messages})
}

/// The first choice's text, or the error the server sent.
pub(super) fn parse_response(body: &str) -> Result<String, AiError> {
    let v: Value = serde_json::from_str(body).map_err(|_| AiError::BadResponse(snippet(body, 300)))?;
    if let Some(msg) = v["error"]["message"].as_str() {
        return Err(AiError::BadResponse(msg.to_string()));
    }
    v["choices"][0]["message"]["content"].as_str().map(str::to_string).ok_or_else(|| AiError::BadResponse(snippet(body, 300)))
}
