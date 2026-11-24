//! Shared data types used by the UI and the database workers.

use eframe::egui;
use serde::{Deserialize, Serialize};

pub type Cell = Option<String>;
pub type Row = Vec<Cell>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DbKind {
    MySql,
    MariaDb,
    PostgreSql,
    Sample,
}

impl DbKind {
    pub const ALL: [DbKind; 4] = [DbKind::MySql, DbKind::MariaDb, DbKind::PostgreSql, DbKind::Sample];

    pub fn label(self) -> &'static str {
        match self {
            DbKind::MySql => "MySQL",
            DbKind::MariaDb => "MariaDB",
            DbKind::PostgreSql => "PostgreSQL",
            DbKind::Sample => "Sample Data (built-in)",
        }
    }
    pub fn default_port(self) -> u16 {
        match self {
            DbKind::MySql | DbKind::MariaDb => 3306,
            DbKind::PostgreSql => 5432,
            DbKind::Sample => 0,
        }
    }
    pub fn default_user(self) -> &'static str {
        match self {
            DbKind::MySql | DbKind::MariaDb => "root",
            DbKind::PostgreSql => "postgres",
            DbKind::Sample => "",
        }
    }
    /// SQL dialect used when generating statements.
    pub fn is_pg(self) -> bool {
        self == DbKind::PostgreSql
    }
    pub fn color(self) -> egui::Color32 {
        match self {
            DbKind::MySql => egui::Color32::from_rgb(0x2b, 0x7a, 0xb8),
            DbKind::MariaDb => egui::Color32::from_rgb(0x9a, 0x6a, 0x3a),
            DbKind::PostgreSql => egui::Color32::from_rgb(0x33, 0x5d, 0x8f),
            DbKind::Sample => egui::Color32::from_rgb(0x3c, 0x9a, 0x5c),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnConfig {
    pub id: u64,
    pub name: String,
    pub kind: DbKind,
    pub host: String,
    pub port: u16,
    pub user: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "yes")]
    pub save_password: bool,
    #[serde(default)]
    pub initial_db: String,
    #[serde(default)]
    pub connect_timeout: u64,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Default)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    pub affected: u64,
    /// The statement that produced this set (for the Message tab).
    pub statement: String,
    pub has_rows: bool,
}

#[derive(Clone, Debug, Default)]
pub struct TableInfo {
    pub name: String,
    pub is_view: bool,
    pub rows: Option<u64>,
    pub data_length: Option<u64>,
    pub engine: String,
    pub created: String,
    pub modified: String,
    pub comment: String,
    pub collation: String,
}

#[derive(Clone, Debug, Default)]
pub struct RoutineInfo {
    pub name: String,
    pub kind: String,
    pub returns: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldDef {
    pub name: String,
    pub ty: String,
    pub length: String,
    pub decimals: String,
    pub not_null: bool,
    pub key: bool,
    pub auto_inc: bool,
    pub default: String,
    pub comment: String,
    /// Name before editing in the designer (None = new field).
    pub orig_name: Option<String>,
}

impl FieldDef {
    pub fn type_text(&self) -> String {
        let t = self.ty.trim();
        let size = if self.length.trim().is_empty() {
            String::new()
        } else if self.decimals.trim().is_empty() {
            format!("({})", self.length.trim())
        } else {
            format!("({},{})", self.length.trim(), self.decimals.trim())
        };
        // modifiers that follow the size, e.g. "int(11) unsigned", "timestamp(3) with time zone"
        for suffix in [" unsigned zerofill", " unsigned", " zerofill", " without time zone", " with time zone"] {
            if let Some(base) = t.strip_suffix(suffix) {
                return format!("{}{}{}", base, size, suffix);
            }
        }
        format!("{}{}", t, size)
    }

    /// Split a full type such as "decimal(10,2) unsigned" into (type, length, decimals).
    pub fn split_type(full: &str) -> (String, String, String) {
        let full = full.trim();
        if let (Some(a), Some(b)) = (full.find('('), full.find(')')) {
            if b > a {
                let base = full[..a].trim();
                let inner = &full[a + 1..b];
                let rest = full[b + 1..].trim();
                let ty = if rest.is_empty() { base.to_string() } else { format!("{} {}", base, rest) };
                if base.eq_ignore_ascii_case("enum") || base.eq_ignore_ascii_case("set") {
                    return (ty, inner.to_string(), String::new());
                }
                let mut parts = inner.splitn(2, ',');
                let len = parts.next().unwrap_or("").trim().to_string();
                let dec = parts.next().unwrap_or("").trim().to_string();
                return (ty, len, dec);
            }
        }
        (full.to_string(), String::new(), String::new())
    }
}

pub fn fmt_bytes(n: u64) -> String {
    if n >= 1 << 30 {
        format!("{:.2} GB", n as f64 / (1u64 << 30) as f64)
    } else if n >= 1 << 20 {
        format!("{:.2} MB", n as f64 / (1u64 << 20) as f64)
    } else {
        format!("{} KB", n.div_ceil(1024))
    }
}
