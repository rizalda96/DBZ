//! CREATE, DROP and ALTER for databases and tables.

use super::coldef::{parse_coldef, set_primary_key};
use super::parser::{Parser, Res};
use super::server::{default_cell, find_field, SampleDb, SampleServer, SampleTable};
use crate::db::DbError;
use crate::domain::{FieldDef, Row};
use crate::shared::now_string;

impl SampleServer {
    pub(super) fn create(&mut self, db: &str, p: &mut Parser) -> Res<()> {
        if p.kw("DATABASE") || p.kw("SCHEMA") {
            let name = p.ident()?;
            if self.dbs.iter().any(|d| d.name.eq_ignore_ascii_case(&name)) {
                return Err(DbError::query(format!("Can't create database '{}'; database exists", name)));
            }
            self.dbs.push(SampleDb { name, tables: vec![] });
            return Ok(());
        }
        p.expect_kw("TABLE")?;
        let if_not = p.kw("IF");
        if if_not {
            p.expect_kw("NOT")?;
            p.expect_kw("EXISTS")?;
        }
        let name = p.ident()?;
        let fields = table_body(p)?;
        let comment = table_options(p)?;
        let d = self.db_mut(db)?;
        if d.tables.iter().any(|t| t.name.eq_ignore_ascii_case(&name)) {
            return if if_not { Ok(()) } else { Err(DbError::query(format!("Table '{}' already exists", name))) };
        }
        let now = now_string();
        d.tables.push(SampleTable { name, fields, rows: vec![], comment, auto_inc: 0, created: now.clone(), modified: now });
        d.tables.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(())
    }

    pub(super) fn drop(&mut self, db: &str, p: &mut Parser) -> Res<()> {
        if p.kw("DATABASE") || p.kw("SCHEMA") {
            let name = p.ident()?;
            let before = self.dbs.len();
            self.dbs.retain(|d| !d.name.eq_ignore_ascii_case(&name));
            if before == self.dbs.len() {
                return Err(DbError::query(format!("Can't drop database '{}'; database doesn't exist", name)));
            }
            return Ok(());
        }
        p.expect_kw("TABLE")?;
        let if_exists = p.kw("IF");
        if if_exists {
            p.expect_kw("EXISTS")?;
        }
        for name in p.comma_list(|p| p.ident())? {
            let d = self.db_mut(db)?;
            let before = d.tables.len();
            d.tables.retain(|t| !t.name.eq_ignore_ascii_case(&name));
            if before == d.tables.len() && !if_exists {
                return Err(DbError::query(format!("Unknown table '{}'", name)));
            }
        }
        Ok(())
    }

    pub(super) fn alter(&mut self, db: &str, p: &mut Parser) -> Res<()> {
        p.expect_kw("TABLE")?;
        let t = p.ident()?;
        let tb = self.table_mut(db, &t)?;
        let mut plan = AlterPlan::new(tb);
        p.comma_list(|p| plan.action(p))?;
        p.done()?;
        tb.rows = tb.rows.iter().map(|r| plan.migrate(r)).collect();
        tb.fields = plan.fields;
        tb.comment = plan.comment;
        tb.modified = now_string();
        if let Some(n) = plan.rename_to {
            self.table_mut(db, &t)?.name = n;
            self.db_mut(db)?.tables.sort_by(|a, b| a.name.cmp(&b.name));
        }
        Ok(())
    }
}

/// `( coldef | PRIMARY KEY (...) | KEY name (...) , ... )`
fn table_body(p: &mut Parser) -> Res<Vec<FieldDef>> {
    p.expect_sym("(")?;
    let mut fields: Vec<FieldDef> = vec![];
    loop {
        if p.kw("PRIMARY") {
            p.expect_kw("KEY")?;
            let keys = p.ident_list()?;
            set_primary_key(&mut fields, &keys)?;
        } else if p.kw("KEY") || p.kw("INDEX") || p.kw("UNIQUE") {
            skip_index(p)?;
        } else {
            fields.push(parse_coldef(p)?);
        }
        if !p.sym(",") {
            break;
        }
    }
    p.expect_sym(")")?;
    Ok(fields)
}

fn skip_index(p: &mut Parser) -> Res<()> {
    p.kw("KEY");
    if !p.is_sym("(") {
        p.ident()?;
    }
    p.ident_list()?;
    Ok(())
}

/// Table options after the column list; only COMMENT is kept.
fn table_options(p: &mut Parser) -> Res<String> {
    let mut comment = String::new();
    while !p.at_end() && !p.sym(";") {
        if p.kw("COMMENT") {
            p.sym("=");
            comment = p.literal()?.unwrap_or_default();
        } else {
            p.i += 1;
        }
    }
    Ok(comment)
}

/// The new shape of a table being altered. `map[i]` is the old column that new
/// column `i` takes its data from (None = a new column).
struct AlterPlan {
    fields: Vec<FieldDef>,
    map: Vec<Option<usize>>,
    comment: String,
    rename_to: Option<String>,
}

impl AlterPlan {
    fn new(tb: &SampleTable) -> Self {
        AlterPlan { fields: tb.fields.clone(), map: (0..tb.fields.len()).map(Some).collect(), comment: tb.comment.clone(), rename_to: None }
    }

    fn action(&mut self, p: &mut Parser) -> Res<()> {
        if p.kw("ADD") {
            self.add(p)
        } else if p.kw("DROP") {
            self.drop(p)
        } else if p.kw("CHANGE") {
            self.change(p, true)
        } else if p.kw("MODIFY") {
            self.change(p, false)
        } else if p.kw("RENAME") {
            self.rename(p)
        } else if p.kw("COMMENT") {
            p.sym("=");
            self.comment = p.literal()?.unwrap_or_default();
            Ok(())
        } else {
            Err(DbError::query(format!("Unsupported ALTER TABLE action near {}", p.near())))
        }
    }

    fn add(&mut self, p: &mut Parser) -> Res<()> {
        if p.kw("PRIMARY") {
            p.expect_kw("KEY")?;
            let keys = p.ident_list()?;
            return set_primary_key(&mut self.fields, &keys);
        }
        p.kw("COLUMN");
        let f = parse_coldef(p)?;
        let pos = p.position(&self.fields)?.unwrap_or(self.fields.len());
        self.fields.insert(pos, f);
        self.map.insert(pos, None);
        Ok(())
    }

    fn drop(&mut self, p: &mut Parser) -> Res<()> {
        if p.kw("PRIMARY") {
            p.expect_kw("KEY")?;
            self.fields.iter_mut().for_each(|f| f.key = false);
            return Ok(());
        }
        p.kw("COLUMN");
        let c = p.ident()?;
        let i = self
            .fields
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(&c))
            .ok_or_else(|| DbError::query(format!("Can't DROP '{}'; check that column exists", c)))?;
        self.fields.remove(i);
        self.map.remove(i);
        Ok(())
    }

    /// CHANGE old new-def / MODIFY def, optionally with FIRST / AFTER.
    fn change(&mut self, p: &mut Parser, renames: bool) -> Res<()> {
        p.kw("COLUMN");
        let old = if renames { Some(p.ident()?) } else { None };
        let mut f = parse_coldef(p)?;
        let old = old.unwrap_or_else(|| f.name.clone());
        let i = find_field(&self.fields, &old)?;
        f.key = self.fields[i].key || f.key;
        let src = self.map.remove(i);
        self.fields.remove(i);
        let pos = p.position(&self.fields)?.unwrap_or(i);
        self.fields.insert(pos, f);
        self.map.insert(pos, src);
        Ok(())
    }

    fn rename(&mut self, p: &mut Parser) -> Res<()> {
        if p.kw("COLUMN") {
            let a = p.ident()?;
            p.expect_kw("TO")?;
            let b = p.ident()?;
            let i = find_field(&self.fields, &a)?;
            self.fields[i].name = b;
            return Ok(());
        }
        if !p.kw("TO") {
            p.kw("AS");
        }
        self.rename_to = Some(p.ident()?);
        Ok(())
    }

    /// An old row in the new column layout.
    fn migrate(&self, r: &Row) -> Row {
        self.map.iter().zip(&self.fields).map(|(m, f)| m.map(|o| r[o].clone()).unwrap_or_else(|| default_cell(f))).collect()
    }
}
