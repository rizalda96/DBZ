//! Small pure helpers used across modules (no egui, no database).

mod bytes;
mod paths;
mod time;

pub use bytes::fmt_bytes;
pub use paths::{data_dir, default_dir, sanitize};
pub use time::{fmt_time, now_string};
