//! Small vector icons drawn with the painter, plus the bundled DBZ logo.

use crate::ui::theme::pal;
use eframe::egui::{self, pos2, vec2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Connection,
    NewQuery,
    Table,
    View,
    MatView,
    Function,
    User,
    Query,
    Backup,
    Database { open: bool },
    Schema,
    Folder,
    Key,
    Conn { color: Color32, open: bool },
    Refresh,
    Import,
    Export,
    Delete,
    Add,
    Design,
    Run,
    Save,
    Filter,
    Check,
    Discard,
    Minus,
    MoveUp,
    MoveDown,
}

const BLUE: Color32 = Color32::from_rgb(0x2f, 0x7d, 0xd1);
const GREEN: Color32 = Color32::from_rgb(0x3c, 0xa5, 0x5c);
const ORANGE: Color32 = Color32::from_rgb(0xe8, 0x8f, 0x2a);
const RED: Color32 = Color32::from_rgb(0xd9, 0x4b, 0x4b);
const PURPLE: Color32 = Color32::from_rgb(0x8a, 0x5c, 0xd6);
const TEAL: Color32 = Color32::from_rgb(0x1f, 0x9e, 0xa8);

fn cylinder(p: &Painter, r: Rect, body: Color32, top: Color32) {
    let ry = (r.height() * 0.16).max(1.5);
    let rx = r.width() / 2.0;
    let body_rect = Rect::from_min_max(pos2(r.left(), r.top() + ry), pos2(r.right(), r.bottom() - ry));
    p.rect_filled(body_rect, 0.0, body);
    p.add(Shape::ellipse_filled(pos2(r.center().x, r.bottom() - ry), vec2(rx, ry), body));
    let band = Color32::from_white_alpha(70);
    p.add(Shape::ellipse_stroke(pos2(r.center().x, r.center().y + ry * 0.6), vec2(rx, ry), Stroke::new(1.0, band)));
    p.add(Shape::ellipse_filled(pos2(r.center().x, r.top() + ry), vec2(rx, ry), top));
}

fn page(p: &Painter, r: Rect, accent: Color32) {
    let fold = r.width() * 0.3;
    let pts = vec![r.left_top(), pos2(r.right() - fold, r.top()), pos2(r.right(), r.top() + fold), r.right_bottom(), r.left_bottom()];
    p.add(Shape::convex_polygon(pts, Color32::WHITE, Stroke::new(1.0, Color32::from_gray(140))));
    p.add(Shape::convex_polygon(vec![pos2(r.right() - fold, r.top()), pos2(r.right(), r.top() + fold), pos2(r.right() - fold, r.top() + fold)], Color32::from_gray(215), Stroke::new(1.0, Color32::from_gray(140))));
    let n = 4;
    for i in 0..n {
        let y = r.top() + r.height() * (0.38 + i as f32 * 0.14);
        let w = if i % 2 == 0 { 0.7 } else { 0.5 };
        p.line_segment([pos2(r.left() + r.width() * 0.18, y), pos2(r.left() + r.width() * (0.18 + w * 0.82), y)], Stroke::new((r.height() / 16.0).max(1.0), accent));
    }
}

fn grid(p: &Painter, r: Rect, head: Color32) {
    p.rect_filled(r, 2.0, Color32::WHITE);
    let hh = r.height() * 0.28;
    p.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), hh)), egui::CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, head);
    let line = Stroke::new(1.0, Color32::from_gray(170));
    for i in 1..3 {
        let y = r.top() + hh + (r.height() - hh) * i as f32 / 3.0;
        p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], line);
    }
    let x = r.left() + r.width() * 0.4;
    p.line_segment([pos2(x, r.top() + hh), pos2(x, r.bottom())], line);
    p.rect_stroke(r, 2.0, Stroke::new(1.0, head), StrokeKind::Inside);
}

/// A plain arrow pointing up or down (move a row).
fn move_arrow(p: &Painter, r: Rect, up: bool) {
    let s = r.width();
    let st = Stroke::new(s * 0.12, pal().icon_stroke);
    let (top, bottom) = (r.top() + s * 0.18, r.bottom() - s * 0.18);
    let (tip, tail, back) = if up { (top, bottom, s * 0.28) } else { (bottom, top, -s * 0.28) };
    let x = r.center().x;
    p.line_segment([pos2(x, tail), pos2(x, tip)], st);
    p.line_segment([pos2(x - s * 0.28, tip + back), pos2(x, tip)], st);
    p.line_segment([pos2(x + s * 0.28, tip + back), pos2(x, tip)], st);
}

pub fn paint(p: &Painter, r: Rect, icon: Icon) {
    let s = r.width().min(r.height());
    let r = Rect::from_center_size(r.center(), vec2(s, s));
    match icon {
        Icon::Connection => {
            cylinder(p, r.shrink2(vec2(s * 0.18, s * 0.08)).translate(vec2(-s * 0.08, 0.0)), Color32::from_rgb(0x5a, 0x8f, 0xd8), Color32::from_rgb(0x9c, 0xc3, 0xf2));
            let c = pos2(r.right() - s * 0.2, r.bottom() - s * 0.22);
            p.circle_filled(c, s * 0.2, GREEN);
            let w = s * 0.11;
            p.line_segment([pos2(c.x - w, c.y), pos2(c.x + w, c.y)], Stroke::new(s * 0.06, Color32::WHITE));
            p.line_segment([pos2(c.x, c.y - w), pos2(c.x, c.y + w)], Stroke::new(s * 0.06, Color32::WHITE));
        }
        Icon::NewQuery | Icon::Query => {
            page(p, r.shrink2(vec2(s * 0.16, s * 0.06)), BLUE);
            if icon == Icon::NewQuery {
                let c = pos2(r.right() - s * 0.18, r.bottom() - s * 0.2);
                p.circle_filled(c, s * 0.18, ORANGE);
                let w = s * 0.1;
                p.line_segment([pos2(c.x - w, c.y), pos2(c.x + w, c.y)], Stroke::new(s * 0.06, Color32::WHITE));
                p.line_segment([pos2(c.x, c.y - w), pos2(c.x, c.y + w)], Stroke::new(s * 0.06, Color32::WHITE));
            }
        }
        Icon::Table => grid(p, r.shrink2(vec2(s * 0.06, s * 0.14)), BLUE),
        Icon::View | Icon::MatView => {
            let col = if icon == Icon::View { TEAL } else { RED };
            grid(p, r.shrink2(vec2(s * 0.06, s * 0.14)), col);
            let c = pos2(r.right() - s * 0.25, r.bottom() - s * 0.25);
            p.add(Shape::ellipse_filled(c, vec2(s * 0.22, s * 0.13), Color32::WHITE));
            p.add(Shape::ellipse_stroke(c, vec2(s * 0.22, s * 0.13), Stroke::new(1.2, col)));
            p.circle_filled(c, s * 0.07, col);
        }
        Icon::Function => {
            p.rect_filled(r.shrink(s * 0.08), s * 0.15, PURPLE);
            p.text(r.center(), egui::Align2::CENTER_CENTER, "f(x)", FontId::proportional(s * 0.34), Color32::WHITE);
        }
        Icon::User => {
            let head = pos2(r.center().x, r.top() + s * 0.32);
            p.circle_filled(head, s * 0.18, ORANGE);
            let body = Rect::from_min_max(pos2(r.left() + s * 0.18, r.top() + s * 0.55), pos2(r.right() - s * 0.18, r.bottom() - s * 0.08));
            p.rect_filled(body, egui::CornerRadius { nw: (s * 0.3) as u8, ne: (s * 0.3) as u8, sw: 2, se: 2 }, ORANGE);
        }
        Icon::Backup => {
            let b = Rect::from_min_max(pos2(r.left() + s * 0.1, r.top() + s * 0.38), pos2(r.right() - s * 0.1, r.bottom() - s * 0.08));
            p.rect_filled(b, 2.0, Color32::from_rgb(0xb0, 0x84, 0x4c));
            p.rect_filled(Rect::from_min_max(pos2(b.left() - s * 0.04, b.top()), pos2(b.right() + s * 0.04, b.top() + s * 0.14)), 2.0, Color32::from_rgb(0x8c, 0x63, 0x33));
            let c = pos2(r.center().x, r.top() + s * 0.06);
            p.line_segment([c, pos2(c.x, b.top() + s * 0.22)], Stroke::new(s * 0.09, GREEN));
            p.add(Shape::convex_polygon(vec![pos2(c.x - s * 0.16, b.top() + s * 0.08), pos2(c.x + s * 0.16, b.top() + s * 0.08), pos2(c.x, b.top() + s * 0.3)], GREEN, Stroke::NONE));
        }
        Icon::Database { open } => {
            let body = if open { Color32::from_rgb(0x4c, 0xa0, 0x6a) } else { Color32::from_gray(150) };
            let top = if open { Color32::from_rgb(0x95, 0xd4, 0xa8) } else { Color32::from_gray(200) };
            cylinder(p, r.shrink2(vec2(s * 0.16, s * 0.06)), body, top);
        }
        Icon::Schema => {
            // Three stacked layers: a namespace that holds tables.
            for (i, col) in [BLUE, TEAL, PURPLE].into_iter().enumerate() {
                let top = r.top() + s * (0.14 + i as f32 * 0.26);
                p.rect_filled(Rect::from_min_size(pos2(r.left() + s * 0.1, top), vec2(s * 0.8, s * 0.2)), 2.0, col);
            }
        }
        Icon::Folder => {
            let b = r.shrink2(vec2(s * 0.04, s * 0.16));
            p.rect_filled(Rect::from_min_size(b.min, vec2(b.width() * 0.45, b.height() * 0.3)), 1.5, Color32::from_rgb(0xd9, 0xa4, 0x30));
            p.rect_filled(Rect::from_min_max(pos2(b.left(), b.top() + b.height() * 0.18), b.max), 1.5, Color32::from_rgb(0xf2, 0xc1, 0x4e));
        }
        Icon::Key => {
            let c = pos2(r.left() + s * 0.3, r.center().y);
            p.circle_stroke(c, s * 0.18, Stroke::new(s * 0.1, Color32::from_rgb(0xd4, 0xa0, 0x17)));
            let st = Stroke::new(s * 0.1, Color32::from_rgb(0xd4, 0xa0, 0x17));
            p.line_segment([pos2(c.x + s * 0.18, c.y), pos2(r.right() - s * 0.08, c.y)], st);
            p.line_segment([pos2(r.right() - s * 0.16, c.y), pos2(r.right() - s * 0.16, c.y + s * 0.18)], st);
        }
        Icon::Conn { color, open } => {
            let c = if open { color } else { Color32::from_gray(150) };
            let top = if open { color.gamma_multiply(0.55) } else { Color32::from_gray(205) };
            cylinder(p, r.shrink2(vec2(s * 0.14, s * 0.04)), c, top);
            if open {
                p.circle_filled(pos2(r.right() - s * 0.14, r.bottom() - s * 0.14), s * 0.14, GREEN);
            }
        }
        Icon::Refresh => {
            let c = r.center();
            let rad = s * 0.3;
            let pts: Vec<Pos2> = (0..=20).map(|i| {
                let a = 0.5 + i as f32 / 20.0 * 4.9;
                pos2(c.x + rad * a.cos(), c.y + rad * a.sin())
            }).collect();
            let e = pts[pts.len() - 1];
            p.add(Shape::line(pts, Stroke::new(s * 0.1, GREEN)));
            p.add(Shape::convex_polygon(vec![pos2(e.x - s * 0.14, e.y - s * 0.02), pos2(e.x + s * 0.12, e.y - s * 0.12), pos2(e.x + s * 0.06, e.y + s * 0.14)], GREEN, Stroke::NONE));
        }
        Icon::Import | Icon::Export => {
            grid(p, r.shrink2(vec2(s * 0.2, s * 0.2)).translate(vec2(s * 0.12, 0.0)), BLUE);
            let col = if icon == Icon::Import { GREEN } else { ORANGE };
            let y = r.center().y;
            let (a, b) = if icon == Icon::Import { (r.left() + s * 0.02, r.left() + s * 0.42) } else { (r.left() + s * 0.42, r.left() + s * 0.02) };
            p.line_segment([pos2(a, y), pos2(b, y)], Stroke::new(s * 0.1, col));
            let d = if b > a { 1.0 } else { -1.0 };
            p.add(Shape::convex_polygon(vec![pos2(b + d * s * 0.12, y), pos2(b - d * s * 0.04, y - s * 0.14), pos2(b - d * s * 0.04, y + s * 0.14)], col, Stroke::NONE));
        }
        Icon::Delete => {
            let st = Stroke::new(s * 0.13, RED);
            let k = r.shrink(s * 0.24);
            p.line_segment([k.left_top(), k.right_bottom()], st);
            p.line_segment([k.right_top(), k.left_bottom()], st);
        }
        Icon::Add => {
            let st = Stroke::new(s * 0.13, GREEN);
            let k = r.shrink(s * 0.2);
            p.line_segment([pos2(k.left(), k.center().y), pos2(k.right(), k.center().y)], st);
            p.line_segment([pos2(k.center().x, k.top()), pos2(k.center().x, k.bottom())], st);
        }
        Icon::Design => {
            grid(p, r.shrink2(vec2(s * 0.06, s * 0.14)), BLUE);
            let st = Stroke::new(s * 0.12, ORANGE);
            p.line_segment([pos2(r.right() - s * 0.08, r.top() + s * 0.45), pos2(r.right() - s * 0.42, r.bottom() - s * 0.08)], st);
        }
        Icon::Run => {
            p.add(Shape::convex_polygon(vec![pos2(r.left() + s * 0.25, r.top() + s * 0.15), pos2(r.right() - s * 0.15, r.center().y), pos2(r.left() + s * 0.25, r.bottom() - s * 0.15)], GREEN, Stroke::NONE));
        }
        Icon::Save => {
            let b = r.shrink(s * 0.12);
            p.rect_filled(b, 2.0, BLUE);
            p.rect_filled(Rect::from_min_max(pos2(b.left() + b.width() * 0.2, b.top()), pos2(b.right() - b.width() * 0.2, b.top() + b.height() * 0.35)), 0.0, Color32::WHITE);
            p.rect_filled(Rect::from_min_max(pos2(b.left() + b.width() * 0.25, b.bottom() - b.height() * 0.35), pos2(b.right() - b.width() * 0.25, b.bottom())), 0.0, Color32::from_rgb(0xd5, 0xe6, 0xf8));
        }
        Icon::Filter => {
            let k = r.shrink(s * 0.12);
            p.add(Shape::convex_polygon(vec![k.left_top(), k.right_top(), pos2(k.center().x + s * 0.08, k.center().y), pos2(k.center().x - s * 0.08, k.center().y)], BLUE, Stroke::NONE));
            p.rect_filled(Rect::from_min_max(pos2(k.center().x - s * 0.08, k.center().y - 0.5), pos2(k.center().x + s * 0.08, k.bottom())), 1.0, BLUE);
        }
        Icon::Check => {
            let st = Stroke::new(s * 0.14, GREEN);
            p.line_segment([pos2(r.left() + s * 0.18, r.center().y), pos2(r.left() + s * 0.42, r.bottom() - s * 0.25)], st);
            p.line_segment([pos2(r.left() + s * 0.42, r.bottom() - s * 0.25), pos2(r.right() - s * 0.15, r.top() + s * 0.22)], st);
        }
        Icon::Discard => {
            let st = Stroke::new(s * 0.12, pal().icon_stroke);
            let k = r.shrink(s * 0.26);
            p.line_segment([k.left_top(), k.right_bottom()], st);
            p.line_segment([k.right_top(), k.left_bottom()], st);
        }
        Icon::Minus => {
            let k = r.shrink(s * 0.2);
            p.line_segment([pos2(k.left(), k.center().y), pos2(k.right(), k.center().y)], Stroke::new(s * 0.13, RED));
        }
        Icon::MoveUp | Icon::MoveDown => move_arrow(p, r, icon == Icon::MoveUp),
    }
}

/// The DBZ logo (square app icon, transparent background).
pub const LOGO_PNG: &[u8] = include_bytes!("../../assets/logo/icon.png");

/// The DBZ logo texture, decoded once and cached in egui memory.
pub fn logo_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::unique("dbz_logo");
    ctx.data_mut(|d| d.get_temp::<egui::TextureHandle>(id)).unwrap_or_else(|| {
        let img = image::load_from_memory(LOGO_PNG).expect("bundled logo is a valid PNG").to_rgba8();
        let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
        let opts = egui::TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..egui::TextureOptions::LINEAR };
        let t = ctx.load_texture("dbz_logo", ci, opts);
        ctx.data_mut(|d| d.insert_temp(id, t.clone()));
        t
    })
}

/// Shows the DBZ logo at `size` points.
pub fn logo(ui: &mut egui::Ui, size: f32) {
    let tex = logo_texture(ui.ctx());
    ui.add(egui::Image::new(&tex).fit_to_exact_size(vec2(size, size)));
}
