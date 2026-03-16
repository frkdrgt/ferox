use std::collections::HashMap;
use std::sync::mpsc::Sender;

use crate::db::{query::CellValue, DbCommand, QueryResult};
use crate::history::QueryHistory;
use crate::ui::explain::{render_explain, ExplainResult};
use crate::ui::result_table::ResultTable;
use crate::ui::syntax::highlight_sql;

const PAGE_SIZE: usize = 100;
const MAX_LOG: usize = 200;

// ── Message log ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum LogKind {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone)]
struct LogEntry {
    kind: LogKind,
    text: String,
    time: chrono::DateTime<chrono::Local>,
}

impl LogEntry {
    fn error(text: impl Into<String>) -> Self {
        Self { kind: LogKind::Error, text: text.into(), time: chrono::Local::now() }
    }
    fn warning(text: impl Into<String>) -> Self {
        Self { kind: LogKind::Warning, text: text.into(), time: chrono::Local::now() }
    }
    fn info(text: impl Into<String>) -> Self {
        Self { kind: LogKind::Info, text: text.into(), time: chrono::Local::now() }
    }
}

// ── Cell popup ────────────────────────────────────────────────────────────────

struct CellPopup {
    col_name: String,
    /// Full string representation of the cell value.
    value: String,
    /// Position in the result table — used if user clicks "Edit".
    display_row: usize,
    col_idx: usize,
}

// ── Tabs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Default, PartialEq)]
enum PanelTab {
    #[default]
    Results,
    Plan,
    History,
    Messages,
}

/// State for table data-browser (separate from free-form SQL editor).
#[derive(Debug, Clone)]
struct BrowseState {
    schema: String,
    table: String,
    page: usize,
    sort_col: Option<String>,
    sort_asc: bool,
}

impl BrowseState {
    fn new(schema: String, table: String) -> Self {
        Self {
            schema,
            table,
            page: 0,
            sort_col: None,
            sort_asc: true,
        }
    }

    fn label(&self) -> String {
        format!("\"{}\".\"{}\"", self.schema, self.table)
    }

    fn build_sql(&self) -> String {
        let order_clause = match &self.sort_col {
            Some(col) => format!(
                " ORDER BY \"{}\" {}",
                col,
                if self.sort_asc { "ASC" } else { "DESC" }
            ),
            None => String::new(),
        };
        let offset = self.page * PAGE_SIZE;
        format!(
            "SELECT * FROM \"{}\".\"{}\"{}  LIMIT {} OFFSET {};",
            self.schema, self.table, order_clause, PAGE_SIZE, offset
        )
    }
}

pub struct QueryPanel {
    sql: String,
    result: Option<QueryResult>,
    running: bool,
    last_elapsed_ms: Option<f64>,
    active_tab: PanelTab,
    /// Event log shown in the Messages tab.
    log: Vec<LogEntry>,
    history_search: String,
    split_ratio: f32,
    /// Active data-browser state (None = free-form query mode).
    browse: Option<BrowseState>,
    /// Whether the last result came from a browse query.
    browse_result: bool,
    /// Parsed EXPLAIN plan (set after a successful EXPLAIN ANALYZE run).
    explain_plan: Option<ExplainResult>,
    /// True while waiting for an EXPLAIN query result.
    explain_mode: bool,
    // ── Inline edit ──────────────────────────────────────────────────────────
    /// (display_row, col_idx, current_text) — persisted across frames.
    edit_state: Option<(usize, usize, String)>,
    /// Auto-focus the TextEdit on the next frame.
    edit_needs_focus: bool,
    /// Primary key columns cached per (schema, table).
    pk_cols: HashMap<(String, String), Vec<String>>,
    /// After a DML completes in browse mode, reload the page.
    pending_refresh: bool,
    /// Floating popup showing the full value of a double-clicked cell.
    cell_popup: Option<CellPopup>,
}

impl Default for QueryPanel {
    fn default() -> Self {
        Self {
            sql: String::new(),
            result: None,
            running: false,
            last_elapsed_ms: None,
            active_tab: PanelTab::Results,
            log: Vec::new(),
            history_search: String::new(),
            split_ratio: 0.35,
            browse: None,
            browse_result: false,
            explain_plan: None,
            explain_mode: false,
            edit_state: None,
            edit_needs_focus: false,
            pk_cols: HashMap::new(),
            pending_refresh: false,
            cell_popup: None,
        }
    }
}

impl QueryPanel {
    pub fn current_sql(&self) -> &str {
        &self.sql
    }

    pub fn set_sql(&mut self, sql: String) {
        self.sql = sql;
        self.browse = None;
        self.browse_result = false;
        self.explain_mode = false;
    }

    pub fn set_running(&mut self) {
        self.running = true;
    }

    fn push_log(&mut self, entry: LogEntry) {
        self.log.push(entry);
        if self.log.len() > MAX_LOG {
            self.log.remove(0);
        }
    }

    pub fn set_result(&mut self, result: QueryResult) {
        self.last_elapsed_ms = Some(result.elapsed_ms);
        self.running = false;

        // Detect EXPLAIN JSON result: single column "QUERY PLAN", single row
        if self.explain_mode
            && result.columns.first().map(|c| c == "QUERY PLAN").unwrap_or(false)
        {
            self.explain_mode = false;
            if let Some(CellValue::Text(json)) =
                result.rows.first().and_then(|r| r.first())
            {
                if let Some(plan) = ExplainResult::parse(json) {
                    self.explain_plan = Some(plan);
                    self.active_tab = PanelTab::Plan;
                    return;
                }
            }
        }

        self.explain_mode = false;

        // DML in browse mode (UPDATE/INSERT/DELETE) → schedule a page refresh.
        if self.browse.is_some() && result.rows_affected.is_some() {
            let n = result.rows_affected.unwrap();
            let ms = result.elapsed_ms;
            self.push_log(LogEntry::info(format!(
                "OK — {n} row{} affected  ({ms:.1} ms)",
                if n == 1 { "" } else { "s" }
            )));
            self.pending_refresh = true;
            return;
        }

        // DML outside browse mode.
        if let Some(n) = result.rows_affected {
            let ms = result.elapsed_ms;
            self.push_log(LogEntry::info(format!(
                "OK — {n} row{} affected  ({ms:.1} ms)",
                if n == 1 { "" } else { "s" }
            )));
            self.active_tab = PanelTab::Messages;
        }

        self.result = Some(result);
        if self.result.as_ref().map(|r| !r.columns.is_empty()).unwrap_or(false) {
            self.active_tab = PanelTab::Results;
        }
    }

    pub fn set_primary_key(&mut self, schema: &str, table: &str, cols: Vec<String>) {
        self.pk_cols.insert((schema.to_owned(), table.to_owned()), cols);
    }

    pub fn set_error(&mut self, msg: String) {
        self.push_log(LogEntry::error(msg));
        self.running = false;
        self.active_tab = PanelTab::Messages;
    }

    pub fn set_export_done(&mut self, path: String) {
        self.push_log(LogEntry::info(format!("Exported → {path}")));
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn last_query_duration(&self) -> Option<f64> {
        self.last_elapsed_ms
    }

    pub fn result_row_count(&self) -> Option<usize> {
        self.result.as_ref().map(|r| r.row_count())
    }

    /// Start browsing a table — called by app.rs when sidebar double-clicks.
    pub fn start_browse(&mut self, schema: String, table: String, db_tx: &Sender<DbCommand>) {
        // Request PK if not already cached.
        let key = (schema.clone(), table.clone());
        if !self.pk_cols.contains_key(&key) {
            let _ = db_tx.send(DbCommand::LoadPrimaryKey {
                schema: schema.clone(),
                table: table.clone(),
            });
        }
        self.edit_state = None;
        self.browse = Some(BrowseState::new(schema, table));
        self.browse_result = true;
        self.run_browse_page(db_tx);
    }

    fn run_browse_page(&mut self, db_tx: &Sender<DbCommand>) {
        if let Some(state) = &self.browse {
            let sql = state.build_sql();
            self.set_running();
            let _ = db_tx.send(DbCommand::Execute(sql));
        }
    }

    pub fn trigger_export_csv(&mut self, db_tx: &Sender<DbCommand>) {
        let sql = self.export_sql();
        if let Some(path) = pick_save_path("csv") {
            let _ = db_tx.send(DbCommand::ExportCsv { sql, path });
        }
    }

    pub fn trigger_export_json(&mut self, db_tx: &Sender<DbCommand>) {
        let sql = self.export_sql();
        if let Some(path) = pick_save_path("json") {
            let _ = db_tx.send(DbCommand::ExportJson { sql, path });
        }
    }

    /// SQL to use for export: if in browse mode, export the current page query.
    fn export_sql(&self) -> String {
        if let Some(state) = &self.browse {
            state.build_sql()
        } else {
            self.sql.clone()
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        db_tx: &Sender<DbCommand>,
        history: &mut QueryHistory,
    ) {
        // Auto-refresh after a DML (UPDATE/INSERT/DELETE) in browse mode.
        if self.pending_refresh {
            self.pending_refresh = false;
            self.run_browse_page(db_tx);
        }

        let total_height = ui.available_height();
        let editor_height = (total_height * self.split_ratio).max(80.0);
        // browse banner height ~28px, pagination bar ~28px, tabs ~24px, separator ~4px
        let chrome_height = 28.0 + 24.0 + 4.0
            + if self.browse.is_some() { 28.0 } else { 0.0 };
        let results_height = (total_height - editor_height - chrome_height).max(60.0);

        // ── SQL editor + toolbar ─────────────────────────────────────────────
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(4.0, 2.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let run_label = if self.running { "⏳ Running…" } else { "▶ Run" };
                    if ui
                        .add_enabled(!self.running, egui::Button::new(run_label))
                        .clicked()
                        && !self.sql.trim().is_empty()
                    {
                        // Free-form run exits browse mode
                        self.browse = None;
                        self.browse_result = false;
                        history.push(self.sql.clone());
                        let _ = history.save();
                        self.set_running();
                        let _ = db_tx.send(DbCommand::Execute(self.sql.clone()));
                    }

                    if ui
                        .add_enabled(self.running, egui::Button::new("✕ Cancel"))
                        .clicked()
                    {
                        let _ = db_tx.send(DbCommand::CancelQuery);
                    }

                    if ui
                        .add_enabled(
                            !self.running && !self.sql.trim().is_empty(),
                            egui::Button::new("⚡ Explain"),
                        )
                        .on_hover_text("Run EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)")
                        .clicked()
                    {
                        self.browse = None;
                        self.explain_mode = true;
                        let explain_sql = format!(
                            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)\n{}",
                            self.sql
                        );
                        history.push(self.sql.clone());
                        let _ = history.save();
                        self.set_running();
                        let _ = db_tx.send(DbCommand::Execute(explain_sql));
                    }

                    ui.separator();

                    if ui.button("⬆ Hist").clicked() {
                        if let Some(entry) = history.prev() {
                            self.sql = entry.to_owned();
                            self.browse = None;
                        }
                    }
                    if ui.button("⬇").clicked() {
                        match history.next() {
                            Some(entry) => {
                                self.sql = entry.to_owned();
                                self.browse = None;
                            }
                            None => self.sql.clear(),
                        }
                    }

                    ui.separator();
                    if ui.button("Format")
                        .on_hover_text("Format SQL (Shift+Alt+F)")
                        .clicked()
                        || ui.input(|i| {
                            i.modifiers.shift
                                && i.modifiers.alt
                                && i.key_pressed(egui::Key::F)
                        })
                    {
                        self.sql = sqlformat::format(
                            &self.sql,
                            &sqlformat::QueryParams::default(),
                            sqlformat::FormatOptions {
                                indent: sqlformat::Indent::Spaces(2),
                                uppercase: true,
                                lines_between_queries: 1,
                            },
                        );
                    }

                    ui.separator();
                    if ui.button("CSV").clicked() {
                        self.trigger_export_csv(db_tx);
                    }
                    if ui.button("JSON").clicked() {
                        self.trigger_export_json(db_tx);
                    }

                });

                let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                    let job = highlight_sql(ui, text, wrap_width);
                    ui.fonts(|f| f.layout_job(job))
                };
                let editor = egui::TextEdit::multiline(&mut self.sql)
                    .layouter(&mut layouter)
                    .desired_rows(6)
                    .desired_width(f32::INFINITY)
                    .hint_text("Enter SQL… (F5 or Ctrl+Enter to execute)");
                let resp = ui.add_sized([ui.available_width(), editor_height], editor);

                if resp.has_focus()
                    && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter))
                    && !self.sql.trim().is_empty()
                {
                    self.browse = None;
                    self.browse_result = false;
                    history.push(self.sql.clone());
                    let _ = history.save();
                    self.set_running();
                    let _ = db_tx.send(DbCommand::Execute(self.sql.clone()));
                }
            });

        ui.separator();

        // ── Browse-mode banner ───────────────────────────────────────────────
        if let Some(state) = self.browse.clone() {
            egui::Frame::none()
                .fill(ui.visuals().faint_bg_color)
                .inner_margin(egui::Margin::symmetric(6.0, 3.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Browse: {}", state.label()))
                                .strong()
                                .color(egui::Color32::from_rgb(100, 180, 255)),
                        );
                        if let Some(col) = &state.sort_col {
                            ui.label(
                                egui::RichText::new(format!(
                                    "  sorted by {} {}",
                                    col,
                                    if state.sort_asc { "▲" } else { "▼" }
                                ))
                                .small(),
                            );
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✕ Exit browse").clicked() {
                                self.browse = None;
                                self.browse_result = false;
                            }
                        });
                    });
                });
        }

        // ── Result tabs ──────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_tab, PanelTab::Results, "Results");
            if self.explain_plan.is_some() {
                ui.selectable_value(
                    &mut self.active_tab,
                    PanelTab::Plan,
                    egui::RichText::new("⚡ Plan").color(egui::Color32::from_rgb(100, 200, 255)),
                );
            }
            let error_count = self.log.iter().filter(|e| e.kind == LogKind::Error).count();
            let msg_label = if error_count > 0 {
                egui::RichText::new(format!("Messages ({})", error_count))
                    .color(egui::Color32::from_rgb(220, 80, 80))
            } else {
                egui::RichText::new("Messages")
            };
            ui.selectable_value(&mut self.active_tab, PanelTab::Messages, msg_label);
            ui.selectable_value(&mut self.active_tab, PanelTab::History, "History");
        });

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .max_height(results_height)
            .show(ui, |ui| match self.active_tab {
                PanelTab::Results => {
                    if self.result.is_some() {
                        // ── Build and show table ─────────────────────────────
                        let output = {
                            let result = self.result.as_ref().unwrap();
                            let mut table = ResultTable::new(result);
                            table.db_sort_mode = self.browse.is_some();

                            // Restore sort indicator
                            if let Some(state) = &self.browse {
                                if let Some(col_name) = &state.sort_col {
                                    if let Some(idx) =
                                        result.columns.iter().position(|c| c == col_name)
                                    {
                                        table.sort_col = Some(idx);
                                        table.sort_asc = state.sort_asc;
                                    }
                                }
                            }

                            // Restore edit state
                            if let Some((r, c, ref v)) = self.edit_state {
                                table.edit_row = Some(r);
                                table.edit_col = Some(c);
                                table.edit_value = v.clone();
                                table.edit_needs_focus = self.edit_needs_focus;
                            }

                            let out = table.show(ui);

                            // Save back edit state (value may have changed)
                            if let (Some(r), Some(c)) = (table.edit_row, table.edit_col) {
                                self.edit_state = Some((r, c, table.edit_value.clone()));
                                self.edit_needs_focus = false;
                            }

                            (out, table.sorted_indices.clone())
                        }; // borrow of self.result released here

                        let (output, sorted_indices) = output;

                        // ── Handle sort ──────────────────────────────────────
                        if let (Some((col_name, asc)), Some(state)) =
                            (output.sort_changed, &mut self.browse)
                        {
                            state.sort_col = Some(col_name);
                            state.sort_asc = asc;
                            state.page = 0;
                            self.edit_state = None;
                            let sql = state.build_sql();
                            self.set_running();
                            let _ = db_tx.send(DbCommand::Execute(sql));
                        }

                        // ── Handle cell double-click → open value popup ──────
                        if let Some((row, col)) = output.cell_double_clicked {
                            if let Some(result) = &self.result {
                                if col < result.columns.len() {
                                    let col_name =
                                        result.columns[col].clone();
                                    let value = result
                                        .rows
                                        .get(sorted_indices[row])
                                        .and_then(|r| r.get(col))
                                        .map(|c| c.to_string())
                                        .unwrap_or_default();
                                    self.cell_popup = Some(CellPopup {
                                        col_name,
                                        value,
                                        display_row: row,
                                        col_idx: col,
                                    });
                                }
                            }
                        }

                        // ── Handle edit committed ─────────────────────────────
                        if let Some((disp_row, col_idx, new_val)) = output.edit_committed {
                            self.edit_state = None;
                            self.edit_needs_focus = false;
                            self.commit_cell_edit(
                                disp_row,
                                col_idx,
                                new_val,
                                &sorted_indices,
                                db_tx,
                            );
                        }

                        if output.edit_cancelled {
                            self.edit_state = None;
                            self.edit_needs_focus = false;
                        }
                    } else if self.running {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Running…");
                        });
                    } else {
                        ui.label("No results yet. Run a query with F5 or Ctrl+Enter.");
                    }
                }
                PanelTab::Plan => {
                    if let Some(plan) = &self.explain_plan {
                        render_explain(ui, plan);
                    } else if self.running {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Running EXPLAIN…");
                        });
                    }
                }
                PanelTab::Messages => {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{} events", self.log.len()))
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui.small_button("Clear").clicked() {
                                    self.log.clear();
                                }
                            },
                        );
                    });
                    ui.separator();

                    if self.log.is_empty() {
                        ui.add_space(12.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("No messages yet.")
                                    .color(egui::Color32::GRAY)
                                    .italics(),
                            );
                        });
                    } else {
                        // Newest first
                        for entry in self.log.iter().rev() {
                            let (icon, color) = match entry.kind {
                                LogKind::Error   => ("✕", egui::Color32::from_rgb(220, 80,  80)),
                                LogKind::Warning => ("⚠", egui::Color32::from_rgb(220, 170, 60)),
                                LogKind::Info    => ("✓", egui::Color32::from_rgb(80,  200, 120)),
                            };
                            let time_str = entry.time.format("%H:%M:%S").to_string();

                            egui::Frame::none()
                                .fill(match entry.kind {
                                    LogKind::Error   => egui::Color32::from_rgba_premultiplied(80, 20, 20, 60),
                                    LogKind::Warning => egui::Color32::from_rgba_premultiplied(80, 60, 10, 40),
                                    LogKind::Info    => egui::Color32::TRANSPARENT,
                                })
                                .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                                .show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.colored_label(color, icon);
                                        ui.label(
                                            egui::RichText::new(&time_str)
                                                .small()
                                                .monospace()
                                                .color(egui::Color32::GRAY),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&entry.text)
                                                    .color(color)
                                                    .monospace()
                                                    .small(),
                                            )
                                            .wrap(true),
                                        );
                                    });
                                });
                            ui.add_space(2.0);
                        }
                    }
                }
                PanelTab::History => {
                    ui.horizontal(|ui| {
                        ui.label("Search:");
                        ui.text_edit_singleline(&mut self.history_search);
                    });
                    ui.separator();
                    let search = self.history_search.to_lowercase();
                    let entries: Vec<String> = history
                        .all()
                        .iter()
                        .filter(|e| search.is_empty() || e.to_lowercase().contains(&search))
                        .cloned()
                        .rev()
                        .collect();

                    for entry in &entries {
                        let preview: String =
                            entry.lines().next().unwrap_or("").chars().take(80).collect();
                        let resp = ui.add(
                            egui::Label::new(egui::RichText::new(preview).monospace())
                                .sense(egui::Sense::click()),
                        );
                        if resp.double_clicked() {
                            self.sql = entry.clone();
                            self.browse = None;
                        }
                        resp.on_hover_text(entry.as_str());
                    }
                }
            });

        // ── Pagination bar (only in browse mode) ────────────────────────────
        if self.browse.is_some() {
            ui.separator();
            ui.horizontal(|ui| {
                let page = self.browse.as_ref().map(|s| s.page).unwrap_or(0);
                let row_count = self.result.as_ref().map(|r| r.row_count()).unwrap_or(0);

                let can_prev = page > 0;
                let can_next = row_count == PAGE_SIZE; // if we got a full page, there may be more

                if ui.add_enabled(can_prev, egui::Button::new("← Prev")).clicked() {
                    if let Some(state) = &mut self.browse {
                        state.page -= 1;
                        let sql = state.build_sql();
                        self.set_running();
                        let _ = db_tx.send(DbCommand::Execute(sql));
                    }
                }

                ui.label(format!(" Page {} ", page + 1));

                if ui.add_enabled(can_next, egui::Button::new("Next →")).clicked() {
                    if let Some(state) = &mut self.browse {
                        state.page += 1;
                        let sql = state.build_sql();
                        self.set_running();
                        let _ = db_tx.send(DbCommand::Execute(sql));
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{} rows/page", PAGE_SIZE))
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                });
            });
        }

        // Floating popup — rendered last so it draws on top of everything.
        let ctx = ui.ctx().clone();
        self.show_cell_popup(&ctx);
    }

    // ── Cell value popup ─────────────────────────────────────────────────────

    fn show_cell_popup(&mut self, ctx: &egui::Context) {
        let Some(popup) = self.cell_popup.take() else { return };

        let mut open = true;
        let mut start_edit = false;
        let mut close_clicked = false;
        let is_browse = self.browse.is_some();

        egui::Window::new(format!(" {} ", &popup.col_name))
            .collapsible(false)
            .resizable(true)
            .default_size([420.0, 220.0])
            .min_size([260.0, 80.0])
            .open(&mut open)
            .show(ctx, |ui| {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .max_height(260.0)
                    .show(ui, |ui| {
                        let mut text = popup.value.clone();
                        ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Monospace)
                                .interactive(false),
                        );
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Copy").clicked() {
                        ctx.copy_text(popup.value.clone());
                    }
                    if is_browse && ui.button("Edit").clicked() {
                        start_edit = true;
                    }
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui.button("Close").clicked() {
                                close_clicked = true;
                            }
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} chars",
                                    popup.value.chars().count()
                                ))
                                .small()
                                .color(egui::Color32::GRAY),
                            );
                        },
                    );
                });
            });

        if start_edit {
            self.edit_state =
                Some((popup.display_row, popup.col_idx, popup.value));
            self.edit_needs_focus = true;
        } else if open && !close_clicked {
            self.cell_popup = Some(popup);
        }
    }

    // ── Inline edit helpers ───────────────────────────────────────────────────

    /// Build and execute an UPDATE query for the edited cell.
    fn commit_cell_edit(
        &mut self,
        disp_row: usize,
        col_idx: usize,
        new_val: String,
        sorted_indices: &[usize],
        db_tx: &Sender<DbCommand>,
    ) {
        // Need browse context.
        let (schema, table) = match &self.browse {
            Some(b) => (b.schema.clone(), b.table.clone()),
            None => return,
        };

        // Check we have PK info.
        let pk_cols = self
            .pk_cols
            .get(&(schema.clone(), table.clone()))
            .cloned()
            .unwrap_or_default();

        if pk_cols.is_empty() {
            self.push_log(LogEntry::warning(format!(
                "Cannot edit: no primary key found on \"{schema}\".\"{table}\""
            )));
            self.active_tab = PanelTab::Messages;
            return;
        }

        // Extract column names and the actual row data.
        let actual_idx = sorted_indices[disp_row];
        let (col_names, row_data) = match &self.result {
            Some(r) => (
                r.columns.clone(),
                r.rows.get(actual_idx).cloned().unwrap_or_default(),
            ),
            None => return,
        };

        // Build WHERE from PK columns.
        let where_parts: Vec<String> = pk_cols
            .iter()
            .filter_map(|pk| {
                let idx = col_names.iter().position(|c| c == pk)?;
                let val = row_data.get(idx)?.to_string();
                Some(format!("\"{}\" = '{}'", pk, val.replace('\'', "''")))
            })
            .collect();

        if where_parts.len() != pk_cols.len() {
            self.push_log(LogEntry::warning(
                "Cannot edit: PK columns missing from result set",
            ));
            self.active_tab = PanelTab::Messages;
            return;
        }

        let col_name = col_names
            .get(col_idx)
            .cloned()
            .unwrap_or_default();

        let set_expr = if new_val.eq_ignore_ascii_case("null") {
            format!("\"{}\" = NULL", col_name)
        } else {
            format!("\"{}\" = '{}'", col_name, new_val.replace('\'', "''"))
        };

        let sql = format!(
            "UPDATE \"{schema}\".\"{table}\" SET {set_expr} WHERE {};",
            where_parts.join(" AND ")
        );

        self.set_running();
        let _ = db_tx.send(DbCommand::Execute(sql));
    }
}

/// Native save-file dialog via `rfd`. Falls back to home dir if dialog is cancelled.
fn pick_save_path(ext: &str) -> Option<String> {
    let filter_name = match ext {
        "csv" => "CSV files",
        "json" => "JSON files",
        _ => "All files",
    };
    let default_name = format!(
        "pgclient_export_{}.{}",
        chrono::Local::now().format("%Y%m%d_%H%M%S"),
        ext
    );

    rfd::FileDialog::new()
        .add_filter(filter_name, &[ext])
        .set_file_name(&default_name)
        .save_file()
        .map(|p| p.to_string_lossy().into_owned())
}
