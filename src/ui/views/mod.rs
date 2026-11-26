//! The work tabs: SQL query editor, table data viewer/editor, table designer,
//! materialized view designer and view editor.
//! Each tab is a folder with its state (no egui) and its drawing code.

mod data;
mod design;
mod matview;
mod query;
mod view_def;

pub use data::{data_view, DataTab, RowState};
pub use design::{design_view, DesignTab};
pub use matview::{matview_view, MatViewTab};
pub use query::{query_view, QueryTab};
pub use view_def::{view_def_view, ViewTab};
