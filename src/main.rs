#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backend;
mod demo;
mod dialogs;
mod icons;
mod model;
mod panes;
mod sqlgen;
mod views;
#[cfg(test)]
mod tests;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot = Some(args[i + 1].clone());
            i += 1;
        } else if let Some(p) = args[i].strip_prefix("--screenshot=") {
            screenshot = Some(p.to_string());
        }
        i += 1;
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("DBZ")
        .with_icon(eframe::icon_data::from_png_bytes(icons::LOGO_PNG).expect("bundled logo is a valid PNG"))
        .with_inner_size([1366.0, 820.0])
        .with_min_inner_size([900.0, 560.0]);
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, persist_window: false, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        options.event_loop_builder = Some(Box::new(|b| {
            b.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(
        "DBZ",
        options,
        Box::new(move |cc| Ok(Box::new(app::Dbz::new(cc, screenshot)))),
    )
}
