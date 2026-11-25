//! The work tabs next to "Objects".

use super::state::ConnState;
use crate::ui::icons::Icon;
use crate::ui::views::{DataTab, DesignTab, QueryTab};

pub enum Tab {
    Query(QueryTab),
    Data(DataTab),
    Design(DesignTab),
}

impl Tab {
    pub fn id(&self) -> u64 {
        match self {
            Tab::Query(t) => t.id,
            Tab::Data(t) => t.id,
            Tab::Design(t) => t.id,
        }
    }

    pub fn cid(&self) -> u64 {
        match self {
            Tab::Query(t) => t.cid,
            Tab::Data(t) => t.cid,
            Tab::Design(t) => t.cid,
        }
    }

    pub fn title(&self, conns: &[ConnState]) -> String {
        let cname = conns.iter().find(|c| c.cfg.id == self.cid()).map(|c| c.cfg.name.as_str()).unwrap_or_default();
        let star = |dirty: bool| if dirty { "* " } else { "" };
        match self {
            Tab::Query(t) => format!("{}{} @{} ({})", star(t.dirty), t.title, t.db, cname),
            Tab::Data(t) => format!("{}{} @{} ({})", star(t.dirty()), t.table, t.db, cname),
            Tab::Design(t) => format!("{} @{} ({})", if t.table.is_some() { t.name.as_str() } else { "Untitled" }, t.db, cname),
        }
    }

    pub fn icon(&self) -> Icon {
        match self {
            Tab::Query(_) => Icon::Query,
            Tab::Data(t) if t.is_view => Icon::View,
            Tab::Data(_) => Icon::Table,
            Tab::Design(_) => Icon::Design,
        }
    }

    /// Whether closing would lose work.
    pub fn has_unsaved(&self) -> bool {
        match self {
            Tab::Data(d) => d.dirty(),
            Tab::Query(q) => q.dirty && !q.sql.trim().is_empty(),
            Tab::Design(d) => !d.statements().is_empty(),
        }
    }
}
