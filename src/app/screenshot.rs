//! `--screenshot out.png`: renders a few frames, optionally opens a screen
//! (DBZ_SHOT_VIEW=data|query|design), saves a picture of the window and exits.

use eframe::egui;

use super::{Act, Dbz, ObjRef, Tab};
use crate::worker::Req;

const OPEN_VIEW_FRAME: u32 = 20;
const CAPTURE_FRAME: u32 = 120;
const DEMO_SQL: &str = "-- Orders per status\nSELECT id, customer_id, ordered_at, status, total\nFROM orders\nWHERE total > 50\nORDER BY total DESC\nLIMIT 20;";

pub struct Screenshot {
    path: String,
    frames: u32,
    sent: bool,
}

impl Screenshot {
    pub fn new(path: String) -> Self {
        Screenshot { path, frames: 0, sent: false }
    }
}

impl Dbz {
    pub(super) fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        shot.frames += 1;
        ctx.request_repaint();
        let frames = shot.frames;
        if frames >= CAPTURE_FRAME && !shot.sent {
            shot.sent = true;
            capture(ctx, shot.path.clone());
        }
        if frames == OPEN_VIEW_FRAME {
            self.open_shot_view();
        }
    }

    fn open_shot_view(&mut self) {
        let cid = self.ws.conns.first().map(|c| c.cfg.id).unwrap_or(1);
        match std::env::var("DBZ_SHOT_VIEW").unwrap_or_default().as_str() {
            "data" => self.do_act(Act::OpenTable(ObjRef::new(cid, "shop", "customers"), false)),
            "design" => self.do_act(Act::DesignTable(cid, "shop".into(), Some("orders".into()))),
            "query" => {
                self.do_act(Act::NewQuery(cid, "shop".into(), DEMO_SQL.into()));
                let Some(Tab::Query(q)) = self.ws.tabs.last_mut() else { return };
                q.running = true;
                let id = q.id;
                self.do_act(Act::Send(cid, Req::Query { tab: id, db: "shop".into(), sql: DEMO_SQL.into() }));
            }
            _ => {}
        }
    }
}

/// Saves the next frame to `path` as PNG and exits (code 1 if saving failed).
fn capture(ctx: &egui::Context, path: String) {
    ctx.request_screenshot(move |img| {
        let [w, h] = img.size;
        let buf: Vec<u8> = img.pixels.iter().flat_map(|p| [p.r(), p.g(), p.b(), 255]).collect();
        if let Some(parent) = std::path::Path::new(&path).parent().filter(|p| !p.as_os_str().is_empty()) {
            // A failure here shows up as the save error below.
            let _ = std::fs::create_dir_all(parent);
        }
        let code = match image::save_buffer(&path, &buf, w as u32, h as u32, image::ColorType::Rgba8) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("could not save screenshot: {}", e);
                1
            }
        };
        std::process::exit(code);
    });
}
