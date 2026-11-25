//! The inside of a searchable drop-down: a search box on top and the entries that
//! match it. Works in any popup (`egui::Popup`, `ComboBox::show_ui`) whose close
//! behaviour is `CloseOnClickOutside`, so clicking the search box keeps it open.

use eframe::egui::{self, RichText, Ui};

use crate::ui::theme::pal;

/// How an empty entry (the one that clears an optional value) is listed.
const NONE_LABEL: &str = "(none)";
const LIST_HEIGHT: f32 = 240.0;

/// Draws the search box and the entries containing its text (any case). Returns the
/// entry picked by a click, or the first match on Enter, and closes the popup then.
/// `id` keys the search text in egui memory.
pub fn search_list<'o>(ui: &mut Ui, id: egui::Id, current: &str, options: impl IntoIterator<Item = &'o str>) -> Option<String> {
    let mut query: String = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let search = ui.add(egui::TextEdit::singleline(&mut query).hint_text("Search…").desired_width(f32::INFINITY));
    // The box is the only thing in the popup that takes keys, so it keeps focus.
    if !search.has_focus() && !search.lost_focus() {
        search.request_focus();
    }
    let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
    let enter = enter && search.lost_focus();
    if escape {
        ui.close();
    }
    let found = matching(options, &query);
    let mut picked = if enter { found.first().map(|o| o.to_string()) } else { None };
    egui::ScrollArea::vertical().max_height(LIST_HEIGHT).show(ui, |ui| {
        for o in &found {
            if ui.selectable_label(*o == current, entry_label(o)).clicked() {
                picked = Some(o.to_string());
            }
        }
    });
    if found.is_empty() {
        ui.label(RichText::new("No match").color(pal().muted));
    }
    if picked.is_some() {
        forget_search(ui, id);
        ui.close();
    } else {
        ui.data_mut(|d| d.insert_temp(id, query));
    }
    picked
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

fn entry_label(o: &str) -> RichText {
    if o.is_empty() {
        RichText::new(NONE_LABEL).color(pal().muted)
    } else {
        RichText::new(o)
    }
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
