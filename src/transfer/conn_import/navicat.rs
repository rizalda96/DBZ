//! Navicat: an exported connections file (`.ncx`, File › Export Connections) or, on
//! Windows, the servers it keeps in the registry (one branch per database type).

use std::path::Path;

use super::navicat_cipher::decrypt;
use super::xml_attrs::elements;
use super::{navicat_cred, parse_port, read_text, registry, value, ConnImporter, Imported};
use crate::db::DbError;
use crate::domain::{ConnConfig, DbKind, SqlAuth};

const REG_ROOTS: [(&str, DbKind); 4] = [
    (r"HKEY_CURRENT_USER\Software\PremiumSoft\Navicat\Servers", DbKind::MySql),
    (r"HKEY_CURRENT_USER\Software\PremiumSoft\NavicatMARIADB\Servers", DbKind::MariaDb),
    (r"HKEY_CURRENT_USER\Software\PremiumSoft\NavicatPG\Servers", DbKind::PostgreSql),
    (r"HKEY_CURRENT_USER\Software\PremiumSoft\NavicatMSSQL\Servers", DbKind::SqlServer),
];

pub(super) struct Navicat;

impl ConnImporter for Navicat {
    fn label(&self) -> &'static str {
        "Navicat"
    }

    fn hint(&self) -> &'static str {
        "A .ncx file from Navicat's File › Export Connections, with \"Export Password\" ticked. Leave it empty to read the connections Navicat keeps in the Windows registry."
    }

    fn filter(&self) -> (&'static str, &'static [&'static str]) {
        ("Navicat connections", &["ncx"])
    }

    fn read(&self, path: Option<&Path>) -> Result<Imported, DbError> {
        match path {
            Some(p) => Ok(from_ncx(&read_text(p)?)),
            None => from_registry(),
        }
    }
}

pub(super) fn from_ncx(xml: &str) -> Imported {
    let mut out = Imported::default();
    for attrs in elements(xml, "Connection") {
        let name = value(&attrs, "ConnectionName").unwrap_or_default();
        let conn_type = value(&attrs, "ConnType").unwrap_or_default();
        let conn = ncx_kind(conn_type).map(|kind| connection(name, kind, &attrs, None)).ok_or("this database type is not supported");
        out.add(name, conn);
    }
    out
}

fn ncx_kind(conn_type: &str) -> Option<DbKind> {
    match conn_type.to_ascii_uppercase().as_str() {
        "MYSQL" => Some(DbKind::MySql),
        "MARIADB" => Some(DbKind::MariaDb),
        "POSTGRESQL" => Some(DbKind::PostgreSql),
        "SQLSERVER" | "MSSQL" => Some(DbKind::SqlServer),
        _ => None,
    }
}

fn from_registry() -> Result<Imported, DbError> {
    let mut out = Imported::default();
    // Navicat 12.1+ keeps the password in `Pwd_2`, which needs this machine's key; read
    // it once (None off Windows, or when Navicat was never set up here).
    let v3_key = navicat_cred::key();
    for (root, kind) in REG_ROOTS {
        // Only the servers themselves; their subkeys hold per-database settings.
        for key in registry::read(root)?.iter().filter(|k| !k.path.contains('\\')) {
            out.add(key.name(), Ok(connection(key.name(), kind, &key.values, v3_key.as_ref())));
        }
    }
    Ok(out)
}

/// A connection from the values of an `.ncx` element or a registry key; the two use
/// slightly different names for the password and the database.
fn connection(name: &str, kind: DbKind, values: &[(String, String)], v3_key: Option<&[u8; 32]>) -> ConnConfig {
    let get = |n: &str| value(values, n).unwrap_or_default().to_string();
    let mut cfg = ConnConfig::blank(0, kind);
    cfg.name = name.to_string();
    cfg.host = get("Host");
    cfg.port = parse_port(&get("Port")).unwrap_or(cfg.port);
    cfg.user = get("UserName");
    cfg.password = password(values, v3_key);
    cfg.initial_db = value(values, "Database").filter(|d| !d.is_empty()).or_else(|| value(values, "InitialDatabase")).unwrap_or_default().to_string();
    if kind == DbKind::SqlServer && get("MSSQLAuthenMode").eq_ignore_ascii_case("WINDOWS") {
        cfg.auth = SqlAuth::Windows;
    }
    cfg
}

/// The decrypted password: `Pwd`/`Password` (Navicat 11/12, portable) first, else the
/// machine-bound `Pwd_2` (Navicat 12.1+) when its key could be read.
fn password(values: &[(String, String)], v3_key: Option<&[u8; 32]>) -> String {
    if let Some(enc) = value(values, "Password").or_else(|| value(values, "Pwd")).filter(|p| !p.is_empty()) {
        return decrypt(enc);
    }
    let pwd_2 = value(values, "Pwd_2").filter(|p| !p.is_empty());
    pwd_2.zip(v3_key).and_then(|(enc, key)| navicat_cred::v3_decrypt_hex(key, enc)).unwrap_or_default()
}
