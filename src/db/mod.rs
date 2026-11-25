//! Database access. Each server kind is a [`Driver`] strategy; [`open_driver`] is the
//! single place that picks one.

mod driver;
mod error;
mod mysql;
mod postgres;
mod sample;

pub use driver::{open_driver, Driver};
pub use error::DbError;
pub use sample::{sample_server, SharedSample};
#[cfg(test)]
pub use sample::SampleServer;
