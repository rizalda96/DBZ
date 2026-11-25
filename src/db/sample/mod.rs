//! A small in-memory database used for the built-in "Sample Data" connection, so the
//! app is useful (and testable) without a running server. It understands a practical
//! subset of MySQL-flavoured SQL.

mod coldef;
mod cond;
mod ddl;
mod dml;
mod driver;
mod parser;
mod seed;
mod select;
mod server;
mod tokens;

pub use driver::SampleDriver;
pub use seed::sample_server;
pub use server::SampleServer;

use std::sync::{Arc, Mutex};

/// The sample server shared by every sample connection in the app.
pub type SharedSample = Arc<Mutex<SampleServer>>;
