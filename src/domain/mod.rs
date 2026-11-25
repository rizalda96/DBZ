//! Plain data types shared by every layer: connections, schema objects and result sets.
//! Nothing here depends on egui or on a database client.

mod connection;
mod index_choices;
mod result;
mod schema;
mod table_parts;

pub use connection::{ConnConfig, DbKind};
pub use index_choices::{Collation, IndexChoices, OpClass};
pub use result::{first_cell, Cell, ResultSet, Row};
pub use schema::{FieldDef, RoutineInfo, TableInfo};
pub use table_parts::{CheckDef, ExcludeDef, ForeignKeyDef, IndexColumn, IndexDef, Part, RuleDef, TableOptions, TableParts, TriggerDef, UniqueDef};
