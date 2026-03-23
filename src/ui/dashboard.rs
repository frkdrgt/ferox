use egui_extras::{Column, TableBuilder};

use crate::db::metadata::{ConnInfo, IndexStat, TableStat};

// ── State ─────────────────────────────────────────────────────────────────────

enum DashboardState {
    Empty,
    Loading,
    Loaded {
        table_stats: Vec<TableStat>,
        connections: Vec<ConnInfo>,
        index_stats: Vec<IndexStat>,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum DashTab {
    TableSizes,
    Connections,
    IndexStats,
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

pub struct Dashboard {
    state: DashboardState,
    active_tab: DashTab,
}

impl Default for Dashboard {
    fn default() -> Self {
        Self {
            state: DashboardState::Empty,
            active_tab: DashTab::TableSizes,
        }
    }
}

impl Dashboard {
    /// Reset state back to Empty so that it triggers a reload.
    pub fn reset(&mut self) {
        self.state = DashboardState::Empty;
    }

    pub fn set_loading(&mut self) {
        self.state = DashboardState::Loading;
    }

    pub fn set_data(
        &mut self,
        table_stats: Vec<TableStat>,
        connections: Vec<ConnInfo>,
        index_stats: Vec<IndexStat>,
    ) {
        self.state = DashboardState::Loaded { table_stats, connections, index_stats };
    }

    /// Returns true when the dashboard needs to load data (state is Empty).
    pub fn needs_load(&self) -> bool {
        matches!(self.state, DashboardState::Empty)
    }

    /// Render the dashboard content inline (no Window wrapper).
    /// Returns `(refresh_clicked, kill_pid)`.
    pub fn show_inline(&mut self, ui: &mut egui::Ui) -> (bool, Option<String>) {
        let mut refresh_clicked = false;
        let mut kill_pid: Option<String> = None;

        // Toolbar
        ui.horizontal(|ui| {
            if ui.button("↺ Refresh").clicked() {
                refresh_clicked = true;
            }
            ui.separator();

            ui.selectable_value(&mut self.active_tab, DashTab::TableSizes, "Table Sizes");
            ui.selectable_value(&mut self.active_tab, DashTab::Connections, "Connections");
            ui.selectable_value(&mut self.active_tab, DashTab::IndexStats, "Index Stats");
        });

        ui.separator();

        match &self.state {
            DashboardState::Empty => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("Loading…")
                            .color(egui::Color32::GRAY)
                            .italics(),
                    );
                });
            }
            DashboardState::Loading => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Loading dashboard data…");
                    });
                });
            }
            DashboardState::Loaded { table_stats, connections, index_stats } => {
                match self.active_tab {
                    DashTab::TableSizes => {
                        show_table_sizes(ui, table_stats);
                    }
                    DashTab::Connections => {
                        kill_pid = show_connections(ui, connections);
                    }
                    DashTab::IndexStats => {
                        show_index_stats(ui, index_stats);
                    }
                }
            }
        }

        (refresh_clicked, kill_pid)
    }
}

// ── Table rendering helpers ───────────────────────────────────────────────────

fn show_table_sizes(ui: &mut egui::Ui, stats: &[TableStat]) {
    let available = ui.available_height();
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .min_scrolled_height(available)
        .column(Column::initial(120.0).resizable(true))  // Schema
        .column(Column::remainder().resizable(true))      // Table
        .column(Column::initial(90.0).resizable(true))   // Total
        .column(Column::initial(90.0).resizable(true))   // Table
        .column(Column::initial(90.0).resizable(true))   // Indexes
        .header(22.0, |mut header| {
            header.col(|ui| { ui.strong("Schema"); });
            header.col(|ui| { ui.strong("Table"); });
            header.col(|ui| { ui.strong("Total"); });
            header.col(|ui| { ui.strong("Table"); });
            header.col(|ui| { ui.strong("Indexes"); });
        })
        .body(|mut body| {
            for stat in stats {
                body.row(18.0, |mut row| {
                    row.col(|ui| { ui.label(&stat.schema); });
                    row.col(|ui| { ui.label(&stat.table); });
                    row.col(|ui| { ui.label(&stat.total_size); });
                    row.col(|ui| { ui.label(&stat.table_size); });
                    row.col(|ui| { ui.label(&stat.index_size); });
                });
            }
        });
}

fn show_connections(ui: &mut egui::Ui, conns: &[ConnInfo]) -> Option<String> {
    let available = ui.available_height();
    let mut kill_pid: Option<String> = None;

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .min_scrolled_height(available)
        .column(Column::initial(60.0).resizable(true))   // PID
        .column(Column::initial(100.0).resizable(true))  // User
        .column(Column::initial(120.0).resizable(true))  // App
        .column(Column::initial(80.0).resizable(true))   // State
        .column(Column::initial(70.0).resizable(true))   // Duration
        .column(Column::remainder().resizable(true))     // Query
        .column(Column::initial(50.0))                   // Kill
        .header(22.0, |mut header| {
            header.col(|ui| { ui.strong("PID"); });
            header.col(|ui| { ui.strong("User"); });
            header.col(|ui| { ui.strong("App"); });
            header.col(|ui| { ui.strong("State"); });
            header.col(|ui| { ui.strong("Duration"); });
            header.col(|ui| { ui.strong("Query"); });
            header.col(|ui| { ui.strong(""); });
        })
        .body(|mut body| {
            for conn in conns {
                body.row(22.0, |mut row| {
                    row.col(|ui| { ui.label(egui::RichText::new(&conn.pid).monospace().small()); });
                    row.col(|ui| { ui.label(&conn.username); });
                    row.col(|ui| { ui.label(&conn.app_name); });
                    row.col(|ui| {
                        let (color, label) = match conn.state.as_str() {
                            "active" => (egui::Color32::from_rgb(80, 200, 120), "active"),
                            "idle" => (egui::Color32::from_rgb(110, 123, 139), "idle"),
                            "idle in transaction" => (egui::Color32::from_rgb(220, 160, 60), "idle/tx"),
                            s => (egui::Color32::from_rgb(220, 160, 60), s),
                        };
                        ui.colored_label(color, label);
                    });
                    row.col(|ui| { ui.label(egui::RichText::new(&conn.duration).small()); });
                    row.col(|ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&conn.query_preview)
                                    .monospace()
                                    .small(),
                            )
                            .wrap(true),
                        );
                    });
                    row.col(|ui| {
                        let btn = egui::Button::new(
                            egui::RichText::new("Kill")
                                .small()
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        )
                        .fill(egui::Color32::TRANSPARENT);
                        if ui.add(btn)
                            .on_hover_text(format!("Terminate PID {}", conn.pid))
                            .clicked()
                        {
                            kill_pid = Some(conn.pid.clone());
                        }
                    });
                });
            }
        });

    kill_pid
}

fn show_index_stats(ui: &mut egui::Ui, stats: &[IndexStat]) {
    let available = ui.available_height();
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .min_scrolled_height(available)
        .column(Column::initial(100.0).resizable(true))  // Schema
        .column(Column::initial(120.0).resizable(true))  // Table
        .column(Column::remainder().resizable(true))     // Index
        .column(Column::initial(80.0).resizable(true))  // Size
        .column(Column::initial(70.0).resizable(true))  // Scans
        .header(22.0, |mut header| {
            header.col(|ui| { ui.strong("Schema"); });
            header.col(|ui| { ui.strong("Table"); });
            header.col(|ui| { ui.strong("Index"); });
            header.col(|ui| { ui.strong("Size"); });
            header.col(|ui| { ui.strong("Scans"); });
        })
        .body(|mut body| {
            for stat in stats {
                body.row(18.0, |mut row| {
                    row.col(|ui| { ui.label(&stat.schema); });
                    row.col(|ui| { ui.label(&stat.table); });
                    row.col(|ui| { ui.label(&stat.index_name); });
                    row.col(|ui| { ui.label(&stat.size); });
                    row.col(|ui| { ui.label(stat.scans.to_string()); });
                });
            }
        });
}
