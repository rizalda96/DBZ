//! The unique fields editor's rows (no egui): every field of the table with a tick,
//! the ticked ones first and in key order. Moving a row changes that order; OK writes
//! the ticked ones back as "a, b".

use super::state::{move_selected, move_target};

/// "a, b" → ["a", "b"].
pub(super) fn list_items(value: &str) -> Vec<String> {
    value.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

pub struct TickRow {
    pub name: String,
    pub on: bool,
}

pub struct TickEditor {
    /// Row of the unique constraint being edited.
    pub target: usize,
    pub rows: Vec<TickRow>,
    pub sel: Option<usize>,
    /// The ticked fields as opened, to tell whether anything changed.
    orig: Vec<String>,
}

impl TickEditor {
    /// `columns` ticked (in their order, even names the table no longer has), then the
    /// rest of `fields` unticked.
    pub fn open(target: usize, columns: &str, fields: &[String]) -> Self {
        let orig = list_items(columns);
        let ticked = orig.iter().map(|n| TickRow { name: n.clone(), on: true });
        let rest = fields.iter().filter(|f| !orig.contains(f)).map(|f| TickRow { name: f.clone(), on: false });
        let rows: Vec<TickRow> = ticked.chain(rest).collect();
        let sel = (!rows.is_empty()).then_some(0);
        TickEditor { target, rows, sel, orig }
    }

    pub fn can_move(&self, delta: isize) -> bool {
        move_target(self.rows.len(), self.sel, delta).is_some()
    }

    pub fn move_sel(&mut self, delta: isize) {
        self.sel = move_selected(&mut self.rows, self.sel, delta);
    }

    /// The ticked fields as "a, b"; None when they are as opened.
    pub fn result(&self) -> Option<String> {
        let ticked: Vec<String> = self.rows.iter().filter(|r| r.on).map(|r| r.name.clone()).collect();
        (ticked != self.orig).then(|| ticked.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::TickEditor;

    fn fields() -> Vec<String> {
        ["id", "id_transporter", "id_site", "is_deleted"].map(String::from).to_vec()
    }

    #[test]
    fn ticked_fields_come_first_in_key_order() {
        let e = TickEditor::open(0, "id_transporter, id_site", &fields());
        let rows: Vec<(&str, bool)> = e.rows.iter().map(|r| (r.name.as_str(), r.on)).collect();
        assert_eq!(rows, [("id_transporter", true), ("id_site", true), ("id", false), ("is_deleted", false)]);
        assert_eq!(e.result(), None);
    }

    #[test]
    fn ticking_and_moving_change_the_list() {
        let mut e = TickEditor::open(0, "id_transporter, id_site", &fields());
        e.sel = Some(1);
        e.move_sel(-1);
        e.rows[3].on = true;
        assert_eq!(e.result().as_deref(), Some("id_site, id_transporter, is_deleted"));
        assert!(!e.can_move(-1));
    }
}
