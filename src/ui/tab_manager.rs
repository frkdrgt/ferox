use std::sync::mpsc::Sender;

use egui::{Color32, RichText, Sense};

use crate::db::{DbCommand, QueryResult};
use crate::history::QueryHistory;
use crate::ui::query_panel::QueryPanel;

// ── Tab ───────────────────────────────────────────────────────────────────────

struct Tab {
    id: usize,
    title: String,
    panel: QueryPanel,
}

// ── TabManager ────────────────────────────────────────────────────────────────

pub struct TabManager {
    tabs: Vec<Tab>,
    /// Index of the currently visible tab.
    active: usize,
    /// Index of the tab that is waiting for a DB result.
    running_tab: Option<usize>,
    next_id: usize,
    next_num: usize,
}

impl Default for TabManager {
    fn default() -> Self {
        Self {
            tabs: vec![Tab {
                id: 0,
                title: "Query 1".to_owned(),
                panel: QueryPanel::default(),
            }],
            active: 0,
            running_tab: None,
            next_id: 1,
            next_num: 2,
        }
    }
}

impl TabManager {
    // ── Private helpers ───────────────────────────────────────────────────────

    fn active_panel(&self) -> &QueryPanel {
        &self.tabs[self.active].panel
    }

    fn active_panel_mut(&mut self) -> &mut QueryPanel {
        &mut self.tabs[self.active].panel
    }

    // ── Tab lifecycle ─────────────────────────────────────────────────────────

    pub fn new_tab(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        let num = self.next_num;
        self.next_num += 1;
        self.tabs.push(Tab {
            id,
            title: format!("Query {num}"),
            panel: QueryPanel::default(),
        });
        self.active = self.tabs.len() - 1;
    }

    pub fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(idx);
        // Keep running_tab index consistent.
        match self.running_tab {
            Some(rt) if rt == idx => self.running_tab = None,
            Some(rt) if rt > idx => self.running_tab = Some(rt - 1),
            _ => {}
        }
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
    }

    fn next_tab(&mut self) {
        self.active = (self.active + 1) % self.tabs.len();
    }

    fn prev_tab(&mut self) {
        if self.active == 0 {
            self.active = self.tabs.len() - 1;
        } else {
            self.active -= 1;
        }
    }

    // ── Public API — delegates to active / running tab ────────────────────────

    pub fn current_sql(&self) -> &str {
        self.active_panel().current_sql()
    }

    pub fn set_sql(&mut self, sql: String) {
        self.active_panel_mut().set_sql(sql);
    }

    /// Mark the active tab as running and record it as the pending result owner.
    pub fn set_running(&mut self) {
        self.running_tab = Some(self.active);
        self.active_panel_mut().set_running();
    }

    /// Route a successful query result to the tab that started the query.
    pub fn set_result(&mut self, result: QueryResult) {
        let idx = self.running_tab.take().unwrap_or(self.active);
        if let Some(t) = self.tabs.get_mut(idx) {
            t.panel.set_result(result);
        }
    }

    /// Route a query error to the tab that started the query.
    pub fn set_error(&mut self, msg: String) {
        let idx = self.running_tab.take().unwrap_or(self.active);
        if let Some(t) = self.tabs.get_mut(idx) {
            t.panel.set_error(msg);
        }
    }

    /// Broadcast primary-key info to every tab (any tab may browse that table).
    pub fn set_primary_key(&mut self, schema: &str, table: &str, cols: Vec<String>) {
        for t in &mut self.tabs {
            t.panel.set_primary_key(schema, table, cols.clone());
        }
    }

    pub fn set_export_done(&mut self, path: String) {
        let idx = self.running_tab.unwrap_or(self.active);
        if let Some(t) = self.tabs.get_mut(idx) {
            t.panel.set_export_done(path);
        }
    }

    /// True if any tab is currently awaiting a DB result.
    pub fn is_running(&self) -> bool {
        self.running_tab.is_some()
    }

    pub fn last_query_duration(&self) -> Option<f64> {
        self.active_panel().last_query_duration()
    }

    pub fn result_row_count(&self) -> Option<usize> {
        self.active_panel().result_row_count()
    }

    /// Start browsing `schema.table` in the active tab, updating its title.
    pub fn start_browse(&mut self, schema: String, table: String, db_tx: &Sender<DbCommand>) {
        self.tabs[self.active].title = table.clone();
        self.running_tab = Some(self.active);
        self.tabs[self.active].panel.start_browse(schema, table, db_tx);
    }

    pub fn trigger_export_csv(&mut self, db_tx: &Sender<DbCommand>) {
        self.active_panel_mut().trigger_export_csv(db_tx);
    }

    pub fn trigger_export_json(&mut self, db_tx: &Sender<DbCommand>) {
        self.active_panel_mut().trigger_export_json(db_tx);
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        db_tx: &Sender<DbCommand>,
        history: &mut QueryHistory,
    ) {
        // ── Keyboard shortcuts ────────────────────────────────────────────────
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::T)) {
            self.new_tab();
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::W))
            && self.tabs.len() > 1
        {
            let idx = self.active;
            self.close_tab(idx);
        }
        // Ctrl+Tab / Ctrl+Shift+Tab — cycle tabs
        let tab_key = egui::Key::Tab;
        if ui.input(|i| i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(tab_key)) {
            self.next_tab();
        }
        if ui.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(tab_key)) {
            self.prev_tab();
        }

        // ── Tab bar ───────────────────────────────────────────────────────────
        let (to_select, to_close, want_new) = self.render_tab_bar(ui);
        if let Some(idx) = to_select {
            self.active = idx;
        }
        if let Some(idx) = to_close {
            self.close_tab(idx);
        }
        if want_new {
            self.new_tab();
        }

        // ── Active panel ──────────────────────────────────────────────────────
        // Detect if the panel starts a query from within its own show()
        // (e.g. the user clicks the Run button inside the editor).
        let was_running = self.tabs[self.active].panel.is_running();
        self.tabs[self.active].panel.show(ui, db_tx, history);
        if !was_running && self.tabs[self.active].panel.is_running() {
            self.running_tab = Some(self.active);
        }
    }

    // ── Tab bar rendering ─────────────────────────────────────────────────────

    fn render_tab_bar(
        &self,
        ui: &mut egui::Ui,
    ) -> (Option<usize>, Option<usize>, bool) {
        let mut to_select: Option<usize> = None;
        let mut to_close: Option<usize> = None;
        let mut want_new = false;

        let tab_count = self.tabs.len();

        // Pre-compute display data (including stable id) to avoid capturing `self` in closures.
        let tab_data: Vec<(usize, usize, String, bool, bool)> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                (i, t.id, t.title.clone(), i == self.active, self.running_tab == Some(i))
            })
            .collect();

        // Borrow the painter before entering child UIs.
        let painter = ui.painter().clone();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;

            for (i, tab_id, title, is_active, is_running) in &tab_data {
                // Truncate long titles.
                let short: String = truncate(title, 24);
                let display = if *is_running {
                    format!("⏳ {short}")
                } else {
                    short
                };

                // Read hover state saved from previous frame so the bg is set
                // BEFORE the frame renders (painter-over-text problem avoided).
                let hover_id = egui::Id::new(("tab_hov", *tab_id));
                let was_hovered: bool =
                    ui.ctx().data(|d| d.get_temp(hover_id).unwrap_or(false));

                let text_color = if *is_active {
                    Color32::from_rgb(220, 220, 220)
                } else if was_hovered {
                    Color32::from_rgb(210, 215, 225)
                } else {
                    Color32::from_gray(140)
                };
                let bg = if *is_active {
                    Color32::from_rgb(32, 36, 46)
                } else if was_hovered {
                    Color32::from_rgb(38, 52, 78)
                } else {
                    Color32::TRANSPARENT
                };

                let r = egui::Frame::none()
                    .fill(bg)
                    .inner_margin(egui::Margin {
                        left: 10.0,
                        right: 6.0,
                        top: 5.0,
                        bottom: 5.0,
                    })
                    .show(ui, |ui| {
                        ui.push_id(*tab_id, |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Label::new(
                                        RichText::new(&display).size(12.0).color(text_color),
                                    )
                                    .sense(Sense::click()),
                                )
                                .clicked()
                            {
                                to_select = Some(*i);
                            }

                            if tab_count > 1 {
                                let xr = ui.add(
                                    egui::Button::new(
                                        RichText::new("×")
                                            .size(12.0)
                                            .color(Color32::from_gray(120)),
                                    )
                                    .frame(false)
                                    .min_size(egui::Vec2::splat(14.0)),
                                );
                                if xr.clicked() {
                                    to_close = Some(*i);
                                }
                            }
                        }); // horizontal
                        }); // push_id
                    }); // Frame::show

                // Save hover state for next frame.
                let is_hovered_now = r.response.hovered();
                ui.ctx().data_mut(|d| d.insert_temp(hover_id, is_hovered_now));

                let tab_rect = r.response.rect;

                if *is_active {
                    // Blue bottom accent line.
                    painter.hline(
                        tab_rect.x_range(),
                        tab_rect.bottom() - 1.0,
                        egui::Stroke::new(2.0, Color32::from_rgb(86, 156, 214)),
                    );
                }
            }

            // "+" new-tab button
            ui.add_space(4.0);
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("+").size(14.0).color(Color32::from_gray(160)),
                    )
                    .frame(false)
                    .min_size(egui::Vec2::new(26.0, 26.0)),
                )
                .on_hover_text("New tab  (Ctrl+T)")
                .clicked()
            {
                want_new = true;
            }
        });

        ui.separator();

        (to_select, to_close, want_new)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn truncate(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let collected: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{collected}…")
    } else {
        collected
    }
}
