//! The AI chat panel: the conversation, what the AI is doing right now, and the
//! message box. Sending, stopping and opening SQL go out as `Act`s.

use eframe::egui::{self, FontId, RichText, Stroke, Ui};

use crate::app::{Act, AiBusy, AiChat, ChatItem};
use crate::domain::AiImage;
use crate::shared::fmt_bytes;
use crate::domain::Segment;
use crate::ui::theme::pal;
use crate::ui::widgets::sql_layouter;

const INPUT_ID: &str = "dbz_ai_input";

pub fn ai_chat(ui: &mut Ui, chat: &mut AiChat, can_start: bool, acts: &mut Vec<Act>) {
    dropped_images(ui, can_start, acts);
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        input(ui, chat, can_start, acts);
        if !chat.attachments.is_empty() {
            attachments(ui, &chat.attachments, acts);
        }
        if let Some(busy) = &chat.busy {
            busy_row(ui, busy, acts);
        }
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            header(ui, chat, can_start, acts);
            ui.separator();
            messages(ui, &chat.items, acts);
        });
    });
}

fn header(ui: &mut Ui, chat: &AiChat, can_start: bool, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("AI Assistant").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("×").on_hover_text("Hide (Ctrl+Shift+A)").clicked() {
                acts.push(Act::AiToggle);
            }
            if ui.small_button("Settings").clicked() {
                acts.push(Act::ShowAiSettings);
            }
            if ui.add_enabled(!chat.items.is_empty() || chat.target.is_some(), egui::Button::new("New").small()).on_hover_text("Start a new conversation").clicked() {
                acts.push(Act::AiNew);
            }
        });
    });
    let target = match (&chat.target, can_start) {
        (Some(t), _) => t.label.clone(),
        (None, true) => "Works on the selected database".into(),
        (None, false) => "Select a database in the tree to start".into(),
    };
    ui.add(egui::Label::new(RichText::new(target).size(11.5).color(pal().muted)).truncate());
}

fn messages(ui: &mut Ui, items: &[ChatItem], acts: &mut Vec<Act>) {
    egui::ScrollArea::vertical().auto_shrink(false).stick_to_bottom(true).show(ui, |ui| {
        if items.is_empty() {
            ui.label(RichText::new("Ask about this database: write or fix SQL, explain a slow query, design a view or function…").color(pal().muted));
        }
        for (i, item) in items.iter().enumerate() {
            chat_item(ui, i, item, acts);
            ui.add_space(6.0);
        }
    });
}

fn chat_item(ui: &mut Ui, i: usize, item: &ChatItem, acts: &mut Vec<Act>) {
    match item {
        ChatItem::User { text, images } => user_message(ui, text, images),
        ChatItem::Assistant(parts) => {
            for (j, part) in parts.iter().enumerate() {
                match part {
                    Segment::Text(t) => ui.add(egui::Label::new(t).wrap().selectable(true)),
                    Segment::Sql(sql) => sql_block(ui, (i, j), sql, acts),
                };
            }
        }
        ChatItem::Tool { label, detail, ok } => tool_item(ui, i, (label, detail), *ok),
        ChatItem::Error(msg) => {
            ui.add(egui::Label::new(RichText::new(msg).color(pal().err)).wrap());
        }
    }
}

fn user_message(ui: &mut Ui, text: &str, images: &[String]) {
    egui::Frame::new().fill(pal().sel_bg).corner_radius(4.0).inner_margin(6.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if !text.is_empty() {
            ui.add(egui::Label::new(text).wrap().selectable(true));
        }
        if !images.is_empty() {
            ui.label(RichText::new(format!("Images: {}", images.join(", "))).size(11.5).color(pal().text_dim));
        }
    });
}

/// A SQL block of an answer, with Open in editor and Copy.
fn sql_block(ui: &mut Ui, key: (usize, usize), sql: &str, acts: &mut Vec<Act>) -> egui::Response {
    egui::Frame::new()
        .fill(pal().faint_bg)
        .stroke(Stroke::new(1.0, pal().border))
        .corner_radius(3.0)
        .inner_margin(6.0)
        .show(ui, |ui| {
            let mut text = sql;
            let mut layouter = sql_layouter;
            let view = egui::TextEdit::multiline(&mut text).id_salt(("ai_sql", key)).font(FontId::monospace(12.5)).desired_width(f32::INFINITY).desired_rows(1).frame(egui::Frame::NONE);
            ui.add(view.layouter(&mut layouter));
            ui.horizontal(|ui| {
                if ui.small_button("Open in editor").clicked() {
                    acts.push(Act::AiOpenSql(sql.to_string()));
                }
                if ui.small_button("Copy").clicked() {
                    ui.ctx().copy_text(sql.to_string());
                }
            });
        })
        .response
}

/// A tool the AI used, folded; open it to see what came back.
fn tool_item(ui: &mut Ui, i: usize, (label, detail): (&str, &str), ok: bool) {
    let title = if ok { RichText::new(label).color(pal().text_dim) } else { RichText::new(format!("{} — failed", label)).color(pal().err) };
    egui::CollapsingHeader::new(title.size(12.0)).id_salt(("ai_tool", i)).show(ui, |ui| {
        egui::ScrollArea::vertical().max_height(200.0).id_salt(("ai_tool_scroll", i)).show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(detail).font(FontId::monospace(11.5))).wrap().selectable(true));
        });
    });
}

/// The loading indicator: what the AI is doing, for how long, and Stop.
fn busy_row(ui: &mut Ui, busy: &AiBusy, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(format!("{} ({}s)", busy.label, busy.started.elapsed().as_secs()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Stop").clicked() {
                acts.push(Act::AiStop);
            }
        });
    });
}

/// Images waiting to go with the next message, each with a remove button.
fn attachments(ui: &mut Ui, images: &[AiImage], acts: &mut Vec<Act>) {
    ui.horizontal_wrapped(|ui| {
        for (i, img) in images.iter().enumerate() {
            ui.label(RichText::new(format!("{} ({})", img.name, fmt_bytes(img.data.len() as u64))).size(11.5).color(pal().text_dim));
            if ui.small_button("×").on_hover_text("Remove").clicked() {
                acts.push(Act::AiRemoveAttachment(i));
            }
        }
    });
}

/// Image files dropped on the window while the chat is open are attached.
fn dropped_images(ui: &Ui, can_start: bool, acts: &mut Vec<Act>) {
    let dropped: Vec<std::path::PathBuf> = ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
    if can_start {
        acts.extend(dropped.into_iter().map(Act::AiAttachFile));
    }
}

/// The message box: Enter sends, Shift+Enter starts a new line, Ctrl+V pastes a picture.
fn input(ui: &mut Ui, chat: &mut AiChat, can_start: bool, acts: &mut Vec<Act>) {
    let enabled = can_start && chat.busy.is_none();
    let id = egui::Id::unique(INPUT_ID);
    let focused = ui.memory(|m| m.has_focus(id));
    // Taken before the text box sees it, so Enter sends instead of adding a line.
    let enter = enabled && focused && ui.input_mut(|i| !i.modifiers.shift && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    if enabled && focused {
        let pasted = ui.input(|i| i.events.iter().find_map(|e| if let egui::Event::PasteImage(img) = e { Some(img.clone()) } else { None }));
        acts.extend(pasted.map(Act::AiAttachPixels));
    }
    let has_content = !chat.input.trim().is_empty() || !chat.attachments.is_empty();
    // Right to left: Send sits at the end and the text box takes the rest of the row.
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
        let send = ui.add_enabled(enabled && has_content, egui::Button::new("Send"));
        if ui.add_enabled(enabled, egui::Button::new("Image…")).on_hover_text("Attach images (or paste one with Ctrl+V, or drop files here)").clicked() {
            acts.push(Act::AiPickImages);
        }
        let hint = if enabled { "Ask about this database…" } else { "" };
        let edit = egui::TextEdit::multiline(&mut chat.input).id(id).hint_text(hint).desired_rows(2).desired_width(f32::INFINITY);
        ui.add_enabled(enabled, edit);
        if (send.clicked() || enter) && has_content {
            acts.push(Act::AiSend);
        }
    });
}
