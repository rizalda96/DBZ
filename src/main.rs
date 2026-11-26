#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod app;
mod db;
mod domain;
mod shared;
mod sql;
mod transfer;
mod ui;
mod worker;
#[cfg(test)]
mod tests;

use eframe::egui;

/// `--screenshot out.png` or `--screenshot=out.png`.
fn screenshot_arg(args: &[String]) -> Option<String> {
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        if a == "--screenshot" {
            return it.next().cloned();
        }
        if let Some(p) = a.strip_prefix("--screenshot=") {
            return Some(p.to_string());
        }
    }
    None
}

fn native_options(screenshot: bool) -> eframe::NativeOptions {
    let icon = eframe::icon_data::from_png_bytes(ui::icons::LOGO_PNG).expect("bundled logo is a valid PNG");
    // Opens maximized; the inner size is what "Restore Down" goes back to.
    let mut viewport = egui::ViewportBuilder::default().with_title("DBZ").with_icon(icon).with_inner_size([1366.0, 820.0]).with_min_inner_size([900.0, 560.0]);
    if screenshot {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    } else {
        // Screenshot runs keep the fixed size so pictures come out the same.
        viewport = viewport.with_maximized(true);
    }
    #[allow(unused_mut)] // only changed on macOS
    let mut options = eframe::NativeOptions { viewport, persist_window: false, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        options.event_loop_builder = Some(Box::new(|b| {
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    options
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let screenshot = screenshot_arg(&args);
    let options = native_options(screenshot.is_some());
    eframe::run_native("DBZ", options, Box::new(move |cc| Ok(Box::new(app::Dbz::new(cc, screenshot)))))
}
