//! The AI chat loop: send the conversation to the AI, run the tool it asks for (reads
//! on its own connection, the active editor, or a change the user confirms and that
//! opens in an editor), send the result back, until the AI answers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use eframe::egui::ColorImage;

use super::ai_images::{image_from_file, image_from_pixels};
use super::ai_state::{AiTarget, Step};
use super::{Dbz, Dialog, ObjRef};
use crate::ai::{clip, system_prompt, PromptContext};
use crate::domain::{AiConfig, AiImage, AiTool, ChatMsg, DbRead, Role};
use crate::shared::data_dir;
use crate::worker::{self, ask_ai, AiAsk, AiReply, Reply, ReplyTo, Req, Resp};

/// The turn number of AI Settings › Test; never a chat turn.
const TEST_TURN: u64 = u64::MAX;
const OPENED: &str = "Opened in an editor tab. Nothing was run: the user will review it and run or save it themselves.";
const NO_VIEW: &str = "The view was not found in this database, so the query opened by itself in a new query tab, not as the view. Nothing was run.";
const REJECTED: &str = "The user rejected this change; nothing was opened or run.";

/// The AI's own database connection; a fresh channel each time it opens, so a reply
/// from a stopped one can never reach a later turn.
struct AiDb {
    tx: Sender<Req>,
    rx: Receiver<Reply>,
}

/// A change the AI proposed, waiting for the user's answer.
struct Proposal {
    /// The turn it belongs to; an answer after Stop or New is dropped.
    turn: u64,
    sql: String,
    view: Option<String>,
}

/// Channels and in-flight work of the AI chat.
pub(super) struct AiRuntime {
    ai_tx: Sender<AiReply>,
    ai_rx: Receiver<AiReply>,
    db: Option<AiDb>,
    cancel: Option<Arc<AtomicBool>>,
    /// A read was sent to the AI's connection and its answer hasn't come yet.
    reading: bool,
    proposal: Option<Proposal>,
    /// What the running tool is called in the chat.
    tool_label: String,
    /// A view change waiting for its Design View tab: (connection, database, view, query).
    pending_view: Option<(u64, String, String, String)>,
}

impl AiRuntime {
    pub(super) fn new() -> Self {
        let (ai_tx, ai_rx) = std::sync::mpsc::channel();
        AiRuntime { ai_tx, ai_rx, db: None, cancel: None, reading: false, proposal: None, tool_label: String::new(), pending_view: None }
    }
}

fn ai_dir() -> std::path::PathBuf {
    data_dir().join("ai")
}

impl Dbz {
    pub(super) fn ai_send(&mut self) {
        let text = self.ai.input.trim().to_string();
        if (text.is_empty() && self.ai.attachments.is_empty()) || self.ai.busy.is_some() {
            return;
        }
        if self.ai.target.is_none() {
            let Some(target) = self.ai_target() else {
                self.status = "Select a database for the AI to work on".into();
                return;
            };
            self.ai.target = Some(target);
        }
        if let Some(cid) = self.ai.target.as_ref().map(|t| t.cid) {
            self.ensure_open(cid);
        }
        self.ai.input.clear();
        self.ai.begin_turn(text);
        self.ai_ask();
    }

    /// The selected database, as the target of a new conversation.
    fn ai_target(&self) -> Option<AiTarget> {
        let (cid, db) = self.ws.sel.conn_db()?;
        let c = self.ws.conn(cid)?;
        let schema = self.query_schema(cid, &db);
        let version = if c.version.is_empty() { String::new() } else { format!(" {}", c.version) };
        let label = format!("{} @ {} ({}{})", db, c.cfg.name, c.cfg.kind.label(), version);
        Some(AiTarget { cid, db, schema, kind: c.cfg.kind, label })
    }

    /// Sends the conversation so far to the AI.
    fn ai_ask(&mut self) {
        let Some(t) = &self.ai.target else { return };
        let conn = self.ws.conn(t.cid);
        let version = conn.map(|c| c.version.clone()).unwrap_or_default();
        let tables: Vec<String> = conn.and_then(|c| c.db(&t.db)).map(|d| d.tables.iter().map(|x| x.name.clone()).collect()).unwrap_or_default();
        let system = system_prompt(&PromptContext { kind: t.kind, version: &version, db: &t.db, schema: &t.schema, tables: &tables });
        let ask = AiAsk { turn: self.ai.turn(), cfg: self.settings.ai.clone(), system, msgs: self.ai.history().to_vec(), work_dir: ai_dir() };
        self.ai_rt.cancel = Some(ask_ai(ask, self.ai_rt.ai_tx.clone(), self.ctx().clone()));
    }

    /// Handles AI replies and the AI connection's answers that arrived since the last frame.
    pub(super) fn ai_poll(&mut self) {
        while let Ok(reply) = self.ai_rt.ai_rx.try_recv() {
            if reply.turn == TEST_TURN {
                self.ai_test_done(reply.result.map_err(|e| e.to_string()));
                continue;
            }
            let step = self.ai.accept(reply.turn, reply.result);
            self.ai_step(step);
        }
        let mut answers = vec![];
        if let Some(db) = &self.ai_rt.db {
            answers.extend(db.rx.try_iter().map(|(_, resp)| resp));
        }
        for resp in answers {
            self.ai_db_answer(resp);
        }
    }

    fn ai_step(&mut self, step: Step) {
        match step {
            Step::Done | Step::LimitReached | Step::Stale => {}
            Step::Retry => self.ai_ask(),
            Step::Run(AiTool::Db(read)) => self.ai_read(read),
            Step::Run(AiTool::ActiveEditor) => {
                self.ai_rt.tool_label = "Read the active editor".into();
                let result = self.active_editor_sql();
                self.ai_tool_done(result);
            }
            Step::Run(AiTool::ProposeChange { title, sql, view }) => {
                self.ai_rt.tool_label = format!("Proposed change: {}", title);
                let msg = format!("The AI proposes this change. OK opens it in an editor; nothing runs until you run or save it.\n\n{}", sql);
                self.dialogs.ai_confirm = Some((format!("AI: {}", title), msg));
                self.ai_rt.proposal = Some(Proposal { turn: self.ai.turn(), sql, view });
            }
        }
    }

    /// Sends a read tool to the AI's own connection, opening it first if needed.
    fn ai_read(&mut self, read: DbRead) {
        let Some(t) = self.ai.target.clone() else { return };
        self.ai_rt.tool_label = read.label().trim_end_matches('…').to_string();
        if self.ai_rt.db.is_none() {
            self.ai_rt.db = self.ws.conn(t.cid).map(|c| {
                let (tx, rx) = std::sync::mpsc::channel();
                let reply = ReplyTo { tx, ctx: self.ctx().clone() };
                AiDb { tx: worker::spawn(c.cfg.clone(), reply), rx }
            });
        }
        let req = Req::AiRead { db: t.db, schema: t.schema, read };
        let sent = self.ai_rt.db.as_ref().is_some_and(|db| db.tx.send(req).is_ok());
        self.ai_rt.reading = sent;
        if !sent {
            self.ai_rt.db = None;
            self.ai_tool_done(Err("the AI's database connection is closed; try again".into()));
        }
    }

    fn ai_db_answer(&mut self, resp: Resp) {
        self.ai_rt.reading = false;
        let result = match resp {
            Resp::AiRead(r) => r.map_err(|e| e.to_string()),
            Resp::ConnectFailed(e) => {
                self.ai_rt.db = None;
                Err(format!("could not connect: {}", e))
            }
            // This worker is only ever sent `Req::AiRead`.
            _ => return,
        };
        self.ai_tool_done(result);
    }

    /// Gives a tool's outcome to the AI and asks it again, unless the turn was stopped.
    fn ai_tool_done(&mut self, result: Result<String, String>) {
        if self.ai.busy.is_none() {
            return;
        }
        let label = std::mem::take(&mut self.ai_rt.tool_label);
        self.ai.tool_result(label, result);
        self.ai_ask();
    }

    /// SQL in the active editor tab on the conversation's connection.
    fn active_editor_sql(&mut self) -> Result<String, String> {
        let cid = self.ai.target.as_ref().map(|t| t.cid);
        let active = self.ws.active;
        let tab = active.checked_sub(1).and_then(|i| self.ws.tabs.get_mut(i)).filter(|t| Some(t.cid()) == cid);
        let sql = tab.and_then(|t| t.query_mut()).map(|q| q.sql.clone()).filter(|s| !s.trim().is_empty());
        sql.map(|s| clip(&s)).ok_or_else(|| "no SQL editor with text is open on this connection".into())
    }

    /// The user answered a proposed change. OK opens it in an editor, unrun; either way
    /// the AI hears the outcome. An answer for a stopped or cleared turn is dropped.
    pub(super) fn ai_change_answer(&mut self, accepted: bool) {
        let Some(p) = self.ai_rt.proposal.take().filter(|p| p.turn == self.ai.turn()) else { return };
        if !accepted {
            return self.ai_tool_done(Err(REJECTED.into()));
        }
        let outcome = self.open_proposal(p.sql, p.view);
        self.ai_tool_done(Ok(outcome.into()));
    }

    /// Opens a confirmed change: an existing view's query in its Design View, anything
    /// else in a new query tab. Says what was opened, for the AI.
    fn open_proposal(&mut self, sql: String, view: Option<String>) -> &'static str {
        let Some(t) = self.ai.target.clone() else { return "Nothing was opened: the conversation has no database." };
        let conn_db = self.ws.conn(t.cid).and_then(|c| c.db(&t.db));
        let existing = view.as_ref().filter(|v| conn_db.is_some_and(|d| d.tables.iter().any(|x| x.is_view && x.name == **v))).cloned();
        match existing {
            Some(v) => {
                self.ai_rt.pending_view = Some((t.cid, t.db.clone(), v.clone(), sql));
                self.design_view(ObjRef { cid: t.cid, db: t.db, name: v });
                OPENED
            }
            None => {
                self.new_query(t.cid, t.db, sql);
                if view.is_some() {
                    NO_VIEW
                } else {
                    OPENED
                }
            }
        }
    }

    /// The AI's query for `view`, when a confirmed change is waiting for its tab.
    pub(super) fn take_pending_view(&mut self, cid: u64, db: &str, view: &str) -> Option<String> {
        let matches = self.ai_rt.pending_view.as_ref().is_some_and(|(c, d, v, _)| *c == cid && d == db && v == view);
        if !matches {
            return None;
        }
        self.ai_rt.pending_view.take().map(|(.., sql)| sql)
    }

    pub(super) fn ai_open_sql(&mut self, sql: String) {
        match self.ai.target.clone().map(|t| (t.cid, t.db)).or_else(|| self.ws.sel.conn_db()) {
            Some((cid, db)) => self.new_query(cid, db, sql),
            None => self.status = "Select a connection to open the SQL in".into(),
        }
    }

    /// Adds an image to the next message, or says in the status bar why not.
    pub(super) fn ai_attach(&mut self, image: Result<AiImage, String>) {
        match image.and_then(|img| self.ai.attach(img)) {
            Ok(()) => self.status = format!("{} image(s) will go with your next message", self.ai.attachments.len()),
            Err(e) => self.status = e,
        }
    }

    pub(super) fn ai_pick_images(&mut self) {
        let picked = rfd::FileDialog::new().set_title("Attach Images").add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"]).pick_files();
        for path in picked.unwrap_or_default() {
            self.ai_attach(image_from_file(&path));
        }
    }

    /// A picture pasted into the chat box (Ctrl+V), as PNG.
    pub(super) fn ai_attach_pixels(&mut self, img: &ColorImage) {
        let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
        let size = (img.size[0] as u32, img.size[1] as u32);
        let name = format!("pasted-{}.png", self.ai.attachments.len() + 1);
        let image = image_from_pixels(size, rgba, name);
        self.ai_attach(image);
    }

    pub(super) fn ai_stop(&mut self) {
        self.ai_cancel();
        if self.ai.busy.is_some() {
            self.ai.stop();
        }
    }

    pub(super) fn ai_new(&mut self) {
        self.ai_cancel();
        self.ai.reset();
    }

    /// Stops the running request and drops the AI's connection (its worker ends after
    /// the statement it is on).
    fn ai_cancel(&mut self) {
        if let Some(flag) = self.ai_rt.cancel.take() {
            flag.store(true, Ordering::Relaxed);
        }
        self.ai_rt.db = None;
        self.ai_rt.reading = false;
        self.ai_rt.pending_view = None;
        self.ai_rt.proposal = None;
        self.dialogs.ai_confirm = None;
    }

    /// The user closed the conversation's connection; the AI's own one goes too, and a
    /// read waiting on it fails instead of hanging the turn.
    pub(super) fn ai_conn_closed(&mut self, cid: u64) {
        if !self.ai.target.as_ref().is_some_and(|t| t.cid == cid) {
            return;
        }
        self.ai_rt.db = None;
        if std::mem::take(&mut self.ai_rt.reading) {
            self.ai_tool_done(Err("the connection was closed".into()));
        }
    }

    pub(super) fn show_ai_settings(&mut self) {
        self.dialogs.dialog = Some(Dialog::AiSettings { cfg: self.settings.ai.clone(), test: None, testing: false });
    }

    pub(super) fn save_ai_settings(&mut self, cfg: AiConfig) {
        self.settings.ai = cfg;
        match self.store.save_settings(&self.settings) {
            Ok(()) => self.status = "AI settings saved".into(),
            Err(e) => self.status = e,
        }
    }

    /// AI Settings › Test: a one-word question with the settings in the dialog.
    pub(super) fn test_ai(&mut self, cfg: AiConfig) {
        if let Some(Dialog::AiSettings { test, testing, .. }) = &mut self.dialogs.dialog {
            *test = None;
            *testing = true;
        }
        let msgs = vec![ChatMsg::new(Role::User, "ping")];
        let ask = AiAsk { turn: TEST_TURN, cfg, system: "Reply with the single word OK.".into(), msgs, work_dir: ai_dir() };
        // Nothing stops a test; a reply after the dialog closed is dropped in `ai_test_done`.
        ask_ai(ask, self.ai_rt.ai_tx.clone(), self.ctx().clone());
    }

    fn ai_test_done(&mut self, result: Result<String, String>) {
        let Some(Dialog::AiSettings { test, testing, .. }) = &mut self.dialogs.dialog else { return };
        *testing = false;
        *test = Some(result.map(|r| format!("Works. The AI replied: {}", r.chars().take(80).collect::<String>())));
    }
}
