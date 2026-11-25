//! The work tabs: SQL query editor, table data viewer/editor and table designer.
//! Each tab is a folder with its state (no egui) and its drawing code.

mod data;
mod design;
mod query;

pub use data::{data_view, DataTab, RowState};
pub use design::{design_view, DesignTab};
pub use query::{query_view, QueryTab};
