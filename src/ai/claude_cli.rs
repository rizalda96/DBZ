//! Backend that runs the Claude command line once per reply. The CLI gets no tools,
//! no MCP servers and an empty working folder, so it can only answer in text.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::backend::{snippet, AiBackend, AiError, AI_TIMEOUT};
use crate::domain::{ChatMsg, Role};

const POLL: Duration = Duration::from_millis(100);
/// Numbers each run's system prompt file, so a Test and a chat reply running at the
/// same time don't overwrite each other's prompt.
static RUNS: AtomicU64 = AtomicU64::new(0);
const LAST_LINE: &str = "Reply as the assistant to the last message.";

pub struct ClaudeCli {
    pub path: String,
    pub model: String,
    /// An empty folder of DBZ's own, so no project files or CLAUDE.md are picked up.
    pub work_dir: PathBuf,
}

impl AiBackend for ClaudeCli {
    fn complete(&self, system: &str, msgs: &[ChatMsg], cancel: &AtomicBool) -> Result<String, AiError> {
        let io = |e: std::io::Error| AiError::Spawn(e.to_string());
        std::fs::create_dir_all(&self.work_dir).map_err(io)?;
        let system_file = self.work_dir.join(format!("system-prompt-{}.txt", RUNS.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(&system_file, system).map_err(io)?;
        let result = self.run(&system_file, msgs, cancel);
        // Only a leftover temp file if this fails; the next run uses its own name.
        let _ = std::fs::remove_file(&system_file);
        result
    }
}

impl ClaudeCli {
    fn run(&self, system_file: &Path, msgs: &[ChatMsg], cancel: &AtomicBool) -> Result<String, AiError> {
        let io = |e: std::io::Error| AiError::Spawn(e.to_string());
        let mut child = command(&self.path)
            .args(args(&self.model, system_file))
            .current_dir(&self.work_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AiError::Spawn(format!("could not start \"{}\": {}", self.path, e)))?;
        // Both pipes are drained on their own threads so a long reply can't fill one and stall the CLI.
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        if let Err(e) = send_input(&mut child, &transcript(msgs)) {
            stop(&mut child);
            return Err(io(e));
        }
        let status = wait(&mut child, cancel)?;
        let (out, err) = (joined(stdout), joined(stderr));
        if !status.success() {
            let why = if err.trim().is_empty() { &out } else { &err };
            return Err(AiError::Spawn(snippet(why, 500)));
        }
        parse_output(&out)
    }
}

/// Print mode, JSON output, every built-in tool off, no MCP servers, nothing saved.
/// A CLI too old for one of these flags exits with an error instead of running with tools.
pub(super) fn args(model: &str, system_file: &Path) -> Vec<String> {
    let mut a: Vec<String> = ["-p", "--output-format", "json", "--tools", "", "--strict-mcp-config", "--no-session-persistence", "--system-prompt-file"].iter().map(|s| s.to_string()).collect();
    a.push(system_file.to_string_lossy().to_string());
    if !model.trim().is_empty() {
        a.push("--model".into());
        a.push(model.trim().to_string());
    }
    a
}

/// The whole conversation as one prompt; the CLI keeps no state between replies.
pub(super) fn transcript(msgs: &[ChatMsg]) -> String {
    let mut out = String::from("Conversation so far:\n\n");
    for m in msgs {
        let who = match m.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::ToolResult => "Tool result",
        };
        out.push_str(&format!("### {}\n{}\n\n", who, m.text));
    }
    out.push_str(LAST_LINE);
    out.push('\n');
    out
}

/// The `result` of the CLI's JSON output.
pub(super) fn parse_output(stdout: &str) -> Result<String, AiError> {
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).map_err(|_| AiError::BadResponse(snippet(stdout, 300)))?;
    let result = v["result"].as_str().unwrap_or_default().to_string();
    if v["is_error"].as_bool().unwrap_or(false) {
        return Err(AiError::BadResponse(if result.is_empty() { snippet(stdout, 300) } else { result }));
    }
    if result.is_empty() {
        return Err(AiError::BadResponse(snippet(stdout, 300)));
    }
    Ok(result)
}

fn command(path: &str) -> Command {
    no_console(Command::new(program(path)))
}

/// Keeps a console window from flashing up for the CLI.
#[cfg(windows)]
fn no_console(mut cmd: Command) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
fn no_console(cmd: Command) -> Command {
    cmd
}

/// The executable `path` names: a folder means the `claude` inside it, a bare name is
/// looked up on PATH. On Windows the file may be `claude.exe` (native install) or
/// `claude.cmd` (npm), and `Command` only tries `.exe` by itself.
pub(super) fn program(path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_dir() {
        let inside = p.join("claude");
        return executable(&inside).unwrap_or(inside);
    }
    if p.components().count() > 1 {
        return executable(p).unwrap_or_else(|| p.to_path_buf());
    }
    let dirs = std::env::var_os("PATH").map(|v| std::env::split_paths(&v).collect::<Vec<_>>()).unwrap_or_default();
    dirs.iter().find_map(|d| executable(&d.join(path))).unwrap_or_else(|| p.to_path_buf())
}

/// `base` itself, or on Windows `base.exe` / `base.cmd`, whichever is a file.
fn executable(base: &Path) -> Option<PathBuf> {
    let mut candidates = vec![base.to_path_buf()];
    if cfg!(windows) && base.extension().is_none() {
        candidates.extend(["exe", "cmd"].map(|ext| base.with_extension(ext)));
    }
    candidates.into_iter().find(|f| f.is_file())
}

fn send_input(child: &mut Child, text: &str) -> std::io::Result<()> {
    let Some(mut stdin) = child.stdin.take() else { return Ok(()) };
    stdin.write_all(text.as_bytes())
    // `stdin` drops here, closing the pipe so the CLI starts working.
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> Option<JoinHandle<String>> {
    pipe.map(|mut p| {
        std::thread::spawn(move || {
            let mut s = String::new();
            // A broken pipe only means the CLI was killed; whatever was read is kept.
            let _ = p.read_to_string(&mut s);
            s
        })
    })
}

fn joined(h: Option<JoinHandle<String>>) -> String {
    h.and_then(|h| h.join().ok()).unwrap_or_default()
}

fn wait(child: &mut Child, cancel: &AtomicBool) -> Result<ExitStatus, AiError> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| AiError::Spawn(e.to_string()))? {
            return Ok(status);
        }
        if cancel.load(Ordering::Relaxed) {
            stop(child);
            return Err(AiError::Cancelled);
        }
        if start.elapsed() > AI_TIMEOUT {
            stop(child);
            return Err(AiError::Timeout);
        }
        std::thread::sleep(POLL);
    }
}

fn stop(child: &mut Child) {
    kill_tree(child);
    // Fails only when the process already ended, which is what we want.
    let _ = child.wait();
}

/// An npm install runs `claude.cmd`, whose node child outlives a plain kill of cmd.exe
/// and would keep answering (and spending tokens).
#[cfg(windows)]
fn kill_tree(child: &mut Child) {
    let pid = child.id().to_string();
    // Best effort; the plain kill below still ends the direct child.
    let _ = no_console(Command::new("taskkill")).args(["/T", "/F", "/PID", &pid]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    let _ = child.kill();
}

#[cfg(not(windows))]
fn kill_tree(child: &mut Child) {
    // Fails only when the process already ended.
    let _ = child.kill();
}
