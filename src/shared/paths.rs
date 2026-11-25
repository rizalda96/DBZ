//! Where DBZ keeps its files, and file-name cleanup.

use std::path::PathBuf;

/// `%APPDATA%\DBZ`, or `./DBZ` when APPDATA is not set.
pub fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("DBZ")
}

/// The user's Documents folder, the default place for exports.
pub fn default_dir() -> PathBuf {
    std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Documents")).unwrap_or_else(|| PathBuf::from("."))
}

/// Replaces characters that are unsafe in a file name with `_`.
pub fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') { c } else { '_' }).collect()
}
