//! Splits SQL into words, quoted text, comments and single symbols, remembering
//! whether blanks came before each token, for the beautifier.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Word,
    /// 'text', "ident", `ident` or [ident], with doubled quotes inside.
    Quoted,
    /// `-- …` up to the end of its line.
    LineComment,
    /// A block comment or a symbol.
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Tok<'a> {
    pub text: &'a str,
    pub kind: Kind,
    /// Blanks came before it in the source.
    pub space: bool,
}

impl Tok<'_> {
    /// Whether it is the word `w` (any case).
    pub fn is(&self, w: &str) -> bool {
        self.kind == Kind::Word && self.text.eq_ignore_ascii_case(w)
    }
}

pub(super) fn tokens(sql: &str) -> Vec<Tok<'_>> {
    let mut out = vec![];
    let mut at = 0;
    let mut space = false;
    while let Some(c) = sql[at..].chars().next() {
        if c.is_whitespace() {
            at += c.len_utf8();
            space = true;
            continue;
        }
        let (len, kind) = token_at(&sql[at..], c);
        out.push(Tok { text: &sql[at..at + len], kind, space });
        at += len;
        space = false;
    }
    out
}

/// Byte length and kind of the token that starts `rest` (whose first char is `c`).
fn token_at(rest: &str, c: char) -> (usize, Kind) {
    if rest.starts_with("--") {
        return (rest.find('\n').unwrap_or(rest.len()), Kind::LineComment);
    }
    if rest.starts_with("/*") {
        return (rest[2..].find("*/").map_or(rest.len(), |i| i + 4), Kind::Other);
    }
    match c {
        '\'' | '"' | '`' => (quoted_len(rest, c), Kind::Quoted),
        '[' => (quoted_len(rest, ']'), Kind::Quoted),
        c if is_word_char(c) => (rest.find(|ch: char| !is_word_char(ch)).unwrap_or(rest.len()), Kind::Word),
        c => (c.len_utf8(), Kind::Other),
    }
}

/// `@var`, `#temp` and `$1` count as words, so they keep their sigil.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '@' | '#' | '$')
}

/// Up to and including the closing `close`; a doubled one is part of the text.
fn quoted_len(rest: &str, close: char) -> usize {
    let mut chars = rest.char_indices().skip(1).peekable();
    while let Some((i, ch)) = chars.next() {
        if ch != close {
            continue;
        }
        if chars.peek().map(|(_, next)| *next) == Some(close) {
            chars.next();
            continue;
        }
        return i + ch.len_utf8();
    }
    rest.len()
}

#[cfg(test)]
mod tests {
    use super::{tokens, Kind};

    #[test]
    fn keeps_quotes_comments_and_spacing() {
        let toks = tokens("SELECT N'it''s',[a]]b] -- note\n FROM t");
        let texts: Vec<&str> = toks.iter().map(|t| t.text).collect();
        assert_eq!(texts, ["SELECT", "N", "'it''s'", ",", "[a]]b]", "-- note", "FROM", "t"]);
        assert!(!toks[2].space && toks[6].space);
        assert_eq!(toks[5].kind, Kind::LineComment);
    }
}
