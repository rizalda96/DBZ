//! Background workers. Each open connection gets its own thread that owns the driver
//! and answers requests one at a time, so the window never freezes.

mod handlers;
mod protocol;

pub use protocol::{PageQuery, Purpose, Reply, Req, Resp, TablePage};

use std::sync::mpsc::{Receiver, Sender};

use eframe::egui;

use crate::db::{open_driver, SharedSample};
use crate::domain::ConnConfig;

/// Where a worker sends its replies, and how it wakes the UI.
#[derive(Clone)]
pub struct ReplyTo {
    pub tx: Sender<Reply>,
    pub ctx: egui::Context,
}

impl ReplyTo {
    fn send(&self, id: u64, resp: Resp) {
        // The UI owns the receiver; if it is gone the app is closing and nobody needs the reply.
        let _ = self.tx.send((id, resp));
        self.ctx.request_repaint();
    }
}

/// Starts a worker for `cfg` and returns the channel to send it requests.
pub fn spawn(cfg: ConnConfig, sample: SharedSample, reply: ReplyTo) -> Sender<Req> {
    let (rtx, rrx) = std::sync::mpsc::channel::<Req>();
    std::thread::spawn(move || run(cfg, sample, rrx, reply));
    rtx
}

/// Connects on the first request, then serves requests until the sender is dropped.
fn run(cfg: ConnConfig, sample: SharedSample, rx: Receiver<Req>, reply: ReplyTo) {
    let Ok(first) = rx.recv() else { return };
    let mut driver = match open_driver(&cfg, &sample) {
        Ok(d) => d,
        Err(e) => return reply.send(cfg.id, Resp::ConnectFailed(e)),
    };
    reply.send(cfg.id, handlers::handle(driver.as_mut(), &cfg, first));
    while let Ok(req) = rx.recv() {
        reply.send(cfg.id, handlers::handle(driver.as_mut(), &cfg, req));
    }
}
