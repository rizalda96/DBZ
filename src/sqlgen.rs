//! SQL text helpers: quoting, statement splitting, DDL generation, CSV.

use crate::model::{Cell, FieldDef};

pub fn quote_ident(pg: bool, name: &str) -> String {
    if pg {
        if let Some((schema, table)) = name.split_once('.') {
            return format!("\"{}\".\"{}\"", schema.replace('"', "\"\""), table.replace('"', "\"\""));
        }
        format!("\"{}\"", name.replace('"', "\"\""))
    } else {
        format!("`{}`", name.replace('`', "``"))
    }
}

pub fn literal(pg: bool, v: &Cell) -> String {
    match v {
        None => "NULL".to_string(),
        Some(s) => {
            let mut e = s.replace('\'', "''");
            if !pg {
                e = e.replace('\\', "\\\\");
            }
            format!("'{}'", e)
        }
    }
}

/// Split a script into statements on `;`, respecting quotes and comments.
pub fn split_statements(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            cur.push(c);
            if c == '\\' && q != '"' && i + 1 < chars.len() {
                cur.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                quote = Some(c);
                cur.push(c);
            }
            '-' if i + 1 < chars.len() && chars[i + 1] == '-' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                cur.push('\n');
            }
            '#' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                cur.push('\n');
            }
            '/' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i += 1;
            }
            ';' => {
                let s = cur.trim().to_string();
                if !s.is_empty() {
                    out.push(s);
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
        i += 1;
    }
    let s = cur.trim().to_string();
    if !s.is_empty() {
        out.push(s);
    }
    out
}

pub fn column_def(pg: bool, f: &FieldDef) -> String {
    let mut ty = f.type_text();
    if pg && f.auto_inc {
        ty = match f.ty.to_lowercase().as_str() {
            "bigint" | "int8" => "bigserial".into(),
            "smallint" | "int2" => "smallserial".into(),
            _ => "serial".into(),
        };
    }
    let mut s = format!("{} {}", quote_ident(pg, &f.name), ty);
    if f.not_null {
        s.push_str(" NOT NULL");
    }
    if !f.default.trim().is_empty() && !(pg && f.auto_inc) {
        s.push_str(&format!(" DEFAULT {}", default_expr(pg, f.default.trim())));
    }
    if !pg && f.auto_inc {
        s.push_str(" AUTO_INCREMENT");
    }
    if !pg && !f.comment.is_empty() {
        s.push_str(&format!(" COMMENT {}", literal(false, &Some(f.comment.clone()))));
    }
    s
}

fn default_expr(pg: bool, d: &str) -> String {
    let upper = d.to_uppercase();
    let is_num = d.parse::<f64>().is_ok();
    if is_num
        || upper == "NULL"
        || upper.starts_with("CURRENT_")
        || upper.ends_with(')')
        || d.starts_with('\'')
        || upper == "TRUE"
        || upper == "FALSE"
    {
        d.to_string()
    } else {
        literal(pg, &Some(d.to_string()))
    }
}

pub fn create_table(pg: bool, table: &str, fields: &[FieldDef], comment: &str) -> Vec<String> {
    let mut parts: Vec<String> = fields.iter().filter(|f| !f.name.trim().is_empty()).map(|f| format!("  {}", column_def(pg, f))).collect();
    let keys: Vec<String> = fields.iter().filter(|f| f.key && !f.name.trim().is_empty()).map(|f| quote_ident(pg, &f.name)).collect();
    if !keys.is_empty() {
        parts.push(format!("  PRIMARY KEY ({})", keys.join(", ")));
    }
    let mut sql = format!("CREATE TABLE {} (\n{}\n)", quote_ident(pg, table), parts.join(",\n"));
    if !pg && !comment.is_empty() {
        sql.push_str(&format!(" COMMENT = {}", literal(false, &Some(comment.to_string()))));
    }
    let mut out = vec![sql];
    if pg {
        if !comment.is_empty() {
            out.push(format!("COMMENT ON TABLE {} IS {}", quote_ident(true, table), literal(true, &Some(comment.into()))));
        }
        for f in fields.iter().filter(|f| !f.comment.is_empty()) {
            out.push(format!(
                "COMMENT ON COLUMN {}.{} IS {}",
                quote_ident(true, table),
                quote_ident(true, &f.name),
                literal(true, &Some(f.comment.clone()))
            ));
        }
    }
    out
}

/// Statements that turn `old` into `new` for an existing table.
pub fn alter_table(
    pg: bool,
    table: &str,
    old: &[FieldDef],
    new: &[FieldDef],
    old_comment: &str,
    comment: &str,
    new_name: &str,
) -> Vec<String> {
    let t = quote_ident(pg, table);
    let mut out = Vec::new();
    let mut actions = Vec::new();
    // dropped fields
    for o in old {
        if !new.iter().any(|n| n.orig_name.as_deref() == Some(o.name.as_str())) {
            if pg {
                out.push(format!("ALTER TABLE {} DROP COLUMN {}", t, quote_ident(pg, &o.name)));
            } else {
                actions.push(format!("DROP COLUMN {}", quote_ident(pg, &o.name)));
            }
        }
    }
    let old_keys: Vec<&str> = old.iter().filter(|f| f.key).map(|f| f.name.as_str()).collect();
    let new_keys: Vec<String> = new.iter().filter(|f| f.key && !f.name.trim().is_empty()).map(|f| f.name.clone()).collect();
    let keys_changed = old_keys.len() != new_keys.len()
        || new.iter().filter(|f| f.key).any(|f| f.orig_name.as_deref().map(|o| !old_keys.contains(&o)).unwrap_or(true))
        || new.iter().any(|f| !f.key && f.orig_name.as_deref().map(|o| old_keys.contains(&o)).unwrap_or(false));
    if keys_changed && !old_keys.is_empty() {
        if pg {
            out.push(format!("ALTER TABLE {} DROP CONSTRAINT IF EXISTS {}", t, quote_ident(true, &format!("{}_pkey", table.rsplit('.').next().unwrap_or(table)))));
        } else {
            actions.push("DROP PRIMARY KEY".into());
        }
    }
    let mut prev: Option<&str> = None;
    for n in new {
        if n.name.trim().is_empty() {
            continue;
        }
        let pos = if pg {
            String::new()
        } else {
            match prev {
                None => " FIRST".into(),
                Some(p) => format!(" AFTER {}", quote_ident(false, p)),
            }
        };
        match &n.orig_name {
            None => {
                if pg {
                    out.push(format!("ALTER TABLE {} ADD COLUMN {}", t, column_def(true, n)));
                    if !n.comment.is_empty() {
                        out.push(format!("COMMENT ON COLUMN {}.{} IS {}", t, quote_ident(true, &n.name), literal(true, &Some(n.comment.clone()))));
                    }
                } else {
                    actions.push(format!("ADD COLUMN {}{}", column_def(false, n), pos));
                }
            }
            Some(orig) => {
                let Some(o) = old.iter().find(|o| &o.name == orig) else { continue };
                let mut a = o.clone();
                a.orig_name = None;
                a.key = false;
                let mut b = n.clone();
                b.orig_name = None;
                b.key = false;
                b.name = orig.clone();
                let renamed = &n.name != orig;
                if a == b && !renamed {
                    prev = Some(&n.name);
                    continue;
                }
                if pg {
                    let col = quote_ident(true, &n.name);
                    if renamed {
                        out.push(format!("ALTER TABLE {} RENAME COLUMN {} TO {}", t, quote_ident(true, orig), col));
                    }
                    if o.type_text() != n.type_text() {
                        out.push(format!("ALTER TABLE {} ALTER COLUMN {} TYPE {} USING {}::{}", t, col, n.type_text(), col, n.type_text()));
                    }
                    if o.not_null != n.not_null {
                        out.push(format!("ALTER TABLE {} ALTER COLUMN {} {} NOT NULL", t, col, if n.not_null { "SET" } else { "DROP" }));
                    }
                    if o.default != n.default {
                        if n.default.trim().is_empty() {
                            out.push(format!("ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT", t, col));
                        } else {
                            out.push(format!("ALTER TABLE {} ALTER COLUMN {} SET DEFAULT {}", t, col, default_expr(true, n.default.trim())));
                        }
                    }
                    if o.comment != n.comment {
                        out.push(format!("COMMENT ON COLUMN {}.{} IS {}", t, col, literal(true, &Some(n.comment.clone()))));
                    }
                } else {
                    actions.push(format!("CHANGE COLUMN {} {}{}", quote_ident(false, orig), column_def(false, n), pos));
                }
            }
        }
        prev = Some(&n.name);
    }
    if keys_changed && !new_keys.is_empty() {
        let cols: Vec<String> = new_keys.iter().map(|k| quote_ident(pg, k)).collect();
        if pg {
            out.push(format!("ALTER TABLE {} ADD PRIMARY KEY ({})", t, cols.join(", ")));
        } else {
            actions.push(format!("ADD PRIMARY KEY ({})", cols.join(", ")));
        }
    }
    if !pg && old_comment != comment {
        actions.push(format!("COMMENT = {}", literal(false, &Some(comment.to_string()))));
    }
    if pg && old_comment != comment {
        out.push(format!("COMMENT ON TABLE {} IS {}", t, literal(true, &Some(comment.to_string()))));
    }
    if !actions.is_empty() {
        out.insert(0, format!("ALTER TABLE {}\n  {}", t, actions.join(",\n  ")));
    }
    if !new_name.is_empty() && new_name != table {
        if pg {
            out.push(format!("ALTER TABLE {} RENAME TO {}", t, quote_ident(true, new_name.rsplit('.').next().unwrap_or(new_name))));
        } else {
            out.push(format!("ALTER TABLE {} RENAME TO {}", t, quote_ident(false, new_name)));
        }
    }
    out
}

// ---------- CSV ----------

pub fn csv_escape(v: &Cell) -> String {
    match v {
        None => String::new(),
        Some(s) => {
            if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.clone()
            }
        }
    }
}

pub fn parse_csv(text: &str, delim: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_q = false;
    let mut chars = text.chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        any = true;
        if in_q {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_q = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' {
            in_q = true;
        } else if c == delim {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' {
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
            any = false;
        } else if c != '\r' {
            field.push(c);
        }
    }
    if any {
        row.push(field);
        rows.push(row);
    }
    rows.retain(|r| !(r.len() == 1 && r[0].is_empty()));
    rows
}

pub fn json_escape(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}
