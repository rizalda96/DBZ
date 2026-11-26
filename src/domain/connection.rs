//! Connection settings and the kinds of server DBZ can talk to.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::sql_auth::SqlAuth;

const DEFAULT_TIMEOUT_SECS: u64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DbKind {
    MySql,
    MariaDb,
    PostgreSql,
    SqlServer,
}

impl DbKind {
    pub const ALL: [DbKind; 4] = [DbKind::MySql, DbKind::MariaDb, DbKind::PostgreSql, DbKind::SqlServer];

    pub fn label(self) -> &'static str {
        match self {
            DbKind::MySql => "MySQL",
            DbKind::MariaDb => "MariaDB",
            DbKind::PostgreSql => "PostgreSQL",
            DbKind::SqlServer => "SQL Server",
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            DbKind::MySql | DbKind::MariaDb => 3306,
            DbKind::PostgreSql => 5432,
            DbKind::SqlServer => 1433,
        }
    }

    /// The server's own dump format and the client tools that write and read it;
    /// None where SQL dumps are all there is.
    pub fn archive_tools(self) -> Option<ArchiveTools> {
        match self {
            DbKind::PostgreSql => Some(ArchiveTools { dump: "pg_dump", restore: "pg_restore", extension: "dump" }),
            DbKind::MySql | DbKind::MariaDb | DbKind::SqlServer => None,
        }
    }

    pub fn default_user(self) -> &'static str {
        match self {
            DbKind::MySql | DbKind::MariaDb => "root",
            DbKind::PostgreSql => "postgres",
            DbKind::SqlServer => "sa",
        }
    }
}

/// Programs for a server's archive dumps (e.g. PostgreSQL's custom format).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveTools {
    pub dump: &'static str,
    pub restore: &'static str,
    /// File extension, without the dot.
    pub extension: &'static str,
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
    /// Accept the server's TLS certificate without checking who issued it (SQL Server).
    /// Local servers often use a self-signed one.
    #[serde(default)]
    pub trust_cert: bool,
    /// SQL Server only; the other kinds always log in with user name and password.
    #[serde(default)]
    pub auth: SqlAuth,
    /// Microsoft Entra tenant (ID or domain) for the Entra methods that use one.
    #[serde(default)]
    pub tenant: String,
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
            trust_cert: false,
            auth: SqlAuth::default(),
            tenant: String::new(),
        }
    }

    /// Connect timeout, falling back to the default when none is set.
    pub fn timeout(&self) -> Duration {
        let secs = if self.connect_timeout == 0 { DEFAULT_TIMEOUT_SECS } else { self.connect_timeout };
        Duration::from_secs(secs)
    }

    /// Same server and login, whatever the connection is called; used to skip
    /// connections that are already in the list when importing.
    pub fn same_login(&self, other: &ConnConfig) -> bool {
        self.kind == other.kind && self.host.eq_ignore_ascii_case(&other.host) && self.port == other.port && self.user == other.user
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
