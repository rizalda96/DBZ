//! One background thread per AI request, so waiting for an answer never blocks the
//! window or a connection's worker.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use eframe::egui;

use crate::ai::{backend_for, AiError};
use crate::domain::{AiConfig, ChatMsg};

pub struct AiAsk {
    /// Which turn of the conversation this answers; stale replies are dropped by it.
    pub turn: u64,
    pub cfg: AiConfig,
    pub system: String,
    pub msgs: Vec<ChatMsg>,
    pub work_dir: PathBuf,
}

pub struct AiReply {
    pub turn: u64,
    pub result: Result<String, AiError>,
}

/// Asks the AI in the background; setting the returned flag stops the request where
/// the backend can stop.
pub fn ask_ai(ask: AiAsk, tx: Sender<AiReply>, ctx: egui::Context) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    std::thread::spawn(move || {
        let backend = backend_for(&ask.cfg, ask.work_dir);
        let result = backend.complete(&ask.system, &ask.msgs, &flag);
        // The app owns the receiver; if it is gone the app is closing and nobody needs the reply.
        let _ = tx.send(AiReply { turn: ask.turn, result });
        ctx.request_repaint();
    });
    cancel
}
