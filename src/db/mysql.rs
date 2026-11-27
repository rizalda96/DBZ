//! MySQL / MariaDB driver.

use mysql::prelude::Queryable;

use super::driver::{number, text, Driver};
use super::{mysql_parts, DbError};
use crate::domain::{Cell, ConnConfig, FieldDef, IndexChoices, ResultSet, RoutineInfo, Row, SchemaName, TableInfo, TableParts};
use crate::sql::{Dialect, MySqlDialect};

static DIALECT: MySqlDialect = MySqlDialect;

const TABLES_SQL: &str = "SELECT TABLE_NAME, TABLE_TYPE, TABLE_ROWS, DATA_LENGTH, ENGINE, CREATE_TIME, UPDATE_TIME, TABLE_COMMENT, TABLE_COLLATION \
    FROM information_schema.TABLES WHERE TABLE_SCHEMA = {db} ORDER BY TABLE_NAME";
const COLUMNS_SQL: &str = "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_KEY, COLUMN_DEFAULT, EXTRA, COLUMN_COMMENT \
    FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = {db} AND TABLE_NAME = {table} ORDER BY ORDINAL_POSITION";
const ROUTINES_SQL: &str = "SELECT ROUTINE_NAME, ROUTINE_TYPE, DTD_IDENTIFIER FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA = {db} ORDER BY ROUTINE_NAME";
const USERS_SQL: &str = "SELECT User, Host, plugin AS Plugin, account_locked AS Locked FROM mysql.user ORDER BY User, Host";
/// Base tables of every user database: what a foreign key can reference.
const REF_TABLES_SQL: &str = "SELECT TABLE_SCHEMA, TABLE_NAME FROM information_schema.TABLES WHERE TABLE_TYPE = 'BASE TABLE' \
    AND TABLE_SCHEMA NOT IN ('mysql', 'sys', 'performance_schema', 'information_schema') ORDER BY TABLE_SCHEMA, TABLE_NAME";

pub struct MySqlDriver {
    conn: mysql::Conn,
    current: String,
}

fn query_err(e: mysql::Error) -> DbError {
    DbError::Query(e.to_string())
}

impl MySqlDriver {
    pub fn connect(cfg: &ConnConfig) -> Result<Self, DbError> {
        let db_name = (!cfg.initial_db.is_empty()).then(|| cfg.initial_db.clone());
        let opts = mysql::OptsBuilder::new()
            .ip_or_hostname(Some(cfg.host.clone()))
            .tcp_port(cfg.port)
            .user(Some(cfg.user.clone()))
            .pass(Some(cfg.password.clone()))
            .db_name(db_name)
            .tcp_connect_timeout(Some(cfg.timeout()));
        let conn = mysql::Conn::new(opts).map_err(|e| DbError::Connect(e.to_string()))?;
        Ok(MySqlDriver { conn, current: cfg.initial_db.clone() })
    }

    fn use_db(&mut self, db: &str) -> Result<(), DbError> {
        if !db.is_empty() && self.current != db {
            self.conn.query_drop(format!("USE {}", DIALECT.quote_ident(db))).map_err(query_err)?;
            self.current = db.to_string();
        }
        Ok(())
    }
}

impl Driver for MySqlDriver {
    fn query(&mut self, db: &str, sql: &str) -> Result<Vec<ResultSet>, DbError> {
        self.use_db(db)?;
        let mut result = self.conn.query_iter(sql).map_err(query_err)?;
        let mut out = vec![];
        while let Some(set) = result.iter() {
            let columns: Vec<String> = set.columns().as_ref().iter().map(|c| c.name_str().to_string()).collect();
            let affected = set.affected_rows();
            let has_rows = !columns.is_empty();
            // `mysql::Row::unwrap` turns a row into its values; it does not panic.
            let rows = set.map(|row| row.map(|r| r.unwrap().into_iter().map(cell).collect()).map_err(query_err)).collect::<Result<Vec<Row>, _>>()?;
            out.push(ResultSet { columns, rows, affected, statement: sql.to_string(), has_rows });
        }
        Ok(out)
    }

    fn databases(&mut self) -> Result<Vec<String>, DbError> {
        Ok(self.rows("", "SHOW DATABASES")?.iter().map(|r| text(&r[0])).collect())
    }

    fn tables(&mut self, db: &str) -> Result<Vec<TableInfo>, DbError> {
        let sql = TABLES_SQL.replace("{db}", &DIALECT.str_literal(db));
        Ok(self.rows(db, &sql)?.iter().map(table_info).collect())
    }

    fn routines(&mut self, db: &str) -> Result<Vec<RoutineInfo>, DbError> {
        let sql = ROUTINES_SQL.replace("{db}", &DIALECT.str_literal(db));
        Ok(self.rows(db, &sql)?.iter().map(|r| RoutineInfo { name: text(&r[0]), kind: text(&r[1]), returns: text(&r[2]), ..Default::default() }).collect())
    }

    fn columns(&mut self, db: &str, table: &str) -> Result<Vec<FieldDef>, DbError> {
        let sql = COLUMNS_SQL.replace("{db}", &DIALECT.str_literal(db)).replace("{table}", &DIALECT.str_literal(table));
        Ok(self.rows(db, &sql)?.iter().map(field_def).collect())
    }

    fn ddl(&mut self, db: &str, table: &str, is_view: bool) -> Result<String, DbError> {
        let sql = format!("SHOW CREATE {} {}", if is_view { "VIEW" } else { "TABLE" }, DIALECT.quote_ident(table));
        let rows = self.rows(db, &sql)?;
        Ok(rows.first().and_then(|r| r.get(1).cloned().flatten()).map(|d| format!("{};", d)).unwrap_or_default())
    }

    fn routine_ddl(&mut self, db: &str, routine: &RoutineInfo) -> Result<String, DbError> {
        let sql = format!("SHOW CREATE {} {}", routine_keyword(routine), DIALECT.quote_ident(&routine.name));
        let rows = self.rows(db, &sql)?;
        // Columns: name, sql_mode, the CREATE statement, …
        let def = rows.first().and_then(|r| r.get(2).cloned().flatten());
        def.map(|d| format!("{};", d)).ok_or_else(|| DbError::Query(format!("No definition found for {}", routine.name)))
    }

    fn table_parts(&mut self, db: &str, table: &str) -> Result<TableParts, DbError> {
        let lits = (DIALECT.str_literal(db), DIALECT.str_literal(table));
        mysql_parts::table_parts(self, db, (&lits.0, &lits.1))
    }

    /// Only the tables a foreign key can reference; MySQL index columns have no
    /// collations or operator classes to offer.
    fn index_choices(&mut self, db: &str) -> Result<IndexChoices, DbError> {
        let tables = self.rows(db, REF_TABLES_SQL)?.iter().map(|r| SchemaName { schema: text(&r[0]), name: text(&r[1]) }).collect();
        Ok(IndexChoices { tables, ..Default::default() })
    }

    fn ref_columns(&mut self, db: &str, schema: &str, table: &str) -> Result<Vec<String>, DbError> {
        let in_db = if schema.trim().is_empty() { db } else { schema.trim() };
        Ok(self.columns(in_db, table)?.into_iter().map(|f| f.name).collect())
    }

    fn users(&mut self) -> Result<ResultSet, DbError> {
        self.first_set("", USERS_SQL)
    }

    fn dialect(&self) -> &'static dyn Dialect {
        &DIALECT
    }
}

fn table_info(r: &Row) -> TableInfo {
    TableInfo {
        name: text(&r[0]),
        is_view: text(&r[1]).contains("VIEW"),
        rows: number(&r[2]),
        data_length: number(&r[3]),
        engine: text(&r[4]),
        created: text(&r[5]),
        modified: text(&r[6]),
        comment: text(&r[7]),
        collation: text(&r[8]),
        ..Default::default()
    }
}

fn field_def(r: &Row) -> FieldDef {
    let (ty, length, decimals) = FieldDef::split_type(&text(&r[1]));
    FieldDef {
        name: text(&r[0]),
        ty,
        length,
        decimals,
        not_null: text(&r[2]) == "NO",
        key: text(&r[3]) == "PRI",
        auto_inc: text(&r[5]).contains("auto_increment"),
        default: text(&r[4]),
        comment: text(&r[6]),
        orig_name: None,
    }
    .loaded()
}

fn cell(v: mysql::Value) -> Cell {
    use mysql::Value::*;
    match v {
        NULL => None,
        Bytes(b) => Some(String::from_utf8(b).unwrap_or_else(|e| format!("(BLOB) {} bytes", e.as_bytes().len()))),
        Int(i) => Some(i.to_string()),
        UInt(u) => Some(u.to_string()),
        Float(f) => Some(f.to_string()),
        Double(d) => Some(d.to_string()),
        Date(y, m, d, 0, 0, 0, 0) => Some(format!("{:04}-{:02}-{:02}", y, m, d)),
        Date(y, m, d, h, mi, s, _) => Some(format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, h, mi, s)),
        Time(neg, d, h, m, s, _) => Some(format!("{}{:02}:{:02}:{:02}", if neg { "-" } else { "" }, d * 24 + h as u32, m, s)),
    }
}

/// FUNCTION or PROCEDURE, as `SHOW CREATE` wants it.
fn routine_keyword(r: &RoutineInfo) -> &'static str {
    if r.is_procedure() {
        "PROCEDURE"
    } else {
        "FUNCTION"
    }
}
