//! Plain data types shared by every layer: connections, schema objects and result sets.
//! Nothing here depends on egui or on a database client.

mod connection;
mod result;
mod schema;

pub use connection::{ConnConfig, DbKind};
pub use result::{first_cell, Cell, ResultSet, Row};
pub use schema::{FieldDef, RoutineInfo, TableInfo};
