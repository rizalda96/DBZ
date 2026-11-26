//! Ctrl+F (⌘F on macOS) jumps to a search box: the object list's while it shows,
//! the connection tree's while a work tab is open.

use eframe::egui::{self, Id, Key, KeyboardShortcut, Modifiers};

const SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchBox {
    Tree,
    Objects,
}

impl SearchBox {
    pub(super) fn id(self) -> Id {
        Id::unique(match self {
            SearchBox::Tree => "tree_search",
            SearchBox::Objects => "objects_search",
        })
    }
}

/// On Ctrl/⌘+F, focuses `target` with its `text` selected, so typing replaces the old search.
pub fn search_shortcut(ctx: &egui::Context, target: SearchBox, text: &str) {
    if !ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT)) {
        return;
    }
    let id = target.id();
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
    state.cursor.set_char_range(Some(all));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// "Search (Ctrl+F)", written the platform's way.
pub(super) fn hint(ctx: &egui::Context, label: &str) -> String {
    format!("{} ({})", label, ctx.format_shortcut(&SHORTCUT))
}
