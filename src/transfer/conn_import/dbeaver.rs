//! DBeaver: `data-sources.json` of a workspace project, with user names and passwords
//! from the AES-encrypted `credentials-config.json` next to it.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{aes_cbc, parse_port, read_text, ConnImporter, Imported};
use crate::db::DbError;
use crate::domain::{ConnConfig, DbKind};

/// DBeaver's fixed key for `credentials-config.json`; the file starts with the IV.
pub(super) const CREDENTIALS_KEY: [u8; 16] = [0xba, 0xbb, 0x4a, 0x9f, 0x77, 0x4a, 0xb8, 0x53, 0xc9, 0x6c, 0x2d, 0x65, 0x3d, 0xfe, 0x54, 0x4a];
const CREDENTIALS_FILE: &str = "credentials-config.json";
const IV_LEN: usize = 16;

pub(super) struct DBeaver;

impl ConnImporter for DBeaver {
    fn label(&self) -> &'static str {
        "DBeaver"
    }

    fn hint(&self) -> &'static str {
        "data-sources.json in a DBeaver project's .dbeaver folder; passwords come from credentials-config.json next to it. Leave it empty to use the default workspace."
    }

    fn filter(&self) -> (&'static str, &'static [&'static str]) {
        ("DBeaver data sources", &["json"])
    }

    fn read(&self, path: Option<&Path>) -> Result<Imported, DbError> {
        let path = match path {
            Some(p) => p.to_path_buf(),
            None => default_file().ok_or_else(|| DbError::Invalid("DBeaver's workspace was not found. Choose its data-sources.json.".into()))?,
        };
        let sources: Value = serde_json::from_str(&read_text(&path)?).map_err(|e| DbError::Invalid(format!("{} is not valid JSON: {}", path.display(), e)))?;
        let creds = credentials(&path.with_file_name(CREDENTIALS_FILE))?;
        Ok(connections(&sources, &creds))
    }
}

/// `data-sources.json` of the default project in DBeaver's default workspace.
fn default_file() -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let root = match std::env::consts::OS {
        "windows" => var("APPDATA").map(|p| p.join("DBeaverData")),
        "macos" => var("HOME").map(|h| h.join("Library").join("DBeaverData")),
        _ => var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local").join("share"))).map(|p| p.join("DBeaverData")),
    }?;
    let file = root.join("workspace6").join("General").join(".dbeaver").join("data-sources.json");
    file.is_file().then_some(file)
}

/// The decrypted credentials, or Null when the file is missing (nothing saved).
fn credentials(path: &Path) -> Result<Value, DbError> {
    let Ok(bytes) = std::fs::read(path) else { return Ok(Value::Null) };
    let bad = || DbError::Invalid(format!("Could not decrypt {}.", path.display()));
    if bytes.len() < IV_LEN {
        return Err(bad());
    }
    let (iv, data) = bytes.split_at(IV_LEN);
    let iv: &[u8; IV_LEN] = iv.try_into().expect("split_at gives exactly IV_LEN bytes");
    let plain = aes_cbc::decrypt(&CREDENTIALS_KEY, iv, data).ok_or_else(bad)?;
    serde_json::from_slice(&plain).map_err(|_| bad())
}

pub(super) fn connections(sources: &Value, creds: &Value) -> Imported {
    let mut out = Imported::default();
    let Some(conns) = sources.get("connections").and_then(Value::as_object) else { return out };
    for (id, conn) in conns {
        let name = text(conn, "name").unwrap_or(id);
        let login = creds.get(id).and_then(|c| c.get("#connection"));
        out.add(name, connection(name, conn, login));
    }
    out
}

fn connection(name: &str, conn: &Value, login: Option<&Value>) -> Result<ConnConfig, &'static str> {
    let kind = kind_of(text(conn, "provider").unwrap_or_default(), text(conn, "driver").unwrap_or_default()).ok_or("this database type is not supported")?;
    let conf = conn.get("configuration").unwrap_or(&Value::Null);
    let host = text(conf, "host").filter(|h| !h.is_empty()).ok_or("it has no host (connections set up by URL are not supported)")?;
    // Older versions kept the login in data-sources.json itself.
    let login_text = |key: &str| login.and_then(|l| text(l, key)).or_else(|| text(conf, key));
    let mut cfg = ConnConfig::blank(0, kind);
    cfg.name = name.to_string();
    cfg.host = host.to_string();
    cfg.port = port(conf.get("port")).unwrap_or(cfg.port);
    cfg.initial_db = text(conf, "database").unwrap_or_default().to_string();
    cfg.user = login_text("user").map(str::to_string).unwrap_or(cfg.user);
    cfg.password = login_text("password").unwrap_or_default().to_string();
    Ok(cfg)
}

fn kind_of(provider: &str, driver: &str) -> Option<DbKind> {
    if provider.to_ascii_lowercase().contains("maria") || driver.to_ascii_lowercase().contains("maria") {
        return Some(DbKind::MariaDb);
    }
    match provider.to_ascii_lowercase().as_str() {
        "mysql" => Some(DbKind::MySql),
        "postgresql" => Some(DbKind::PostgreSql),
        "sqlserver" | "mssql" => Some(DbKind::SqlServer),
        _ => None,
    }
}

fn text<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// DBeaver writes the port as a string; accept a number too.
fn port(v: Option<&Value>) -> Option<u16> {
    match v? {
        Value::String(s) => parse_port(s),
        Value::Number(n) => n.as_u64().and_then(|n| u16::try_from(n).ok()),
        _ => None,
    }
}
