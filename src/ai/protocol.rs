//! The text protocol between DBZ and the AI: a reply either answers, or calls one tool
//! in a ```` ```dbz ```` block of JSON. Tool results go back as plain text, cut to size.

use serde::Deserialize;

use crate::domain::{AiTool, DbRead, FieldDef, ForeignKeyDef, ResultSet, RoutineInfo, Segment, TableInfo, TableParts, TriggerDef};

/// Rows of a result the AI sees; the server is asked for one more to tell it was cut.
pub const MAX_ROWS: usize = 200;
const MAX_CELL: usize = 2_000;
/// Characters of one tool result.
pub const MAX_TOTAL: usize = 30_000;
const FENCE: &str = "```";
const DBZ_FENCE: &str = "```dbz";

#[derive(Debug, PartialEq)]
pub enum Parsed {
    Answer(String),
    Call { text: String, tool: AiTool },
    Invalid { text: String, error: String },
}

/// The JSON inside a ```` ```dbz ```` block.
#[derive(Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
enum Call {
    ListObjects,
    DescribeTable {
        table: String,
    },
    GetDdl {
        name: String,
        #[serde(default)]
        is_view: bool,
    },
    RunSelect {
        sql: String,
    },
    Explain {
        sql: String,
    },
    ActiveEditor,
    ProposeChange {
        title: String,
        sql: String,
        #[serde(default)]
        view: Option<String>,
    },
}

impl From<Call> for AiTool {
    fn from(c: Call) -> Self {
        match c {
            Call::ListObjects => AiTool::Db(DbRead::ListObjects),
            Call::DescribeTable { table } => AiTool::Db(DbRead::DescribeTable { table }),
            Call::GetDdl { name, is_view } => AiTool::Db(DbRead::GetDdl { name, is_view }),
            Call::RunSelect { sql } => AiTool::Db(DbRead::Select { sql }),
            Call::Explain { sql } => AiTool::Db(DbRead::Explain { sql }),
            Call::ActiveEditor => AiTool::ActiveEditor,
            Call::ProposeChange { title, sql, view } => AiTool::ProposeChange { title, sql, view: view.filter(|v| !v.trim().is_empty()) },
        }
    }
}

/// Splits an AI reply into its text and the one tool it calls, if any.
pub fn parse_reply(reply: &str) -> Parsed {
    let reply = reply.trim();
    let Some(start) = find_dbz(reply) else { return Parsed::Answer(reply.to_string()) };
    let text = reply[..start].trim().to_string();
    let after = &reply[start + DBZ_FENCE.len()..];
    let Some(end) = after.find(FENCE) else {
        return Parsed::Invalid { text, error: "the ```dbz block is not closed".into() };
    };
    if find_dbz(&after[end + FENCE.len()..]).is_some() {
        return Parsed::Invalid { text, error: "only one tool call per message is allowed; send one and wait for its result".into() };
    }
    match serde_json::from_str::<Call>(after[..end].trim()) {
        Ok(call) => Parsed::Call { text, tool: call.into() },
        Err(e) => Parsed::Invalid { text, error: e.to_string() },
    }
}

/// Where a ```` ```dbz ```` fence starts (not ```` ```dbzx ````).
fn find_dbz(s: &str) -> Option<usize> {
    s.match_indices(DBZ_FENCE).map(|(i, _)| i).find(|i| s[i + DBZ_FENCE.len()..].chars().next().is_none_or(char::is_whitespace))
}

/// An answer as text and SQL pieces: ```` ```sql ```` and bare ```` ``` ```` fences are SQL,
/// other fences stay text.
pub fn segments(answer: &str) -> Vec<Segment> {
    let mut out = vec![];
    let mut rest = answer;
    while let Some(open) = rest.find(FENCE) {
        push_text(&mut out, &rest[..open]);
        let after = &rest[open + FENCE.len()..];
        let (lang, body) = after.split_once('\n').unwrap_or((after, ""));
        let Some(close) = body.find(FENCE) else {
            push_text(&mut out, &rest[open..]);
            return out;
        };
        let code = body[..close].trim_matches('\n').trim_end();
        match lang.trim().to_lowercase().as_str() {
            "" | "sql" => out.push(Segment::Sql(code.to_string())),
            _ => push_text(&mut out, &rest[open..open + FENCE.len() + lang.len() + 1 + close + FENCE.len()]),
        }
        rest = &body[close + FENCE.len()..];
    }
    push_text(&mut out, rest);
    out
}

fn push_text(out: &mut Vec<Segment>, text: &str) {
    let text = text.trim();
    if !text.is_empty() {
        out.push(Segment::Text(text.to_string()));
    }
}

/// `text` cut to [`MAX_TOTAL`] characters.
pub fn clip(text: &str) -> String {
    if text.chars().count() <= MAX_TOTAL {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(MAX_TOTAL).collect();
    cut.push_str("\n(truncated)");
    cut
}

/// Result sets as `col | col` lines, at most [`MAX_ROWS`] rows each.
pub fn format_sets(sets: &[ResultSet]) -> String {
    let parts: Vec<String> = sets.iter().map(format_set).collect();
    clip(&parts.join("\n\n"))
}

fn format_set(set: &ResultSet) -> String {
    if !set.has_rows {
        return format!("OK, {} row(s) affected", set.affected);
    }
    let mut lines = vec![set.columns.join(" | ")];
    lines.extend(set.rows.iter().take(MAX_ROWS).map(|row| row.iter().map(|c| cell(c.as_deref())).collect::<Vec<_>>().join(" | ")));
    if set.rows.len() > MAX_ROWS {
        lines.push(format!("(truncated: only the first {} rows are shown)", MAX_ROWS));
    } else {
        lines.push(format!("({} row(s))", set.rows.len()));
    }
    lines.join("\n")
}

fn cell(value: Option<&str>) -> String {
    let Some(v) = value else { return "NULL".into() };
    let v = v.replace('\n', "\\n");
    if v.chars().count() <= MAX_CELL {
        return v;
    }
    let mut cut: String = v.chars().take(MAX_CELL).collect();
    cut.push('…');
    cut
}

/// Tables, views and routines of a database, one per line.
pub fn format_objects(tables: &[TableInfo], routines: &[RoutineInfo]) -> String {
    let mut out = String::new();
    let groups: [(&str, fn(&TableInfo) -> bool); 3] = [("Tables", |t| !t.is_view), ("Views", |t| t.is_view && !t.materialized), ("Materialized views", |t| t.materialized)];
    for (title, holds) in groups {
        let names: Vec<String> = tables.iter().filter(|t| holds(t)).map(table_line).collect();
        section(&mut out, title, &names);
    }
    let names: Vec<String> = routines.iter().map(|r| format!("{} ({}{})", r.name, r.kind.to_lowercase(), returns(&r.returns))).collect();
    section(&mut out, "Routines", &names);
    clip(out.trim_end())
}

fn table_line(t: &TableInfo) -> String {
    match t.rows {
        Some(n) => format!("{} (~{} rows)", t.name, n),
        None => t.name.clone(),
    }
}

fn returns(r: &str) -> String {
    if r.is_empty() {
        String::new()
    } else {
        format!(", returns {}", r)
    }
}

fn section(out: &mut String, title: &str, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    out.push_str(title);
    out.push_str(":\n");
    for l in lines {
        out.push_str("- ");
        out.push_str(l);
        out.push('\n');
    }
}

/// A table's columns, keys, constraints and triggers.
pub fn format_table(fields: &[FieldDef], parts: &TableParts) -> String {
    let mut out = String::new();
    section(&mut out, "Columns", &fields.iter().map(column_line).collect::<Vec<_>>());
    let indexes: Vec<String> = parts.indexes.iter().map(|i| format!("{} ({}){}{}", i.name, i.columns, if i.unique { " UNIQUE" } else { "" }, opt(" USING ", &i.method))).collect();
    section(&mut out, "Indexes", &indexes);
    section(&mut out, "Foreign keys", &parts.foreign_keys.iter().map(fk_line).collect::<Vec<_>>());
    section(&mut out, "Unique constraints", &parts.uniques.iter().map(|u| format!("{} ({})", u.name, u.columns)).collect::<Vec<_>>());
    section(&mut out, "Checks", &parts.checks.iter().map(|c| format!("{}: {}", c.name, c.expr)).collect::<Vec<_>>());
    section(&mut out, "Triggers", &parts.triggers.iter().map(|t| format!("{} {} {}", t.name, t.fires, trigger_events(t))).collect::<Vec<_>>());
    clip(out.trim_end())
}

fn fk_line(f: &ForeignKeyDef) -> String {
    let schema = if f.ref_schema.is_empty() { String::new() } else { format!("{}.", f.ref_schema) };
    format!("{} ({}) -> {}{}({}){}{}", f.name, f.columns, schema, f.ref_table, f.ref_columns, opt(" ON DELETE ", &f.on_delete), opt(" ON UPDATE ", &f.on_update))
}

fn column_line(f: &FieldDef) -> String {
    let mut s = format!("{} {}", f.name, f.type_text());
    for (on, word) in [(f.not_null, " NOT NULL"), (f.key, " PRIMARY KEY"), (f.auto_inc, " AUTO_INCREMENT")] {
        if on {
            s.push_str(word);
        }
    }
    s.push_str(&opt(" DEFAULT ", &f.default));
    s.push_str(&opt(" -- ", &f.comment));
    s
}

fn trigger_events(t: &TriggerDef) -> String {
    let events = [(t.insert, "INSERT"), (t.update, "UPDATE"), (t.delete, "DELETE"), (t.truncate, "TRUNCATE")];
    events.iter().filter(|(on, _)| *on).map(|(_, e)| *e).collect::<Vec<_>>().join(" OR ")
}

/// `prefix` + `value`, or nothing when the value is empty.
fn opt(prefix: &str, value: &str) -> String {
    if value.trim().is_empty() {
        String::new()
    } else {
        format!("{}{}", prefix, value.trim())
    }
}
