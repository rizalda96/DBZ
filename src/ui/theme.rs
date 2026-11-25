//! Light and dark color palettes, and the user's theme choice.
//!
//! egui keeps a separate style for each theme and picks one every frame (following the
//! OS when the preference is "System"). `sync` mirrors that choice so `pal()` hands the
//! custom-painted parts of the UI the matching colors.

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, Color32};
use serde::{Deserialize, Serialize};

use crate::domain::DbKind;

pub struct Palette {
    pub text: Color32,
    pub text_dim: Color32,
    pub muted: Color32,
    pub disabled: Color32,
    pub accent: Color32,
    pub err: Color32,
    pub ok: Color32,
    /// Main content areas: panels, grids, editors.
    pub surface: Color32,
    pub panel_fill: Color32,
    pub faint_bg: Color32,
    pub header_bg: Color32,
    pub toolbar_bg: Color32,
    pub ribbon_bg: Color32,
    pub sel_bg: Color32,
    pub hover_bg: Color32,
    pub tree_hover: Color32,
    pub border: Color32,
    pub grid_line: Color32,
    pub focus_border: Color32,
    pub button_bg: Color32,
    pub button_border: Color32,
    pub tab_strip: Color32,
    pub tab_inactive: Color32,
    pub tab_hover: Color32,
    pub tab_close_hover: Color32,
    pub expander: Color32,
    pub icon_stroke: Color32,
    pub edited_bg: Color32,
    pub new_bg: Color32,
    pub deleted_bg: Color32,
    pub syn_keyword: Color32,
    pub syn_string: Color32,
    pub syn_number: Color32,
    pub syn_comment: Color32,
    pub syn_ident: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub const LIGHT: Palette = Palette {
    text: rgb(0x22, 0x26, 0x2b),
    text_dim: rgb(0x55, 0x5b, 0x64),
    muted: rgb(0x8a, 0x90, 0x99),
    disabled: rgb(0xa5, 0xa5, 0xa5),
    accent: rgb(0x1f, 0x6f, 0xd1),
    err: rgb(0xb0, 0x20, 0x20),
    ok: rgb(0x2a, 0x8a, 0x4a),
    surface: Color32::WHITE,
    panel_fill: rgb(0xfa, 0xfa, 0xfb),
    faint_bg: rgb(0xf6, 0xf8, 0xfb),
    header_bg: rgb(0xf0, 0xf1, 0xf3),
    toolbar_bg: rgb(0xf7, 0xf7, 0xf8),
    ribbon_bg: rgb(0xf4, 0xf5, 0xf7),
    sel_bg: rgb(0xcc, 0xe2, 0xfa),
    hover_bg: rgb(0xe6, 0xef, 0xfa),
    tree_hover: rgb(0xee, 0xf3, 0xfa),
    border: rgb(0xd4, 0xd7, 0xdc),
    grid_line: rgb(0xe3, 0xe5, 0xe8),
    focus_border: rgb(0x9c, 0xc0, 0xec),
    button_bg: rgb(0xf1, 0xf2, 0xf4),
    button_border: rgb(0xd0, 0xd4, 0xda),
    tab_strip: rgb(0xec, 0xee, 0xf1),
    tab_inactive: rgb(0xe2, 0xe5, 0xe9),
    tab_hover: rgb(0xf6, 0xf7, 0xf9),
    tab_close_hover: rgb(0xd6, 0xdb, 0xe2),
    expander: rgb(0x80, 0x86, 0x8f),
    icon_stroke: rgb(0x45, 0x4f, 0x5c),
    edited_bg: rgb(0xff, 0xf6, 0xd5),
    new_bg: rgb(0xe4, 0xf6, 0xe7),
    deleted_bg: rgb(0xfb, 0xe3, 0xe3),
    syn_keyword: rgb(0x1a, 0x4f, 0xc4),
    syn_string: rgb(0xb8, 0x3a, 0x26),
    syn_number: rgb(0x0a, 0x86, 0x6e),
    syn_comment: rgb(0x6a, 0x9a, 0x55),
    syn_ident: rgb(0x8a, 0x3f, 0x9e),
};

pub const DARK: Palette = Palette {
    text: rgb(0xe3, 0xe6, 0xea),
    text_dim: rgb(0xb4, 0xba, 0xc3),
    muted: rgb(0x8f, 0x96, 0xa0),
    disabled: rgb(0x68, 0x6e, 0x76),
    accent: rgb(0x5a, 0xa5, 0xf5),
    err: rgb(0xf0, 0x74, 0x74),
    ok: rgb(0x5c, 0xc4, 0x7e),
    surface: rgb(0x1e, 0x20, 0x24),
    panel_fill: rgb(0x22, 0x25, 0x29),
    faint_bg: rgb(0x25, 0x28, 0x2d),
    header_bg: rgb(0x2a, 0x2e, 0x34),
    toolbar_bg: rgb(0x26, 0x29, 0x2e),
    ribbon_bg: rgb(0x26, 0x29, 0x2e),
    sel_bg: rgb(0x1f, 0x40, 0x68),
    hover_bg: rgb(0x29, 0x37, 0x4a),
    tree_hover: rgb(0x29, 0x33, 0x40),
    border: rgb(0x3a, 0x3f, 0x46),
    grid_line: rgb(0x32, 0x36, 0x3c),
    focus_border: rgb(0x3f, 0x72, 0xae),
    button_bg: rgb(0x31, 0x35, 0x3b),
    button_border: rgb(0x48, 0x4e, 0x56),
    tab_strip: rgb(0x19, 0x1b, 0x1e),
    tab_inactive: rgb(0x2a, 0x2d, 0x32),
    tab_hover: rgb(0x33, 0x37, 0x3d),
    tab_close_hover: rgb(0x46, 0x4b, 0x53),
    expander: rgb(0x8f, 0x96, 0xa0),
    icon_stroke: rgb(0xb8, 0xc0, 0xca),
    edited_bg: rgb(0x4a, 0x42, 0x1f),
    new_bg: rgb(0x1f, 0x3e, 0x28),
    deleted_bg: rgb(0x4b, 0x24, 0x27),
    syn_keyword: rgb(0x6c, 0xb2, 0xff),
    syn_string: rgb(0xf0, 0x92, 0x7c),
    syn_number: rgb(0x4f, 0xd1, 0xb0),
    syn_comment: rgb(0x82, 0xb0, 0x6c),
    syn_ident: rgb(0xd2, 0x9c, 0xf0),
};

static DARK_ACTIVE: AtomicBool = AtomicBool::new(false);

/// The palette for the theme egui is currently showing.
pub fn pal() -> &'static Palette {
    if DARK_ACTIVE.load(Ordering::Relaxed) {
        &DARK
    } else {
        &LIGHT
    }
}

/// Call once per frame, before drawing, so `pal()` matches egui's active theme.
pub fn sync(ctx: &egui::Context) {
    DARK_ACTIVE.store(ctx.theme() == egui::Theme::Dark, Ordering::Relaxed);
}

/// `c` made translucent, used to grey out disabled icons.
pub fn veil(c: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 170)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePref {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePref {
    pub const ALL: [ThemePref; 3] = [ThemePref::System, ThemePref::Light, ThemePref::Dark];

    pub fn label(self) -> &'static str {
        match self {
            ThemePref::System => "Follow System",
            ThemePref::Light => "Light",
            ThemePref::Dark => "Dark",
        }
    }

    pub fn apply(self, ctx: &egui::Context) {
        ctx.set_theme(match self {
            ThemePref::System => egui::ThemePreference::System,
            ThemePref::Light => egui::ThemePreference::Light,
            ThemePref::Dark => egui::ThemePreference::Dark,
        });
    }
}

/// Brand colour of each server kind, used for its connection icon.
pub fn kind_color(kind: DbKind) -> Color32 {
    match kind {
        DbKind::MySql => rgb(0x2b, 0x7a, 0xb8),
        DbKind::MariaDb => rgb(0x9a, 0x6a, 0x3a),
        DbKind::PostgreSql => rgb(0x33, 0x5d, 0x8f),
    }
}

/// Colour of the "changed row" dot in the data grid.
pub const CHANGED_DOT: Color32 = rgb(0xe8, 0x8f, 0x2a);
/// Warning badge of confirmation dialogs.
pub const WARNING: Color32 = rgb(0xf0, 0xa0, 0x20);
