//! Indexes, constraints, triggers and table options in the sample engine. They are
//! kept as the designer shows them; the engine doesn't enforce them.

use super::parser::{Parser, Res};
use super::server::SampleServer;
use super::tokens::Tok;
use crate::db::DbError;
use crate::domain::{CheckDef, ForeignKeyDef, IndexDef, TableOptions, TableParts, TriggerDef, UniqueDef};

impl SampleServer {
    /// `CREATE [UNIQUE] INDEX n ON t (cols) [USING m] [COMMENT 'c']`, after INDEX.
    pub(super) fn create_index(&mut self, db: &str, p: &mut Parser, unique: bool) -> Res<()> {
        let name = p.ident()?;
        p.expect_kw("ON")?;
        let table = p.ident()?;
        let columns = raw_parens(p)?;
        let mut ix = IndexDef { name, columns, unique, ..Default::default() };
        while !p.at_end() && !p.is_sym(";") {
            if p.kw("USING") {
                ix.method = p.ident()?.to_uppercase();
            } else if p.kw("COMMENT") {
                ix.comment = p.literal()?.unwrap_or_default();
            } else {
                return Err(DbError::query(format!("Unexpected {} in CREATE INDEX", p.near())));
            }
        }
        let parts = &mut self.table_mut(db, &table)?.parts;
        if parts.indexes.iter().any(|i| i.name.eq_ignore_ascii_case(&ix.name)) {
            return Err(DbError::query(format!("Duplicate key name '{}'", ix.name)));
        }
        parts.indexes.push(ix);
        Ok(())
    }

    /// `DROP INDEX n ON t`, after INDEX.
    pub(super) fn drop_index(&mut self, db: &str, p: &mut Parser) -> Res<()> {
        let name = p.ident()?;
        p.expect_kw("ON")?;
        let table = p.ident()?;
        p.done()?;
        remove_named(&mut self.table_mut(db, &table)?.parts.indexes, &name, |i| &i.name)
    }

    /// `CREATE TRIGGER n BEFORE|AFTER event ON t FOR EACH ROW body`, after TRIGGER.
    pub(super) fn create_trigger(&mut self, db: &str, p: &mut Parser) -> Res<()> {
        let name = p.ident()?;
        let fires = if p.kw("BEFORE") { "BEFORE" } else { p.expect_kw("AFTER").map(|_| "AFTER")? };
        let event = p.ident()?.to_uppercase();
        p.expect_kw("ON")?;
        let table = p.ident()?;
        p.expect_kw("FOR")?;
        p.expect_kw("EACH")?;
        p.expect_kw("ROW")?;
        let body = raw_rest(p);
        let trigger = TriggerDef {
            name,
            for_each: "ROW".into(),
            fires: fires.into(),
            insert: event == "INSERT",
            update: event == "UPDATE",
            delete: event == "DELETE",
            enabled: true,
            body,
            ..Default::default()
        };
        self.table_mut(db, &table)?.parts.triggers.push(trigger);
        Ok(())
    }

    /// `DROP TRIGGER n`, after TRIGGER; triggers are named per database.
    pub(super) fn drop_trigger(&mut self, db: &str, p: &mut Parser) -> Res<()> {
        let name = p.ident()?;
        p.done()?;
        let d = self.db_mut(db)?;
        let table = d.tables.iter_mut().find(|t| t.parts.triggers.iter().any(|tr| tr.name.eq_ignore_ascii_case(&name)));
        let Some(table) = table else { return Err(DbError::query(format!("Trigger does not exist: {}", name))) };
        table.parts.triggers.retain(|tr| !tr.name.eq_ignore_ascii_case(&name));
        Ok(())
    }
}

/// ALTER TABLE … ADD CONSTRAINT n FOREIGN KEY | UNIQUE | CHECK, after CONSTRAINT.
pub(super) fn add_constraint(parts: &mut TableParts, p: &mut Parser) -> Res<()> {
    let name = p.ident()?;
    if p.kw("UNIQUE") {
        p.kw("KEY");
        parts.uniques.push(UniqueDef { name, columns: raw_parens(p)?, ..Default::default() });
    } else if p.kw("CHECK") {
        parts.checks.push(CheckDef { name, expr: raw_parens(p)?, ..Default::default() });
    } else {
        p.expect_kw("FOREIGN")?;
        p.expect_kw("KEY")?;
        parts.foreign_keys.push(foreign_key(name, p)?);
    }
    Ok(())
}

fn foreign_key(name: String, p: &mut Parser) -> Res<ForeignKeyDef> {
    let columns = raw_parens(p)?;
    p.expect_kw("REFERENCES")?;
    let (ref_schema, ref_table) = qualified_name(p)?;
    let ref_columns = raw_parens(p)?;
    let mut fk = ForeignKeyDef { name, columns, ref_schema, ref_table, ref_columns, on_delete: "NO ACTION".into(), on_update: "NO ACTION".into(), ..Default::default() };
    while p.kw("ON") {
        let delete = p.kw("DELETE");
        if !delete {
            p.expect_kw("UPDATE")?;
        }
        let action = fk_action(p)?;
        if delete {
            fk.on_delete = action;
        } else {
            fk.on_update = action;
        }
    }
    Ok(fk)
}

fn fk_action(p: &mut Parser) -> Res<String> {
    let action = if p.kw("CASCADE") {
        "CASCADE"
    } else if p.kw("RESTRICT") {
        "RESTRICT"
    } else if p.kw("NO") {
        p.expect_kw("ACTION").map(|_| "NO ACTION")?
    } else {
        p.expect_kw("SET")?;
        if p.kw("NULL") { "SET NULL" } else { p.expect_kw("DEFAULT").map(|_| "SET DEFAULT")? }
    };
    Ok(action.into())
}

/// `db.table` or `table` → (db or "", table).
fn qualified_name(p: &mut Parser) -> Res<(String, String)> {
    let first = name_token(p)?;
    if !p.sym(".") {
        return Ok((String::new(), first));
    }
    Ok((first, name_token(p)?))
}

fn name_token(p: &mut Parser) -> Res<String> {
    match p.peek().cloned() {
        Some(Tok::Ident(s)) | Some(Tok::QIdent(s)) => {
            p.i += 1;
            Ok(s)
        }
        _ => Err(DbError::query(format!("Expected a name near {}", p.near()))),
    }
}

/// ALTER TABLE … DROP FOREIGN KEY | INDEX | KEY | CONSTRAINT | CHECK n, after DROP.
/// False when the action drops something else (a column, the primary key).
pub(super) fn drop_part(parts: &mut TableParts, p: &mut Parser) -> Res<bool> {
    if p.kw("FOREIGN") {
        p.expect_kw("KEY")?;
        let name = p.ident()?;
        remove_named(&mut parts.foreign_keys, &name, |f| &f.name)?;
    } else if p.kw("INDEX") || p.kw("KEY") {
        let name = p.ident()?;
        let before = parts.uniques.len() + parts.indexes.len();
        parts.uniques.retain(|u| !u.name.eq_ignore_ascii_case(&name));
        parts.indexes.retain(|i| !i.name.eq_ignore_ascii_case(&name));
        if before == parts.uniques.len() + parts.indexes.len() {
            return Err(DbError::query(format!("Can't DROP '{}'; check that it exists", name)));
        }
    } else if p.kw("CONSTRAINT") || p.kw("CHECK") {
        let name = p.ident()?;
        let found = remove_named(&mut parts.checks, &name, |c| &c.name).is_ok() || remove_named(&mut parts.uniques, &name, |u| &u.name).is_ok();
        if !found {
            remove_named(&mut parts.foreign_keys, &name, |f| &f.name)?;
        }
    } else {
        return Ok(false);
    }
    Ok(true)
}

/// `ENGINE = x`, `DEFAULT CHARSET = x`, … in ALTER TABLE; false when the action is not
/// a table option.
pub(super) fn table_option(options: &mut TableOptions, p: &mut Parser) -> Res<bool> {
    let key = if p.kw("ENGINE") {
        "engine"
    } else if p.kw("COLLATE") {
        "collation"
    } else if p.kw("AUTO_INCREMENT") {
        "auto_increment"
    } else if p.kw("ROW_FORMAT") {
        "row_format"
    } else if p.kw("CHARSET") || (p.kw("DEFAULT") && (p.kw("CHARSET") || p.kw("CHARACTER"))) || p.kw("CHARACTER") {
        p.kw("SET");
        "charset"
    } else {
        return Ok(false);
    };
    p.sym("=");
    let value = match p.peek().cloned() {
        Some(Tok::Ident(s)) | Some(Tok::QIdent(s)) | Some(Tok::Num(s)) | Some(Tok::Str(s)) => s,
        _ => return Err(DbError::query(format!("Expected a value near {}", p.near()))),
    };
    p.i += 1;
    options.insert(key.to_string(), value);
    Ok(true)
}

fn remove_named<T>(items: &mut Vec<T>, name: &str, name_of: impl Fn(&T) -> &String) -> Res<()> {
    let before = items.len();
    items.retain(|i| !name_of(i).eq_ignore_ascii_case(name));
    if items.len() == before {
        return Err(DbError::query(format!("Can't DROP '{}'; check that it exists", name)));
    }
    Ok(())
}

/// The tokens inside `( … )` as text; the cursor ends after the closing parenthesis.
fn raw_parens(p: &mut Parser) -> Res<String> {
    p.expect_sym("(")?;
    let mut depth = 1;
    let mut toks = vec![];
    loop {
        let Some(tok) = p.peek().cloned() else { return Err(DbError::query("Missing ')'")) };
        p.i += 1;
        match &tok {
            Tok::Sym(s) if s == "(" => depth += 1,
            Tok::Sym(s) if s == ")" => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return Ok(join(&toks));
        }
        toks.push(tok);
    }
}

/// The rest of the statement as text (a trigger body).
fn raw_rest(p: &mut Parser) -> String {
    let rest: Vec<Tok> = p.t[p.i..].iter().filter(|t| !matches!(t, Tok::Sym(s) if s == ";")).cloned().collect();
    p.i = p.t.len();
    join(&rest)
}

fn join(toks: &[Tok]) -> String {
    let mut out = String::new();
    for t in toks {
        let tight_before = matches!(t, Tok::Sym(s) if s == "," || s == ")" || s == "." || (s == "(" && out.ends_with(|c: char| c.is_alphanumeric())));
        if !out.is_empty() && !tight_before && !out.ends_with('(') && !out.ends_with('.') {
            out.push(' ');
        }
        out.push_str(&tok_text(t));
    }
    out
}

fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Ident(s) | Tok::Num(s) | Tok::Sym(s) => s.clone(),
        Tok::QIdent(s) if s.chars().all(|c| c.is_alphanumeric() || c == '_') => s.clone(),
        Tok::QIdent(s) => format!("`{}`", s.replace('`', "``")),
        Tok::Str(s) => format!("'{}'", s.replace('\'', "''")),
    }
}
