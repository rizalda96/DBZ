//! The in-memory server: databases, tables and the statement dispatcher.

use super::parser::{field_index, Parser, Res};
use super::tokens::tokenize;
use crate::db::DbError;
use crate::domain::{FieldDef, ResultSet, Row, RoutineInfo, TableInfo};
use crate::sql::{Dialect, MySqlDialect};

/// Size of one storage page, used to fake a "Data Length".
const PAGE: u64 = 16_384;

pub struct SampleTable {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub rows: Vec<Row>,
    pub comment: String,
    pub auto_inc: u64,
    pub created: String,
    pub modified: String,
}

pub struct SampleDb {
    pub name: String,
    pub tables: Vec<SampleTable>,
}

pub struct SampleServer {
    pub dbs: Vec<SampleDb>,
}

impl SampleTable {
    pub(super) fn col_index(&self, name: &str) -> Res<usize> {
        self.fields
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| DbError::query(format!("Unknown column '{}' in '{}'", name, self.name)))
    }

    /// A row filled with each field's default.
    pub(super) fn default_row(&self) -> Row {
        self.fields.iter().map(default_cell).collect()
    }

    fn info(&self) -> TableInfo {
        let size: usize = self.rows.iter().flat_map(|r| r.iter()).map(|c| c.as_ref().map(|s| s.len()).unwrap_or(1)).sum();
        TableInfo {
            name: self.name.clone(),
            is_view: false,
            rows: Some(self.rows.len() as u64),
            data_length: Some((size as u64).div_ceil(PAGE) * PAGE),
            engine: "Memory".into(),
            created: self.created.clone(),
            modified: self.modified.clone(),
            comment: self.comment.clone(),
            collation: "utf8mb4_general_ci".into(),
        }
    }
}

/// A field's DEFAULT as a cell value (`NULL` and empty mean NULL).
pub(super) fn default_cell(f: &FieldDef) -> Option<String> {
    (!f.default.is_empty() && f.default != "NULL").then(|| f.default.clone())
}

impl SampleServer {
    pub(super) fn db_mut(&mut self, db: &str) -> Res<&mut SampleDb> {
        self.dbs.iter_mut().find(|d| d.name.eq_ignore_ascii_case(db)).ok_or_else(|| unknown_db(db))
    }

    pub(super) fn db(&self, db: &str) -> Res<&SampleDb> {
        self.dbs.iter().find(|d| d.name.eq_ignore_ascii_case(db)).ok_or_else(|| unknown_db(db))
    }

    pub(super) fn table_mut(&mut self, db: &str, t: &str) -> Res<&mut SampleTable> {
        let d = self.db_mut(db)?;
        d.tables.iter_mut().find(|x| x.name.eq_ignore_ascii_case(t)).ok_or_else(|| missing_table(db, t))
    }

    pub fn table(&self, db: &str, t: &str) -> Res<&SampleTable> {
        let d = self.db(db)?;
        d.tables.iter().find(|x| x.name.eq_ignore_ascii_case(t)).ok_or_else(|| missing_table(db, t))
    }

    pub fn databases(&self) -> Vec<String> {
        self.dbs.iter().map(|d| d.name.clone()).collect()
    }

    pub fn tables(&self, db: &str) -> Res<Vec<TableInfo>> {
        Ok(self.db(db)?.tables.iter().map(SampleTable::info).collect())
    }

    pub fn routines(&self, _db: &str) -> Vec<RoutineInfo> {
        vec![]
    }

    pub fn columns(&self, db: &str, t: &str) -> Res<Vec<FieldDef>> {
        Ok(self.table(db, t)?.fields.iter().cloned().map(FieldDef::loaded).collect())
    }

    pub fn ddl(&self, db: &str, t: &str) -> Res<String> {
        let tb = self.table(db, t)?;
        Ok(MySqlDialect.create_table(&tb.name, &tb.fields, &tb.comment).join(";\n") + ";")
    }

    /// Runs one statement.
    pub fn exec(&mut self, db: &str, sql: &str) -> Res<ResultSet> {
        let mut p = Parser::new(tokenize(sql)?);
        let mut rs = ResultSet { statement: sql.to_string(), ..Default::default() };
        if p.kw("SELECT") {
            self.select(db, &mut p, &mut rs)?;
        } else if p.kw("SHOW") {
            self.show(db, &mut p, &mut rs)?;
        } else if p.kw("DESCRIBE") || p.kw("DESC") {
            let t = p.ident()?;
            return self.exec(db, &format!("SHOW COLUMNS FROM `{}`", t));
        } else if p.kw("USE") {
            p.ident()?;
        } else {
            rs.affected = self.exec_change(db, &mut p)?;
        }
        Ok(rs)
    }

    /// Statements that change data or schema; returns the affected row count.
    fn exec_change(&mut self, db: &str, p: &mut Parser) -> Res<u64> {
        if p.kw("INSERT") {
            self.insert(db, p)
        } else if p.kw("UPDATE") {
            self.update(db, p)
        } else if p.kw("DELETE") {
            self.delete(db, p)
        } else if p.kw("TRUNCATE") {
            self.truncate(db, p).map(|_| 0)
        } else if p.kw("CREATE") {
            self.create(db, p).map(|_| 0)
        } else if p.kw("DROP") {
            self.drop(db, p).map(|_| 0)
        } else if p.kw("ALTER") {
            self.alter(db, p).map(|_| 0)
        } else {
            Err(DbError::query(format!(
                "The sample database doesn't understand {} — try SELECT, INSERT, UPDATE, DELETE, CREATE/ALTER/DROP TABLE or SHOW",
                p.near()
            )))
        }
    }

    /// SHOW DATABASES | TABLES | CREATE TABLE t | COLUMNS FROM t
    fn show(&self, db: &str, p: &mut Parser, rs: &mut ResultSet) -> Res<()> {
        rs.has_rows = true;
        if p.kw("DATABASES") {
            rs.columns = vec!["Database".into()];
            rs.rows = self.databases().into_iter().map(|d| vec![Some(d)]).collect();
        } else if p.kw("TABLES") {
            rs.columns = vec![format!("Tables_in_{}", db)];
            rs.rows = self.db(db)?.tables.iter().map(|t| vec![Some(t.name.clone())]).collect();
        } else if p.kw("CREATE") {
            p.expect_kw("TABLE")?;
            let t = p.ident()?;
            rs.columns = vec!["Table".into(), "Create Table".into()];
            rs.rows = vec![vec![Some(t.clone()), Some(self.ddl(db, &t)?)]];
        } else if p.kw("COLUMNS") || p.kw("FIELDS") {
            p.expect_kw("FROM")?;
            let t = p.ident()?;
            rs.columns = ["Field", "Type", "Null", "Key", "Default", "Extra"].iter().map(|s| s.to_string()).collect();
            rs.rows = self.table(db, &t)?.fields.iter().map(describe_row).collect();
        } else {
            return Err(DbError::query("The sample database supports SHOW DATABASES, SHOW TABLES, SHOW COLUMNS and SHOW CREATE TABLE"));
        }
        Ok(())
    }
}

fn describe_row(f: &FieldDef) -> Row {
    vec![
        Some(f.name.clone()),
        Some(f.type_text()),
        Some(if f.not_null { "NO" } else { "YES" }.into()),
        Some(if f.key { "PRI" } else { "" }.into()),
        (!f.default.is_empty()).then(|| f.default.clone()),
        Some(if f.auto_inc { "auto_increment" } else { "" }.into()),
    ]
}

fn unknown_db(db: &str) -> DbError {
    DbError::query(format!("Unknown database '{}'", db))
}

fn missing_table(db: &str, t: &str) -> DbError {
    DbError::query(format!("Table '{}.{}' doesn't exist", db, t))
}

/// Position of a field by name, for ALTER/CREATE; re-exported for the DDL module.
pub(super) fn find_field(fields: &[FieldDef], name: &str) -> Res<usize> {
    field_index(fields, name, "Unknown column")
}
