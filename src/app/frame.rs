//! The eframe entry points: lays out the panes each frame and runs the collected acts.

use eframe::egui::{self, Stroke, Ui};

use super::{Act, Dbz, Tab};
use crate::ui::splash::{show_splash, splash_frame};
use crate::ui::theme::{self, pal};
use crate::ui::{dialogs, panes, views};

impl eframe::App for Dbz {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.shot.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.poll();
        let ctx = ui.ctx().clone();
        theme::sync(&ctx);
        let mut acts: Vec<Act> = vec![];
        if !self.dialogs.any_open() {
            self.shortcuts(&ctx, &mut acts);
        }
        self.layout(ui, &mut acts);
        dialogs::show(&ctx, &mut self.dialogs, &self.ws, &mut acts);
        self.splash_step(&ctx);
        for a in acts {
            self.do_act(a);
        }
        self.screenshot_step(&ctx);
    }
}

impl Dbz {
    /// Draws the splash over the app while it lasts; the app loads underneath.
    fn splash_step(&mut self, ctx: &egui::Context) {
        let Some(frame) = self.splash.and_then(|start| splash_frame(start.elapsed())) else {
            self.splash = None;
            return;
        };
        if show_splash(ctx, frame) {
            self.splash = None;
        }
        ctx.request_repaint();
    }
}

fn bordered(fill: egui::Color32) -> egui::Frame {
    egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, pal().border))
}

impl Dbz {
    fn layout(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let (ws, prefs) = (&self.ws, &mut self.panes);
        egui::Panel::top("menu")
            .frame(egui::Frame::new().fill(pal().surface).inner_margin(egui::Margin::symmetric(6, 2)))
            .show(ui, |ui| panes::menu_bar(ui, ws, prefs, self.settings.theme, acts));
        egui::Panel::top("ribbon").frame(bordered(pal().ribbon_bg).inner_margin(egui::Margin::symmetric(8, 4))).show(ui, |ui| panes::ribbon(ui, ws, acts));
        egui::Panel::bottom("status")
            .exact_size(26.0)
            .frame(bordered(pal().header_bg).inner_margin(egui::Margin::symmetric(6, 0)))
            .show(ui, |ui| panes::status_bar(ui, ws, &self.status));
        egui::Panel::left("nav")
            .resizable(true)
            .default_size(260.0)
            .min_size(170.0)
            .frame(bordered(pal().surface))
            .show(ui, |ui| panes::nav(ui, ws, &mut prefs.tree_filter, acts));
        if prefs.show_info {
            egui::Panel::right("info")
                .resizable(true)
                .default_size(270.0)
                .min_size(180.0)
                .frame(bordered(pal().surface))
                .show(ui, |ui| panes::info(ui, ws, &mut prefs.info_ddl, acts));
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().surface)).show(ui, |ui| self.work_area(ui, acts));
    }

    /// The tab strip and the active tab.
    fn work_area(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        panes::tab_bar(ui, &self.ws, acts);
        if self.ws.active == 0 {
            return panes::objects(ui, &self.ws, &mut self.panes.obj_filter, acts);
        }
        let ws = &mut self.ws;
        let Some(tab) = ws.tabs.get_mut(ws.active - 1) else { return };
        match tab {
            Tab::Query(t) => views::query_view(ui, t, &ws.conns, acts),
            Tab::Data(t) => views::data_view(ui, t, acts),
            Tab::Design(t) => views::design_view(ui, t, acts),
        }
    }

    /// Ctrl+Q new query, Ctrl+W close tab, F5 refresh (or reload the data tab).
    fn shortcuts(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        let (q, w, f5) = ctx.input_mut(|i| {
            (i.consume_key(egui::Modifiers::COMMAND, egui::Key::Q), i.consume_key(egui::Modifiers::COMMAND, egui::Key::W), i.consume_key(egui::Modifiers::NONE, egui::Key::F5))
        });
        if let (true, Some(cid)) = (q, self.ws.query_target()) {
            acts.push(Act::NewQuery(cid, self.ws.sel.db.clone().unwrap_or_default(), String::new()));
        }
        if w && self.ws.active > 0 {
            acts.push(Act::CloseTab(self.ws.active));
        }
        if !f5 {
            return;
        }
        let active = self.ws.active;
        match active.checked_sub(1).and_then(|i| self.ws.tabs.get_mut(i)) {
            Some(Tab::Data(d)) => acts.push(Act::Send(d.cid, d.reload_req())),
            _ => acts.extend(self.ws.refresh_acts()),
        }
    }
}
