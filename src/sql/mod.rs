//! SQL text generation. Everything that differs between servers goes through the
//! [`Dialect`] strategy; the rest (splitting, formatting) is shared.

mod beautify;
mod dialect;
mod diff;
mod dml;
mod keywords;
mod mysql;
mod postgres;
mod split;

pub use beautify::beautify;
pub use dialect::{dialect_for, Dialect, TableChange};
pub use dml::{eq_condition, insert_row};
pub use keywords::is_keyword;
pub use mysql::MySqlDialect;
pub use postgres::PgDialect;
pub use split::split_statements;
