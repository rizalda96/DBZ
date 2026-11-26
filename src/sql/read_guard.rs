//! Checks that a statement the AI runs on its own only reads. It looks at the raw text,
//! quotes and comments included, because servers lex quotes differently (PostgreSQL
//! `$$`, MySQL `\"`) and a guard that skips "strings" can be fooled into skipping
//! real statements. False alarms are fine: the AI is told why and rephrases. The
//! read-only session and transaction on the server are the second line of defence.

use super::Dialect;

/// What the statement is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadKind {
    /// Run, and its rows read back.
    Select,
    /// Only planned; the plan never runs it, so data changes may be planned too.
    Explain,
}

const SELECT_FIRST: [&str; 7] = ["SELECT", "WITH", "SHOW", "DESCRIBE", "DESC", "VALUES", "TABLE"];
const EXPLAIN_FIRST: [&str; 8] = ["SELECT", "WITH", "INSERT", "UPDATE", "DELETE", "MERGE", "VALUES", "TABLE"];

/// Words that start or end another statement or transaction, or write. SQL Server
/// runs several statements without any `;`, so none may appear anywhere.
/// (`END` and `REPLACE` stay allowed: `CASE … END` and `REPLACE()` are everyday reads,
/// and the statements they could start need a `;`, which is refused.)
const STATEMENT_WORDS: &[&str] = &[
    "COMMIT", "ROLLBACK", "SAVEPOINT", "BEGIN", "START", "INSERT", "UPDATE", "DELETE", "MERGE", "CREATE", "ALTER", "DROP", "TRUNCATE", "RENAME",
    "GRANT", "REVOKE", "DENY", "EXEC", "EXECUTE", "CALL", "DO", "COPY", "LOAD", "LOCK", "UNLOCK", "SET", "USE", "KILL", "DECLARE", "WAITFOR",
    "BACKUP", "RESTORE", "DBCC", "SHUTDOWN", "RECONFIGURE", "BULK", "WRITETEXT", "UPDATETEXT", "ENABLE", "DISABLE",
];

/// Functions that act outside the transaction (other servers, sequences, locks, files,
/// sessions) or just hold the connection.
const SIDE_EFFECTS: [&str; 18] = [
    "OPENQUERY",
    "OPENROWSET",
    "OPENDATASOURCE",
    "NEXTVAL",
    "SETVAL",
    "GET_LOCK",
    "RELEASE_LOCK",
    "SLEEP",
    "BENCHMARK",
    "PG_SLEEP",
    "PG_ADVISORY_LOCK",
    "PG_TERMINATE_BACKEND",
    "PG_CANCEL_BACKEND",
    "LO_IMPORT",
    "LO_EXPORT",
    "PG_READ_FILE",
    "LOAD_FILE",
    "DBLINK",
];

/// The single statement in `sql`, trimmed and without its closing `;`, when it only
/// reads (or, for [`ReadKind::Explain`], may be planned); otherwise why not.
pub fn check_read(d: &dyn Dialect, sql: &str, kind: ReadKind) -> Result<String, String> {
    let stmt = sql.trim().trim_end_matches(|c: char| c == ';' || c.is_whitespace()).to_string();
    if stmt.is_empty() {
        return Err("no statement was given".into());
    }
    if let Some(c) = stmt.chars().find(|c| matches!(c, ';' | '$' | '\\')) {
        return Err(format!("the character {} is not allowed here; send one plain statement", c));
    }
    if d.split_script(&stmt).len() > 1 {
        return Err("only one statement is allowed".into());
    }
    let words = words(&stmt);
    let first = words.first().cloned().unwrap_or_default();
    let allowed: &[&str] = match kind {
        ReadKind::Select => &SELECT_FIRST,
        ReadKind::Explain => &EXPLAIN_FIRST,
    };
    if !allowed.contains(&first.as_str()) {
        return Err(format!("a statement starting with {} is not allowed here; allowed: {}", first, allowed.join(", ")));
    }
    forbidden_words(&words, kind)?;
    Ok(stmt)
}

/// Every run of letters, digits and `_`, upper-cased, quotes and comments included.
fn words(sql: &str) -> Vec<String> {
    sql.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).map(str::to_uppercase).collect()
}

fn forbidden_words(words: &[String], kind: ReadKind) -> Result<(), String> {
    let has = |w: &str| words.iter().any(|x| x == w);
    let exempt = explain_exempt(kind, words.first().map(String::as_str).unwrap_or_default());
    // The first word was checked against the allowed list already.
    if let Some(w) = words.iter().skip(1).find(|w| STATEMENT_WORDS.contains(&w.as_str()) && !exempt.contains(&w.as_str())) {
        return Err(format!("{} is not allowed in a read; send one plain statement without it (also not inside text or comments)", w));
    }
    if let Some(w) = words.iter().find(|w| SIDE_EFFECTS.contains(&w.as_str())) {
        return Err(format!("{} has effects outside a read and is not allowed", w));
    }
    if words.windows(2).any(|w| w[0] == "NEXT" && w[1] == "VALUE") {
        return Err("NEXT VALUE FOR changes a sequence and is not allowed".into());
    }
    if has("ANALYZE") {
        return Err("ANALYZE runs the statement; ask for the plan only".into());
    }
    if kind == ReadKind::Select && has("INTO") {
        return Err("INTO writes (a new table, a file or variables) and is not allowed".into());
    }
    let locks = words.windows(2).any(|w| w[0] == "FOR" && ["UPDATE", "SHARE", "NO", "KEY"].contains(&w[1].as_str()));
    if kind == ReadKind::Select && locks {
        return Err("FOR UPDATE / FOR SHARE locks rows and is not allowed".into());
    }
    Ok(())
}

/// Words a planned data change needs after its leading verb.
fn explain_exempt(kind: ReadKind, first: &str) -> &'static [&'static str] {
    match (kind, first) {
        (ReadKind::Explain, "UPDATE") => &["SET"],
        (ReadKind::Explain, "MERGE") => &["UPDATE", "SET", "INSERT", "DELETE"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests;
