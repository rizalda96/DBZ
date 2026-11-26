//! AI Settings: which backend answers the chat, and a Test button.

use eframe::egui::{self, RichText, Ui};

use crate::app::Act;
use crate::domain::{AiBackendKind, AiConfig};
use crate::ui::theme::pal;
use crate::ui::widgets::{buttons, win, Choice};

const FIELD_WIDTH: f32 = 320.0;

pub(super) fn show(ctx: &egui::Context, cfg: &mut AiConfig, test: Option<&Result<String, String>>, testing: bool, acts: &mut Vec<Act>) -> Choice {
    let mut choice = Choice::None;
    win("AI Settings").show(ctx, |ui| {
        ui.set_min_width(460.0);
        ui.horizontal(|ui| {
            for kind in AiBackendKind::ALL {
                ui.radio_value(&mut cfg.backend, kind, kind.label());
            }
        });
        ui.add_space(6.0);
        egui::Grid::new("ai_settings").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| match cfg.backend {
            AiBackendKind::ClaudeCli => cli_fields(ui, cfg),
            AiBackendKind::OpenAiCompat => url_fields(ui, cfg),
        });
        ui.add_space(4.0);
        ui.label(RichText::new(hint(cfg.backend)).size(11.5).color(pal().muted));
        ui.add_space(6.0);
        test_row(ui, cfg, test, testing, acts);
        choice = buttons(ui, "OK", true);
    });
    choice
}

fn cli_fields(ui: &mut Ui, cfg: &mut AiConfig) {
    field(ui, "Claude CLI path:", &mut cfg.cli_path, "claude, claude.exe, or the folder it is in", false);
    field(ui, "Model:", &mut cfg.cli_model, "empty = the CLI's default", false);
}

fn url_fields(ui: &mut Ui, cfg: &mut AiConfig) {
    field(ui, "Base URL:", &mut cfg.base_url, "http://localhost:20128/v1", false);
    field(ui, "API key:", &mut cfg.api_key, "empty when the URL needs none", true);
    field(ui, "Model:", &mut cfg.model, "e.g. a model name your router offers", false);
}

fn field(ui: &mut Ui, label: &str, value: &mut String, hint: &str, secret: bool) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).hint_text(hint).password(secret).desired_width(FIELD_WIDTH));
    ui.end_row();
}

fn hint(kind: AiBackendKind) -> &'static str {
    match kind {
        AiBackendKind::ClaudeCli => "Runs the Claude command line on this computer, with all of its tools turned off.",
        AiBackendKind::OpenAiCompat => "Any OpenAI-style /chat/completions endpoint, such as 9router.",
    }
}

fn test_row(ui: &mut Ui, cfg: &AiConfig, test: Option<&Result<String, String>>, testing: bool, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        if ui.add_enabled(!testing, egui::Button::new("Test")).clicked() {
            acts.push(Act::TestAi(cfg.clone()));
        }
        if testing {
            ui.spinner();
            ui.label("Asking the AI…");
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        } else if let Some(result) = test {
            let (color, msg) = match result {
                Ok(m) => (pal().ok, m),
                Err(m) => (pal().err, m),
            };
            ui.add(egui::Label::new(RichText::new(msg).color(color)).wrap());
        }
    });
}
