//! SQL syntax colouring for text editors.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Color32, FontId, Ui};

use crate::sql::is_keyword;
use crate::ui::theme::pal;

const FONT_SIZE: f32 = 13.5;

/// Colours `text` as SQL.
pub fn highlight(text: &str) -> LayoutJob {
    let font = FontId::monospace(FONT_SIZE);
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut job = LayoutJob::default();
    let mut i = 0;
    while i < chars.len() {
        let (j, color) = token(text, &chars, i);
        let start = chars[i].0;
        let end = chars.get(j).map(|x| x.0).unwrap_or(text.len());
        job.append(&text[start..end], 0.0, TextFormat { font_id: font.clone(), color, ..Default::default() });
        i = j;
    }
    job
}

/// A `TextEdit` layouter that colours SQL.
pub fn sql_layouter(ui: &Ui, text: &dyn egui::TextBuffer, wrap: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = highlight(text.as_str());
    job.wrap.max_width = wrap;
    ui.fonts_mut(|f| f.layout_job(job))
}

/// The token starting at `i`: its end index and colour.
fn token(text: &str, chars: &[(usize, char)], i: usize) -> (usize, Color32) {
    let c = chars[i].1;
    let next = chars.get(i + 1).map(|x| x.1);
    let p = pal();
    if (c == '-' && next == Some('-')) || c == '#' {
        (scan(chars, i + 1, |ch| ch != '\n'), p.syn_comment)
    } else if c == '/' && next == Some('*') {
        (block_comment_end(chars, i), p.syn_comment)
    } else if matches!(c, '\'' | '"' | '`') {
        let end = (scan(chars, i + 1, |ch| ch != c) + 1).min(chars.len());
        (end, if c == '\'' { p.syn_string } else { p.syn_ident })
    } else if c.is_ascii_digit() {
        (scan(chars, i + 1, |ch| ch.is_ascii_digit() || ch == '.'), p.syn_number)
    } else if c.is_alphabetic() || c == '_' {
        let j = scan(chars, i + 1, |ch| ch.is_alphanumeric() || ch == '_');
        let end = chars.get(j).map(|x| x.0).unwrap_or(text.len());
        (j, if is_keyword(&text[chars[i].0..end]) { p.syn_keyword } else { p.text })
    } else {
        (i + 1, p.text)
    }
}

/// First index from `j` where `keep` is false (or the end).
fn scan(chars: &[(usize, char)], mut j: usize, keep: impl Fn(char) -> bool) -> usize {
    while j < chars.len() && keep(chars[j].1) {
        j += 1;
    }
    j
}

fn block_comment_end(chars: &[(usize, char)], i: usize) -> usize {
    let mut j = i + 2;
    while j < chars.len() && !(chars[j].1 == '/' && chars[j - 1].1 == '*') {
        j += 1;
    }
    (j + 1).min(chars.len())
}
