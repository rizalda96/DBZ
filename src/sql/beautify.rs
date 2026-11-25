//! "Beautify SQL": upper-case keywords and break long statements into lines.

use super::keywords::is_keyword;
use super::split::split_statements;

const BREAKS: [&str; 13] = ["FROM", "WHERE", "ORDER BY", "GROUP BY", "HAVING", "LIMIT", "LEFT JOIN", "RIGHT JOIN", "INNER JOIN", "JOIN", "VALUES", "SET", "UNION"];

pub fn beautify(sql: &str) -> String {
    split_statements(sql).iter().map(|stmt| format!("{};", format_statement(stmt))).collect::<Vec<_>>().join("\n\n")
}

fn format_statement(stmt: &str) -> String {
    let mut s = upper_keywords(stmt).split_whitespace().collect::<Vec<_>>().join(" ");
    for b in BREAKS {
        s = s.replace(&format!(" {} ", b), &format!("\n{} ", b));
    }
    s.replace(" AND ", "\n  AND ").replace(" OR ", "\n  OR ")
}

/// Upper-cases keywords outside quotes.
fn upper_keywords(stmt: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    for c in stmt.chars() {
        if let Some(q) = quote {
            out.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        flush_word(&mut word, &mut out);
        if matches!(c, '\'' | '"' | '`') {
            quote = Some(c);
        }
        out.push(c);
    }
    flush_word(&mut word, &mut out);
    out
}

fn flush_word(word: &mut String, out: &mut String) {
    if is_keyword(word) {
        out.push_str(&word.to_uppercase());
    } else {
        out.push_str(word);
    }
    word.clear();
}
