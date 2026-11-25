//! Token cursor with the small parsing helpers every statement uses.

use super::tokens::Tok;
use crate::db::DbError;
use crate::domain::{Cell, FieldDef};
use crate::shared::now_string;

pub(super) type Res<T> = Result<T, DbError>;

/// Words that end a select item, so they are never taken as an alias.
const CLAUSE_WORDS: [&str; 10] = ["FROM", "WHERE", "ORDER", "LIMIT", "GROUP", "OFFSET", "ASC", "DESC", "AND", "OR"];

pub(super) struct Parser {
    pub t: Vec<Tok>,
    pub i: usize,
}

impl Parser {
    pub fn new(t: Vec<Tok>) -> Self {
        Parser { t, i: 0 }
    }

    pub fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    pub fn at_end(&self) -> bool {
        self.i >= self.t.len()
    }

    pub fn is_kw(&self, k: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if s.eq_ignore_ascii_case(k))
    }

    pub fn is_sym(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Sym(x)) if x == s)
    }

    pub fn kw(&mut self, k: &str) -> bool {
        let hit = self.is_kw(k);
        if hit {
            self.i += 1;
        }
        hit
    }

    pub fn expect_kw(&mut self, k: &str) -> Res<()> {
        if self.kw(k) {
            return Ok(());
        }
        Err(DbError::query(format!("Expected {} near {}", k, self.near())))
    }

    pub fn sym(&mut self, s: &str) -> bool {
        let hit = self.is_sym(s);
        if hit {
            self.i += 1;
        }
        hit
    }

    pub fn expect_sym(&mut self, s: &str) -> Res<()> {
        if self.sym(s) {
            return Ok(());
        }
        Err(DbError::query(format!("Expected '{}' near {}", s, self.near())))
    }

    pub fn near(&self) -> String {
        match self.peek() {
            None => "end of statement".into(),
            Some(Tok::Ident(s)) | Some(Tok::Num(s)) | Some(Tok::Sym(s)) | Some(Tok::Str(s)) => format!("'{}'", s),
            Some(Tok::QIdent(s)) => format!("`{}`", s),
        }
    }

    /// A name; for `db.table` or `table.column` the last part is kept.
    pub fn ident(&mut self) -> Res<String> {
        let mut name = match self.peek().cloned() {
            Some(Tok::Ident(s)) | Some(Tok::QIdent(s)) => s,
            _ => return Err(DbError::query(format!("Expected a name near {}", self.near()))),
        };
        self.i += 1;
        while self.sym(".") {
            name = match self.peek().cloned() {
                Some(Tok::Ident(s)) | Some(Tok::QIdent(s)) => s,
                Some(Tok::Sym(s)) if s == "*" => s,
                _ => return Err(DbError::query("Expected a name after '.'")),
            };
            self.i += 1;
        }
        Ok(name)
    }

    pub fn literal(&mut self) -> Res<Cell> {
        let neg = self.sym("-");
        let tok = self.peek().cloned();
        match tok {
            Some(Tok::Str(s)) => {
                self.i += 1;
                Ok(Some(s))
            }
            Some(Tok::Num(n)) => {
                self.i += 1;
                Ok(Some(if neg { format!("-{}", n) } else { n }))
            }
            Some(Tok::Ident(s)) => {
                self.i += 1;
                self.word_literal(&s)
            }
            _ => Err(DbError::query(format!("Expected a value near {}", self.near()))),
        }
    }

    fn word_literal(&mut self, word: &str) -> Res<Cell> {
        let u = word.to_uppercase();
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
            _ => Err(DbError::query(format!("Unsupported value '{}' (the sample database only understands plain values)", word))),
        }
    }

    /// Accepts trailing semicolons and fails on anything else.
    pub fn done(&mut self) -> Res<()> {
        while self.sym(";") {}
        if self.at_end() {
            return Ok(());
        }
        Err(DbError::query(format!("Unexpected {} — the sample database supports simple statements only", self.near())))
    }

    pub fn num(&mut self) -> Res<usize> {
        match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.i += 1;
                n.parse().map_err(|_| DbError::query(format!("Bad number {}", n)))
            }
            _ => Err(DbError::query(format!("Expected a number near {}", self.near()))),
        }
    }

    /// `(a, b, c)`
    pub fn ident_list(&mut self) -> Res<Vec<String>> {
        self.expect_sym("(")?;
        let v = self.comma_list(|p| p.ident())?;
        self.expect_sym(")")?;
        Ok(v)
    }

    /// One or more items separated by commas.
    pub fn comma_list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Res<T>) -> Res<Vec<T>> {
        let mut v = vec![item(self)?];
        while self.sym(",") {
            v.push(item(self)?);
        }
        Ok(v)
    }

    /// `AS name` or a bare name that is not a clause keyword.
    pub fn alias(&mut self) -> Res<Option<String>> {
        if self.kw("AS") {
            return Ok(Some(self.ident()?));
        }
        let (Some(Tok::Ident(s)) | Some(Tok::QIdent(s))) = self.peek().cloned() else { return Ok(None) };
        if CLAUSE_WORDS.contains(&s.to_uppercase().as_str()) {
            return Ok(None);
        }
        self.i += 1;
        Ok(Some(s))
    }

    /// `FIRST` or `AFTER col` in ALTER TABLE; the new index.
    pub fn position(&mut self, fields: &[FieldDef]) -> Res<Option<usize>> {
        if self.kw("FIRST") {
            return Ok(Some(0));
        }
        if !self.kw("AFTER") {
            return Ok(None);
        }
        let c = self.ident()?;
        Ok(Some(field_index(fields, &c, "Unknown column")? + 1))
    }
}

/// Index of the field named `name` (any case), or `"{msg} 'name'"`.
pub(super) fn field_index(fields: &[FieldDef], name: &str, msg: &str) -> Res<usize> {
    fields.iter().position(|f| f.name.eq_ignore_ascii_case(name)).ok_or_else(|| DbError::query(format!("{} '{}'", msg, name)))
}
