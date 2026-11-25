//! Settings and connections saved under `%APPDATA%\DBZ`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::ConnConfig;
use crate::shared::{data_dir, fmt_time, sanitize};
use crate::ui::theme::ThemePref;

use super::state::SavedFile;

const SETTINGS_FILE: &str = "settings.json";
const CONNECTIONS_FILE: &str = "connections.json";
const PREVIEW_CHARS: usize = 1200;

/// Settings saved in `settings.json` next to `connections.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub theme: ThemePref,
}

impl Settings {
    /// Test helper for screenshots: DBZ_THEME=light|dark picks the theme.
    pub fn from_env() -> Self {
        let theme = serde_json::from_value(serde_json::Value::String(std::env::var("DBZ_THEME").unwrap_or_default())).unwrap_or(ThemePref::Light);
        Settings { theme }
    }
}

/// The app's folder. When disabled (screenshot runs) nothing is read or written.
pub struct Store {
    dir: PathBuf,
    enabled: bool,
}

impl Store {
    pub fn new(enabled: bool) -> Self {
        Store { dir: data_dir(), enabled }
    }

    pub fn load_settings(&self) -> Settings {
        self.read_json(SETTINGS_FILE).unwrap_or_default()
    }

    pub fn load_connections(&self) -> Vec<ConnConfig> {
        self.read_json(CONNECTIONS_FILE).unwrap_or_default()
    }

    pub fn save_settings(&self, s: &Settings) -> Result<(), String> {
        self.write_json(SETTINGS_FILE, s)
    }

    /// Saves connections; passwords are dropped unless "Save password" is ticked.
    pub fn save_connections(&self, cfgs: &[ConnConfig]) -> Result<(), String> {
        let stored: Vec<ConnConfig> = cfgs.iter().map(ConnConfig::for_storage).collect();
        self.write_json(CONNECTIONS_FILE, &stored)
    }

    fn read_json<T: serde::de::DeserializeOwned>(&self, name: &str) -> Option<T> {
        if !self.enabled {
            return None;
        }
        let text = std::fs::read_to_string(self.dir.join(name)).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn write_json<T: Serialize>(&self, name: &str, value: &T) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&self.dir).and_then(|_| std::fs::write(self.dir.join(name), text)).map_err(|e| format!("Could not save {}: {}", name, e))
    }

    /// `<data>/<kind>/<connection>/<db>`, e.g. the saved queries of a database.
    pub fn folder(&self, kind: &str, cid: u64, db: &str) -> PathBuf {
        self.dir.join(kind).join(cid.to_string()).join(sanitize(db))
    }

    /// The `.sql` files in `dir`, sorted by name. `preview` reads the start of each.
    pub fn list_sql_files(dir: &Path, preview: bool) -> Vec<SavedFile> {
        let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
        let mut files: Vec<SavedFile> = rd.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "sql")).map(|e| saved_file(&e, preview)).collect();
        files.sort();
        files
    }
}

fn saved_file(e: &std::fs::DirEntry, preview: bool) -> SavedFile {
    let path = e.path();
    let meta = e.metadata().ok();
    let modified = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| fmt_time(d.as_secs() as i64))
        .unwrap_or_default();
    let preview = if preview { std::fs::read_to_string(&path).map(|t| t.chars().take(PREVIEW_CHARS).collect()).unwrap_or_default() } else { String::new() };
    SavedFile { name: path.file_stem().unwrap_or_default().to_string_lossy().to_string(), size: meta.map(|m| m.len()).unwrap_or(0), modified, path, preview }
}
