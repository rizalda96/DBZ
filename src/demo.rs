//! A small in-memory database used for the built-in "Sample Data" connection,
//! so the app is useful (and testable) without a running server.
//! It understands a practical subset of MySQL-flavoured SQL.

use crate::model::{Cell, FieldDef, ResultSet, Row, RoutineInfo, TableInfo};
use crate::sqlgen;

pub struct DemoTable {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub rows: Vec<Row>,
    pub comment: String,
    pub auto_inc: u64,
    pub created: String,
    pub modified: String,
}

pub struct DemoDb {
    pub name: String,
    pub tables: Vec<DemoTable>,
}

pub struct DemoServer {
    pub dbs: Vec<DemoDb>,
}

// ---------------------------------------------------------------- tokens

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    QIdent(String),
    Str(String),
    Num(String),
    Sym(String),
}

fn tokenize(sql: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = sql.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch.is_alphabetic() || ch == '_' {
            let s = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '$') {
                i += 1;
            }
            out.push(Tok::Ident(c[s..i].iter().collect()));
        } else if ch.is_ascii_digit() || (ch == '.' && i + 1 < c.len() && c[i + 1].is_ascii_digit()) {
            let s = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.' || c[i] == 'e' || c[i] == 'E') {
                i += 1;
            }
            out.push(Tok::Num(c[s..i].iter().collect()));
        } else if ch == '\'' || ch == '"' || ch == '`' {
            let q = ch;
            i += 1;
            let mut s = String::new();
            loop {
                if i >= c.len() {
                    return Err("Unterminated quoted text".into());
                }
                if c[i] == '\\' && q == '\'' && i + 1 < c.len() {
                    let n = c[i + 1];
                    s.push(match n {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '0' => '\0',
                        o => o,
                    });
                    i += 2;
                    continue;
                }
                if c[i] == q {
                    if i + 1 < c.len() && c[i + 1] == q {
                        s.push(q);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                s.push(c[i]);
                i += 1;
            }
            out.push(if q == '\'' { Tok::Str(s) } else { Tok::QIdent(s) });
        } else {
            let two: String = c[i..(i + 2).min(c.len())].iter().collect();
            if ["<=", ">=", "<>", "!="].contains(&two.as_str()) {
                out.push(Tok::Sym(two));
                i += 2;
            } else {
                out.push(Tok::Sym(ch.to_string()));
                i += 1;
            }
        }
    }
    Ok(out)
}

struct P {
    t: Vec<Tok>,
    i: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn is_kw(&self, k: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if s.eq_ignore_ascii_case(k))
    }
    fn kw(&mut self, k: &str) -> bool {
        if self.is_kw(k) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect_kw(&mut self, k: &str) -> Result<(), String> {
        if self.kw(k) {
            Ok(())
        } else {
            Err(format!("Expected {} near {}", k, self.near()))
        }
    }
    fn sym(&mut self, s: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Sym(x)) if x == s) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect_sym(&mut self, s: &str) -> Result<(), String> {
        if self.sym(s) {
            Ok(())
        } else {
            Err(format!("Expected '{}' near {}", s, self.near()))
        }
    }
    fn near(&self) -> String {
        match self.peek() {
            None => "end of statement".into(),
            Some(Tok::Ident(s)) | Some(Tok::Num(s)) | Some(Tok::Sym(s)) => format!("'{}'", s),
            Some(Tok::QIdent(s)) => format!("`{}`", s),
            Some(Tok::Str(s)) => format!("'{}'", s),
        }
    }
    fn ident(&mut self) -> Result<String, String> {
        let mut name = match self.peek().cloned() {
            Some(Tok::Ident(s)) | Some(Tok::QIdent(s)) => {
                self.i += 1;
                s
            }
            _ => return Err(format!("Expected a name near {}", self.near())),
        };
        // db.table or table.column: keep the last part
        while self.sym(".") {
            match self.peek().cloned() {
                Some(Tok::Ident(s)) | Some(Tok::QIdent(s)) => {
                    self.i += 1;
                    name = s;
                }
                Some(Tok::Sym(s)) if s == "*" => {
                    self.i += 1;
                    name = "*".into();
                }
                _ => return Err("Expected a name after '.'".into()),
            }
        }
        Ok(name)
    }
    fn literal(&mut self) -> Result<Cell, String> {
        let neg = self.sym("-");
        match self.peek().cloned() {
            Some(Tok::Str(s)) => {
                self.i += 1;
                Ok(Some(s))
            }
            Some(Tok::Num(n)) => {
                self.i += 1;
                Ok(Some(if neg { format!("-{}", n) } else { n }))
            }
            Some(Tok::Ident(s)) => {
                let u = s.to_uppercase();
                self.i += 1;
                match u.as_str() {
                    "NULL" => Ok(None),
                    "TRUE" => Ok(Some("1".into())),
                    "FALSE" => Ok(Some("0".into())),
                    "CURRENT_TIMESTAMP" | "NOW" | "CURRENT_DATE" => {
                        if self.sym("(") {
                            self.expect_sym(")")?;
                        }
                        let now = now_string();
                        Ok(Some(if u == "CURRENT_DATE" { now[..10].to_string() } else { now }))
                    }
                    _ => Err(format!("Unsupported value '{}' (the sample database only understands plain values)", s)),
                }
            }
            _ => Err(format!("Expected a value near {}", self.near())),
        }
    }
    fn done(&mut self) -> Result<(), String> {
        while self.sym(";") {}
        if self.i < self.t.len() {
            Err(format!("Unexpected {} — the sample database supports simple statements only", self.near()))
        } else {
            Ok(())
        }
    }
}

// ---------------------------------------------------------------- conditions

enum Cond {
    Cmp(String, String, Cell),
    IsNull(String, bool),
    In(String, Vec<Cell>),
}

fn parse_where(p: &mut P) -> Result<Vec<Vec<Cond>>, String> {
    // OR of ANDs
    let mut ors = vec![];
    if !p.kw("WHERE") {
        return Ok(ors);
    }
    loop {
        let mut ands = vec![];
        loop {
            let paren = p.sym("(");
            let col = p.ident()?;
            if p.kw("IS") {
                let not = p.kw("NOT");
                p.expect_kw("NULL")?;
                ands.push(Cond::IsNull(col, !not));
            } else if p.kw("IN") {
                p.expect_sym("(")?;
                let mut vals = vec![];
                loop {
                    vals.push(p.literal()?);
                    if !p.sym(",") {
                        break;
                    }
                }
                p.expect_sym(")")?;
                ands.push(Cond::In(col, vals));
            } else {
                let op = if p.kw("LIKE") {
                    "LIKE".to_string()
                } else if p.kw("NOT") {
                    p.expect_kw("LIKE")?;
                    "NOT LIKE".to_string()
                } else {
                    match p.peek().cloned() {
                        Some(Tok::Sym(s)) if ["=", "<", ">", "<=", ">=", "<>", "!="].contains(&s.as_str()) => {
                            p.i += 1;
                            s
                        }
                        _ => return Err(format!("Expected a comparison near {}", p.near())),
                    }
                };
                let v = p.literal()?;
                ands.push(Cond::Cmp(col, op, v));
            }
            if paren {
                p.expect_sym(")")?;
            }
            if !p.kw("AND") {
                break;
            }
        }
        ors.push(ands);
        if !p.kw("OR") {
            break;
        }
    }
    Ok(ors)
}

fn cmp_cells(a: &Cell, b: &Cell) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    match (a, b) {
        (None, None) => Equal,
        (None, _) => Less,
        (_, None) => Greater,
        (Some(x), Some(y)) => match (x.trim().parse::<f64>(), y.trim().parse::<f64>()) {
            (Ok(fx), Ok(fy)) => fx.partial_cmp(&fy).unwrap_or(Equal),
            _ => x.to_lowercase().cmp(&y.to_lowercase()),
        },
    }
}

fn like(text: &str, pat: &str) -> bool {
    fn rec(t: &[char], p: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('%') => (0..=t.len()).any(|k| rec(&t[k..], &p[1..])),
            Some('_') => !t.is_empty() && rec(&t[1..], &p[1..]),
            Some(c) => !t.is_empty() && t[0].to_lowercase().eq(c.to_lowercase()) && rec(&t[1..], &p[1..]),
        }
    }
    let t: Vec<char> = text.chars().collect();
    let p: Vec<char> = pat.chars().collect();
    rec(&t, &p)
}

fn col_index(t: &DemoTable, name: &str) -> Result<usize, String> {
    t.fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("Unknown column '{}' in '{}'", name, t.name))
}

fn row_matches(t: &DemoTable, row: &Row, ors: &[Vec<Cond>]) -> Result<bool, String> {
    if ors.is_empty() {
        return Ok(true);
    }
    for ands in ors {
        let mut ok = true;
        for c in ands {
            let r = match c {
                Cond::IsNull(col, want_null) => row[col_index(t, col)?].is_none() == *want_null,
                Cond::In(col, vals) => {
                    let v = &row[col_index(t, col)?];
                    v.is_some() && vals.iter().any(|x| cmp_cells(v, x).is_eq())
                }
                Cond::Cmp(col, op, val) => {
                    let v = &row[col_index(t, col)?];
                    if v.is_none() || val.is_none() {
                        false
                    } else {
                        let o = cmp_cells(v, val);
                        match op.as_str() {
                            "=" => o.is_eq(),
                            "<>" | "!=" => !o.is_eq(),
                            "<" => o.is_lt(),
                            ">" => o.is_gt(),
                            "<=" => o.is_le(),
                            ">=" => o.is_ge(),
                            "LIKE" => like(v.as_deref().unwrap_or(""), val.as_deref().unwrap_or("")),
                            "NOT LIKE" => !like(v.as_deref().unwrap_or(""), val.as_deref().unwrap_or("")),
                            _ => false,
                        }
                    }
                }
            };
            if !r {
                ok = false;
                break;
            }
        }
        if ok {
            return Ok(true);
        }
    }
    Ok(false)
}

// ---------------------------------------------------------------- column definitions

fn parse_type(p: &mut P) -> Result<(String, String, String), String> {
    let mut ty = p.ident()?.to_lowercase();
    if ty == "double" && p.kw("PRECISION") {
        ty = "double precision".into();
    }
    if ty == "character" && p.kw("VARYING") {
        ty = "varchar".into();
    }
    let (mut len, mut dec) = (String::new(), String::new());
    if p.sym("(") {
        let mut parts = vec![];
        loop {
            match p.peek().cloned() {
                Some(Tok::Num(n)) => {
                    p.i += 1;
                    parts.push(n)
                }
                Some(Tok::Str(s)) => {
                    p.i += 1;
                    parts.push(format!("'{}'", s))
                }
                _ => return Err(format!("Expected a size near {}", p.near())),
            }
            if !p.sym(",") {
                break;
            }
        }
        p.expect_sym(")")?;
        if ty == "enum" || ty == "set" {
            len = parts.join(",");
        } else {
            len = parts.first().cloned().unwrap_or_default();
            dec = parts.get(1).cloned().unwrap_or_default();
        }
    }
    while p.kw("UNSIGNED") || p.kw("ZEROFILL") {}
    if p.kw("CHARACTER") {
        p.expect_kw("SET")?;
        p.ident()?;
    }
    if p.kw("COLLATE") {
        p.ident()?;
    }
    Ok((ty, len, dec))
}

fn parse_coldef(p: &mut P) -> Result<FieldDef, String> {
    let name = p.ident()?;
    let (ty, length, decimals) = parse_type(p)?;
    let mut f = FieldDef { name, ty, length, decimals, ..Default::default() };
    loop {
        if p.kw("NOT") {
            p.expect_kw("NULL")?;
            f.not_null = true;
        } else if p.kw("NULL") {
            f.not_null = false;
        } else if p.kw("DEFAULT") {
            f.default = match p.literal()? {
                None => "NULL".into(),
                Some(v) => v,
            };
        } else if p.kw("AUTO_INCREMENT") {
            f.auto_inc = true;
        } else if p.kw("PRIMARY") {
            p.expect_kw("KEY")?;
            f.key = true;
        } else if p.kw("UNIQUE") {
            p.kw("KEY");
        } else if p.kw("COMMENT") {
            f.comment = p.literal()?.unwrap_or_default();
        } else if p.kw("ON") {
            p.expect_kw("UPDATE")?;
            p.literal()?;
        } else {
            break;
        }
    }
    Ok(f)
}

fn ident_list(p: &mut P) -> Result<Vec<String>, String> {
    p.expect_sym("(")?;
    let mut v = vec![];
    loop {
        v.push(p.ident()?);
        if !p.sym(",") {
            break;
        }
    }
    p.expect_sym(")")?;
    Ok(v)
}

// ---------------------------------------------------------------- server

impl DemoServer {
    fn db_mut(&mut self, db: &str) -> Result<&mut DemoDb, String> {
        self.dbs.iter_mut().find(|d| d.name.eq_ignore_ascii_case(db)).ok_or_else(|| format!("Unknown database '{}'", db))
    }
    fn db(&self, db: &str) -> Result<&DemoDb, String> {
        self.dbs.iter().find(|d| d.name.eq_ignore_ascii_case(db)).ok_or_else(|| format!("Unknown database '{}'", db))
    }
    fn table_mut(&mut self, db: &str, t: &str) -> Result<&mut DemoTable, String> {
        let d = self.db_mut(db)?;
        d.tables.iter_mut().find(|x| x.name.eq_ignore_ascii_case(t)).ok_or_else(|| format!("Table '{}.{}' doesn't exist", db, t))
    }
    pub fn table(&self, db: &str, t: &str) -> Result<&DemoTable, String> {
        let d = self.db(db)?;
        d.tables.iter().find(|x| x.name.eq_ignore_ascii_case(t)).ok_or_else(|| format!("Table '{}.{}' doesn't exist", db, t))
    }

    pub fn databases(&self) -> Vec<String> {
        self.dbs.iter().map(|d| d.name.clone()).collect()
    }

    pub fn tables(&self, db: &str) -> Result<Vec<TableInfo>, String> {
        Ok(self
            .db(db)?
            .tables
            .iter()
            .map(|t| {
                let size: usize = t.rows.iter().map(|r| r.iter().map(|c| c.as_ref().map(|s| s.len()).unwrap_or(1)).sum::<usize>()).sum();
                TableInfo {
                    name: t.name.clone(),
                    is_view: false,
                    rows: Some(t.rows.len() as u64),
                    data_length: Some((size as u64 + 16383) / 16384 * 16384),
                    engine: "Memory".into(),
                    created: t.created.clone(),
                    modified: t.modified.clone(),
                    comment: t.comment.clone(),
                    collation: "utf8mb4_general_ci".into(),
                }
            })
            .collect())
    }

    pub fn routines(&self, _db: &str) -> Vec<RoutineInfo> {
        vec![]
    }

    pub fn columns(&self, db: &str, t: &str) -> Result<Vec<FieldDef>, String> {
        Ok(self.table(db, t)?.fields.iter().map(|f| FieldDef { orig_name: Some(f.name.clone()), ..f.clone() }).collect())
    }

    pub fn ddl(&self, db: &str, t: &str) -> Result<String, String> {
        let tb = self.table(db, t)?;
        let mut s = sqlgen::create_table(false, &tb.name, &tb.fields, &tb.comment).join(";\n");
        s.push(';');
        Ok(s)
    }

    pub fn exec(&mut self, db: &str, sql: &str) -> Result<ResultSet, String> {
        let toks = tokenize(sql)?;
        let mut p = P { t: toks, i: 0 };
        let mut rs = ResultSet { statement: sql.to_string(), ..Default::default() };
        if p.kw("SELECT") {
            self.select(db, &mut p, &mut rs)?;
        } else if p.kw("SHOW") {
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
                rs.rows = self
                    .table(db, &t)?
                    .fields
                    .iter()
                    .map(|f| {
                        vec![
                            Some(f.name.clone()),
                            Some(f.type_text()),
                            Some(if f.not_null { "NO" } else { "YES" }.into()),
                            Some(if f.key { "PRI" } else { "" }.into()),
                            if f.default.is_empty() { None } else { Some(f.default.clone()) },
                            Some(if f.auto_inc { "auto_increment" } else { "" }.into()),
                        ]
                    })
                    .collect();
            } else {
                return Err("The sample database supports SHOW DATABASES, SHOW TABLES, SHOW COLUMNS and SHOW CREATE TABLE".into());
            }
            rs.has_rows = true;
        } else if p.kw("DESCRIBE") || p.kw("DESC") {
            let t = p.ident()?;
            return self.exec(db, &format!("SHOW COLUMNS FROM `{}`", t));
        } else if p.kw("USE") {
            p.ident()?;
        } else if p.kw("INSERT") {
            p.kw("INTO");
            let t = p.ident()?;
            let cols = if matches!(p.peek(), Some(Tok::Sym(s)) if s == "(") { Some(ident_list(&mut p)?) } else { None };
            if !p.kw("VALUES") {
                p.expect_kw("VALUE")?;
            }
            let mut tuples = vec![];
            loop {
                p.expect_sym("(")?;
                let mut vals = vec![];
                loop {
                    vals.push(p.literal()?);
                    if !p.sym(",") {
                        break;
                    }
                }
                p.expect_sym(")")?;
                tuples.push(vals);
                if !p.sym(",") {
                    break;
                }
            }
            p.done()?;
            let tb = self.table_mut(db, &t)?;
            let idx: Vec<usize> = match &cols {
                Some(c) => c.iter().map(|n| col_index(tb, n)).collect::<Result<_, _>>()?,
                None => (0..tb.fields.len()).collect(),
            };
            let mut new_rows = vec![];
            for vals in tuples {
                if vals.len() != idx.len() {
                    return Err("Column count doesn't match value count".into());
                }
                let mut row: Row = tb.fields.iter().map(|f| if f.default.is_empty() || f.default == "NULL" { None } else { Some(f.default.clone()) }).collect();
                for (k, v) in idx.iter().zip(vals) {
                    row[*k] = v;
                }
                for (k, f) in tb.fields.iter().enumerate() {
                    if f.auto_inc {
                        match &row[k] {
                            None => {
                                tb.auto_inc += 1;
                                row[k] = Some(tb.auto_inc.to_string());
                            }
                            Some(v) => {
                                if let Ok(n) = v.parse::<u64>() {
                                    tb.auto_inc = tb.auto_inc.max(n);
                                }
                            }
                        }
                    }
                    if f.not_null && row[k].is_none() {
                        return Err(format!("Field '{}' doesn't have a default value", f.name));
                    }
                }
                let keys: Vec<usize> = tb.fields.iter().enumerate().filter(|(_, f)| f.key).map(|(i, _)| i).collect();
                if !keys.is_empty() {
                    let dup = tb.rows.iter().chain(new_rows.iter()).any(|r: &Row| keys.iter().all(|&k| cmp_cells(&r[k], &row[k]).is_eq()));
                    if dup {
                        return Err(format!("Duplicate entry for key 'PRIMARY' in '{}'", tb.name));
                    }
                }
                new_rows.push(row);
            }
            rs.affected = new_rows.len() as u64;
            tb.rows.extend(new_rows);
            tb.modified = now_string();
        } else if p.kw("UPDATE") {
            let t = p.ident()?;
            p.expect_kw("SET")?;
            let mut sets = vec![];
            loop {
                let c = p.ident()?;
                p.expect_sym("=")?;
                sets.push((c, p.literal()?));
                if !p.sym(",") {
                    break;
                }
            }
            let w = parse_where(&mut p)?;
            let limit = if p.kw("LIMIT") { Some(num(&mut p)?) } else { None };
            p.done()?;
            let tb = self.table_mut(db, &t)?;
            let sets: Vec<(usize, Cell)> = sets.into_iter().map(|(c, v)| Ok((col_index(tb, &c)?, v))).collect::<Result<_, String>>()?;
            for (k, v) in &sets {
                if tb.fields[*k].not_null && v.is_none() {
                    return Err(format!("Column '{}' cannot be null", tb.fields[*k].name));
                }
            }
            let mut n = 0;
            for r in 0..tb.rows.len() {
                if limit.map(|l| n >= l).unwrap_or(false) {
                    break;
                }
                if row_matches(tb, &tb.rows[r], &w)? {
                    for (k, v) in &sets {
                        tb.rows[r][*k] = v.clone();
                    }
                    n += 1;
                }
            }
            rs.affected = n as u64;
            tb.modified = now_string();
        } else if p.kw("DELETE") {
            p.expect_kw("FROM")?;
            let t = p.ident()?;
            let w = parse_where(&mut p)?;
            let limit = if p.kw("LIMIT") { Some(num(&mut p)?) } else { None };
            p.done()?;
            let tb = self.table_mut(db, &t)?;
            let mut keep = vec![];
            let mut n = 0;
            for r in std::mem::take(&mut tb.rows) {
                if limit.map(|l| n < l).unwrap_or(true) && row_matches(tb, &r, &w)? {
                    n += 1;
                } else {
                    keep.push(r);
                }
            }
            tb.rows = keep;
            rs.affected = n as u64;
            tb.modified = now_string();
        } else if p.kw("TRUNCATE") {
            p.kw("TABLE");
            let t = p.ident()?;
            p.done()?;
            let tb = self.table_mut(db, &t)?;
            tb.rows.clear();
            tb.auto_inc = 0;
        } else if p.kw("CREATE") {
            if p.kw("DATABASE") || p.kw("SCHEMA") {
                let name = p.ident()?;
                if self.dbs.iter().any(|d| d.name.eq_ignore_ascii_case(&name)) {
                    return Err(format!("Can't create database '{}'; database exists", name));
                }
                self.dbs.push(DemoDb { name, tables: vec![] });
                return Ok(rs);
            }
            p.expect_kw("TABLE")?;
            let if_not = if p.kw("IF") {
                p.expect_kw("NOT")?;
                p.expect_kw("EXISTS")?;
                true
            } else {
                false
            };
            let name = p.ident()?;
            p.expect_sym("(")?;
            let mut fields: Vec<FieldDef> = vec![];
            loop {
                if p.kw("PRIMARY") {
                    p.expect_kw("KEY")?;
                    for k in ident_list(&mut p)? {
                        let i = fields.iter().position(|f| f.name.eq_ignore_ascii_case(&k)).ok_or(format!("Unknown key column '{}'", k))?;
                        fields[i].key = true;
                        fields[i].not_null = true;
                    }
                } else if p.kw("KEY") || p.kw("INDEX") || p.kw("UNIQUE") {
                    p.kw("KEY");
                    if !matches!(p.peek(), Some(Tok::Sym(s)) if s == "(") {
                        p.ident()?;
                    }
                    ident_list(&mut p)?;
                } else {
                    let f = parse_coldef(&mut p)?;
                    fields.push(f);
                }
                if !p.sym(",") {
                    break;
                }
            }
            p.expect_sym(")")?;
            let mut comment = String::new();
            while p.i < p.t.len() && !p.sym(";") {
                if p.kw("COMMENT") {
                    p.sym("=");
                    comment = p.literal()?.unwrap_or_default();
                } else {
                    p.i += 1;
                }
            }
            let d = self.db_mut(db)?;
            if d.tables.iter().any(|t| t.name.eq_ignore_ascii_case(&name)) {
                if if_not {
                    return Ok(rs);
                }
                return Err(format!("Table '{}' already exists", name));
            }
            let now = now_string();
            d.tables.push(DemoTable { name, fields, rows: vec![], comment, auto_inc: 0, created: now.clone(), modified: now });
            d.tables.sort_by(|a, b| a.name.cmp(&b.name));
        } else if p.kw("DROP") {
            if p.kw("DATABASE") || p.kw("SCHEMA") {
                let name = p.ident()?;
                let before = self.dbs.len();
                self.dbs.retain(|d| !d.name.eq_ignore_ascii_case(&name));
                if before == self.dbs.len() {
                    return Err(format!("Can't drop database '{}'; database doesn't exist", name));
                }
                return Ok(rs);
            }
            p.expect_kw("TABLE")?;
            let if_exists = if p.kw("IF") {
                p.expect_kw("EXISTS")?;
                true
            } else {
                false
            };
            loop {
                let name = p.ident()?;
                let d = self.db_mut(db)?;
                let before = d.tables.len();
                d.tables.retain(|t| !t.name.eq_ignore_ascii_case(&name));
                if before == d.tables.len() && !if_exists {
                    return Err(format!("Unknown table '{}'", name));
                }
                if !p.sym(",") {
                    break;
                }
            }
        } else if p.kw("ALTER") {
            p.expect_kw("TABLE")?;
            let t = p.ident()?;
            let mut rename_to = None;
            {
                let tb = self.table_mut(db, &t)?;
                let mut fields = tb.fields.clone();
                let mut map: Vec<Option<usize>> = (0..fields.len()).map(Some).collect(); // new index -> old row index
                let mut comment = tb.comment.clone();
                loop {
                    if p.kw("ADD") {
                        if p.kw("PRIMARY") {
                            p.expect_kw("KEY")?;
                            for k in ident_list(&mut p)? {
                                let i = fields.iter().position(|f| f.name.eq_ignore_ascii_case(&k)).ok_or(format!("Unknown key column '{}'", k))?;
                                fields[i].key = true;
                                fields[i].not_null = true;
                            }
                        } else {
                            p.kw("COLUMN");
                            let f = parse_coldef(&mut p)?;
                            let pos = position(&mut p, &fields)?.unwrap_or(fields.len());
                            fields.insert(pos, f);
                            map.insert(pos, None);
                        }
                    } else if p.kw("DROP") {
                        if p.kw("PRIMARY") {
                            p.expect_kw("KEY")?;
                            for f in fields.iter_mut() {
                                f.key = false;
                            }
                        } else {
                            p.kw("COLUMN");
                            let c = p.ident()?;
                            let i = fields.iter().position(|f| f.name.eq_ignore_ascii_case(&c)).ok_or(format!("Can't DROP '{}'; check that column exists", c))?;
                            fields.remove(i);
                            map.remove(i);
                        }
                    } else if p.kw("CHANGE") || p.kw("MODIFY") {
                        let was_change = matches!(&p.t[p.i - 1], Tok::Ident(s) if s.eq_ignore_ascii_case("CHANGE"));
                        p.kw("COLUMN");
                        let old = if was_change { p.ident()? } else { String::new() };
                        let save = p.i;
                        let mut f = parse_coldef(&mut p)?;
                        let old = if was_change { old } else { f.name.clone() };
                        let _ = save;
                        let i = fields.iter().position(|x| x.name.eq_ignore_ascii_case(&old)).ok_or(format!("Unknown column '{}'", old))?;
                        f.key = fields[i].key || f.key;
                        let src = map.remove(i);
                        fields.remove(i);
                        let pos = position(&mut p, &fields)?.unwrap_or(i);
                        fields.insert(pos, f);
                        map.insert(pos, src);
                    } else if p.kw("RENAME") {
                        if p.kw("COLUMN") {
                            let a = p.ident()?;
                            p.expect_kw("TO")?;
                            let b = p.ident()?;
                            let i = fields.iter().position(|x| x.name.eq_ignore_ascii_case(&a)).ok_or(format!("Unknown column '{}'", a))?;
                            fields[i].name = b;
                        } else {
                            if !p.kw("TO") {
                                p.kw("AS");
                            }
                            rename_to = Some(p.ident()?);
                        }
                    } else if p.kw("COMMENT") {
                        p.sym("=");
                        comment = p.literal()?.unwrap_or_default();
                    } else {
                        return Err(format!("Unsupported ALTER TABLE action near {}", p.near()));
                    }
                    if !p.sym(",") {
                        break;
                    }
                }
                p.done()?;
                tb.rows = tb
                    .rows
                    .iter()
                    .map(|r| map.iter().enumerate().map(|(ni, m)| match m {
                        Some(o) => r[*o].clone(),
                        None => {
                            let d = &fields[ni].default;
                            if d.is_empty() || d == "NULL" { None } else { Some(d.clone()) }
                        }
                    }).collect())
                    .collect();
                tb.fields = fields;
                tb.comment = comment;
                tb.modified = now_string();
            }
            if let Some(n) = rename_to {
                self.table_mut(db, &t)?.name = n;
                self.db_mut(db)?.tables.sort_by(|a, b| a.name.cmp(&b.name));
            }
        } else {
            return Err(format!(
                "The sample database doesn't understand {} — try SELECT, INSERT, UPDATE, DELETE, CREATE/ALTER/DROP TABLE or SHOW",
                p.near()
            ));
        }
        Ok(rs)
    }

    fn select(&mut self, db: &str, p: &mut P, rs: &mut ResultSet) -> Result<(), String> {
        rs.has_rows = true;
        // select list
        enum Item {
            Star,
            Col(String, String),
            Count(String),
            Lit(Cell, String),
        }
        let mut items = vec![];
        loop {
            if p.sym("*") {
                items.push(Item::Star);
            } else if p.is_kw("COUNT") && matches!(p.t.get(p.i + 1), Some(Tok::Sym(s)) if s == "(") {
                p.i += 2;
                if !p.sym("*") {
                    p.ident()?;
                }
                p.expect_sym(")")?;
                let alias = alias(p)?.unwrap_or_else(|| "COUNT(*)".into());
                items.push(Item::Count(alias));
            } else if matches!(p.peek(), Some(Tok::Str(_)) | Some(Tok::Num(_))) || p.is_kw("NULL") || p.is_kw("NOW") || p.is_kw("CURRENT_TIMESTAMP") {
                let label = p.near().trim_matches('\'').to_string();
                let v = p.literal()?;
                let a = alias(p)?.unwrap_or(label);
                items.push(Item::Lit(v, a));
            } else if p.is_kw("VERSION") {
                p.i += 1;
                p.expect_sym("(")?;
                p.expect_sym(")")?;
                let a = alias(p)?.unwrap_or_else(|| "VERSION()".into());
                items.push(Item::Lit(Some("8.0-sample".into()), a));
            } else if p.is_kw("DATABASE") {
                p.i += 1;
                p.expect_sym("(")?;
                p.expect_sym(")")?;
                let a = alias(p)?.unwrap_or_else(|| "DATABASE()".into());
                items.push(Item::Lit(Some(db.to_string()), a));
            } else {
                let c = p.ident()?;
                if c == "*" {
                    items.push(Item::Star);
                } else {
                    let a = alias(p)?.unwrap_or_else(|| c.clone());
                    items.push(Item::Col(c, a));
                }
            }
            if !p.sym(",") {
                break;
            }
        }
        if !p.kw("FROM") {
            p.done()?;
            for it in &items {
                match it {
                    Item::Lit(v, a) => {
                        rs.columns.push(a.clone());
                        if rs.rows.is_empty() {
                            rs.rows.push(vec![]);
                        }
                        rs.rows[0].push(v.clone());
                    }
                    _ => return Err("No table used".into()),
                }
            }
            return Ok(());
        }
        let t = p.ident()?;
        let _ = alias(p)?;
        let w = parse_where(p)?;
        let mut order: Vec<(String, bool)> = vec![];
        if p.kw("ORDER") {
            p.expect_kw("BY")?;
            loop {
                let c = p.ident()?;
                let desc = if p.kw("DESC") {
                    true
                } else {
                    p.kw("ASC");
                    false
                };
                order.push((c, desc));
                if !p.sym(",") {
                    break;
                }
            }
        }
        let (mut limit, mut offset) = (None, 0usize);
        if p.kw("LIMIT") {
            let a = num(p)?;
            if p.sym(",") {
                offset = a;
                limit = Some(num(p)?);
            } else {
                limit = Some(a);
                if p.kw("OFFSET") {
                    offset = num(p)?;
                }
            }
        }
        p.done()?;
        let tb = self.table(db, &t)?;
        let mut rows: Vec<&Row> = vec![];
        for r in &tb.rows {
            if row_matches(tb, r, &w)? {
                rows.push(r);
            }
        }
        if !order.is_empty() {
            let idx: Vec<(usize, bool)> = order.iter().map(|(c, d)| Ok((col_index(tb, c)?, *d))).collect::<Result<_, String>>()?;
            rows.sort_by(|a, b| {
                for (k, d) in &idx {
                    let o = cmp_cells(&a[*k], &b[*k]);
                    if !o.is_eq() {
                        return if *d { o.reverse() } else { o };
                    }
                }
                std::cmp::Ordering::Equal
            });
        }
        if items.iter().any(|i| matches!(i, Item::Count(_))) {
            for it in &items {
                match it {
                    Item::Count(a) => rs.columns.push(a.clone()),
                    Item::Lit(_, a) => rs.columns.push(a.clone()),
                    _ => return Err("Mixing COUNT(*) with columns needs GROUP BY, which the sample database doesn't support".into()),
                }
            }
            let mut row = vec![];
            for it in &items {
                match it {
                    Item::Count(_) => row.push(Some(rows.len().to_string())),
                    Item::Lit(v, _) => row.push(v.clone()),
                    _ => {}
                }
            }
            rs.rows = vec![row];
            return Ok(());
        }
        enum Src {
            Col(usize),
            Lit(Cell),
        }
        let mut srcs = vec![];
        for it in &items {
            match it {
                Item::Star => {
                    for (k, f) in tb.fields.iter().enumerate() {
                        rs.columns.push(f.name.clone());
                        srcs.push(Src::Col(k));
                    }
                }
                Item::Col(c, a) => {
                    srcs.push(Src::Col(col_index(tb, c)?));
                    rs.columns.push(a.clone());
                }
                Item::Lit(v, a) => {
                    srcs.push(Src::Lit(v.clone()));
                    rs.columns.push(a.clone());
                }
                Item::Count(_) => {}
            }
        }
        rs.rows = rows
            .into_iter()
            .skip(offset)
            .take(limit.unwrap_or(usize::MAX))
            .map(|r| {
                srcs.iter()
                    .map(|s| match s {
                        Src::Col(k) => r[*k].clone(),
                        Src::Lit(v) => v.clone(),
                    })
                    .collect()
            })
            .collect();
        Ok(())
    }
}

fn alias(p: &mut P) -> Result<Option<String>, String> {
    if p.kw("AS") {
        return Ok(Some(p.ident()?));
    }
    if let Some(Tok::Ident(s)) | Some(Tok::QIdent(s)) = p.peek().cloned() {
        let u = s.to_uppercase();
        if !["FROM", "WHERE", "ORDER", "LIMIT", "GROUP", "OFFSET", "ASC", "DESC", "AND", "OR"].contains(&u.as_str()) {
            p.i += 1;
            return Ok(Some(s));
        }
    }
    Ok(None)
}

fn num(p: &mut P) -> Result<usize, String> {
    match p.peek().cloned() {
        Some(Tok::Num(n)) => {
            p.i += 1;
            n.parse().map_err(|_| format!("Bad number {}", n))
        }
        _ => Err(format!("Expected a number near {}", p.near())),
    }
}

fn position(p: &mut P, fields: &[FieldDef]) -> Result<Option<usize>, String> {
    if p.kw("FIRST") {
        return Ok(Some(0));
    }
    if p.kw("AFTER") {
        let c = p.ident()?;
        let i = fields.iter().position(|f| f.name.eq_ignore_ascii_case(&c)).ok_or(format!("Unknown column '{}'", c))?;
        return Ok(Some(i + 1));
    }
    Ok(None)
}

// ---------------------------------------------------------------- time

pub fn now_string() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, d) = civil(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------- sample content

fn f(name: &str, ty: &str, len: &str, not_null: bool, key: bool, auto: bool, comment: &str) -> FieldDef {
    FieldDef {
        name: name.into(),
        ty: ty.into(),
        length: len.split(',').next().unwrap_or("").into(),
        decimals: len.split(',').nth(1).unwrap_or("").into(),
        not_null,
        key,
        auto_inc: auto,
        comment: comment.into(),
        ..Default::default()
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[(self.next() % v.len() as u64) as usize]
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

pub fn sample_server() -> DemoServer {
    let mut r = Rng(0x9e3779b97f4a7c15);
    let firsts = ["Ava", "Liam", "Noah", "Mia", "Ethan", "Sofia", "Lucas", "Amara", "Kenji", "Lena", "Omar", "Priya", "Mateo", "Hana", "Jonas", "Zara", "Ivan", "Nora", "Felix", "Leila"];
    let lasts = ["Walker", "Nguyen", "Santos", "Kim", "Fischer", "Rossi", "Okafor", "Tanaka", "Dubois", "Haddad", "Larsen", "Patel", "Moreno", "Sato", "Novak"];
    let cities = [("Jakarta", "Indonesia"), ("Surabaya", "Indonesia"), ("Singapore", "Singapore"), ("Berlin", "Germany"), ("Lyon", "France"), ("Osaka", "Japan"), ("Toronto", "Canada"), ("Austin", "USA"), ("Lisbon", "Portugal"), ("Nairobi", "Kenya")];
    let mut customers = vec![];
    for i in 1..=48u64 {
        let fname = r.pick(&firsts);
        let lname = r.pick(&lasts);
        let (city, country) = cities[(r.next() % cities.len() as u64) as usize];
        customers.push(vec![
            Some(i.to_string()),
            Some(fname.into()),
            Some(lname.into()),
            Some(format!("{}.{}{}@example.com", fname.to_lowercase(), lname.to_lowercase(), i)),
            if i % 7 == 0 { None } else { Some(format!("+62 8{:02} {:04} {:04}", r.range(11, 99), r.range(1000, 9999), r.range(1000, 9999))) },
            Some(city.into()),
            Some(country.into()),
            Some(format!("2025-{:02}-{:02} {:02}:{:02}:00", r.range(1, 12), r.range(1, 28), r.range(8, 20), r.range(0, 59))),
        ]);
    }
    let products_src = [
        ("Ceramic Mug", "Kitchen", "8.50"),
        ("Steel Water Bottle", "Outdoor", "19.90"),
        ("Linen Tote Bag", "Accessories", "14.00"),
        ("Desk Lamp", "Home Office", "39.00"),
        ("Notebook A5", "Stationery", "4.75"),
        ("Gel Pen Set", "Stationery", "6.20"),
        ("Wireless Mouse", "Electronics", "24.99"),
        ("USB-C Cable 2m", "Electronics", "9.99"),
        ("Yoga Mat", "Fitness", "29.50"),
        ("Cast Iron Pan", "Kitchen", "45.00"),
        ("Rain Jacket", "Outdoor", "79.00"),
        ("Wall Clock", "Home", "22.40"),
        ("Plant Pot", "Home", "12.30"),
        ("Bluetooth Speaker", "Electronics", "54.00"),
        ("Coffee Grinder", "Kitchen", "35.80"),
        ("Headphone Stand", "Home Office", "17.60"),
    ];
    let products: Vec<Row> = products_src
        .iter()
        .enumerate()
        .map(|(i, (n, c, p))| {
            vec![Some((i + 1).to_string()), Some(format!("SKU-{:04}", 1001 + i)), Some(n.to_string()), Some(c.to_string()), Some(p.to_string()), Some(r.range(0, 240).to_string()), Some(if i % 5 == 4 { "0" } else { "1" }.into())]
        })
        .collect();
    let statuses = ["paid", "shipped", "delivered", "delivered", "cancelled", "pending"];
    let mut orders = vec![];
    let mut items = vec![];
    let mut item_id = 0;
    for i in 1..=64u64 {
        let n = r.range(1, 4);
        let mut total = 0.0;
        for _ in 0..n {
            item_id += 1;
            let p = r.range(1, products_src.len() as u64) as usize;
            let q = r.range(1, 3);
            let price: f64 = products_src[p - 1].2.parse().unwrap();
            total += price * q as f64;
            items.push(vec![Some(item_id.to_string()), Some(i.to_string()), Some(p.to_string()), Some(q.to_string()), Some(format!("{:.2}", price))]);
        }
        orders.push(vec![
            Some(i.to_string()),
            Some(r.range(1, 48).to_string()),
            Some(format!("2026-{:02}-{:02} {:02}:{:02}:{:02}", r.range(1, 10), r.range(1, 28), r.range(0, 23), r.range(0, 59), r.range(0, 59))),
            Some(r.pick(&statuses).into()),
            Some(format!("{:.2}", total)),
            if i % 6 == 0 { Some("Gift wrap please".into()) } else { None },
        ]);
    }
    let depts = ["Sales", "Support", "Warehouse", "Finance", "IT"];
    let titles = ["Associate", "Specialist", "Lead", "Manager"];
    let employees: Vec<Row> = (1..=18u64)
        .map(|i| {
            vec![
                Some(i.to_string()),
                Some(format!("{} {}", r.pick(&firsts), r.pick(&lasts))),
                Some(r.pick(&depts).into()),
                Some(r.pick(&titles).into()),
                Some(format!("20{:02}-{:02}-{:02}", r.range(15, 25), r.range(1, 12), r.range(1, 28))),
                Some(format!("{}.00", r.range(30, 120) * 1000)),
                if i == 1 { None } else { Some(r.range(1, i - 1).to_string()) },
            ]
        })
        .collect();
    let pages = ["/", "/products", "/products/mug", "/cart", "/checkout", "/about", "/blog/spring-sale", "/contact"];
    let devices = ["desktop", "mobile", "tablet"];
    let page_views: Vec<Row> = (1..=120u64)
        .map(|i| {
            vec![
                Some(i.to_string()),
                Some(r.pick(&pages).into()),
                Some(r.pick(&devices).into()),
                Some(format!("{}.{}.{}.{}", r.range(10, 220), r.range(0, 255), r.range(0, 255), r.range(1, 254))),
                Some(r.range(200, 9000).to_string()),
                Some(format!("2026-11-{:02} {:02}:{:02}:{:02}", r.range(1, 23), r.range(0, 23), r.range(0, 59), r.range(0, 59))),
            ]
        })
        .collect();
    let daily: Vec<Row> = (1..=23u64)
        .map(|d| vec![Some(format!("2026-11-{:02}", d)), Some(r.range(300, 1500).to_string()), Some(r.range(80, 600).to_string()), Some(format!("{:.2}", r.range(1500, 6000) as f64 / 100.0))])
        .collect();

    let t = |name: &str, comment: &str, fields: Vec<FieldDef>, rows: Vec<Row>| {
        let auto = rows.len() as u64;
        DemoTable { name: name.into(), fields, rows, comment: comment.into(), auto_inc: auto, created: "2026-01-12 09:30:00".into(), modified: "2026-11-20 16:05:12".into() }
    };
    DemoServer {
        dbs: vec![
            DemoDb {
                name: "analytics".into(),
                tables: vec![
                    t("daily_stats", "Rolled-up traffic per day", vec![f("day", "date", "", true, true, false, ""), f("visits", "int", "", true, false, false, ""), f("unique_visitors", "int", "", true, false, false, ""), f("bounce_rate", "decimal", "5,2", false, false, false, "Percent")], daily),
                    t("page_views", "Raw page view events", vec![f("id", "bigint", "", true, true, true, ""), f("path", "varchar", "255", true, false, false, ""), f("device", "varchar", "20", false, false, false, ""), f("ip_address", "varchar", "45", false, false, false, ""), f("duration_ms", "int", "", false, false, false, "Time on page"), f("viewed_at", "datetime", "", true, false, false, "")], page_views),
                ],
            },
            DemoDb {
                name: "shop".into(),
                tables: vec![
                    t("customers", "Registered customers", vec![f("id", "int", "11", true, true, true, ""), f("first_name", "varchar", "50", true, false, false, ""), f("last_name", "varchar", "50", true, false, false, ""), f("email", "varchar", "120", true, false, false, "Login e-mail"), f("phone", "varchar", "30", false, false, false, ""), f("city", "varchar", "60", false, false, false, ""), f("country", "varchar", "60", false, false, false, ""), f("created_at", "datetime", "", true, false, false, "")], customers),
                    t("employees", "Staff directory", vec![f("id", "int", "11", true, true, true, ""), f("full_name", "varchar", "100", true, false, false, ""), f("department", "varchar", "40", false, false, false, ""), f("title", "varchar", "40", false, false, false, ""), f("hire_date", "date", "", false, false, false, ""), f("salary", "decimal", "10,2", false, false, false, ""), f("manager_id", "int", "11", false, false, false, "")], employees),
                    t("order_items", "Lines of each order", vec![f("id", "int", "11", true, true, true, ""), f("order_id", "int", "11", true, false, false, ""), f("product_id", "int", "11", true, false, false, ""), f("quantity", "int", "11", true, false, false, ""), f("unit_price", "decimal", "10,2", true, false, false, "")], items),
                    t("orders", "Customer orders", vec![f("id", "int", "11", true, true, true, ""), f("customer_id", "int", "11", true, false, false, ""), f("ordered_at", "datetime", "", true, false, false, ""), f("status", "varchar", "20", true, false, false, ""), f("total", "decimal", "10,2", true, false, false, ""), f("note", "text", "", false, false, false, "")], orders),
                    t("products", "Product catalogue", vec![f("id", "int", "11", true, true, true, ""), f("sku", "varchar", "20", true, false, false, ""), f("name", "varchar", "120", true, false, false, ""), f("category", "varchar", "40", false, false, false, ""), f("price", "decimal", "10,2", true, false, false, ""), f("stock", "int", "11", true, false, false, ""), f("active", "tinyint", "1", true, false, false, "")], products),
                ],
            },
        ],
    }
}
