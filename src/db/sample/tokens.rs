//! Tokenizer for the sample engine's SQL subset.

use crate::db::DbError;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tok {
    Ident(String),
    QIdent(String),
    Str(String),
    Num(String),
    Sym(String),
}

const TWO_CHAR_SYMBOLS: [&str; 4] = ["<=", ">=", "<>", "!="];

pub(super) fn tokenize(sql: &str) -> Result<Vec<Tok>, DbError> {
    let c: Vec<char> = sql.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        let (tok, next) = if ch.is_alphabetic() || ch == '_' {
            read_word(&c, i)
        } else if ch.is_ascii_digit() || (ch == '.' && c.get(i + 1).is_some_and(|n| n.is_ascii_digit())) {
            read_number(&c, i)
        } else if matches!(ch, '\'' | '"' | '`') {
            read_quoted(&c, i)?
        } else {
            read_symbol(&c, i)
        };
        out.push(tok);
        i = next;
    }
    Ok(out)
}

fn take_while(c: &[char], mut i: usize, ok: impl Fn(char) -> bool) -> usize {
    while i < c.len() && ok(c[i]) {
        i += 1;
    }
    i
}

fn read_word(c: &[char], start: usize) -> (Tok, usize) {
    let end = take_while(c, start, |x| x.is_alphanumeric() || x == '_' || x == '$');
    (Tok::Ident(c[start..end].iter().collect()), end)
}

fn read_number(c: &[char], start: usize) -> (Tok, usize) {
    let end = take_while(c, start, |x| x.is_ascii_digit() || matches!(x, '.' | 'e' | 'E'));
    (Tok::Num(c[start..end].iter().collect()), end)
}

fn read_symbol(c: &[char], i: usize) -> (Tok, usize) {
    let two: String = c[i..(i + 2).min(c.len())].iter().collect();
    if TWO_CHAR_SYMBOLS.contains(&two.as_str()) {
        (Tok::Sym(two), i + 2)
    } else {
        (Tok::Sym(c[i].to_string()), i + 1)
    }
}

/// A quoted string ('...') or identifier ("..." / `...`). Doubled quotes and
/// backslash escapes (in strings) are unescaped.
fn read_quoted(c: &[char], start: usize) -> Result<(Tok, usize), DbError> {
    let q = c[start];
    let mut i = start + 1;
    let mut s = String::new();
    loop {
        let Some(&ch) = c.get(i) else { return Err(DbError::query("Unterminated quoted text")) };
        if ch == '\\' && q == '\'' && i + 1 < c.len() {
            s.push(unescape(c[i + 1]));
            i += 2;
        } else if ch == q && c.get(i + 1) == Some(&q) {
            s.push(q);
            i += 2;
        } else if ch == q {
            break;
        } else {
            s.push(ch);
            i += 1;
        }
    }
    let tok = if q == '\'' { Tok::Str(s) } else { Tok::QIdent(s) };
    Ok((tok, i + 1))
}

fn unescape(n: char) -> char {
    match n {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        '0' => '\0',
        o => o,
    }
}
