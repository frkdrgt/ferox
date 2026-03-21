use std::collections::HashMap;
use std::sync::mpsc::Sender;

use egui::{Color32, RichText, Sense};

use crate::db::metadata::{ConnInfo, IndexStat, TableStat};
use crate::db::{DbCommand, QueryResult};
use crate::history::QueryHistory;
use crate::ui::dashboard::Dashboard;
use crate::ui::query_panel::QueryPanel;

// ── Tab content ───────────────────────────────────────────────────────────────

enum TabContent {
    Query(QueryPanel),
    Dashboard,
}

// ── Tab ───────────────────────────────────────────────────────────────────────

struct Tab {
    id: usize,
    title: String,
    content: TabContent,
    /// Which connection this tab belongs to.
    conn_id: usize,
}

impl Tab {
    fn panel(&self) -> Option<&QueryPanel> {
        match &self.content {
            TabContent::Query(p) => Some(p),
            TabContent::Dashboard => None,
        }
    }

    fn panel_mut(&mut self) -> Option<&mut QueryPanel> {
        match &mut self.content {
            TabContent::Query(p) => Some(p),
            TabContent::Dashboard => None,
        }
    }
}

// ── TabManager ────────────────────────────────────────────────────────────────

pub struct TabManager {
    tabs: Vec<Tab>,
    /// Index of the currently visible tab.
    active: usize,
    /// conn_id → tab_idx for tabs currently awaiting a DB result.
    running_tabs: HashMap<usize, usize>,
    next_id: usize,
    next_num: usize,
    /// The shared dashboard widget (state is shared across all Dashboard tabs).
    pub dashboard: Dashboard,
}

impl Default for TabManager {
    fn default() -> Self {
        Self {
            tabs: vec![Tab {
                id: 0,
                title: "Query 1".to_owned(),
                content: TabContent::Query(QueryPanel::default()),
                conn_id: 0,
            }],
            active: 0,
            running_tabs: HashMap::new(),
            next_id: 1,
            next_num: 2,
            dashboard: Dashboard::default(),
        }
    }
}

impl TabManager {
    // ── Private helpers ───────────────────────────────────────────────────────

    fn active_panel(&self) -> Option<&QueryPanel> {
        self.tabs.get(self.active).and_then(|t| t.panel())
    }

    fn active_panel_mut(&mut self) -> Option<&mut QueryPanel> {
        self.tabs.get_mut(self.active).and_then(|t| t.panel_mut())
    }

    // ── Tab lifecycle ─────────────────────────────────────────────────────────

    pub fn new_tab(&mut self, conn_id: usize) {
        let id = self.next_id;
        self.next_id += 1;
        let num = self.next_num;
        self.next_num += 1;
        self.tabs.push(Tab {
            id,
            title: format!("Query {num}"),
            content: TabContent::Query(QueryPanel::default()),
            conn_id,
        });
        self.active = self.tabs.len() - 1;
    }

    /// Find existing Dashboard tab or create one; resets dashboard state to trigger reload.
    pub fn open_or_focus_dashboard(&mut self, conn_id: usize) {
        // Find existing dashboard tab
        if let Some(idx) = self.tabs.iter().position(|t| matches!(t.content, TabContent::Dashboard)) {
            self.active = idx;
            // Update conn_id in case connection switched
            self.tabs[idx].conn_id = conn_id;
        } else {
            // Create new dashboard tab
            let id = self.next_id;
            self.next_id += 1;
            self.tabs.push(Tab {
                id,
                title: "📊 Dashboard".to_owned(),
                content: TabContent::Dashboard,
                conn_id,
            });
            self.active = self.tabs.len() - 1;
        }
        self.dashboard.reset();
    }

    pub fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 {
            return;
        }
        let conn_id = self.tabs[idx].conn_id;
        self.tabs.remove(idx);

        // Keep running_tabs consistent.
        // If the closed tab was a running tab, remove it.
        if let Some(rt) = self.running_tabs.get(&conn_id).copied() {
            if rt == idx {
                self.running_tabs.remove(&conn_id);
            } else if rt > idx {
                self.running_tabs.insert(conn_id, rt - 1);
            }
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

    // ── Dashboard queries ──────────────────────────────────────────────────────

    pub fn dashboard_needs_load(&self) -> bool {
        self.dashboard.needs_load()
    }

    pub fn dashboard_is_active(&self) -> bool {
        self.tabs.get(self.active).map(|t| matches!(t.content, TabContent::Dashboard)).unwrap_or(false)
    }

    /// Returns the conn_id of the Dashboard tab if one exists.
    pub fn dashboard_conn_id(&self) -> Option<usize> {
        self.tabs.iter().find(|t| matches!(t.content, TabContent::Dashboard)).map(|t| t.conn_id)
    }

    pub fn set_dashboard_loading(&mut self) {
        self.dashboard.set_loading();
    }

    pub fn set_dashboard_data(
        &mut self,
        table_stats: Vec<TableStat>,
        connections: Vec<ConnInfo>,
        index_stats: Vec<IndexStat>,
    ) {
        self.dashboard.set_data(table_stats, connections, index_stats);
    }

    // ── Public API — delegates to active / running tab ────────────────────────

    pub fn current_sql(&self) -> &str {
        self.active_panel().map(|p| p.current_sql()).unwrap_or("")
    }

    pub fn set_sql(&mut self, sql: String) {
        if let Some(p) = self.active_panel_mut() {
            p.set_sql(sql);
        }
    }

    /// Returns the conn_id of the currently active tab.
    pub fn active_tab_conn_id(&self) -> usize {
        self.tabs.get(self.active).map(|t| t.conn_id).unwrap_or(0)
    }

    /// Mark the active tab (for the given conn_id) as running.
    pub fn set_running_for(&mut self, conn_id: usize) {
        self.running_tabs.insert(conn_id, self.active);
        if let Some(p) = self.active_panel_mut() {
            p.set_running();
        }
    }

    /// Route a successful query result to the tab that started the query for this conn_id.
    pub fn set_result_for(&mut self, conn_id: usize, result: QueryResult) {
        let idx = self.running_tabs.remove(&conn_id).unwrap_or(self.active);
        if let Some(t) = self.tabs.get_mut(idx) {
            if let Some(p) = t.panel_mut() {
                p.set_result(result);
            }
        }
    }

    /// Route a query error to the tab that started the query for this conn_id.
    pub fn set_error_for(&mut self, conn_id: usize, msg: String) {
        let idx = self.running_tabs.remove(&conn_id).unwrap_or(self.active);
        if let Some(t) = self.tabs.get_mut(idx) {
            if let Some(p) = t.panel_mut() {
                p.set_error(msg);
            }
        }
    }

    /// Broadcast primary-key info to every Query tab (any tab may browse that table).
    pub fn set_primary_key(&mut self, schema: &str, table: &str, cols: Vec<String>) {
        for t in &mut self.tabs {
            if let Some(p) = t.panel_mut() {
                p.set_primary_key(schema, table, cols.clone());
            }
        }
    }

    pub fn set_export_done(&mut self, path: String) {
        // Find the running tab, or fall back to active.
        let idx = self.running_tabs.values().copied().next().unwrap_or(self.active);
        if let Some(t) = self.tabs.get_mut(idx) {
            if let Some(p) = t.panel_mut() {
                p.set_export_done(path);
            }
        }
    }

    /// True if any tab is currently awaiting a DB result.
    pub fn is_running(&self) -> bool {
        !self.running_tabs.is_empty()
    }

    pub fn last_query_duration(&self) -> Option<f64> {
        self.active_panel().and_then(|p| p.last_query_duration())
    }

    pub fn result_row_count(&self) -> Option<usize> {
        self.active_panel().and_then(|p| p.result_row_count())
    }

    /// Start browsing `schema.table` in the active tab, updating its title.
    pub fn start_browse(
        &mut self,
        schema: String,
        table: String,
        conn_id: usize,
        db_tx: &Sender<DbCommand>,
    ) {
        let active = self.active;
        self.tabs[active].title = table.clone();
        self.tabs[active].conn_id = conn_id;
        self.running_tabs.insert(conn_id, active);
        if let Some(p) = self.tabs[active].panel_mut() {
            p.start_browse(schema, table, db_tx);
        }
    }

    pub fn update_completion_data_for(&mut self, conn_id: usize, tables: Vec<String>, columns: Vec<String>) {
        for t in &mut self.tabs {
            if t.conn_id == conn_id {
                if let Some(p) = t.panel_mut() {
                    p.set_completion_data(tables.clone(), columns.clone());
                }
            }
        }
    }

    pub fn trigger_export_csv(&mut self, db_tx: &Sender<DbCommand>) {
        if let Some(p) = self.active_panel_mut() {
            p.trigger_export_csv(db_tx);
        }
    }

    pub fn trigger_export_json(&mut self, db_tx: &Sender<DbCommand>) {
        if let Some(p) = self.active_panel_mut() {
            p.trigger_export_json(db_tx);
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    /// `conns` is a slice of (conn_id, db_tx) pairs.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        conns: &[(usize, &Sender<DbCommand>)],
        history: &mut QueryHistory,
    ) {
        // ── Keyboard shortcuts ────────────────────────────────────────────────
        let active_conn_id = self.active_tab_conn_id();
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::T)) {
            self.new_tab(active_conn_id);
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
            let cid = self.active_tab_conn_id();
            self.new_tab(cid);
        }

        // ── Active panel ──────────────────────────────────────────────────────
        let active_idx = self.active;
        let is_dashboard = matches!(
            self.tabs.get(active_idx).map(|t| &t.content),
            Some(TabContent::Dashboard)
        );

        if is_dashboard {
            if self.dashboard.show_inline(ui) {
                // Refresh clicked
                self.dashboard.set_loading();
                // Signal to app that dashboard needs reload — the app handles sending the command.
                // We store the conn_id so app can find it via dashboard_conn_id().
            }
        } else if let Some(tab) = self.tabs.get(active_idx) {
            let tab_conn_id = tab.conn_id;
            // Find the db_tx for this tab's conn_id
            let db_tx_opt = conns.iter().find(|(id, _)| *id == tab_conn_id).map(|(_, tx)| *tx);

            if let Some(db_tx) = db_tx_opt {
                // Detect if the panel starts a query from within its own show()
                let was_running = self.tabs[active_idx].panel().map(|p| p.is_running()).unwrap_or(false);
                if let Some(p) = self.tabs[active_idx].panel_mut() {
                    p.show(ui, db_tx, history);
                }
                let is_running_now = self.tabs[active_idx].panel().map(|p| p.is_running()).unwrap_or(false);
                if !was_running && is_running_now {
                    self.running_tabs.insert(tab_conn_id, active_idx);
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("Connection not available")
                            .color(egui::Color32::GRAY)
                            .italics(),
                    );
                });
            }
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
        let tab_data: Vec<(usize, usize, String, bool, bool, bool)> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let is_running = self.running_tabs.values().any(|&rt| rt == i);
                let is_dashboard = matches!(t.content, TabContent::Dashboard);
                (i, t.id, t.title.clone(), i == self.active, is_running, is_dashboard)
            })
            .collect();

        // Borrow the painter before entering child UIs.
        let painter = ui.painter().clone();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;

            for (i, tab_id, title, is_active, is_running, is_dashboard) in &tab_data {
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

                            // Dashboard tab: no close button if it's the only tab.
                            // Query tabs: show close button if there are multiple tabs.
                            let can_close = if *is_dashboard {
                                tab_count > 1
                            } else {
                                tab_count > 1
                            };

                            if can_close {
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
