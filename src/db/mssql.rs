//! SQL Server driver. tiberius is async, so the driver owns a single-thread tokio
//! runtime and blocks on it; from outside it is as synchronous as the other drivers.

use std::time::Duration;

use futures_util::TryStreamExt;
use tiberius::error::Error as TdsError;
use tiberius::{AuthMethod, Client, Config, EncryptionLevel, QueryItem, QueryStream, SqlBrowser};
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use super::driver::{number, text, Driver};
use super::mssql_auth::auth_method;
use super::mssql_cells::{cell, column_type, default_text};
use super::mssql_parts;
use super::DbError;
use crate::domain::{first_cell, ConnConfig, FieldDef, IndexChoices, ResultSet, RoutineInfo, Row, TableInfo, TableParts};
use crate::sql::{Dialect, MssqlDialect};

static DIALECT: MssqlDialect = MssqlDialect;

/// Where server-level work runs when no database is chosen.
const MASTER: &str = "master";
const DEFAULT_SCHEMA: &str = "dbo";
const CERT_HINT: &str = "If the server uses a self-signed certificate, tick \"Trust server certificate\" in the connection settings.";

const DATABASES_SQL: &str = "SELECT name FROM sys.databases WHERE HAS_DBACCESS(name) = 1 ORDER BY name";
/// User schemas: not guest, INFORMATION_SCHEMA, sys or the fixed database-role schemas.
const SCHEMAS_SQL: &str = "SELECT name FROM sys.schemas WHERE schema_id NOT IN (2, 3, 4) AND schema_id < 16384 ORDER BY CASE WHEN name = N'dbo' THEN 0 ELSE 1 END, name";
const TABLES_SQL: &str = "SELECT o.name, s.name, o.type, \
    (SELECT SUM(p.rows) FROM sys.partitions p WHERE p.object_id = o.object_id AND p.index_id IN (0, 1)), \
    (SELECT SUM(a.total_pages) * 8192 FROM sys.partitions p JOIN sys.allocation_units a ON a.container_id = p.partition_id WHERE p.object_id = o.object_id), \
    CONVERT(varchar(19), o.create_date, 120), CONVERT(varchar(19), o.modify_date, 120), CAST(ep.value AS nvarchar(4000)) \
    FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id \
    LEFT JOIN sys.extended_properties ep ON ep.class = 1 AND ep.major_id = o.object_id AND ep.minor_id = 0 AND ep.name = N'MS_Description' \
    WHERE o.type IN ('U', 'V') AND o.is_ms_shipped = 0 ORDER BY CASE WHEN s.name = N'dbo' THEN 0 ELSE 1 END, s.name, o.name";
const ROUTINES_SQL: &str = "SELECT o.name, CASE WHEN o.type IN ('P', 'PC') THEN 'PROCEDURE' ELSE 'FUNCTION' END, \
    CASE WHEN o.type IN ('IF', 'TF', 'FT') THEN 'TABLE' ELSE ISNULL((SELECT TYPE_NAME(r.user_type_id) FROM sys.parameters r WHERE r.object_id = o.object_id AND r.parameter_id = 0), '') END, s.name \
    FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id \
    WHERE o.type IN ('P', 'PC', 'FN', 'IF', 'TF', 'FS', 'FT') AND o.is_ms_shipped = 0 ORDER BY CASE WHEN s.name = N'dbo' THEN 0 ELSE 1 END, s.name, o.name";
const COLUMNS_SQL: &str = "SELECT c.name, TYPE_NAME(c.user_type_id), c.max_length, c.precision, c.scale, c.is_nullable, c.is_identity, OBJECT_DEFINITION(c.default_object_id), \
    CAST(ep.value AS nvarchar(4000)), CASE WHEN EXISTS (SELECT 1 FROM sys.indexes i JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id \
    WHERE i.object_id = c.object_id AND i.is_primary_key = 1 AND ic.column_id = c.column_id) THEN 1 ELSE 0 END \
    FROM sys.columns c LEFT JOIN sys.extended_properties ep ON ep.class = 1 AND ep.major_id = c.object_id AND ep.minor_id = c.column_id AND ep.name = N'MS_Description' \
    WHERE c.object_id = OBJECT_ID({obj}) ORDER BY c.column_id";
const TABLE_COMMENT_SQL: &str = "SELECT CAST(value AS nvarchar(4000)) FROM sys.extended_properties WHERE class = 1 AND major_id = OBJECT_ID({obj}) AND minor_id = 0 AND name = N'MS_Description'";
const USERS_SQL: &str = "SELECT name AS [Login], type_desc AS [Type], is_disabled AS [Disabled], default_database_name AS [Default database], CONVERT(varchar(19), create_date, 120) AS [Created] \
    FROM sys.server_principals WHERE type IN ('S', 'U', 'G', 'E', 'X') AND name NOT LIKE N'##%' ORDER BY name";

type Conn = Client<Compat<TcpStream>>;

pub struct MssqlDriver {
    rt: Runtime,
    client: Conn,
    /// The session's database; None when a script may have switched it with USE.
    current: Option<String>,
}

impl MssqlDriver {
    pub fn connect(cfg: &ConnConfig) -> Result<Self, DbError> {
        let auth = auth_method(cfg)?;
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| DbError::Connect(e.to_string()))?;
        let client = rt.block_on(open(config(cfg, auth), cfg.timeout()))?;
        let current = Some(if cfg.initial_db.is_empty() { String::new() } else { cfg.initial_db.clone() });
        Ok(MssqlDriver { rt, client, current })
    }

    fn use_db(&mut self, db: &str) -> Result<(), DbError> {
        let db = if db.is_empty() { MASTER } else { db };
        if self.current.as_deref() == Some(db) {
            return Ok(());
        }
        self.run(&format!("USE {}", DIALECT.quote_ident(db)))?;
        self.current = Some(db.to_string());
        Ok(())
    }

    /// Sends one batch and collects every result set it returns.
    fn run(&mut self, batch: &str) -> Result<Vec<ResultSet>, DbError> {
        let client = &mut self.client;
        self.rt.block_on(async move {
            let stream = client.simple_query(batch).await.map_err(query_err)?;
            collect_sets(stream).await
        })
    }

    /// `OBJECT_ID` argument for a table as the tree names it.
    fn object(table: &str) -> String {
        DIALECT.str_literal(&DIALECT.quote_ident(table))
    }
}

fn config(cfg: &ConnConfig, auth: AuthMethod) -> Config {
    let mut c = Config::new();
    match cfg.host.split_once('\\') {
        // A named instance's port comes from the SQL Server Browser.
        Some((host, instance)) => {
            c.host(host);
            c.instance_name(instance);
        }
        None => {
            c.host(&cfg.host);
            c.port(cfg.port);
        }
    }
    c.authentication(auth);
    if !cfg.initial_db.is_empty() {
        c.database(&cfg.initial_db);
    }
    c.application_name("DBZ");
    c.encryption(EncryptionLevel::Required);
    if cfg.trust_cert {
        c.trust_cert();
    }
    c
}

/// Connects within `timeout`, following one redirect (Azure SQL sends those).
async fn open(config: Config, timeout: Duration) -> Result<Conn, DbError> {
    let attempt = async {
        match connect_tcp(config.clone()).await {
            Err(TdsError::Routing { host, port }) => {
                let mut moved = config;
                moved.host(host);
                moved.port(port);
                connect_tcp(moved).await
            }
            other => other,
        }
    };
    match tokio::time::timeout(timeout, attempt).await {
        Ok(result) => result.map_err(connect_err),
        Err(_) => Err(DbError::Connect(format!("Timed out after {} s", timeout.as_secs()))),
    }
}

async fn connect_tcp(config: Config) -> Result<Conn, TdsError> {
    let tcp = TcpStream::connect_named(&config).await?;
    tcp.set_nodelay(true)?;
    Client::connect(config, tcp.compat_write()).await
}

fn message(e: &TdsError) -> String {
    match e {
        TdsError::Server(t) => t.message().to_string(),
        other => other.to_string(),
    }
}

fn connect_err(e: TdsError) -> DbError {
    let msg = message(&e);
    let tls = matches!(e, TdsError::Tls(_)) || msg.to_lowercase().contains("certificate");
    DbError::Connect(if tls { format!("{}\n\n{}", msg, CERT_HINT) } else { msg })
}

fn query_err(e: TdsError) -> DbError {
    DbError::Query(message(&e))
}

async fn collect_sets(mut stream: QueryStream<'_>) -> Result<Vec<ResultSet>, DbError> {
    let mut out: Vec<ResultSet> = vec![];
    while let Some(item) = stream.try_next().await.map_err(query_err)? {
        add_item(&mut out, item);
    }
    Ok(out)
}

fn add_item(out: &mut Vec<ResultSet>, item: QueryItem) {
    match item {
        QueryItem::Metadata(m) => {
            let columns = m.columns().iter().map(|c| c.name().to_string()).collect();
            out.push(ResultSet { columns, has_rows: true, ..Default::default() });
        }
        QueryItem::Row(r) => {
            if let Some(set) = out.last_mut() {
                set.rows.push(r.into_iter().map(cell).collect());
            }
        }
    }
}

/// The first word of `sql` after leading comments and blanks, upper-cased.
fn first_word(sql: &str) -> String {
    let mut rest = sql.trim_start();
    loop {
        if let Some(line) = rest.strip_prefix("--") {
            rest = line.split_once('\n').map_or("", |(_, r)| r).trim_start();
        } else if let Some(block) = rest.strip_prefix("/*") {
            rest = block.split_once("*/").map_or("", |(_, r)| r).trim_start();
        } else {
            break;
        }
    }
    rest.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_uppercase()
}

/// Data changes get their row count from `@@ROWCOUNT`; the server's DONE count isn't
/// passed on by tiberius' batch API.
fn counts_rows(sql: &str) -> bool {
    matches!(first_word(sql).as_str(), "INSERT" | "UPDATE" | "DELETE" | "MERGE")
}

/// Whether a batch may switch database with USE.
fn may_switch_db(sql: &str) -> bool {
    sql.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w.eq_ignore_ascii_case("use"))
}

impl Driver for MssqlDriver {
    fn query(&mut self, db: &str, sql: &str) -> Result<Vec<ResultSet>, DbError> {
        self.use_db(db)?;
        let counted = counts_rows(sql);
        let batch = if counted { format!("{}\nSELECT @@ROWCOUNT", sql) } else { sql.to_string() };
        let result = self.run(&batch);
        if may_switch_db(sql) {
            self.current = None;
        }
        let mut sets = result?;
        if counted {
            let count = sets.pop().map(|s| vec![s]).and_then(|s| first_cell(&s)).and_then(|n| n.parse().ok()).unwrap_or(0);
            sets.push(ResultSet { affected: count, ..Default::default() });
        }
        for s in &mut sets {
            s.statement = sql.to_string();
        }
        Ok(sets)
    }

    fn databases(&mut self) -> Result<Vec<String>, DbError> {
        Ok(self.rows("", DATABASES_SQL)?.iter().map(|r| text(&r[0])).collect())
    }

    fn tables(&mut self, db: &str) -> Result<Vec<TableInfo>, DbError> {
        Ok(self.rows(db, TABLES_SQL)?.iter().map(table_info).collect())
    }

    fn routines(&mut self, db: &str) -> Result<Vec<RoutineInfo>, DbError> {
        Ok(self.rows(db, ROUTINES_SQL)?.iter().map(routine_info).collect())
    }

    fn schemas(&mut self, db: &str) -> Result<Vec<String>, DbError> {
        Ok(self.rows(db, SCHEMAS_SQL)?.iter().map(|r| text(&r[0])).collect())
    }

    fn columns(&mut self, db: &str, table: &str) -> Result<Vec<FieldDef>, DbError> {
        let sql = COLUMNS_SQL.replace("{obj}", &Self::object(table));
        Ok(self.rows(db, &sql)?.iter().map(field_def).collect())
    }

    fn ddl(&mut self, db: &str, table: &str, is_view: bool) -> Result<String, DbError> {
        if is_view {
            let sets = self.query(db, &format!("SELECT OBJECT_DEFINITION(OBJECT_ID({}))", Self::object(table)))?;
            return Ok(first_cell(&sets).unwrap_or_default().trim().to_string());
        }
        let cols = self.columns(db, table)?;
        let sets = self.query(db, &TABLE_COMMENT_SQL.replace("{obj}", &Self::object(table)))?;
        let comment = first_cell(&sets).unwrap_or_default();
        Ok(DIALECT.create_table(table, &cols, &comment).join(";\n") + ";")
    }

    fn routine_ddl(&mut self, db: &str, routine: &RoutineInfo) -> Result<String, DbError> {
        let sets = self.query(db, &format!("SELECT OBJECT_DEFINITION(OBJECT_ID({}))", Self::object(&routine.name)))?;
        let def = first_cell(&sets).map(|d| d.trim().to_string()).filter(|d| !d.is_empty());
        def.ok_or_else(|| DbError::Query(format!("No definition found for {} (it may be encrypted)", routine.name)))
    }

    fn table_parts(&mut self, db: &str, table: &str) -> Result<TableParts, DbError> {
        mssql_parts::table_parts(self, db, &Self::object(table))
    }

    fn index_choices(&mut self, db: &str) -> Result<IndexChoices, DbError> {
        mssql_parts::index_choices(self, db)
    }

    fn ref_columns(&mut self, db: &str, schema: &str, table: &str) -> Result<Vec<String>, DbError> {
        let name = match schema.trim() {
            "" | DEFAULT_SCHEMA => table.to_string(),
            s => format!("{}.{}", s, table),
        };
        Ok(self.columns(db, &name)?.into_iter().map(|f| f.name).collect())
    }

    fn users(&mut self) -> Result<ResultSet, DbError> {
        self.first_set("", USERS_SQL)
    }

    fn dialect(&self) -> &'static dyn Dialect {
        &DIALECT
    }
}

/// `name` as other code addresses it: bare in dbo, "schema.name" elsewhere.
fn qualified(schema: &str, name: String) -> String {
    if schema == DEFAULT_SCHEMA { name } else { format!("{}.{}", schema, name) }
}

fn table_info(r: &Row) -> TableInfo {
    let schema = text(&r[1]);
    TableInfo {
        name: qualified(&schema, text(&r[0])),
        schema,
        is_view: text(&r[2]).trim() == "V",
        rows: number(&r[3]),
        data_length: number(&r[4]),
        created: text(&r[5]),
        modified: text(&r[6]),
        comment: text(&r[7]),
        ..Default::default()
    }
}

fn routine_info(r: &Row) -> RoutineInfo {
    let schema = text(&r[3]);
    RoutineInfo { name: qualified(&schema, text(&r[0])), schema, kind: text(&r[1]), returns: text(&r[2]) }
}

fn field_def(r: &Row) -> FieldDef {
    let (ty, length, decimals) = column_type(&text(&r[1]), number(&r[2]).unwrap_or(0), &text(&r[3]), &text(&r[4]));
    FieldDef {
        name: text(&r[0]),
        ty,
        length,
        decimals,
        not_null: text(&r[5]) == "0",
        key: text(&r[9]) == "1",
        auto_inc: text(&r[6]) == "1",
        default: default_text(&text(&r[7])),
        comment: text(&r[8]),
        orig_name: None,
    }
    .loaded()
}

#[cfg(test)]
mod tests {
    use super::{counts_rows, first_word, may_switch_db, MssqlDriver};
    use crate::db::Driver;
    use crate::domain::{ConnConfig, DbKind};

    /// Talks to a real server: `DBZ_TEST_MSSQL="host;user;password"` (self-signed
    /// certificates are trusted), then `cargo test -- --ignored live_server`.
    #[test]
    #[ignore]
    fn live_server() {
        let spec = std::env::var("DBZ_TEST_MSSQL").expect("DBZ_TEST_MSSQL is not set");
        let parts: Vec<&str> = spec.splitn(3, ';').collect();
        let cfg = ConnConfig { host: parts[0].into(), user: parts[1].into(), password: parts[2].into(), trust_cert: true, ..ConnConfig::blank(1, DbKind::SqlServer) };
        let mut d = MssqlDriver::connect(&cfg).unwrap_or_else(|e| panic!("connect: {}", e));
        assert!(d.databases().expect("databases").iter().any(|n| n == "tempdb"));
        let sets = d.query("tempdb", "CREATE TABLE #t (id int IDENTITY(1,1) PRIMARY KEY, name nvarchar(20) DEFAULT N'x', at datetime2(3));\nINSERT INTO #t (name, at) VALUES (N'é', '2024-02-29 10:00:00.5'), (NULL, NULL)").expect("create");
        assert_eq!(sets.last().map(|s| s.affected), Some(2));
        let sets = d.query("tempdb", "UPDATE #t SET name = N'y' WHERE name IS NULL").expect("update");
        assert_eq!(sets.last().map(|s| s.affected), Some(1));
        let rows = d.rows("tempdb", "SELECT id, name, at FROM #t ORDER BY id OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY").expect("select");
        assert_eq!(rows, vec![vec![Some("1".into()), Some("é".into()), Some("2024-02-29 10:00:00.500".into())], vec![Some("2".into()), Some("y".into()), None]]);
        let err = d.query("tempdb", "SELECT * FROM no_such_table").expect_err("bad table");
        assert!(err.to_string().contains("no_such_table"), "{}", err);
        assert!(d.query("", "SELECT 1").is_ok(), "the connection survives an error");
    }

    #[test]
    fn reads_the_statement_kind() {
        assert_eq!(first_word("  -- note\n/* more */ update t set a = 1"), "UPDATE");
        assert!(counts_rows("DELETE FROM t"));
        assert!(!counts_rows("SELECT * FROM t"));
        assert!(!counts_rows("CREATE PROCEDURE p AS UPDATE t SET a = 1"));
        assert!(may_switch_db("SELECT 1;\nuse [shop]"));
        assert!(!may_switch_db("SELECT used FROM users"));
    }
}
