//! Column definitions in CREATE TABLE and ALTER TABLE.

use super::parser::{Parser, Res};
use super::tokens::Tok;
use crate::db::DbError;
use crate::domain::FieldDef;

/// `type[(size[, decimals])] [UNSIGNED] [CHARACTER SET x] [COLLATE y]`
fn parse_type(p: &mut Parser) -> Res<(String, String, String)> {
    let mut ty = p.ident()?.to_lowercase();
    if ty == "double" && p.kw("PRECISION") {
        ty = "double precision".into();
    }
    if ty == "character" && p.kw("VARYING") {
        ty = "varchar".into();
    }
    let (mut len, mut dec) = (String::new(), String::new());
    if p.sym("(") {
        let parts = p.comma_list(size_part)?;
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

fn size_part(p: &mut Parser) -> Res<String> {
    let part = match p.peek().cloned() {
        Some(Tok::Num(n)) => n,
        Some(Tok::Str(s)) => format!("'{}'", s),
        _ => return Err(DbError::query(format!("Expected a size near {}", p.near()))),
    };
    p.i += 1;
    Ok(part)
}

pub(super) fn parse_coldef(p: &mut Parser) -> Res<FieldDef> {
    let name = p.ident()?;
    let (ty, length, decimals) = parse_type(p)?;
    let mut f = FieldDef { name, ty, length, decimals, ..Default::default() };
    while column_option(p, &mut f)? {}
    Ok(f)
}

/// Reads one option after the type; false when there are no more.
fn column_option(p: &mut Parser, f: &mut FieldDef) -> Res<bool> {
    if p.kw("NOT") {
        p.expect_kw("NULL")?;
        f.not_null = true;
    } else if p.kw("NULL") {
        f.not_null = false;
    } else if p.kw("DEFAULT") {
        f.default = p.literal()?.unwrap_or_else(|| "NULL".into());
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
        return Ok(false);
    }
    Ok(true)
}

/// Marks the named fields as the primary key (and NOT NULL).
pub(super) fn set_primary_key(fields: &mut [FieldDef], keys: &[String]) -> Res<()> {
    for k in keys {
        let i = super::parser::field_index(fields, k, "Unknown key column")?;
        fields[i].key = true;
        fields[i].not_null = true;
    }
    Ok(())
}
