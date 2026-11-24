//! Database workers. Each open connection gets its own thread that owns the
//! client and answers requests one at a time, so the window never freezes.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::demo::DemoServer;
use crate::model::*;
use crate::sqlgen::{self, quote_ident};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Tsv,
    Json,
    Sql,
    Html,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 5] = [ExportFormat::Csv, ExportFormat::Tsv, ExportFormat::Json, ExportFormat::Sql, ExportFormat::Html];
    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Csv => "CSV file (*.csv)",
            ExportFormat::Tsv => "Text file, tab separated (*.txt)",
            ExportFormat::Json => "JSON file (*.json)",
            ExportFormat::Sql => "SQL script (*.sql)",
            ExportFormat::Html => "HTML table (*.html)",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Tsv => "txt",
            ExportFormat::Json => "json",
            ExportFormat::Sql => "sql",
            ExportFormat::Html => "html",
        }
    }
}

pub enum Req {
    Connect,
    Databases,
    Tables(String),
    Routines(String),
    Columns { db: String, table: String },
    Ddl { db: String, table: String, is_view: bool },
    Query { tab: u64, db: String, sql: String },
    TableData { tab: u64, db: String, table: String, filter: String, order: Option<(String, bool)>, limit: usize, offset: usize },
    Exec { tab: u64, db: String, stmts: Vec<String>, purpose: Purpose },
    Export { db: String, table: String, format: ExportFormat, path: String, header: bool },
    Import { db: String, table: String, path: String, header: bool, delim: char, create: bool },
    Dump { db: String, path: String, data: bool },
    RunFile { db: String, path: String, continue_on_error: bool },
    Users,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Purpose {
    SaveData,
    SaveDesign { new_name: String },
    Refresh,
    DropTables,
    Other(String),
}

pub enum Resp {
    Connected { version: String },
    ConnectFailed(String),
    Databases(Result<Vec<String>, String>),
    Tables(String, Result<Vec<TableInfo>, String>),
    Routines(String, Result<Vec<RoutineInfo>, String>),
    Columns { db: String, table: String, result: Result<Vec<FieldDef>, String> },
    Ddl { db: String, table: String, result: Result<String, String> },
    Query { tab: u64, results: Vec<ResultSet>, error: Option<(String, String)>, elapsed: Duration },
    TableData { tab: u64, result: Result<(Vec<FieldDef>, ResultSet, Option<u64>), String>, sql: String, elapsed: Duration },
    Exec { tab: u64, db: String, result: Result<u64, String>, purpose: Purpose },
    Task(Result<String, String>),
    Users(Result<ResultSet, String>),
}

pub type Reply = (u64, Resp);

enum Client {
    Sample(Arc<Mutex<DemoServer>>),
    My { conn: mysql::Conn, current: String },
    Pg { clients: HashMap<String, postgres::Client> },
}

pub fn spawn(cfg: ConnConfig, sample: Arc<Mutex<DemoServer>>, tx: Sender<Reply>, ctx: egui::Context) -> Sender<Req> {
    let (rtx, rrx) = std::sync::mpsc::channel::<Req>();
    std::thread::spawn(move || worker(cfg, sample, rrx, tx, ctx));
    rtx
}

fn worker(cfg: ConnConfig, sample: Arc<Mutex<DemoServer>>, rx: Receiver<Req>, tx: Sender<Reply>, ctx: egui::Context) {
    let id = cfg.id;
    let send = |r: Resp| {
        let _ = tx.send((id, r));
        ctx.request_repaint();
    };
    let mut client: Option<Client> = None;
    while let Ok(req) = rx.recv() {
        if client.is_none() {
            match open(&cfg, &sample) {
                Ok(c) => client = Some(c),
                Err(e) => {
                    send(Resp::ConnectFailed(e));
                    return;
                }
            }
        }
        let c = client.as_mut().unwrap();
        let resp = match req {
            Req::Connect => {
                let version = c.query(&cfg, &cfg.initial_db, "SELECT VERSION()").ok().and_then(|rs| rs.first().and_then(|r| r.rows.first().and_then(|row| row.first().cloned().flatten()))).unwrap_or_default();
                Resp::Connected { version }
            }
            Req::Databases => Resp::Databases(c.databases(&cfg)),
            Req::Tables(db) => {
                let r = c.tables(&cfg, &db);
                Resp::Tables(db, r)
            }
            Req::Routines(db) => {
                let r = c.routines(&cfg, &db);
                Resp::Routines(db, r)
            }
            Req::Columns { db, table } => {
                let result = c.columns(&cfg, &db, &table);
                Resp::Columns { db, table, result }
            }
            Req::Ddl { db, table, is_view } => {
                let result = c.ddl(&cfg, &db, &table, is_view);
                Resp::Ddl { db, table, result }
            }
            Req::Query { tab, db, sql } => {
                let start = Instant::now();
                let mut results = vec![];
                let mut error = None;
                for stmt in sqlgen::split_statements(&sql) {
                    match c.query(&cfg, &db, &stmt) {
                        Ok(mut sets) => {
                            if sets.is_empty() {
                                sets.push(ResultSet { statement: stmt.clone(), ..Default::default() });
                            }
                            for s in &mut sets {
                                s.statement = stmt.clone();
                            }
                            results.extend(sets)
                        }
                        Err(e) => {
                            error = Some((stmt, e));
                            break;
                        }
                    }
                }
                Resp::Query { tab, results, error, elapsed: start.elapsed() }
            }
            Req::TableData { tab, db, table, filter, order, limit, offset } => {
                let start = Instant::now();
                let pg = cfg.kind.is_pg();
                let qt = quote_ident(pg, &table);
                let mut where_ = String::new();
                if !filter.trim().is_empty() {
                    where_ = format!(" WHERE {}", filter.trim());
                }
                let mut sql = format!("SELECT * FROM {}{}", qt, where_);
                if let Some((col, desc)) = &order {
                    sql.push_str(&format!(" ORDER BY {} {}", quote_ident(pg, col), if *desc { "DESC" } else { "ASC" }));
                }
                sql.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));
                let result = (|| {
                    let fields = c.columns(&cfg, &db, &table)?;
                    let rs = c.query(&cfg, &db, &sql)?.into_iter().next().unwrap_or_default();
                    let total = c
                        .query(&cfg, &db, &format!("SELECT COUNT(*) FROM {}{}", qt, where_))
                        .ok()
                        .and_then(|r| r.first().and_then(|s| s.rows.first().and_then(|row| row.first().cloned().flatten())))
                        .and_then(|s| s.parse().ok());
                    Ok((fields, rs, total))
                })();
                Resp::TableData { tab, result, sql, elapsed: start.elapsed() }
            }
            Req::Exec { tab, db, stmts, purpose } => {
                let mut total = 0;
                let mut err = None;
                let transactional = purpose == Purpose::SaveData && stmts.len() > 1;
                if transactional {
                    let _ = c.query(&cfg, &db, if cfg.kind.is_pg() { "BEGIN" } else { "START TRANSACTION" });
                }
                for s in &stmts {
                    match c.query(&cfg, &db, s) {
                        Ok(r) => total += r.iter().map(|x| x.affected).sum::<u64>(),
                        Err(e) => {
                            err = Some(format!("{}\n\nStatement:\n{}", e, s));
                            break;
                        }
                    }
                }
                if transactional {
                    let _ = c.query(&cfg, &db, if err.is_some() { "ROLLBACK" } else { "COMMIT" });
                }
                Resp::Exec { tab, db, result: err.map(Err).unwrap_or(Ok(total)), purpose }
            }
            Req::Export { db, table, format, path, header } => Resp::Task(export(c, &cfg, &db, &table, format, &path, header)),
            Req::Import { db, table, path, header, delim, create } => Resp::Task(import(c, &cfg, &db, &table, &path, header, delim, create)),
            Req::Dump { db, path, data } => Resp::Task(dump(c, &cfg, &db, &path, data)),
            Req::RunFile { db, path, continue_on_error } => Resp::Task(run_file(c, &cfg, &db, &path, continue_on_error)),
            Req::Users => Resp::Users(c.users(&cfg)),
        };
        send(resp);
    }
}

fn open(cfg: &ConnConfig, sample: &Arc<Mutex<DemoServer>>) -> Result<Client, String> {
    let timeout = Duration::from_secs(if cfg.connect_timeout == 0 { 10 } else { cfg.connect_timeout });
    match cfg.kind {
        DbKind::Sample => Ok(Client::Sample(sample.clone())),
        DbKind::MySql | DbKind::MariaDb => {
            let opts = mysql::OptsBuilder::new()
                .ip_or_hostname(Some(cfg.host.clone()))
                .tcp_port(cfg.port)
                .user(Some(cfg.user.clone()))
                .pass(Some(cfg.password.clone()))
                .db_name(if cfg.initial_db.is_empty() { None } else { Some(cfg.initial_db.clone()) })
                .tcp_connect_timeout(Some(timeout));
            let conn = mysql::Conn::new(opts).map_err(|e| e.to_string())?;
            Ok(Client::My { conn, current: cfg.initial_db.clone() })
        }
        DbKind::PostgreSql => {
            let mut c = Client::Pg { clients: HashMap::new() };
            let db = if cfg.initial_db.is_empty() { "postgres".to_string() } else { cfg.initial_db.clone() };
            c.pg(cfg, &db)?;
            Ok(c)
        }
    }
}

fn mysql_value(v: mysql::Value) -> Cell {
    use mysql::Value::*;
    match v {
        NULL => None,
        Bytes(b) => match String::from_utf8(b) {
            Ok(s) => Some(s),
            Err(e) => Some(format!("(BLOB) {} bytes", e.as_bytes().len())),
        },
        Int(i) => Some(i.to_string()),
        UInt(u) => Some(u.to_string()),
        Float(f) => Some(f.to_string()),
        Double(d) => Some(d.to_string()),
        Date(y, m, d, h, mi, s, us) => Some(if h == 0 && mi == 0 && s == 0 && us == 0 {
            format!("{:04}-{:02}-{:02}", y, m, d)
        } else {
            format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, h, mi, s)
        }),
        Time(neg, d, h, m, s, _) => Some(format!("{}{:02}:{:02}:{:02}", if neg { "-" } else { "" }, d * 24 + h as u32, m, s)),
    }
}

fn s(c: &Cell) -> String {
    c.clone().unwrap_or_default()
}

fn sq(v: &str) -> String {
    format!("'{}'", v.replace('\'', "''").replace('\\', "\\\\"))
}

fn sq_pg(v: &str) -> String {
    format!("'{}'", v.replace('\'', "''"))
}

impl Client {
    fn pg(&mut self, cfg: &ConnConfig, db: &str) -> Result<&mut postgres::Client, String> {
        let Client::Pg { clients } = self else { unreachable!() };
        if !clients.contains_key(db) {
            let timeout = Duration::from_secs(if cfg.connect_timeout == 0 { 10 } else { cfg.connect_timeout });
            let client = postgres::Config::new()
                .host(&cfg.host)
                .port(cfg.port)
                .user(&cfg.user)
                .password(&cfg.password)
                .dbname(db)
                .connect_timeout(timeout)
                .application_name("DBZ")
                .connect(postgres::NoTls)
                .map_err(|e| pg_err(&e))?;
            clients.insert(db.to_string(), client);
        }
        Ok(clients.get_mut(db).unwrap())
    }

    /// Run one statement (or a few) and return every result set.
    fn query(&mut self, cfg: &ConnConfig, db: &str, sql: &str) -> Result<Vec<ResultSet>, String> {
        match self {
            Client::Sample(s) => {
                let mut srv = s.lock().unwrap();
                let mut out = vec![];
                for st in sqlgen::split_statements(sql) {
                    out.push(srv.exec(db, &st)?);
                }
                Ok(out)
            }
            Client::My { conn, current } => {
                use mysql::prelude::Queryable;
                if !db.is_empty() && current != db {
                    conn.query_drop(format!("USE {}", quote_ident(false, db))).map_err(|e| e.to_string())?;
                    *current = db.to_string();
                }
                let mut result = conn.query_iter(sql).map_err(|e| e.to_string())?;
                let mut out = vec![];
                while let Some(set) = result.iter() {
                    let columns: Vec<String> = set.columns().as_ref().iter().map(|c| c.name_str().to_string()).collect();
                    let affected = set.affected_rows();
                    let has_rows = !columns.is_empty();
                    let mut rows = vec![];
                    for row in set {
                        let row = row.map_err(|e| e.to_string())?;
                        rows.push(row.unwrap().into_iter().map(mysql_value).collect());
                    }
                    out.push(ResultSet { columns, rows, affected, statement: sql.to_string(), has_rows });
                }
                Ok(out)
            }
            Client::Pg { .. } => {
                let dbn = if db.is_empty() { if cfg.initial_db.is_empty() { "postgres".to_string() } else { cfg.initial_db.clone() } } else { db.to_string() };
                let client = self.pg(cfg, &dbn)?;
                let msgs = client.simple_query(sql).map_err(|e| pg_err(&e))?;
                let mut out = vec![];
                let mut cur: Option<ResultSet> = None;
                for m in msgs {
                    match m {
                        postgres::SimpleQueryMessage::RowDescription(cols) => {
                            cur = Some(ResultSet { columns: cols.iter().map(|c| c.name().to_string()).collect(), has_rows: true, statement: sql.to_string(), ..Default::default() });
                        }
                        postgres::SimpleQueryMessage::Row(r) => {
                            if cur.is_none() {
                                cur = Some(ResultSet { columns: r.columns().iter().map(|c| c.name().to_string()).collect(), has_rows: true, statement: sql.to_string(), ..Default::default() });
                            }
                            let set = cur.as_mut().unwrap();
                            set.rows.push((0..r.len()).map(|i| r.get(i).map(|v| v.to_string())).collect());
                        }
                        postgres::SimpleQueryMessage::CommandComplete(n) => {
                            let mut set = cur.take().unwrap_or_else(|| ResultSet { statement: sql.to_string(), ..Default::default() });
                            set.affected = n;
                            out.push(set);
                        }
                        _ => {}
                    }
                }
                if let Some(set) = cur.take() {
                    out.push(set);
                }
                Ok(out)
            }
        }
    }

    fn rows(&mut self, cfg: &ConnConfig, db: &str, sql: &str) -> Result<Vec<Row>, String> {
        Ok(self.query(cfg, db, sql)?.into_iter().next().map(|r| r.rows).unwrap_or_default())
    }

    fn databases(&mut self, cfg: &ConnConfig) -> Result<Vec<String>, String> {
        match self {
            Client::Sample(s) => Ok(s.lock().unwrap().databases()),
            Client::My { .. } => Ok(self.rows(cfg, "", "SHOW DATABASES")?.into_iter().map(|r| s(&r[0])).collect()),
            Client::Pg { .. } => Ok(self.rows(cfg, "", "SELECT datname FROM pg_database WHERE NOT datistemplate ORDER BY datname")?.into_iter().map(|r| s(&r[0])).collect()),
        }
    }

    fn tables(&mut self, cfg: &ConnConfig, db: &str) -> Result<Vec<TableInfo>, String> {
        match self {
            Client::Sample(s) => s.lock().unwrap().tables(db),
            Client::My { .. } => {
                let sql = format!(
                    "SELECT TABLE_NAME, TABLE_TYPE, TABLE_ROWS, DATA_LENGTH, ENGINE, CREATE_TIME, UPDATE_TIME, TABLE_COMMENT, TABLE_COLLATION FROM information_schema.TABLES WHERE TABLE_SCHEMA = {} ORDER BY TABLE_NAME",
                    sq(db)
                );
                Ok(self
                    .rows(cfg, db, &sql)?
                    .into_iter()
                    .map(|r| TableInfo {
                        name: s(&r[0]),
                        is_view: s(&r[1]).contains("VIEW"),
                        rows: r[2].as_ref().and_then(|v| v.parse().ok()),
                        data_length: r[3].as_ref().and_then(|v| v.parse().ok()),
                        engine: s(&r[4]),
                        created: s(&r[5]),
                        modified: s(&r[6]),
                        comment: s(&r[7]),
                        collation: s(&r[8]),
                    })
                    .collect())
            }
            Client::Pg { .. } => {
                let sql = "SELECT c.relname, n.nspname, c.relkind, c.reltuples::bigint, pg_total_relation_size(c.oid), obj_description(c.oid, 'pg_class'), pg_get_userbyid(c.relowner) \
                    FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                    WHERE c.relkind IN ('r','v','m','p') AND n.nspname NOT IN ('pg_catalog','information_schema') AND n.nspname NOT LIKE 'pg_toast%' \
                    ORDER BY n.nspname <> 'public', n.nspname, c.relname";
                Ok(self
                    .rows(cfg, db, sql)?
                    .into_iter()
                    .map(|r| {
                        let schema = s(&r[1]);
                        let name = if schema == "public" { s(&r[0]) } else { format!("{}.{}", schema, s(&r[0])) };
                        let kind = s(&r[2]);
                        TableInfo {
                            name,
                            is_view: kind == "v" || kind == "m",
                            rows: r[3].as_ref().and_then(|v| v.parse::<i64>().ok()).map(|v| v.max(0) as u64),
                            data_length: r[4].as_ref().and_then(|v| v.parse().ok()),
                            engine: format!("owner: {}", s(&r[6])),
                            created: String::new(),
                            modified: String::new(),
                            comment: s(&r[5]),
                            collation: String::new(),
                        }
                    })
                    .collect())
            }
        }
    }

    fn routines(&mut self, cfg: &ConnConfig, db: &str) -> Result<Vec<RoutineInfo>, String> {
        let sql = match self {
            Client::Sample(s) => return Ok(s.lock().unwrap().routines(db)),
            Client::My { .. } => format!("SELECT ROUTINE_NAME, ROUTINE_TYPE, DTD_IDENTIFIER FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA = {} ORDER BY ROUTINE_NAME", sq(db)),
            Client::Pg { .. } => "SELECT p.proname, CASE p.prokind WHEN 'p' THEN 'PROCEDURE' ELSE 'FUNCTION' END, pg_get_function_result(p.oid) FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace WHERE n.nspname NOT IN ('pg_catalog','information_schema') AND p.prokind IN ('f','p') ORDER BY 1".to_string(),
        };
        Ok(self.rows(cfg, db, &sql)?.into_iter().map(|r| RoutineInfo { name: s(&r[0]), kind: s(&r[1]), returns: s(&r[2]) }).collect())
    }

    fn columns(&mut self, cfg: &ConnConfig, db: &str, table: &str) -> Result<Vec<FieldDef>, String> {
        match self {
            Client::Sample(s) => s.lock().unwrap().columns(db, table),
            Client::My { .. } => {
                let sql = format!(
                    "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_KEY, COLUMN_DEFAULT, EXTRA, COLUMN_COMMENT FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {} ORDER BY ORDINAL_POSITION",
                    sq(db),
                    sq(table)
                );
                Ok(self
                    .rows(cfg, db, &sql)?
                    .into_iter()
                    .map(|r| {
                        let (ty, length, decimals) = FieldDef::split_type(&s(&r[1]));
                        FieldDef {
                            name: s(&r[0]),
                            ty,
                            length,
                            decimals,
                            not_null: s(&r[2]) == "NO",
                            key: s(&r[3]) == "PRI",
                            auto_inc: s(&r[5]).contains("auto_increment"),
                            default: s(&r[4]),
                            comment: s(&r[6]),
                            orig_name: Some(s(&r[0])),
                        }
                    })
                    .collect())
            }
            Client::Pg { .. } => {
                let reg = sq_pg(&quote_ident(true, table));
                let sql = format!(
                    "SELECT a.attname, format_type(a.atttypid, a.atttypmod), a.attnotnull, pg_get_expr(d.adbin, d.adrelid), col_description(a.attrelid, a.attnum), \
                     EXISTS (SELECT 1 FROM pg_index i WHERE i.indrelid = a.attrelid AND i.indisprimary AND a.attnum = ANY(i.indkey)) \
                     FROM pg_attribute a LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
                     WHERE a.attrelid = {}::regclass AND a.attnum > 0 AND NOT a.attisdropped ORDER BY a.attnum",
                    reg
                );
                Ok(self
                    .rows(cfg, db, &sql)?
                    .into_iter()
                    .map(|r| {
                        let (ty, length, decimals) = FieldDef::split_type(&s(&r[1]));
                        let default = s(&r[3]);
                        let auto = default.starts_with("nextval(");
                        FieldDef {
                            name: s(&r[0]),
                            ty,
                            length,
                            decimals,
                            not_null: s(&r[2]) == "t",
                            key: s(&r[5]) == "t",
                            auto_inc: auto,
                            default: if auto { String::new() } else { default },
                            comment: s(&r[4]),
                            orig_name: Some(s(&r[0])),
                        }
                    })
                    .collect())
            }
        }
    }

    fn ddl(&mut self, cfg: &ConnConfig, db: &str, table: &str, is_view: bool) -> Result<String, String> {
        match self {
            Client::Sample(s) => s.lock().unwrap().ddl(db, table),
            Client::My { .. } => {
                let sql = format!("SHOW CREATE {} {}", if is_view { "VIEW" } else { "TABLE" }, quote_ident(false, table));
                let rows = self.rows(cfg, db, &sql)?;
                Ok(rows.first().and_then(|r| r.get(1).cloned().flatten()).map(|d| format!("{};", d)).unwrap_or_default())
            }
            Client::Pg { .. } => {
                if is_view {
                    let rows = self.rows(cfg, db, &format!("SELECT pg_get_viewdef({}::regclass, true)", sq_pg(&quote_ident(true, table))))?;
                    let def = rows.first().map(|r| s(&r[0])).unwrap_or_default();
                    return Ok(format!("CREATE OR REPLACE VIEW {} AS\n{}", quote_ident(true, table), def));
                }
                let cols = self.columns(cfg, db, table)?;
                let comment = self
                    .rows(cfg, db, &format!("SELECT obj_description({}::regclass, 'pg_class')", sq_pg(&quote_ident(true, table))))?
                    .first()
                    .map(|r| s(&r[0]))
                    .unwrap_or_default();
                Ok(sqlgen::create_table(true, table, &cols, &comment).join(";\n") + ";")
            }
        }
    }

    fn users(&mut self, cfg: &ConnConfig) -> Result<ResultSet, String> {
        let sql = match self {
            Client::Sample(_) => {
                return Ok(ResultSet {
                    columns: vec!["User".into(), "Host".into(), "Note".into()],
                    rows: vec![vec![Some("sample".into()), Some("localhost".into()), Some("The built-in sample database has a single user".into())]],
                    has_rows: true,
                    ..Default::default()
                })
            }
            Client::My { .. } => "SELECT User, Host, plugin AS Plugin, account_locked AS Locked FROM mysql.user ORDER BY User, Host",
            Client::Pg { .. } => "SELECT rolname AS \"Role\", rolsuper AS \"Superuser\", rolcanlogin AS \"Can login\", rolcreatedb AS \"Create DB\", rolvaliduntil AS \"Valid until\" FROM pg_roles WHERE rolname NOT LIKE 'pg\\_%' ORDER BY 1",
        };
        self.query(cfg, "", sql).map(|v| v.into_iter().next().unwrap_or_default())
    }
}

fn pg_err(e: &postgres::Error) -> String {
    if let Some(db) = e.as_db_error() {
        format!("{}: {}", db.severity(), db.message())
    } else {
        e.to_string()
    }
}

// ---------------------------------------------------------------- file tasks

fn export(c: &mut Client, cfg: &ConnConfig, db: &str, table: &str, format: ExportFormat, path: &str, header: bool) -> Result<String, String> {
    let pg = cfg.kind.is_pg();
    let rs = c.query(cfg, db, &format!("SELECT * FROM {}", quote_ident(pg, table)))?.into_iter().next().unwrap_or_default();
    let mut out = String::new();
    match format {
        ExportFormat::Csv | ExportFormat::Tsv => {
            let sep = if format == ExportFormat::Csv { "," } else { "\t" };
            let esc = |v: &Cell| if format == ExportFormat::Csv { sqlgen::csv_escape(v) } else { v.clone().unwrap_or_default().replace(['\t', '\n'], " ") };
            if header {
                out.push_str(&rs.columns.iter().map(|c| esc(&Some(c.clone()))).collect::<Vec<_>>().join(sep));
                out.push_str("\r\n");
            }
            for r in &rs.rows {
                out.push_str(&r.iter().map(esc).collect::<Vec<_>>().join(sep));
                out.push_str("\r\n");
            }
        }
        ExportFormat::Json => {
            out.push_str("[\n");
            for (i, r) in rs.rows.iter().enumerate() {
                let fields: Vec<String> = rs
                    .columns
                    .iter()
                    .zip(r)
                    .map(|(c, v)| format!("\"{}\": {}", c.replace('"', "\\\""), v.as_ref().map(|x| sqlgen::json_escape(x)).unwrap_or_else(|| "null".into())))
                    .collect();
                out.push_str(&format!("  {{{}}}{}\n", fields.join(", "), if i + 1 < rs.rows.len() { "," } else { "" }));
            }
            out.push_str("]\n");
        }
        ExportFormat::Sql => {
            let cols: Vec<String> = rs.columns.iter().map(|c| quote_ident(pg, c)).collect();
            for r in &rs.rows {
                out.push_str(&format!(
                    "INSERT INTO {} ({}) VALUES ({});\n",
                    quote_ident(pg, table),
                    cols.join(", "),
                    r.iter().map(|v| sqlgen::literal(pg, v)).collect::<Vec<_>>().join(", ")
                ));
            }
        }
        ExportFormat::Html => {
            let h = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            out.push_str(&format!("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>{}</title></head><body>\n<table border=\"1\" cellspacing=\"0\" cellpadding=\"4\">\n", h(table)));
            if header {
                out.push_str("<tr>");
                for c in &rs.columns {
                    out.push_str(&format!("<th>{}</th>", h(c)));
                }
                out.push_str("</tr>\n");
            }
            for r in &rs.rows {
                out.push_str("<tr>");
                for v in r {
                    out.push_str(&format!("<td>{}</td>", v.as_deref().map(h).unwrap_or_default()));
                }
                out.push_str("</tr>\n");
            }
            out.push_str("</table>\n</body></html>\n");
        }
    }
    std::fs::write(path, out).map_err(|e| format!("Could not write {}: {}", path, e))?;
    Ok(format!("Exported {} record(s) from {} to {}", rs.rows.len(), table, path))
}

#[allow(clippy::too_many_arguments)]
fn import(c: &mut Client, cfg: &ConnConfig, db: &str, table: &str, path: &str, header: bool, delim: char, create: bool) -> Result<String, String> {
    let pg = cfg.kind.is_pg();
    let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read {}: {}", path, e))?;
    let text = text.trim_start_matches('\u{feff}');
    let mut rows = sqlgen::parse_csv(text, delim);
    if rows.is_empty() {
        return Err("The file has no rows".into());
    }
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let names: Vec<String> = if header { rows.remove(0) } else { (1..=width).map(|i| format!("column_{}", i)).collect() };
    if create {
        let fields: Vec<FieldDef> = names.iter().map(|n| FieldDef { name: n.clone(), ty: if pg { "text".into() } else { "varchar".into() }, length: if pg { String::new() } else { "255".into() }, ..Default::default() }).collect();
        for st in sqlgen::create_table(pg, table, &fields, "") {
            c.query(cfg, db, &st)?;
        }
    }
    let existing = c.columns(cfg, db, table)?;
    let mut targets = vec![];
    for (i, n) in names.iter().enumerate() {
        let col = if header { existing.iter().find(|f| f.name.eq_ignore_ascii_case(n.trim())) } else { existing.get(i) };
        if let Some(f) = col {
            targets.push((i, f.name.clone()));
        }
    }
    if targets.is_empty() {
        return Err("None of the file's columns match the table's fields".into());
    }
    let cols: Vec<String> = targets.iter().map(|(_, n)| quote_ident(pg, n)).collect();
    let mut done = 0;
    for chunk in rows.chunks(100) {
        let tuples: Vec<String> = chunk
            .iter()
            .map(|r| {
                format!(
                    "({})",
                    targets
                        .iter()
                        .map(|(i, _)| {
                            let v = r.get(*i).cloned().unwrap_or_default();
                            if v.is_empty() { "NULL".to_string() } else { sqlgen::literal(pg, &Some(v)) }
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect();
        let sql = format!("INSERT INTO {} ({}) VALUES {}", quote_ident(pg, table), cols.join(", "), tuples.join(", "));
        c.query(cfg, db, &sql).map_err(|e| format!("Stopped after {} record(s): {}", done, e))?;
        done += chunk.len();
    }
    Ok(format!("Imported {} record(s) into {} ({} matching field(s))", done, table, targets.len()))
}

fn dump(c: &mut Client, cfg: &ConnConfig, db: &str, path: &str, data: bool) -> Result<String, String> {
    let pg = cfg.kind.is_pg();
    let tables = c.tables(cfg, db)?;
    let mut out = format!(
        "-- DBZ SQL dump\n-- Connection: {} ({})\n-- Database: {}\n-- Date: {}\n\n",
        cfg.name,
        cfg.kind.label(),
        db,
        crate::demo::now_string()
    );
    if !pg {
        out.push_str("SET FOREIGN_KEY_CHECKS = 0;\n\n");
    }
    let mut n_rows = 0;
    for t in tables.iter().filter(|t| !t.is_view) {
        out.push_str(&format!("-- ----------------------------\n-- Table structure for {}\n-- ----------------------------\n", t.name));
        out.push_str(&format!("DROP TABLE IF EXISTS {};\n", quote_ident(pg, &t.name)));
        out.push_str(&c.ddl(cfg, db, &t.name, false)?);
        out.push_str("\n\n");
        if data {
            let rs = c.query(cfg, db, &format!("SELECT * FROM {}", quote_ident(pg, &t.name)))?.into_iter().next().unwrap_or_default();
            if !rs.rows.is_empty() {
                out.push_str(&format!("-- ----------------------------\n-- Records of {}\n-- ----------------------------\n", t.name));
                let cols: Vec<String> = rs.columns.iter().map(|c| quote_ident(pg, c)).collect();
                for r in &rs.rows {
                    out.push_str(&format!(
                        "INSERT INTO {} ({}) VALUES ({});\n",
                        quote_ident(pg, &t.name),
                        cols.join(", "),
                        r.iter().map(|v| sqlgen::literal(pg, v)).collect::<Vec<_>>().join(", ")
                    ));
                }
                n_rows += rs.rows.len();
                out.push('\n');
            }
        }
    }
    for t in tables.iter().filter(|t| t.is_view) {
        out.push_str(&format!("-- View structure for {}\n", t.name));
        if let Ok(d) = c.ddl(cfg, db, &t.name, true) {
            out.push_str(&d);
            out.push_str(";\n\n");
        }
    }
    if !pg {
        out.push_str("SET FOREIGN_KEY_CHECKS = 1;\n");
    }
    std::fs::write(path, out).map_err(|e| format!("Could not write {}: {}", path, e))?;
    Ok(format!("Dumped {} table(s){} from {} to {}", tables.iter().filter(|t| !t.is_view).count(), if data { format!(" and {} record(s)", n_rows) } else { String::new() }, db, path))
}

fn run_file(c: &mut Client, cfg: &ConnConfig, db: &str, path: &str, continue_on_error: bool) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read {}: {}", path, e))?;
    let stmts = sqlgen::split_statements(&text);
    let (mut ok, mut failed) = (0, 0);
    let mut first_err = String::new();
    for st in &stmts {
        if cfg.kind == DbKind::Sample && st.to_uppercase().starts_with("SET ") {
            ok += 1;
            continue;
        }
        match c.query(cfg, db, st) {
            Ok(_) => ok += 1,
            Err(e) => {
                failed += 1;
                if first_err.is_empty() {
                    first_err = format!("{} (in: {})", e, st.chars().take(80).collect::<String>());
                }
                if !continue_on_error {
                    return Err(format!("Stopped after {} statement(s). {}", ok, first_err));
                }
            }
        }
    }
    if failed > 0 {
        Ok(format!("Ran {} statement(s), {} failed. First error: {}", ok, failed, first_err))
    } else {
        Ok(format!("Ran {} statement(s) from {} without errors", ok, path))
    }
}
