use egui_extras::{Column, TableBuilder};

use crate::db::metadata::{ConnInfo, IndexStat, TableStat};
use crate::i18n::I18n;
use crate::ui::theme::{self as theme, icon};

// ── State ─────────────────────────────────────────────────────────────────────

enum DashboardState {
    Empty,
    Loading,
    Loaded {
        table_stats: Vec<TableStat>,
        connections: Vec<ConnInfo>,
        index_stats: Vec<IndexStat>,
        summary: Summary,
    },
}

/// Card figures, computed once when data arrives — not per frame.
struct Summary {
    tables: String,
    total_size: String,
    /// Largest table's bytes — scales the size bars.
    max_bytes: i64,
    connections: String,
    unused_indexes: usize,
    index_count: usize,
}

impl Summary {
    fn new(tables: &[TableStat], conns: &[ConnInfo], indexes: &[IndexStat]) -> Self {
        let active = conns.iter().filter(|c| c.state == "active").count();
        Self {
            tables: tables.len().to_string(),
            total_size: theme::format_bytes(tables.iter().map(|t| t.total_bytes).sum()),
            max_bytes: tables.iter().map(|t| t.total_bytes).max().unwrap_or(0),
            connections: format!("{active} / {}", conns.len()),
            unused_indexes: indexes.iter().filter(|i| i.scans == 0).count(),
            index_count: indexes.len(),
        }
    }
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
        let summary = Summary::new(&table_stats, &connections, &index_stats);
        self.state = DashboardState::Loaded { table_stats, connections, index_stats, summary };
    }

    /// Returns true when the dashboard needs to load data (state is Empty).
    pub fn needs_load(&self) -> bool {
        matches!(self.state, DashboardState::Empty)
    }

    /// Render the dashboard content inline (no Window wrapper).
    /// Returns `(refresh_clicked, kill_pid)`.
    pub fn show_inline(&mut self, ui: &mut egui::Ui, i18n: &I18n) -> (bool, Option<String>) {
        let mut refresh_clicked = false;
        let mut kill_pid: Option<String> = None;

        // ── Summary cards ─────────────────────────────────────────────────────
        if let DashboardState::Loaded { summary, .. } = &self.state {
            ui.add_space(6.0);
            let unused_color = if summary.unused_indexes > 0 { theme::WARNING } else { theme::TEXT };
            let unused = format!("{} / {}", summary.unused_indexes, summary.index_count);
            ui.columns(4, |cols| {
                theme::stat_card(&mut cols[0], icon::TABLE, i18n.dash_card_tables(), &summary.tables, theme::TEXT);
                theme::stat_card(&mut cols[1], icon::HARD_DRIVES, i18n.dash_card_total_size(), &summary.total_size, theme::TEXT);
                theme::stat_card(&mut cols[2], icon::PLUGS_CONNECTED, i18n.dash_card_connections(), &summary.connections, theme::TEXT);
                theme::stat_card(&mut cols[3], icon::LIST_NUMBERS, i18n.dash_card_unused_indexes(), &unused, unused_color);
            });
            ui.add_space(8.0);
        }

        // ── Toolbar ───────────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            if theme::tab_toggle(ui, self.active_tab == DashTab::TableSizes, i18n.dash_tab_table_sizes()).clicked() {
                self.active_tab = DashTab::TableSizes;
            }
            if theme::tab_toggle(ui, self.active_tab == DashTab::Connections, i18n.dash_tab_connections()).clicked() {
                self.active_tab = DashTab::Connections;
            }
            if theme::tab_toggle(ui, self.active_tab == DashTab::IndexStats, i18n.dash_tab_index_stats()).clicked() {
                self.active_tab = DashTab::IndexStats;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(i18n.btn_refresh()).clicked() {
                    refresh_clicked = true;
                }
            });
        });

        ui.add_space(2.0);

        match &self.state {
            DashboardState::Empty => {
                theme::empty_state(ui, icon::GAUGE, i18n.lbl_loading(), "");
            }
            DashboardState::Loading => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.label(egui::RichText::new(i18n.lbl_loading_dashboard()).color(theme::TEXT_MUTED));
                });
            }
            DashboardState::Loaded { table_stats, connections, index_stats, summary } => {
                match self.active_tab {
                    DashTab::TableSizes => {
                        show_table_sizes(ui, table_stats, summary.max_bytes, i18n);
                    }
                    DashTab::Connections => {
                        kill_pid = show_connections(ui, connections, i18n);
                    }
                    DashTab::IndexStats => {
                        show_index_stats(ui, index_stats, i18n);
                    }
                }
            }
        }

        (refresh_clicked, kill_pid)
    }
}

// ── Table rendering helpers ───────────────────────────────────────────────────

/// Right-aligned cell text.
fn right(ui: &mut egui::Ui, text: egui::RichText) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(text);
    });
}

fn show_table_sizes(ui: &mut egui::Ui, stats: &[TableStat], max_bytes: i64, i18n: &I18n) {
    let available = ui.available_height();
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .min_scrolled_height(available)
        .column(Column::initial(120.0).resizable(true))  // Schema
        .column(Column::remainder().resizable(true))      // Table
        .column(Column::initial(160.0).resizable(true))  // Total (+ bar)
        .column(Column::initial(90.0).resizable(true))   // Table
        .column(Column::initial(90.0).resizable(true))   // Indexes
        .header(26.0, |mut header| {
            header.col(|ui| theme::grid_header(ui, i18n.col_schema()));
            header.col(|ui| theme::grid_header(ui, i18n.col_table()));
            header.col(|ui| theme::grid_header(ui, i18n.col_total()));
            header.col(|ui| theme::grid_header(ui, i18n.col_table()));
            header.col(|ui| theme::grid_header(ui, i18n.col_indexes()));
        })
        .body(|mut body| {
            for stat in stats {
                body.row(22.0, |mut row| {
                    row.col(|ui| { ui.label(egui::RichText::new(&stat.schema).color(theme::TEXT_MUTED)); });
                    row.col(|ui| {
                        ui.label(egui::RichText::new(icon::TABLE).color(theme::ACCENT));
                        ui.label(&stat.table);
                    });
                    row.col(|ui| {
                        // Proportional size bar behind the figure.
                        if max_bytes > 0 {
                            let r = ui.max_rect().shrink2(egui::vec2(0.0, 5.0));
                            let frac = (stat.total_bytes as f32 / max_bytes as f32).clamp(0.0, 1.0);
                            let bar = egui::Rect::from_min_size(r.min, egui::vec2(r.width() * frac, r.height()));
                            ui.painter().rect_filled(bar, 3.0, theme::ACCENT.gamma_multiply(0.25));
                        }
                        right(ui, egui::RichText::new(&stat.total_size).color(theme::NUMBER));
                    });
                    row.col(|ui| right(ui, egui::RichText::new(&stat.table_size).color(theme::TEXT_MUTED)));
                    row.col(|ui| right(ui, egui::RichText::new(&stat.index_size).color(theme::TEXT_MUTED)));
                });
            }
        });
}

fn show_connections(ui: &mut egui::Ui, conns: &[ConnInfo], i18n: &I18n) -> Option<String> {
    let available = ui.available_height();
    let mut kill_pid: Option<String> = None;

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .min_scrolled_height(available)
        .column(Column::initial(60.0).resizable(true))   // PID
        .column(Column::initial(100.0).resizable(true))  // User
        .column(Column::initial(120.0).resizable(true))  // App
        .column(Column::initial(90.0).resizable(true))   // State
        .column(Column::initial(70.0).resizable(true))   // Duration
        .column(Column::remainder().resizable(true))     // Query
        .column(Column::initial(60.0))                   // Kill
        .header(26.0, |mut header| {
            header.col(|ui| theme::grid_header(ui, i18n.col_pid()));
            header.col(|ui| theme::grid_header(ui, i18n.col_user()));
            header.col(|ui| theme::grid_header(ui, i18n.col_app()));
            header.col(|ui| theme::grid_header(ui, i18n.col_state()));
            header.col(|ui| theme::grid_header(ui, i18n.col_duration()));
            header.col(|ui| theme::grid_header(ui, i18n.col_query()));
            header.col(|ui| theme::grid_header(ui, ""));
        })
        .body(|mut body| {
            for conn in conns {
                body.row(24.0, |mut row| {
                    row.col(|ui| { ui.label(egui::RichText::new(&conn.pid).monospace().small().color(theme::NUMBER)); });
                    row.col(|ui| { ui.label(&conn.username); });
                    row.col(|ui| { ui.label(egui::RichText::new(&conn.app_name).color(theme::TEXT_MUTED)); });
                    row.col(|ui| {
                        let (color, label) = match conn.state.as_str() {
                            "active" => (theme::SUCCESS, "active"),
                            "idle" => (theme::TEXT_MUTED, "idle"),
                            "idle in transaction" => (theme::WARNING, "idle/tx"),
                            s => (theme::WARNING, s),
                        };
                        ui.label(egui::RichText::new("●").small().color(color));
                        ui.label(egui::RichText::new(label).color(color));
                    });
                    row.col(|ui| right(ui, egui::RichText::new(&conn.duration).small().color(theme::TEXT_MUTED)));
                    row.col(|ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&conn.query_preview)
                                    .monospace()
                                    .small(),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&conn.query_preview);
                    });
                    row.col(|ui| {
                        let btn = egui::Button::new(
                            egui::RichText::new(format!("{} {}", icon::X_CIRCLE, i18n.btn_kill()))
                                .small()
                                .color(theme::DANGER),
                        )
                        .fill(egui::Color32::TRANSPARENT);
                        if ui.add(btn)
                            .on_hover_text(i18n.hover_kill(&conn.pid))
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

fn show_index_stats(ui: &mut egui::Ui, stats: &[IndexStat], i18n: &I18n) {
    let available = ui.available_height();
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .min_scrolled_height(available)
        .column(Column::initial(100.0).resizable(true))  // Schema
        .column(Column::initial(140.0).resizable(true))  // Table
        .column(Column::remainder().resizable(true))     // Index
        .column(Column::initial(80.0).resizable(true))  // Size
        .column(Column::initial(70.0).resizable(true))  // Scans
        .header(26.0, |mut header| {
            header.col(|ui| theme::grid_header(ui, i18n.col_schema()));
            header.col(|ui| theme::grid_header(ui, i18n.col_table()));
            header.col(|ui| theme::grid_header(ui, i18n.col_index()));
            header.col(|ui| theme::grid_header(ui, i18n.col_size()));
            header.col(|ui| theme::grid_header(ui, i18n.col_scans()));
        })
        .body(|mut body| {
            for stat in stats {
                body.row(22.0, |mut row| {
                    row.col(|ui| { ui.label(egui::RichText::new(&stat.schema).color(theme::TEXT_MUTED)); });
                    row.col(|ui| { ui.label(&stat.table); });
                    row.col(|ui| { ui.label(egui::RichText::new(&stat.index_name).monospace().small()); });
                    row.col(|ui| right(ui, egui::RichText::new(&stat.size).color(theme::TEXT_MUTED)));
                    row.col(|ui| {
                        // Never-scanned indexes cost writes without serving reads.
                        let color = if stat.scans == 0 { theme::WARNING } else { theme::NUMBER };
                        right(ui, egui::RichText::new(stat.scans.to_string()).color(color));
                    });
                });
            }
        });
}
