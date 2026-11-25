//! The inside of a searchable drop-down: a search box on top and the entries that
//! match it. Works in any popup (`egui::Popup`, `ComboBox::show_ui`) whose close
//! behaviour is `CloseOnClickOutside`, so clicking the search box keeps it open.

use eframe::egui::{self, pos2, vec2, Align2, Response, RichText, Sense, Stroke, Ui};

use crate::ui::theme::pal;

/// How an empty entry (the one that clears an optional value) is listed.
const NONE_LABEL: &str = "(none)";
const LIST_HEIGHT: f32 = 240.0;
const ENTRY_HEIGHT: f32 = 24.0;
/// Space around the search box, and before each entry's text.
const PAD: f32 = 6.0;

/// Draws the search box and the entries containing its text (any case). Returns the
/// entry picked by a click, or the first match on Enter, and closes the popup then.
/// `id` keys the search text in egui memory.
pub fn search_list<'o>(ui: &mut Ui, id: egui::Id, current: &str, options: impl IntoIterator<Item = &'o str>) -> Option<String> {
    let (query, enter) = search_box(ui, id);
    let found = matching(options, &query);
    let clicked = entry_list(ui, &found, |o| o == current, false);
    let picked = clicked.or(if enter { found.first().copied() } else { None }).map(str::to_string);
    if picked.is_some() {
        forget_search(ui, id);
        ui.close();
    }
    picked
}

/// Like [`search_list`], but every entry has a tick box and the popup stays open:
/// returns the entry the user ticked or unticked (a click, or the first match on
/// Enter). `chosen` are the entries ticked now.
pub fn multi_search_list<'o>(ui: &mut Ui, id: egui::Id, chosen: &[String], options: impl IntoIterator<Item = &'o str>) -> Option<String> {
    let (query, enter) = search_box(ui, id);
    let found = matching(options, &query);
    let clicked = entry_list(ui, &found, |o| chosen.iter().any(|c| c == o), true);
    clicked.or(if enter { found.first().copied() } else { None }).map(str::to_string)
}

/// The padded search box at the top; its text and whether Enter was pressed in it.
/// Escape closes the popup.
fn search_box(ui: &mut Ui, id: egui::Id) -> (String, bool) {
    let mut query: String = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let boxed = egui::TextEdit::singleline(&mut query).hint_text("Search…").desired_width(f32::INFINITY).margin(vec2(PAD, 4.0));
    let search = egui::Frame::new().inner_margin(PAD).show(ui, |ui| ui.add(boxed)).inner;
    // The box is the only thing in the popup that takes keys, so it keeps focus.
    if !search.has_focus() && !search.lost_focus() {
        search.request_focus();
    }
    let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
    if escape {
        ui.close();
    }
    ui.data_mut(|d| d.insert_temp(id, query.clone()));
    (query, enter && search.lost_focus())
}

/// The scrolling list; returns the entry clicked. `marked` entries are highlighted
/// (one value) or ticked (`ticks`).
fn entry_list<'o>(ui: &mut Ui, found: &[&'o str], marked: impl Fn(&str) -> bool, ticks: bool) -> Option<&'o str> {
    let mut clicked = None;
    egui::ScrollArea::vertical().max_height(LIST_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for o in found {
            if entry(ui, o, marked(o), ticks).clicked() {
                clicked = Some(*o);
            }
        }
    });
    if found.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(PAD);
            ui.label(RichText::new("No match").color(pal().muted));
        });
    }
    clicked
}

/// A `ComboBox` `width` points wide whose list is a [`search_list`] led by an empty
/// entry (to clear the value); true when a value was picked.
pub fn search_combo(ui: &mut Ui, (salt, width): (impl std::hash::Hash + std::fmt::Debug, f32), value: &mut String, options: &[String]) -> bool {
    // Its own search text, next to the combo's id.
    let search = ui.scope_id().with(("combo_search", &salt));
    let combo = egui::ComboBox::from_id_salt(salt).selected_text(value.clone()).width(width);
    let entries = std::iter::once("").chain(options.iter().map(String::as_str));
    let shown = combo.close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show_ui(ui, |ui| search_list(ui, search, value, entries));
    let Some(picked) = shown.inner else {
        forget_search(ui, search);
        return false;
    };
    let Some(p) = picked else { return false };
    *value = p;
    true
}

/// Drops the search text, so the list opens unfiltered next time. Call it while the
/// popup is closed.
pub fn forget_search(ui: &Ui, id: egui::Id) {
    ui.data_mut(|d| d.remove::<String>(id));
}

/// Entries containing `query`, any case; all of them while it is empty.
fn matching<'o>(options: impl IntoIterator<Item = &'o str>, query: &str) -> Vec<&'o str> {
    let needle = query.trim().to_lowercase();
    options.into_iter().filter(|o| needle.is_empty() || o.to_lowercase().contains(&needle)).collect()
}

/// One full-width row of the list: highlighted under the pointer, marked when it is
/// the current value.
fn entry(ui: &mut Ui, o: &str, marked: bool, ticks: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ENTRY_HEIGHT), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 2.0, pal().hover_bg);
    } else if marked && !ticks {
        p.rect_filled(rect, 2.0, pal().sel_bg);
    }
    let mut text_x = rect.left() + PAD + 2.0;
    if ticks {
        tick_box(p, pos2(text_x + 6.0, rect.center().y), marked);
        text_x += 20.0;
    }
    let (text, color) = if o.is_empty() { (NONE_LABEL, pal().muted) } else { (o, pal().text) };
    let font = egui::TextStyle::Body.resolve(ui.style());
    let at = pos2(text_x, rect.center().y);
    p.with_clip_rect(rect).text(at, Align2::LEFT_CENTER, text, font, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A 12-point tick box centred on `c`, filled and ticked when `on`.
fn tick_box(p: &egui::Painter, c: egui::Pos2, on: bool) {
    let r = egui::Rect::from_center_size(c, vec2(12.0, 12.0));
    if !on {
        p.rect_stroke(r, 2.0, Stroke::new(1.0, pal().border), egui::StrokeKind::Inside);
        return;
    }
    p.rect_filled(r, 2.0, pal().accent);
    let tick = Stroke::new(1.6, pal().surface);
    p.line_segment([pos2(r.left() + 2.5, c.y), pos2(c.x - 1.0, r.bottom() - 3.0)], tick);
    p.line_segment([pos2(c.x - 1.0, r.bottom() - 3.0), pos2(r.right() - 2.5, r.top() + 3.0)], tick);
}

#[cfg(test)]
mod tests {
    use super::matching;

    #[test]
    fn filters_by_any_part_in_any_case() {
        let all = ["", "id", "id_site", "Created_At", "site_name"];
        assert_eq!(matching(all, ""), all);
        assert_eq!(matching(all, "SITE"), ["id_site", "site_name"]);
        assert_eq!(matching(all, " at "), ["Created_At"]);
        assert!(matching(all, "zzz").is_empty());
    }
}
