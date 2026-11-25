//! `--screenshot out.png`: renders a few frames, saves a picture of the window and exits.

use eframe::egui;

use super::Dbz;

const CAPTURE_FRAME: u32 = 120;

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
        if shot.frames >= CAPTURE_FRAME && !shot.sent {
            shot.sent = true;
            capture(ctx, shot.path.clone());
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
