//! Database access. Each server kind is a [`Driver`] strategy; [`open_driver`] is the
//! single place that picks one.

mod driver;
mod entra;
mod entra_browser;
mod error;
mod mssql;
mod mssql_auth;
mod mssql_cells;
mod mssql_parts;
mod mysql;
mod mysql_parts;
mod postgres;
mod postgres_parts;

pub use driver::{open_driver, Driver};
pub use error::DbError;
