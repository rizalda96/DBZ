//! SQL text generation. Everything that differs between servers goes through the
//! [`Dialect`] strategy; the rest (splitting, formatting) is shared.

mod beautify;
mod dialect;
mod diff;
mod dml;
mod exclude_rule;
mod index_columns;
mod keywords;
mod mysql;
mod mysql_parts;
mod parts;
mod postgres;
mod postgres_parts;
mod split;

pub use beautify::beautify;
pub use dialect::{dialect_for, Dialect, TableChange};
pub use parts::{parts_sql, DesignerCaps, IndexParamSpec, OptionKind, OptionSpec, PartsChange, FK_ACTIONS};
pub use dml::{eq_condition, insert_row};
pub use exclude_rule::RULE_EVENTS;
pub use index_columns::{index_column_ref, index_columns_text, parse_index_columns, NULLS_ORDERS, SORT_ORDERS};
pub use keywords::is_keyword;
pub use mysql::MySqlDialect;
pub use postgres::PgDialect;
pub use split::split_statements;
