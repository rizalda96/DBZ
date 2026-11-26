//! SQL text generation. Everything that differs between servers goes through the
//! [`Dialect`] strategy; the rest (splitting, formatting) is shared.

mod batches;
mod beautify;
mod dialect;
mod diff;
mod dml;
mod exclude_rule;
mod index_columns;
mod matview;
mod keywords;
mod mssql;
mod mssql_alter;
mod mssql_parts;
mod mysql;
mod mysql_parts;
mod parts;
mod postgres;
mod postgres_parts;
mod read_guard;
mod split;
mod sql_tokens;
mod view_source;

pub use beautify::beautify;
pub use dialect::{dialect_for, CapRows, Dialect, Explain, TableChange};
pub use read_guard::{check_read, ReadKind};
pub use parts::{parts_sql, DesignerCaps, IndexParamSpec, OptionKind, OptionSpec, PartsChange, FK_ACTIONS};
pub use dml::{eq_condition, insert_row};
pub use exclude_rule::RULE_EVENTS;
pub use mssql_parts::EXECUTE_AS;
pub use view_source::{split_view, view_statement};
pub use matview::{create_matview, UNTITLED};
pub use index_columns::{index_column_ref, index_columns_text, parse_index_columns, NULLS_ORDERS, SORT_ORDERS};
pub use keywords::is_keyword;
pub use mssql::MssqlDialect;
pub use mysql::MySqlDialect;
pub use postgres::PgDialect;
