//! Where DBZ keeps its files, and file-name cleanup.

use std::ffi::OsString;
use std::path::PathBuf;

/// Where DBZ keeps settings, connections, saved queries and backups:
/// Windows `%APPDATA%\DBZ`, macOS `~/Library/Application Support/DBZ`, Linux (and
/// other Unix) `$XDG_CONFIG_HOME/dbz`, which defaults to `~/.config/dbz`.
pub fn data_dir() -> PathBuf {
    data_dir_for(std::env::consts::OS, |k| std::env::var_os(k))
}

/// The user's Documents folder (the home folder when there is none), the default
/// place for exports.
pub fn default_dir() -> PathBuf {
    let docs = documents_for(std::env::consts::OS, |k| std::env::var_os(k));
    match docs.parent() {
        Some(home) if !docs.is_dir() => home.to_path_buf(),
        _ => docs,
    }
}

/// [`data_dir`] for `os` (as `std::env::consts::OS` names it), reading the
/// environment through `var`. Falls back to `./DBZ` when the variables are missing.
fn data_dir_for(os: &str, var: impl Fn(&str) -> Option<OsString>) -> PathBuf {
    let set = |k: &str| var(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let dir = match os {
        "windows" => set("APPDATA").map(|p| p.join("DBZ")),
        "macos" => set("HOME").map(|h| h.join("Library").join("Application Support").join("DBZ")),
        _ => set("XDG_CONFIG_HOME").or_else(|| set("HOME").map(|h| h.join(".config"))).map(|p| p.join("dbz")),
    };
    dir.unwrap_or_else(|| PathBuf::from("DBZ"))
}

fn documents_for(os: &str, var: impl Fn(&str) -> Option<OsString>) -> PathBuf {
    let home = if os == "windows" { "USERPROFILE" } else { "HOME" };
    var(home).filter(|v| !v.is_empty()).map(|h| PathBuf::from(h).join("Documents")).unwrap_or_else(|| PathBuf::from("."))
}

/// Replaces characters that are unsafe in a file name with `_`.
pub fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') { c } else { '_' }).collect()
}

#[cfg(test)]
mod tests {
    use super::{data_dir_for, documents_for};
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        move |k| pairs.iter().find(|(name, _)| *name == k).map(|(_, v)| OsString::from(v))
    }

    #[test]
    fn each_os_keeps_its_files_in_the_usual_place() {
        let windows = env(&[("APPDATA", r"C:\Users\me\AppData\Roaming")]);
        assert_eq!(data_dir_for("windows", windows), PathBuf::from(r"C:\Users\me\AppData\Roaming").join("DBZ"));
        let mac = env(&[("HOME", "/Users/me")]);
        assert_eq!(data_dir_for("macos", mac), PathBuf::from("/Users/me/Library/Application Support/DBZ"));
        let linux = env(&[("HOME", "/home/me")]);
        assert_eq!(data_dir_for("linux", linux), PathBuf::from("/home/me/.config/dbz"));
        let xdg = env(&[("HOME", "/home/me"), ("XDG_CONFIG_HOME", "/home/me/cfg")]);
        assert_eq!(data_dir_for("linux", xdg), PathBuf::from("/home/me/cfg/dbz"));
        assert_eq!(data_dir_for("linux", env(&[])), PathBuf::from("DBZ"));
    }

    #[test]
    fn documents_live_in_the_home_folder() {
        assert_eq!(documents_for("linux", env(&[("HOME", "/home/me")])), PathBuf::from("/home/me/Documents"));
        assert_eq!(documents_for("windows", env(&[("USERPROFILE", r"C:\Users\me")])), PathBuf::from(r"C:\Users\me").join("Documents"));
    }
}
