//! egui spacing, fonts and widget colours for both themes.

use eframe::egui::{self, vec2, FontId, Stroke};

use super::theme::{self, Palette, ThemePref};

/// Installs DBZ's style for light and dark, then applies the user's theme choice.
pub fn setup_style(ctx: &egui::Context, pref: ThemePref) {
    for (t, p) in [(egui::Theme::Light, &theme::LIGHT), (egui::Theme::Dark, &theme::DARK)] {
        ctx.style_mut_of(t, |s| apply(s, p));
    }
    pref.apply(ctx);
    theme::sync(ctx);
}

fn apply(s: &mut egui::Style, p: &Palette) {
    s.spacing.item_spacing = vec2(6.0, 4.0);
    s.spacing.button_padding = vec2(8.0, 3.0);
    s.spacing.interact_size.y = 22.0;
    let v = &mut s.visuals;
    v.panel_fill = p.panel_fill;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.surface;
    v.faint_bg_color = p.faint_bg;
    v.selection.bg_fill = p.sel_bg;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.accent;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.weak_bg_fill = p.button_bg;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.button_border);
    v.widgets.hovered.weak_bg_fill = p.hover_bg;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.focus_border);
    v.widgets.active.weak_bg_fill = p.sel_bg;
    v.striped = true;
    s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(13.5));
    s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(13.5));
    s.text_styles.insert(egui::TextStyle::Monospace, FontId::monospace(13.0));
}
