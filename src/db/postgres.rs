//! PostgreSQL driver. Postgres connections are per database, so one client is kept
//! open for each database used.

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use postgres::SimpleQueryMessage;

use super::driver::{number, text, Driver};
use super::{postgres_parts, DbError};
use crate::domain::{ConnConfig, FieldDef, IndexChoices, ResultSet, RoutineInfo, Row, TableInfo, TableParts};
use crate::sql::{Dialect, PgDialect};

static DIALECT: PgDialect = PgDialect;

const MAINTENANCE_DB: &str = "postgres";
const DATABASES_SQL: &str = "SELECT datname FROM pg_database WHERE NOT datistemplate ORDER BY datname";
const TABLES_SQL: &str = "SELECT c.relname, n.nspname, c.relkind, c.reltuples::bigint, pg_total_relation_size(c.oid), obj_description(c.oid, 'pg_class'), pg_get_userbyid(c.relowner) \
    FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
    WHERE c.relkind IN ('r','v','m','p') AND n.nspname NOT IN ('pg_catalog','information_schema') AND n.nspname NOT LIKE 'pg_toast%' \
    ORDER BY n.nspname <> 'public', n.nspname, c.relname";
const ROUTINES_SQL: &str = "SELECT p.proname, CASE p.prokind WHEN 'p' THEN 'PROCEDURE' ELSE 'FUNCTION' END, pg_get_function_result(p.oid), n.nspname \
    FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
    WHERE n.nspname NOT IN ('pg_catalog','information_schema') AND p.prokind IN ('f','p') ORDER BY n.nspname <> 'public', n.nspname, 1";
/// Every overload of one function or procedure name in one schema.
const ROUTINE_DDL_SQL: &str = "SELECT pg_get_functiondef(p.oid) FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \n    WHERE n.nspname = {schema} AND p.proname = {name} AND p.prokind IN ('f','p') ORDER BY p.oid";
const SCHEMAS_SQL: &str = "SELECT nspname FROM pg_namespace WHERE nspname <> 'information_schema' AND nspname !~ '^pg_' ORDER BY nspname <> 'public', nspname";
const COLUMNS_SQL: &str = "SELECT a.attname, format_type(a.atttypid, a.atttypmod), a.attnotnull, pg_get_expr(d.adbin, d.adrelid), col_description(a.attrelid, a.attnum), \
     EXISTS (SELECT 1 FROM pg_index i WHERE i.indrelid = a.attrelid AND i.indisprimary AND a.attnum = ANY(i.indkey)) \
     FROM pg_attribute a LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
     WHERE a.attrelid = {reg}::regclass AND a.attnum > 0 AND NOT a.attisdropped ORDER BY a.attnum";
const USERS_SQL: &str = "SELECT rolname AS \"Role\", rolsuper AS \"Superuser\", rolcanlogin AS \"Can login\", rolcreatedb AS \"Create DB\", rolvaliduntil AS \"Valid until\" \
    FROM pg_roles WHERE rolname NOT LIKE 'pg\\_%' ORDER BY 1";

pub struct PgDriver {
    cfg: ConnConfig,
    clients: HashMap<String, postgres::Client>,
}

fn pg_err(e: &postgres::Error) -> String {
    match e.as_db_error() {
        Some(db) => format!("{}: {}", db.severity(), db.message()),
        None => e.to_string(),
    }
}

impl PgDriver {
    pub fn connect(cfg: &ConnConfig) -> Result<Self, DbError> {
        let mut driver = PgDriver { cfg: cfg.clone(), clients: HashMap::new() };
        let db = driver.default_db();
        driver.client(&db)?;
        Ok(driver)
    }

    fn default_db(&self) -> String {
        if self.cfg.initial_db.is_empty() { MAINTENANCE_DB.to_string() } else { self.cfg.initial_db.clone() }
    }

    fn client(&mut self, db: &str) -> Result<&mut postgres::Client, DbError> {
        let cfg = &self.cfg;
        match self.clients.entry(db.to_string()) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => Ok(e.insert(open_client(cfg, db)?)),
        }
    }

    /// `'"schema"."table"'`, ready for `::regclass`.
    fn regclass(table: &str) -> String {
        DIALECT.str_literal(&DIALECT.quote_ident(table))
    }
}

fn open_client(cfg: &ConnConfig, db: &str) -> Result<postgres::Client, DbError> {
    postgres::Config::new()
        .host(&cfg.host)
        .port(cfg.port)
        .user(&cfg.user)
        .password(&cfg.password)
        .dbname(db)
        .connect_timeout(cfg.timeout())
        .application_name("DBZ")
        .connect(postgres::NoTls)
        .map_err(|e| DbError::Connect(pg_err(&e)))
}

impl Driver for PgDriver {
    fn query(&mut self, db: &str, sql: &str) -> Result<Vec<ResultSet>, DbError> {
        let db = if db.is_empty() { self.default_db() } else { db.to_string() };
        let msgs = self.client(&db)?.simple_query(sql).map_err(|e| DbError::Query(pg_err(&e)))?;
        Ok(collect_sets(msgs, sql))
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
        let sql = COLUMNS_SQL.replace("{reg}", &Self::regclass(table));
        Ok(self.rows(db, &sql)?.iter().map(field_def).collect())
    }

    fn routine_ddl(&mut self, db: &str, routine: &RoutineInfo) -> Result<String, DbError> {
        let bare = routine.bare_name();
        let sql = ROUTINE_DDL_SQL.replace("{schema}", &DIALECT.str_literal(&routine.schema)).replace("{name}", &DIALECT.str_literal(bare));
        let defs: Vec<String> = self.rows(db, &sql)?.iter().map(|r| format!("{};", text(&r[0]).trim_end())).collect();
        if defs.is_empty() {
            return Err(DbError::Query(format!("No definition found for {}", routine.name)));
        }
        Ok(defs.join("

"))
    }

    fn ddl(&mut self, db: &str, table: &str, is_view: bool) -> Result<String, DbError> {
        let reg = Self::regclass(table);
        if is_view {
            let rows = self.rows(db, &format!("SELECT pg_get_viewdef({0}::regclass, true), relkind FROM pg_class WHERE oid = {0}::regclass", reg))?;
            let (def, kind) = rows.first().map(|r| (text(&r[0]), text(&r[1]))).unwrap_or_default();
            let create = if kind == "m" { "CREATE MATERIALIZED VIEW" } else { "CREATE OR REPLACE VIEW" };
            return Ok(format!("{} {} AS\n{}", create, DIALECT.quote_ident(table), def));
        }
        let cols = self.columns(db, table)?;
        let rows = self.rows(db, &format!("SELECT obj_description({}::regclass, 'pg_class')", reg))?;
        let comment = rows.first().map(|r| text(&r[0])).unwrap_or_default();
        Ok(DIALECT.create_table(table, &cols, &comment).join(";\n") + ";")
    }

    fn table_parts(&mut self, db: &str, table: &str) -> Result<TableParts, DbError> {
        postgres_parts::table_parts(self, db, &Self::regclass(table))
    }

    fn index_choices(&mut self, db: &str) -> Result<IndexChoices, DbError> {
        postgres_parts::index_choices(self, db)
    }

    fn ref_columns(&mut self, db: &str, schema: &str, table: &str) -> Result<Vec<String>, DbError> {
        let name = match schema.trim() {
            "" => table.to_string(),
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

/// Groups the simple-query protocol messages into result sets.
fn collect_sets(msgs: Vec<SimpleQueryMessage>, sql: &str) -> Vec<ResultSet> {
    let new_set = |columns: Vec<String>| ResultSet { columns, has_rows: true, statement: sql.to_string(), ..Default::default() };
    let mut out = vec![];
    let mut cur: Option<ResultSet> = None;
    for m in msgs {
        match m {
            SimpleQueryMessage::RowDescription(cols) => cur = Some(new_set(cols.iter().map(|c| c.name().to_string()).collect())),
            SimpleQueryMessage::Row(r) => {
                let set = cur.get_or_insert_with(|| new_set(r.columns().iter().map(|c| c.name().to_string()).collect()));
                set.rows.push((0..r.len()).map(|i| r.get(i).map(str::to_string)).collect());
            }
            SimpleQueryMessage::CommandComplete(n) => {
                let mut set = cur.take().unwrap_or_else(|| ResultSet { statement: sql.to_string(), ..Default::default() });
                set.affected = n;
                out.push(set);
            }
            _ => {}
        }
    }
    out.extend(cur);
    out
}

/// `name` as other code addresses it: bare in `public`, "schema.name" elsewhere.
fn qualified(schema: &str, name: String) -> String {
    if schema == "public" { name } else { format!("{}.{}", schema, name) }
}

fn table_info(r: &Row) -> TableInfo {
    let schema = text(&r[1]);
    let kind = text(&r[2]);
    TableInfo {
        name: qualified(&schema, text(&r[0])),
        schema,
        is_view: kind == "v" || kind == "m",
        materialized: kind == "m",
        rows: number::<i64>(&r[3]).map(|v| v.max(0) as u64),
        data_length: number(&r[4]),
        engine: format!("owner: {}", text(&r[6])),
        comment: text(&r[5]),
        ..Default::default()
    }
}

fn routine_info(r: &Row) -> RoutineInfo {
    let schema = text(&r[3]);
    RoutineInfo { name: qualified(&schema, text(&r[0])), schema, kind: text(&r[1]), returns: text(&r[2]) }
}

fn field_def(r: &Row) -> FieldDef {
    let (ty, length, decimals) = FieldDef::split_type(&text(&r[1]));
    let default = text(&r[3]);
    let auto_inc = default.starts_with("nextval(");
    FieldDef {
        name: text(&r[0]),
        ty,
        length,
        decimals,
        not_null: text(&r[2]) == "t",
        key: text(&r[5]) == "t",
        auto_inc,
        default: if auto_inc { String::new() } else { default },
        comment: text(&r[4]),
        orig_name: None,
    }
    .loaded()
}
