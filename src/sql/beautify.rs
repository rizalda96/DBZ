//! "Beautify SQL": upper-case keywords and give every clause its own lines, with one
//! list item or condition per line. Queries in parentheses (CTEs, subqueries) and
//! CASE … END get their own indented block; other parentheses stay on one line.
//! Blanks between tokens are kept as in the source.

use super::keywords::is_keyword;
use super::split::split_statements;
use super::sql_tokens::{tokens, Kind, Tok};

const INDENT: &str = "  ";
/// Clauses that start a line; longer phrases come first so they win.
const CLAUSES: &[&str] = &[
    "SELECT DISTINCT", "SELECT", "INSERT INTO", "DELETE FROM", "UPDATE", "FROM", "WHERE", "GROUP BY", "ORDER BY", "HAVING", "LIMIT", "OFFSET", "VALUES", "SET",
    "RETURNING", "UNION ALL", "UNION", "EXCEPT", "INTERSECT",
];
const JOINS: &[&str] = &[
    "LEFT OUTER JOIN", "RIGHT OUTER JOIN", "FULL OUTER JOIN", "LEFT JOIN", "RIGHT JOIN", "FULL JOIN", "INNER JOIN", "CROSS JOIN", "CROSS APPLY", "OUTER APPLY", "JOIN",
];
/// Clauses whose top-level commas end a line.
const LISTS: &[&str] = &["SELECT DISTINCT", "SELECT", "FROM", "GROUP BY", "ORDER BY", "SET", "VALUES", "RETURNING"];
/// Clauses whose top-level AND / OR start a line (FROM for the ON conditions of joins).
const CONDITIONS: &[&str] = &["WHERE", "HAVING", "FROM"];
/// Clauses that keep what follows on their own line.
const INLINE: &[&str] = &["WITH", "LIMIT", "OFFSET", "UNION ALL", "UNION", "EXCEPT", "INTERSECT"];

pub fn beautify(sql: &str) -> String {
    split_statements(sql).iter().map(|stmt| format!("{};", format_statement(stmt))).collect::<Vec<_>>().join("\n\n")
}

fn format_statement(stmt: &str) -> String {
    let toks = tokens(stmt);
    let mut layout = Layout::new();
    let mut i = 0;
    while i < toks.len() {
        i += layout.step(&toks, i);
    }
    layout.out.lines().map(str::trim_end).collect::<Vec<_>>().join("\n").trim().to_string()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    Statement,
    /// A query in parentheses; its `)` ends it.
    Subquery,
    /// CASE … END; its WHEN and ELSE start lines.
    Case,
}

struct Frame {
    block: Block,
    /// Indent level of its clauses (of the WHEN / ELSE lines in a CASE).
    level: usize,
    clause: Option<&'static str>,
    /// Parentheses and CASEs inside it that stay on one line.
    inline: usize,
    inline_cases: usize,
    /// A BETWEEN is waiting for its AND, which doesn't start a line.
    between: bool,
}

impl Frame {
    fn new(block: Block, level: usize) -> Self {
        Frame { block, level, clause: None, inline: 0, inline_cases: 0, between: false }
    }

    fn in_clause(&self, kinds: &[&str]) -> bool {
        self.clause.is_some_and(|c| kinds.contains(&c))
    }
}

struct Layout {
    out: String,
    /// Never empty: the statement's frame stays at the bottom.
    frames: Vec<Frame>,
    /// Indent level of the line being written.
    line_level: usize,
}

impl Layout {
    fn new() -> Self {
        Layout { out: String::new(), frames: vec![Frame::new(Block::Statement, 0)], line_level: 0 }
    }

    fn frame(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("the statement frame is never popped")
    }

    /// Lays out the token(s) at `i`; returns how many it used.
    fn step(&mut self, toks: &[Tok], i: usize) -> usize {
        let t = toks[i];
        let starts_query = toks.get(i + 1).is_some_and(|n| n.is("SELECT") || n.is("WITH"));
        if t.text == "(" && starts_query {
            self.push(&t);
            let level = self.line_level + 1;
            self.frames.push(Frame::new(Block::Subquery, level));
            return 1;
        }
        let (block, inline) = (self.frame().block, self.frame().inline);
        if t.text == ")" && inline == 0 && block == Block::Subquery {
            let level = self.frames.pop().map_or(0, |f| f.level.saturating_sub(1));
            self.line(level);
            self.out.push(')');
            return 1;
        }
        if inline == 0 {
            if let Some(used) = self.structural(toks, i) {
                return used;
            }
        }
        self.track_inline(&t);
        self.push(&t);
        1
    }

    /// Clauses, joins, list commas, conditions and CASE parts outside one-line parentheses.
    fn structural(&mut self, toks: &[Tok], i: usize) -> Option<usize> {
        let t = toks[i];
        if t.is("CASE") {
            self.push(&t);
            let level = self.line_level + 1;
            self.frames.push(Frame::new(Block::Case, level));
            return Some(1);
        }
        if self.frame().block == Block::Case {
            return self.case_part(&t);
        }
        let opens_with = t.is("WITH") && self.frame().clause.is_none();
        if let Some(phrase) = phrase_at(toks, i, CLAUSES).or(opens_with.then_some("WITH")) {
            return Some(self.clause_start(toks, i, phrase));
        }
        let level = self.frame().level + 1;
        if let Some(phrase) = phrase_at(toks, i, JOINS) {
            self.line(level);
            self.out.push_str(phrase);
            return Some(word_count(phrase));
        }
        self.list_or_condition(&t, level)
    }

    fn list_or_condition(&mut self, t: &Tok, level: usize) -> Option<usize> {
        let f = self.frame();
        if t.is("BETWEEN") {
            f.between = true;
        }
        if t.is("AND") && f.between {
            f.between = false;
            return None;
        }
        if (t.is("AND") || t.is("OR")) && f.in_clause(CONDITIONS) {
            self.line(level);
            self.out.push_str(&t.text.to_uppercase());
            return Some(1);
        }
        if t.text == "," && f.in_clause(LISTS) {
            self.out.push(',');
            self.line(level);
            return Some(1);
        }
        None
    }

    /// WHEN and ELSE start lines inside the CASE; END closes it one level out.
    fn case_part(&mut self, t: &Tok) -> Option<usize> {
        if t.is("WHEN") || t.is("ELSE") {
            let level = self.frame().level;
            self.line(level);
        } else if t.is("END") {
            let level = self.frames.pop().map_or(0, |f| f.level.saturating_sub(1));
            self.line(level);
        } else {
            return None;
        }
        self.out.push_str(&t.text.to_uppercase());
        Some(1)
    }

    /// The clause on a line of its own (SELECT keeps a `TOP n`), its contents indented
    /// on the next one.
    fn clause_start(&mut self, toks: &[Tok], i: usize, phrase: &'static str) -> usize {
        let mut used = word_count(phrase);
        let level = self.frame().level;
        self.frame().clause = Some(phrase);
        self.line(level);
        self.out.push_str(phrase);
        let top = toks.get(i + used).filter(|t| t.is("TOP")).and(toks.get(i + used + 1)).filter(|n| n.kind == Kind::Word);
        if let (true, Some(n)) = (phrase.starts_with("SELECT"), top) {
            self.out.push_str(&format!(" TOP {}", n.text));
            used += 2;
        }
        if !INLINE.contains(&phrase) {
            self.line(level + 1);
        }
        used
    }

    /// Ends the line (without trailing blanks) and starts the next at `level`.
    fn line(&mut self, level: usize) {
        let kept = self.out.trim_end().len();
        self.out.truncate(kept);
        if !self.out.is_empty() {
            self.out.push('\n');
        }
        self.out.push_str(&INDENT.repeat(level));
        self.line_level = level;
    }

    fn track_inline(&mut self, t: &Tok) {
        let f = self.frame();
        if t.text == "(" || t.is("CASE") {
            f.inline += 1;
            f.inline_cases += usize::from(t.is("CASE"));
        } else if t.text == ")" || (t.is("END") && f.inline_cases > 0) {
            f.inline = f.inline.saturating_sub(1);
            f.inline_cases -= usize::from(t.is("END"));
        }
    }

    fn push(&mut self, t: &Tok) {
        let line_start = self.out.is_empty() || self.out.ends_with([' ', '\n']);
        if t.space && !line_start {
            self.out.push(' ');
        }
        if t.kind == Kind::Word && is_keyword(t.text) {
            self.out.push_str(&t.text.to_uppercase());
        } else {
            self.out.push_str(t.text);
        }
        if t.kind == Kind::LineComment {
            self.line(self.line_level);
        }
    }
}

/// The first of `phrases` whose words start at token `i`.
fn phrase_at(toks: &[Tok], i: usize, phrases: &[&'static str]) -> Option<&'static str> {
    phrases.iter().copied().find(|p| p.split(' ').enumerate().all(|(k, w)| toks.get(i + k).is_some_and(|t| t.is(w))))
}

fn word_count(phrase: &str) -> usize {
    phrase.split(' ').count()
}

#[cfg(test)]
mod tests;
