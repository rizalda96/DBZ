//! Splitting a script into statements.

/// Split a script into statements on `;`, respecting quotes and comments.
pub fn split_statements(sql: &str) -> Vec<String> {
    let chars: Vec<char> = sql.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            i = quoted_char(&chars, i, q, &mut cur, &mut quote);
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                quote = Some(c);
                cur.push(c);
            }
            '-' if chars.get(i + 1) == Some(&'-') => i = skip_line(&chars, i, &mut cur),
            '#' => i = skip_line(&chars, i, &mut cur),
            '/' if chars.get(i + 1) == Some(&'*') => i = skip_block(&chars, i),
            ';' => push_statement(&mut out, &mut cur),
            _ => cur.push(c),
        }
        i += 1;
    }
    push_statement(&mut out, &mut cur);
    out
}

/// Handles one character inside a quoted run; returns the next index.
fn quoted_char(chars: &[char], i: usize, q: char, cur: &mut String, quote: &mut Option<char>) -> usize {
    let c = chars[i];
    cur.push(c);
    if c == '\\' && q != '"' && i + 1 < chars.len() {
        cur.push(chars[i + 1]);
        return i + 2;
    }
    if c == q {
        *quote = None;
    }
    i + 1
}

/// Skips a `--` or `#` comment; returns the index of the line break (or the end).
fn skip_line(chars: &[char], mut i: usize, cur: &mut String) -> usize {
    while i < chars.len() && chars[i] != '\n' {
        i += 1;
    }
    cur.push('\n');
    i
}

/// Skips a `/* */` comment; returns the index of its closing `/`.
fn skip_block(chars: &[char], mut i: usize) -> usize {
    i += 2;
    while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
        i += 1;
    }
    i + 1
}

fn push_statement(out: &mut Vec<String>, cur: &mut String) {
    let s = cur.trim();
    if !s.is_empty() {
        out.push(s.to_string());
    }
    cur.clear();
}
