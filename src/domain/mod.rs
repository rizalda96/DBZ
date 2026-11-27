//! Plain data types shared by every layer: connections, schema objects and result sets.
//! Nothing here depends on egui or on a database client.

mod ai;
mod connection;
mod index_choices;
mod matview;
mod result;
mod schema;
mod sql_auth;
mod table_parts;

pub use ai::{AiBackendKind, AiConfig, AiImage, AiTool, ChatMsg, DbRead, Role, Segment};
pub use connection::{ArchiveTools, ConnConfig, DbKind};
pub use sql_auth::{Need, SqlAuth};
pub use matview::MatViewDef;
pub use index_choices::{Collation, IndexChoices, OpClass, SchemaName};
pub use result::{first_cell, sort_rows, Cell, ResultSet, Row};
pub use schema::{FieldDef, RelationKind, RoutineInfo, TableInfo};
pub use table_parts::{CheckDef, ExcludeDef, ForeignKeyDef, IndexColumn, IndexDef, Part, RuleDef, TableOptions, TableParts, TriggerDef, UniqueDef};
