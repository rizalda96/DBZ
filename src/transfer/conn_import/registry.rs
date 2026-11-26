//! A branch of the Windows registry, read through `reg query`, for tools that keep their
//! connections there (Navicat, HeidiSQL). Also the shape their settings files map to.

use crate::db::DbError;

/// A key under the branch that was read: its path below the branch root
/// (`Folder\Session`) and its values as text (DWORDs as `0x…`).
#[derive(Debug, Default)]
pub(super) struct RegKey {
    pub path: String,
    pub values: Vec<(String, String)>,
}

impl RegKey {
    /// The last part of the path, i.e. the key's own name.
    pub fn name(&self) -> &str {
        self.path.rsplit('\\').next().unwrap_or(&self.path)
    }
}

/// Every key below `root` (a full `HKEY_CURRENT_USER\…` path). A missing branch is empty.
#[cfg(windows)]
pub(super) fn read(root: &str) -> Result<Vec<RegKey>, DbError> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("reg")
        .args(["query", root, "/s"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| DbError::Io(format!("Could not read the registry: {}", e)))?;
    // reg exits with 1 when the key does not exist, i.e. the tool has nothing saved.
    if !out.status.success() {
        return Ok(vec![]);
    }
    Ok(parse(root, &String::from_utf8_lossy(&out.stdout)))
}

#[cfg(not(windows))]
pub(super) fn read(_root: &str) -> Result<Vec<RegKey>, DbError> {
    Err(DbError::Invalid("This tool keeps its connections in the Windows registry. Export them to a file and choose that file.".into()))
}

/// Parses `reg query /s` output: a line per key, then its values indented by four
/// spaces as `name    TYPE    data`.
#[cfg(any(windows, test))]
pub(super) fn parse(root: &str, text: &str) -> Vec<RegKey> {
    let mut keys: Vec<RegKey> = vec![];
    for line in text.lines() {
        if let Some(path) = below(root, line) {
            keys.push(RegKey { path: path.to_string(), values: vec![] });
            continue;
        }
        let (Some(value), Some(key)) = (line.strip_prefix("    "), keys.last_mut()) else { continue };
        let mut parts = value.splitn(3, "    ");
        let (Some(name), Some(_kind)) = (parts.next(), parts.next()) else { continue };
        key.values.push((name.to_string(), parts.next().unwrap_or_default().to_string()));
    }
    keys.retain(|k| !k.path.is_empty());
    keys
}

/// The path of a key line below `root` (empty for the root itself). Registry paths
/// ignore case, and the output spells keys as they were created.
#[cfg(any(windows, test))]
fn below<'a>(root: &str, line: &'a str) -> Option<&'a str> {
    let head = line.get(..root.len())?;
    if !head.eq_ignore_ascii_case(root) {
        return None;
    }
    let rest = &line[root.len()..];
    if rest.is_empty() {
        return Some(rest);
    }
    rest.strip_prefix('\\')
}
