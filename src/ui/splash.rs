//! The splash screen: logo, name and a loading bar over the whole window for a moment
//! after start-up, then a short fade into the app. A click skips it.

use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Rect, Sense};

use crate::ui::icons;
use crate::ui::theme::pal;

/// Fully shown this long…
const HOLD: Duration = Duration::from_millis(1400);
/// …then fades out over this long.
const FADE: Duration = Duration::from_millis(400);
const LOGO_SIZE: f32 = 112.0;
const BAR_WIDTH: f32 = 220.0;
const TAGLINE: &str = "Database manager for MySQL, MariaDB, PostgreSQL and SQL Server";

/// One frame of the splash.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplashFrame {
    /// 1 = opaque, 0 = gone.
    pub alpha: f32,
    /// How far the loading bar has filled, 0 to 1.
    pub progress: f32,
}

/// The splash `elapsed` after start-up; None once it has faded out.
pub fn splash_frame(elapsed: Duration) -> Option<SplashFrame> {
    let progress = (elapsed.as_secs_f32() / HOLD.as_secs_f32()).min(1.0);
    let Some(fading) = elapsed.checked_sub(HOLD) else { return Some(SplashFrame { alpha: 1.0, progress }) };
    let faded = fading.as_secs_f32() / FADE.as_secs_f32();
    (faded < 1.0).then_some(SplashFrame { alpha: 1.0 - faded, progress })
}

/// Draws the splash on top of everything; true when the user clicked it away.
pub fn show_splash(ctx: &egui::Context, frame: SplashFrame) -> bool {
    let screen = ctx.content_rect();
    let area = egui::Area::new(egui::Id::unique("dbz_splash")).order(egui::Order::Foreground).fixed_pos(screen.min);
    let shown = area.show(ctx, |ui| {
        // Covers the window, so clicks don't reach the app underneath.
        let resp = ui.allocate_rect(screen, Sense::click());
        paint(ui.painter(), screen, frame);
        resp.clicked()
    });
    ctx.move_to_top(shown.response.layer_id);
    shown.inner
}

fn paint(p: &Painter, screen: Rect, frame: SplashFrame) {
    let fade = |c: Color32| c.gamma_multiply(frame.alpha);
    p.rect_filled(screen, 0.0, fade(pal().surface));
    let c = screen.center();
    let logo = Rect::from_center_size(pos2(c.x, c.y - 70.0), vec2(LOGO_SIZE, LOGO_SIZE));
    let full_uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    // A white tint draws the image in its own colours.
    p.image(icons::logo_texture(p.ctx()).id(), logo, full_uv, fade(Color32::WHITE));
    p.text(pos2(c.x, logo.bottom() + 30.0), Align2::CENTER_CENTER, "DBZ", FontId::proportional(34.0), fade(pal().text));
    p.text(pos2(c.x, logo.bottom() + 62.0), Align2::CENTER_CENTER, TAGLINE, FontId::proportional(15.0), fade(pal().muted));
    let bar = Rect::from_center_size(pos2(c.x, logo.bottom() + 96.0), vec2(BAR_WIDTH, 4.0));
    p.rect_filled(bar, 2.0, fade(pal().border));
    p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * frame.progress, bar.height())), 2.0, fade(pal().accent));
    let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
    p.text(pos2(c.x, screen.bottom() - 28.0), Align2::CENTER_CENTER, version, FontId::proportional(12.0), fade(pal().muted));
}

#[cfg(test)]
mod tests {
    use super::{splash_frame, SplashFrame, FADE, HOLD};
    use std::time::Duration;

    #[test]
    fn holds_then_fades_then_ends() {
        assert_eq!(splash_frame(Duration::ZERO), Some(SplashFrame { alpha: 1.0, progress: 0.0 }));
        assert_eq!(splash_frame(HOLD / 2).map(|f| (f.alpha, f.progress)), Some((1.0, 0.5)));
        let mid_fade = splash_frame(HOLD + FADE / 2).expect("still fading");
        assert!((mid_fade.alpha - 0.5).abs() < 1e-3 && mid_fade.progress == 1.0);
        assert_eq!(splash_frame(HOLD + FADE), None);
    }
}
