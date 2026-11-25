//! Connection settings and the kinds of server DBZ can talk to.

use std::time::Duration;

use serde::{Deserialize, Serialize};

const DEFAULT_TIMEOUT_SECS: u64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DbKind {
    MySql,
    MariaDb,
    PostgreSql,
}

impl DbKind {
    pub const ALL: [DbKind; 3] = [DbKind::MySql, DbKind::MariaDb, DbKind::PostgreSql];

    pub fn label(self) -> &'static str {
        match self {
            DbKind::MySql => "MySQL",
            DbKind::MariaDb => "MariaDB",
            DbKind::PostgreSql => "PostgreSQL",
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            DbKind::MySql | DbKind::MariaDb => 3306,
            DbKind::PostgreSql => 5432,
        }
    }

    pub fn default_user(self) -> &'static str {
        match self {
            DbKind::MySql | DbKind::MariaDb => "root",
            DbKind::PostgreSql => "postgres",
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

impl ConnConfig {
    /// A blank connection of `kind` with that server's usual defaults.
    pub fn blank(id: u64, kind: DbKind) -> Self {
        ConnConfig {
            id,
            name: String::new(),
            kind,
            host: "localhost".into(),
            port: kind.default_port(),
            user: kind.default_user().into(),
            password: String::new(),
            save_password: true,
            initial_db: String::new(),
            connect_timeout: DEFAULT_TIMEOUT_SECS,
        }
    }

    /// Connect timeout, falling back to the default when none is set.
    pub fn timeout(&self) -> Duration {
        let secs = if self.connect_timeout == 0 { DEFAULT_TIMEOUT_SECS } else { self.connect_timeout };
        Duration::from_secs(secs)
    }

    /// Copy that is safe to write to disk: drops the password unless the user asked to keep it.
    pub fn for_storage(&self) -> Self {
        let mut cfg = self.clone();
        if !cfg.save_password {
            cfg.password.clear();
        }
        cfg
    }
}
