//! The work tabs next to "Objects".

use super::state::ConnState;
use crate::ui::icons::Icon;
use crate::ui::views::{DataTab, DesignTab, MatViewTab, QueryTab, ViewTab};

pub enum Tab {
    Query(QueryTab),
    Data(DataTab),
    Design(DesignTab),
    MatView(MatViewTab),
    View(ViewTab),
}

impl Tab {
    pub fn id(&self) -> u64 {
        match self {
            Tab::Query(t) => t.id,
            Tab::Data(t) => t.id,
            Tab::Design(t) => t.id,
            Tab::MatView(t) => t.id,
            Tab::View(t) => t.id,
        }
    }

    pub fn cid(&self) -> u64 {
        match self {
            Tab::Query(t) => t.cid,
            Tab::Data(t) => t.cid,
            Tab::Design(t) => t.cid,
            Tab::MatView(t) => t.cid,
            Tab::View(t) => t.cid,
        }
    }

    /// The SQL editor and its results, in tabs that have one (query results come back
    /// to this by tab id).
    pub fn query_mut(&mut self) -> Option<&mut QueryTab> {
        match self {
            Tab::Query(q) => Some(q),
            Tab::MatView(m) => Some(&mut m.query),
            Tab::View(v) => Some(&mut v.query),
            Tab::Data(_) | Tab::Design(_) => None,
        }
    }

    pub fn title(&self, conns: &[ConnState]) -> String {
        let cname = conns.iter().find(|c| c.cfg.id == self.cid()).map(|c| c.cfg.name.as_str()).unwrap_or_default();
        let star = |dirty: bool| if dirty { "* " } else { "" };
        match self {
            Tab::Query(t) => format!("{}{} @{} ({})", star(t.dirty), t.title, t.db, cname),
            Tab::Data(t) => format!("{}{} @{} ({})", star(t.dirty()), t.table, t.db, cname),
            Tab::Design(t) => format!("{} @{} ({})", if t.table.is_some() { t.name.as_str() } else { "Untitled" }, t.db, cname),
            Tab::MatView(t) => format!("{}{} @{}.{} ({}) - Materialized View", star(t.has_unsaved()), t.shown_name(), t.query.db, t.query.schema, cname),
            Tab::View(t) => format!("{}{} @{} ({}) - View", star(t.has_unsaved()), t.name, t.query.db, cname),
        }
    }

    pub fn icon(&self) -> Icon {
        match self {
            Tab::Query(_) => Icon::Query,
            Tab::Data(t) if t.is_view => Icon::View,
            Tab::Data(_) => Icon::Table,
            Tab::Design(_) => Icon::Design,
            Tab::MatView(_) => Icon::MatView,
            Tab::View(_) => Icon::View,
        }
    }

    /// Whether closing would lose work.
    pub fn has_unsaved(&self) -> bool {
        match self {
            Tab::Data(d) => d.dirty(),
            Tab::Query(q) => q.dirty && !q.sql.trim().is_empty(),
            Tab::Design(d) => !d.statements().is_empty(),
            Tab::MatView(m) => m.has_unsaved(),
            Tab::View(v) => v.has_unsaved(),
        }
    }
}
