use std::collections::HashMap;
use std::sync::mpsc::Sender;

use crate::db::{query::CellValue, DbCommand, QueryResult};
use crate::history::QueryHistory;
use crate::i18n::{I18n, Lang};
use crate::ui::autocomplete::Autocomplete;
use crate::ui::explain::{render_explain, ExplainResult};
use crate::ui::result_table::ResultTable;
use crate::ui::syntax::highlight_sql;

const PAGE_SIZE: usize = 100;
const MAX_LOG: usize = 200;

// ── Column width helper ───────────────────────────────────────────────────────

/// Sample up to 200 rows to compute content-aware initial column widths.
/// Called once per result set; never runs per-frame.
fn compute_col_widths(result: &QueryResult) -> Vec<f32> {
    const SAMPLE: usize = 200;
    const CHAR_PX: f32 = 7.5;
    const PAD: f32 = 20.0;

    let mut max_chars: Vec<usize> = result.columns.iter().map(|c| c.len()).collect();
    for row in result.rows.iter().take(SAMPLE) {
        for (i, cell) in row.iter().enumerate() {
            if i < max_chars.len() {
                let len = cell.to_string().len().min(50);
                if len > max_chars[i] {
                    max_chars[i] = len;
                }
            }
        }
    }
    max_chars
        .iter()
        .map(|&n| (n as f32 * CHAR_PX + PAD).max(60.0).min(300.0))
        .collect()
}

// ── SQL statement splitter ────────────────────────────────────────────────────

/// Split SQL at `;` boundaries, correctly skipping `;` inside single-quoted
/// strings and `--` line comments. Returns non-empty, trimmed statements.
fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut stmts = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_line_comment = false;
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_line_comment {
            current.push(ch);
            if ch == '\n' { in_line_comment = false; }
            continue;
        }
        if in_single_quote {
            current.push(ch);
            if ch == '\'' {
                if chars.peek() == Some(&'\'') {
                    current.push(chars.next().unwrap()); // escaped ''
                } else {
                    in_single_quote = false;
                }
            }
            continue;
        }
        match ch {
            '\'' => { in_single_quote = true; current.push(ch); }
            '-' if chars.peek() == Some(&'-') => {
                in_line_comment = true;
                current.push(ch);
                current.push(chars.next().unwrap());
            }
            ';' => {
                let t = current.trim().to_owned();
                if !t.is_empty() { stmts.push(t); }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let t = current.trim().to_owned();
    if !t.is_empty() { stmts.push(t); }
    stmts
}

// ── UTF-8 index helpers ────────────────────────────────────────────────────────

/// Convert a char index (egui `CCursor::index`) to a byte offset in `text`.
fn char_idx_to_byte(text: &str, char_idx: usize) -> usize {
    text.char_indices().nth(char_idx).map(|(i, _)| i).unwrap_or(text.len())
}

/// Convert a byte offset to a char index (suitable for `CCursor::new`).
fn byte_to_char_idx(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset.min(text.len())].chars().count()
}

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
    /// Actual row index in QueryResult (after sort mapping).
    actual_row: usize,
}

// ── Column statistics ─────────────────────────────────────────────────────────

struct ColumnStats {
    col_name: String,
    total: usize,
    null_count: usize,
    distinct: usize,
    min_len: Option<usize>,
    max_len: Option<usize>,
    top_values: Vec<(String, usize)>,
}

impl ColumnStats {
    fn compute(result: &crate::db::query::QueryResult, col_idx: usize) -> Self {
        use std::collections::HashMap;
        let col_name = result.columns[col_idx].clone();
        let total = result.rows.len();
        let mut null_count = 0usize;
        let mut freq: HashMap<String, usize> = HashMap::new();
        let mut min_len = usize::MAX;
        let mut max_len = 0usize;

        for row in &result.rows {
            let cell = &row[col_idx];
            if matches!(cell, CellValue::Null) {
                null_count += 1;
            } else {
                let s = cell.to_string();
                let len = s.chars().count();
                if len < min_len { min_len = len; }
                if len > max_len { max_len = len; }
                *freq.entry(s).or_insert(0) += 1;
            }
        }

        let distinct = freq.len();
        let mut top_values: Vec<(String, usize)> = freq.into_iter().collect();
        top_values.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        top_values.truncate(10);

        ColumnStats {
            col_name,
            total,
            null_count,
            distinct,
            min_len: if min_len == usize::MAX { None } else { Some(min_len) },
            max_len: if null_count == total { None } else { Some(max_len) },
            top_values,
        }
    }
}

// ── Tabs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Default, PartialEq, Clone)]
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
    /// Client-side filter text for the result table.
    result_filter: String,
    /// Currently selected cell (display_row, col_idx) for Ctrl+C.
    selected_cell: Option<(usize, usize)>,
    /// Cached sort order for the current result — reused across frames.
    sorted_indices: Vec<usize>,
    /// Filtered view of sorted_indices — only recomputed when filter/sort/result changes.
    display_indices: Vec<usize>,
    /// Filter text when display_indices was last computed (used for dirty detection).
    display_filter_cache: String,
    /// When true, display_indices must be recomputed before next render.
    display_dirty: bool,
    /// Content-aware initial column widths computed once per result set.
    col_widths: Vec<f32>,
    // ── Autocomplete ─────────────────────────────────────────────────────────
    autocomplete: Autocomplete,
    completion_tables: Vec<String>,
    completion_columns: Vec<String>,
    /// Current UI language — used for log messages generated outside show().
    pub lang: Lang,
    /// Column statistics popup state.
    col_stats: Option<ColumnStats>,
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
            result_filter: String::new(),
            selected_cell: None,
            sorted_indices: Vec::new(),
            display_indices: Vec::new(),
            display_filter_cache: String::new(),
            display_dirty: false,
            col_widths: Vec::new(),
            autocomplete: Autocomplete::default(),
            completion_tables: Vec::new(),
            completion_columns: Vec::new(),
            lang: Lang::En,
            col_stats: None,
        }
    }
}

impl QueryPanel {
    pub fn set_completion_data(&mut self, tables: Vec<String>, columns: Vec<String>) {
        self.completion_tables = tables;
        self.completion_columns = columns;
    }

    pub fn current_sql(&self) -> &str {
        &self.sql
    }

    /// True if this panel has no SQL, no result, and is not browsing a table.
    pub fn is_empty(&self) -> bool {
        self.sql.trim().is_empty() && self.result.is_none() && self.browse.is_none()
    }

    /// The (schema, table) this panel is currently browsing, if any.
    pub fn browsing_table(&self) -> Option<(&str, &str)> {
        self.browse.as_ref().map(|b| (b.schema.as_str(), b.table.as_str()))
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
            let i18n = I18n::new(self.lang);
            self.push_log(LogEntry::info(i18n.log_ok_rows(n as i64, ms)));
            self.pending_refresh = true;
            return;
        }

        // DML outside browse mode.
        if let Some(n) = result.rows_affected {
            let ms = result.elapsed_ms;
            let i18n = I18n::new(self.lang);
            self.push_log(LogEntry::info(i18n.log_ok_rows(n as i64, ms)));
            self.active_tab = PanelTab::Messages;
        }

        let n = result.rows.len();
        self.col_widths = compute_col_widths(&result);
        self.result = Some(result);
        self.sorted_indices = (0..n).collect();
        self.display_indices = (0..n).collect();
        self.display_filter_cache = String::new();
        self.display_dirty = false;
        self.selected_cell = None;
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
        let i18n = I18n::new(self.lang);
        self.push_log(LogEntry::info(i18n.log_exported(&path)));
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

    /// Send current SQL: multiple statements → ExecuteMulti, single → Execute.
    fn send_execute(&self, db_tx: &Sender<DbCommand>) {
        let stmts = split_sql_statements(&self.sql);
        if stmts.len() > 1 {
            let _ = db_tx.send(DbCommand::ExecuteMulti(stmts));
        } else {
            let _ = db_tx.send(DbCommand::Execute(self.sql.clone()));
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

    /// Open a SQL file via native dialog and execute it directly (no editor load).
    pub fn run_sql_file(&mut self, db_tx: &Sender<DbCommand>) {
        let Some(path) = pick_open_sql_file() else { return };
        let i18n = I18n::new(self.lang);
        match std::fs::read_to_string(&path) {
            Ok(sql) => {
                if sql.trim().is_empty() {
                    self.push_log(LogEntry::warning(i18n.log_file_empty(&path)));
                    self.active_tab = PanelTab::Messages;
                    return;
                }
                let filename = std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.clone());
                self.push_log(LogEntry::info(i18n.log_running_file(&filename)));
                self.browse = None;
                self.browse_result = false;
                self.set_running();
                let _ = db_tx.send(DbCommand::Execute(sql));
            }
            Err(e) => {
                self.push_log(LogEntry::error(i18n.log_file_error(&e)));
                self.active_tab = PanelTab::Messages;
            }
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
        i18n: &I18n,
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
                    // ── Palette ──────────────────────────────────────────────
                    let col_green = egui::Color32::from_rgb(73, 156, 84);   // #499c54
                    let col_red   = egui::Color32::from_rgb(199, 84, 80);   // #c75450
                    let col_dim   = egui::Color32::from_rgb(76, 80, 82);    // #4c5052

                    // ── Group 1: Execute ──────────────────────────────────────
                    let run_label = if self.running { i18n.btn_running() } else { i18n.btn_run() };
                    let run_fill  = if self.running { col_dim } else { col_green };
                    if ui
                        .add_enabled(!self.running, egui::Button::new(run_label).fill(run_fill))
                        .clicked()
                        && !self.sql.trim().is_empty()
                    {
                        self.browse = None;
                        self.browse_result = false;
                        history.push(self.sql.clone());
                        let _ = history.save();
                        self.set_running();
                        self.send_execute(db_tx);
                    }

                    let cancel_fill = if self.running { col_red } else { col_dim };
                    if ui
                        .add_enabled(self.running, egui::Button::new(i18n.btn_cancel_query()).fill(cancel_fill))
                        .clicked()
                    {
                        let _ = db_tx.send(DbCommand::CancelQuery);
                    }

                    if ui
                        .add_enabled(
                            !self.running && !self.sql.trim().is_empty(),
                            egui::Button::new(i18n.btn_explain()),
                        )
                        .on_hover_text(i18n.hover_explain())
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

                    // ── Group 2: History ──────────────────────────────────────
                    if ui.button(i18n.btn_hist_prev()).on_hover_text(i18n.hover_hist_prev()).clicked() {
                        if let Some(entry) = history.prev() {
                            self.sql = entry.to_owned();
                            self.browse = None;
                        }
                    }
                    if ui.button("⬇").on_hover_text(i18n.hover_hist_next()).clicked() {
                        match history.next() {
                            Some(entry) => {
                                self.sql = entry.to_owned();
                                self.browse = None;
                            }
                            None => self.sql.clear(),
                        }
                    }

                    ui.separator();

                    // ── Group 3: Format ───────────────────────────────────────
                    if ui.button(i18n.btn_format())
                        .on_hover_text(i18n.hover_format())
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

                    // ── Group 4: Export ───────────────────────────────────────
                    if ui.button("CSV").on_hover_text(i18n.hover_export_csv()).clicked() {
                        self.trigger_export_csv(db_tx);
                    }
                    if ui.button("JSON").on_hover_text(i18n.hover_export_json()).clicked() {
                        self.trigger_export_json(db_tx);
                    }

                    ui.separator();

                    // ── Group 5: Run File ─────────────────────────────────────
                    if ui
                        .add_enabled(!self.running, egui::Button::new(i18n.btn_run_file()))
                        .on_hover_text(i18n.hover_run_file())
                        .clicked()
                    {
                        self.run_sql_file(db_tx);
                    }

                });

                // Ctrl+Space: force-show autocomplete
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Space)) {
                    self.autocomplete.force_show();
                }

                // Read cursor from the PREVIOUS frame before the TextEdit re-renders.
                // We need this for Tab acceptance (Tab must be consumed before the
                // TextEdit sees it, otherwise egui cycles focus to the next widget).
                let prev_cursor_idx: usize = egui::TextEdit::load_state(
                    ui.ctx(),
                    egui::Id::new("ferox_sql_editor"),
                )
                .and_then(|s| s.cursor.char_range())
                .map(|r| r.primary.index)
                .unwrap_or(self.sql.len());

                // Consume Enter (no modifiers) BEFORE the TextEdit if autocomplete is
                // visible. Tab cannot be intercepted reliably because egui cycles
                // focus at the context level before any widget code runs.
                let enter_accepted = self.autocomplete.is_visible()
                    && ui.input_mut(|i| {
                        // Only plain Enter — Ctrl+Enter still runs the query.
                        !i.modifiers.any()
                            && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    });

                let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                    let job = highlight_sql(ui, text, wrap_width);
                    ui.fonts(|f| f.layout_job(job))
                };
                let editor = egui::TextEdit::multiline(&mut self.sql)
                    .id(egui::Id::new("ferox_sql_editor"))
                    .layouter(&mut layouter)
                    .desired_rows(6)
                    .desired_width(f32::INFINITY)
                    .hint_text(i18n.hint_sql_editor());
                // Wrap in a ScrollArea so the layout height is strictly capped at
                // editor_height. Without this, TextEdit grows its layout allocation
                // as content grows, pushing the result tab bar off-screen.
                let scroll_out = egui::ScrollArea::vertical()
                    .id_source("sql_editor_scroll")
                    .max_height(editor_height)
                    .min_scrolled_height(editor_height)
                    .show(ui, |ui| ui.add(editor));
                let resp = scroll_out.inner;

                // Handle Enter acceptance (consumed before the TextEdit).
                if enter_accepted {
                    if let Some(accepted) = self.autocomplete.accept() {
                        // word_start is a byte offset; convert prev_cursor_idx (char) to byte.
                        let word_start = self.autocomplete.word_start;
                        let prev_byte = char_idx_to_byte(&self.sql, prev_cursor_idx);
                        self.sql.replace_range(word_start..prev_byte, &accepted);
                        let new_char = byte_to_char_idx(&self.sql, word_start + accepted.len());
                        if let Some(mut state) = egui::TextEdit::load_state(
                            ui.ctx(),
                            egui::Id::new("ferox_sql_editor"),
                        ) {
                            let ccursor = egui::text::CCursor::new(new_char);
                            state.cursor.set_char_range(Some(
                                egui::text::CCursorRange::one(ccursor),
                            ));
                            state.store(ui.ctx(), egui::Id::new("ferox_sql_editor"));
                        }
                    }
                    resp.request_focus();
                }

                // Get cursor position from this frame's TextEdit state.
                let cursor_idx: usize = egui::TextEdit::load_state(
                    ui.ctx(),
                    egui::Id::new("ferox_sql_editor"),
                )
                .and_then(|s| s.cursor.char_range())
                .map(|r| r.primary.index)
                .unwrap_or(self.sql.len());

                // Update autocomplete suggestions.
                let completion_tables = self.completion_tables.clone();
                let completion_columns = self.completion_columns.clone();
                self.autocomplete.update(
                    &self.sql,
                    cursor_idx,
                    &completion_tables,
                    &completion_columns,
                );
                if resp.changed() && !self.autocomplete.suggestions.is_empty() {
                    self.autocomplete.visible = true;
                }

                // Dismiss autocomplete when the editor loses focus so the popup
                // Area (Order::Foreground) doesn't block clicks on the result table.
                if !resp.has_focus() {
                    self.autocomplete.dismiss();
                }

                // Remaining keyboard navigation (only when editor has focus).
                if self.autocomplete.is_visible() && resp.has_focus() {
                    if ui.input_mut(|i| {
                        i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    }) {
                        self.autocomplete.dismiss();
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                        self.autocomplete.select_next();
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                        self.autocomplete.select_prev();
                    }
                }

                // Show autocomplete popup; handle mouse-click acceptance.
                let editor_rect = scroll_out.inner_rect;
                if let Some(accepted) = self.autocomplete.show(ui, editor_rect) {
                    // word_start is a byte offset; convert cursor_idx (char) to byte.
                    let word_start = self.autocomplete.word_start;
                    let cursor_byte = char_idx_to_byte(&self.sql, cursor_idx);
                    self.sql.replace_range(word_start..cursor_byte, &accepted);
                    let new_char = byte_to_char_idx(&self.sql, word_start + accepted.len());
                    if let Some(mut state) = egui::TextEdit::load_state(
                        ui.ctx(),
                        egui::Id::new("ferox_sql_editor"),
                    ) {
                        let ccursor = egui::text::CCursor::new(new_char);
                        state.cursor.set_char_range(Some(
                            egui::text::CCursorRange::one(ccursor),
                        ));
                        state.store(ui.ctx(), egui::Id::new("ferox_sql_editor"));
                    }
                    resp.request_focus();
                }

                if resp.has_focus()
                    && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter))
                    && !self.sql.trim().is_empty()
                {
                    self.browse = None;
                    self.browse_result = false;
                    history.push(self.sql.clone());
                    let _ = history.save();
                    self.set_running();
                    self.send_execute(db_tx);
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
                            egui::RichText::new(format!("{} {}", i18n.browse_prefix(), state.label()))
                                .strong()
                                .color(egui::Color32::from_rgb(100, 180, 255)),
                        );
                        if let Some(col) = &state.sort_col {
                            ui.label(
                                egui::RichText::new(format!(
                                    "  {} {} {}",
                                    i18n.browse_sorted_by(),
                                    col,
                                    if state.sort_asc { "▲" } else { "▼" }
                                ))
                                .small(),
                            );
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(i18n.btn_exit_browse()).clicked() {
                                self.browse = None;
                                self.browse_result = false;
                            }
                        });
                    });
                });
        }

        // ── Result tabs ──────────────────────────────────────────────────────
        let tab_bg    = egui::Color32::from_rgb(49, 51, 53);   // #313335
        let col_blue  = egui::Color32::from_rgb(78, 159, 222); // #4e9fde
        let text_active = egui::Color32::from_rgb(169, 183, 198); // #a9b7c6
        let text_dim    = egui::Color32::from_rgb(110, 123, 139); // #6e7b8b

        egui::Frame::none()
            .fill(tab_bg)
            .inner_margin(egui::Margin::symmetric(4.0, 2.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let results_label = i18n.tab_results();
                    let history_label = i18n.tab_history();
                    let tabs: &[(PanelTab, &str, Option<egui::Color32>)] = &[
                        (PanelTab::Results, results_label, None),
                        (PanelTab::History, history_label, None),
                    ];

                    for (tab, label, _color) in tabs {
                        let is_active = self.active_tab == *tab;
                        let text = egui::RichText::new(*label)
                            .color(if is_active { text_active } else { text_dim });
                        let btn = egui::Button::new(text)
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE);
                        let resp = ui.add(btn);
                        if is_active {
                            let r = resp.rect;
                            ui.painter().line_segment(
                                [egui::pos2(r.min.x, r.max.y), egui::pos2(r.max.x, r.max.y)],
                                egui::Stroke::new(2.0, col_blue),
                            );
                        }
                        if resp.clicked() {
                            self.active_tab = tab.clone();
                        }
                    }

                    // Explain tab — only when plan exists
                    if self.explain_plan.is_some() {
                        let is_active = self.active_tab == PanelTab::Plan;
                        let text = egui::RichText::new(i18n.tab_plan())
                            .color(if is_active { egui::Color32::from_rgb(100, 200, 255) } else { text_dim });
                        let btn = egui::Button::new(text)
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE);
                        let resp = ui.add(btn);
                        if is_active {
                            let r = resp.rect;
                            ui.painter().line_segment(
                                [egui::pos2(r.min.x, r.max.y), egui::pos2(r.max.x, r.max.y)],
                                egui::Stroke::new(2.0, col_blue),
                            );
                        }
                        if resp.clicked() { self.active_tab = PanelTab::Plan; }
                    }

                    // Messages tab — red badge if errors
                    {
                        let error_count = self.log.iter().filter(|e| e.kind == LogKind::Error).count();
                        let is_active = self.active_tab == PanelTab::Messages;
                        let label_str = if error_count > 0 {
                            i18n.tab_messages_n(error_count)
                        } else {
                            i18n.tab_messages().to_owned()
                        };
                        let msg_color = if error_count > 0 {
                            egui::Color32::from_rgb(220, 80, 80)
                        } else if is_active { text_active } else { text_dim };
                        let text = egui::RichText::new(label_str).color(msg_color);
                        let btn = egui::Button::new(text)
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE);
                        let resp = ui.add(btn);
                        if is_active {
                            let r = resp.rect;
                            ui.painter().line_segment(
                                [egui::pos2(r.min.x, r.max.y), egui::pos2(r.max.x, r.max.y)],
                                egui::Stroke::new(2.0, col_blue),
                            );
                        }
                        if resp.clicked() { self.active_tab = PanelTab::Messages; }
                    }
                });
            });

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .max_height(results_height)
            .show(ui, |ui| match self.active_tab {
                PanelTab::Results => {
                    // ── Filter bar ───────────────────────────────────────────
                    if self.result.is_some() {
                        ui.horizontal(|ui| {
                            let hint = egui::RichText::new(i18n.filter_hint())
                                .color(egui::Color32::from_rgb(90, 95, 100));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.result_filter)
                                    .desired_width(220.0)
                                    .hint_text(hint),
                            );
                            if !self.result_filter.is_empty() && ui.small_button("✕").clicked() {
                                self.result_filter.clear();
                            }
                            // Ctrl+C: copy selected cell value (actual_row stored in selected_cell)
                            if let Some((actual_row, col_idx)) = self.selected_cell {
                                if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::C)) {
                                    if let Some(result) = &self.result {
                                        let val = result.rows
                                            .get(actual_row)
                                            .and_then(|r| r.get(col_idx))
                                            .map(|c| c.to_string())
                                            .unwrap_or_default();
                                        ui.output_mut(|o| o.copied_text = val);
                                    }
                                }
                            }
                        });
                    }

                    if self.result.is_some() {
                        // ── Invalidate display_indices when filter/sort/result changed ──
                        if self.display_dirty || self.result_filter != self.display_filter_cache {
                            if let Some(result) = &self.result {
                                if self.result_filter.is_empty() {
                                    self.display_indices = self.sorted_indices.clone();
                                } else {
                                    let f = self.result_filter.to_lowercase();
                                    self.display_indices = self.sorted_indices.iter().copied()
                                        .filter(|&i| {
                                            result.rows[i].iter().any(|cell| {
                                                cell.to_string().to_lowercase().contains(&f)
                                            })
                                        })
                                        .collect();
                                }
                                self.display_filter_cache = self.result_filter.clone();
                                self.display_dirty = false;
                            }
                        }

                        // ── Build and show table ─────────────────────────────
                        let output = {
                            let result = self.result.as_ref().unwrap();
                            let mut table = ResultTable::with_indices(
                                result,
                                std::mem::take(&mut self.sorted_indices),
                                self.col_widths.clone(),
                            );
                            table.db_sort_mode = self.browse.is_some();
                            if let Some(cell) = self.selected_cell {
                                table.selected_cell = Some(cell);
                            }

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

                            let out = table.show(ui, i18n, &self.display_indices);

                            // Save back edit state (value may have changed)
                            if let (Some(r), Some(c)) = (table.edit_row, table.edit_col) {
                                self.edit_state = Some((r, c, table.edit_value.clone()));
                                self.edit_needs_focus = false;
                            }

                            (out, table.sorted_indices)
                        }; // borrow of self.result released here

                        let (output, sorted_indices) = output;

                        // ── Handle sort ──────────────────────────────────────
                        let sort_did_change = output.sort_changed.is_some();
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

                        // ── Handle cell single-click → track selected cell ───
                        if let Some((row, col)) = output.cell_clicked {
                            self.selected_cell = Some((row, col));
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
                                        actual_row: sorted_indices[row],
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

                        // ── Handle column stats request ───────────────────────
                        if let Some(col_idx) = output.col_stats_requested {
                            if let Some(result) = &self.result {
                                if col_idx < result.columns.len() {
                                    self.col_stats = Some(ColumnStats::compute(result, col_idx));
                                }
                            }
                        }

                        // Save sorted indices back for next frame (avoids per-frame reallocation).
                        self.sorted_indices = sorted_indices;
                        // If sort changed, display_indices must be recomputed next frame.
                        if sort_did_change {
                            self.display_dirty = true;
                        }
                    } else if self.running {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(i18n.lbl_running());
                        });
                    } else {
                        ui.label(i18n.lbl_no_results_yet());
                    }
                }
                PanelTab::Plan => {
                    if let Some(plan) = &self.explain_plan {
                        render_explain(ui, plan, i18n);
                    } else if self.running {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(i18n.lbl_running_explain());
                        });
                    }
                }
                PanelTab::Messages => {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(i18n.lbl_events(self.log.len()))
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui.small_button(i18n.btn_clear()).clicked() {
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
                                egui::RichText::new(i18n.lbl_no_messages())
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
                                    // Header row: icon + timestamp
                                    ui.horizontal(|ui| {
                                        ui.colored_label(color, icon);
                                        ui.label(
                                            egui::RichText::new(&time_str)
                                                .small()
                                                .monospace()
                                                .color(egui::Color32::GRAY),
                                        );
                                    });
                                    // Message body — first line is the main message,
                                    // subsequent lines (Detail: / Hint:) are rendered dimmer.
                                    let mut lines = entry.text.splitn(2, '\n');
                                    let main_line = lines.next().unwrap_or("");
                                    let rest      = lines.next().unwrap_or("");
                                    ui.add_space(1.0);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(main_line).color(color),
                                        )
                                        .wrap(),
                                    );
                                    if !rest.is_empty() {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(rest)
                                                    .small()
                                                    .color(egui::Color32::from_gray(160)),
                                            )
                                            .wrap(),
                                        );
                                    }
                                });
                            ui.add_space(2.0);
                        }
                    }
                }
                PanelTab::History => {
                    ui.horizontal(|ui| {
                        ui.label(i18n.label_search());
                        ui.text_edit_singleline(&mut self.history_search);
                    });
                    ui.separator();
                    let search = self.history_search.to_lowercase();
                    let entries: Vec<crate::history::HistoryEntry> = history
                        .entries()
                        .iter()
                        .filter(|e| {
                            search.is_empty() || e.sql.to_lowercase().contains(&search)
                        })
                        .cloned()
                        .rev()
                        .collect();

                    let text_dim = egui::Color32::from_rgb(110, 123, 139);
                    for entry in &entries {
                        let preview: String =
                            entry.sql.lines().next().unwrap_or("").chars().take(72).collect();
                        let time_str = entry.executed_at.format("%H:%M").to_string();
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&time_str)
                                    .small()
                                    .monospace()
                                    .color(text_dim),
                            );
                            let resp = ui.add(
                                egui::Label::new(egui::RichText::new(preview).monospace())
                                    .sense(egui::Sense::click()),
                            );
                            if resp.double_clicked() {
                                self.sql = entry.sql.clone();
                                self.browse = None;
                            }
                            resp.on_hover_text(entry.sql.as_str());
                        });
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

                if ui.add_enabled(can_prev, egui::Button::new(i18n.btn_prev_page())).clicked() {
                    if let Some(state) = &mut self.browse {
                        state.page -= 1;
                        let sql = state.build_sql();
                        self.set_running();
                        let _ = db_tx.send(DbCommand::Execute(sql));
                    }
                }

                ui.label(i18n.lbl_page(page + 1));

                if ui.add_enabled(can_next, egui::Button::new(i18n.btn_next_page())).clicked() {
                    if let Some(state) = &mut self.browse {
                        state.page += 1;
                        let sql = state.build_sql();
                        self.set_running();
                        let _ = db_tx.send(DbCommand::Execute(sql));
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(i18n.lbl_rows_per_page(PAGE_SIZE))
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                });
            });
        }

        // Floating popups — rendered last so they draw on top of everything.
        let ctx = ui.ctx().clone();
        self.show_cell_popup(&ctx, i18n);
        self.show_col_stats_popup(&ctx, i18n);
    }

    // ── Cell value popup ─────────────────────────────────────────────────────

    fn show_cell_popup(&mut self, ctx: &egui::Context, i18n: &I18n) {
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
                    if ui.button(i18n.btn_copy()).clicked() {
                        ctx.copy_text(popup.value.clone());
                    }
                    if is_browse && ui.button(i18n.btn_edit()).clicked() {
                        start_edit = true;
                    }
                    // Copy as INSERT statement
                    if ui.button(i18n.btn_copy_as_insert()).on_hover_text(i18n.hover_copy_insert()).clicked() {
                        if let Some(result) = &self.result {
                            let cols: Vec<&str> =
                                result.columns.iter().map(|c| c.as_str()).collect();
                            if let Some(row) = result.rows.get(popup.actual_row) {
                                let table_name = self
                                    .browse
                                    .as_ref()
                                    .map(|b| b.label())
                                    .unwrap_or_else(|| "table_name".to_owned());
                                let col_list = cols
                                    .iter()
                                    .map(|c| format!("\"{}\"", c))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                let val_list = row
                                    .iter()
                                    .map(|c| match c {
                                        crate::db::query::CellValue::Null => "NULL".to_owned(),
                                        crate::db::query::CellValue::Boolean(b) => b.to_string(),
                                        crate::db::query::CellValue::Integer(n) => n.to_string(),
                                        crate::db::query::CellValue::Float(f) => f.to_string(),
                                        other => format!("'{}'", other.to_string().replace('\'', "''")),
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                let insert = format!(
                                    "INSERT INTO {table_name} ({col_list}) VALUES ({val_list});"
                                );
                                ctx.copy_text(insert);
                            }
                        }
                    }
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui.button(i18n.btn_close()).clicked() {
                                close_clicked = true;
                            }
                            ui.label(
                                egui::RichText::new(i18n.lbl_chars(popup.value.chars().count()))
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

    // ── Column stats popup ────────────────────────────────────────────────────

    fn show_col_stats_popup(&mut self, ctx: &egui::Context, i18n: &I18n) {
        let Some(stats) = self.col_stats.take() else { return };
        let mut open = true;

        egui::Window::new(i18n.col_stats_title(&stats.col_name))
            .collapsible(false)
            .resizable(false)
            .min_width(260.0)
            .open(&mut open)
            .show(ctx, |ui| {
                let null_pct = if stats.total > 0 {
                    stats.null_count as f64 / stats.total as f64 * 100.0
                } else {
                    0.0
                };

                egui::Grid::new("col_stats_grid")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(i18n.col_stats_total()).strong());
                        ui.label(format!("{}", stats.total));
                        ui.end_row();

                        ui.label(egui::RichText::new(i18n.col_stats_null()).strong());
                        ui.label(format!("{} ({:.1}%)", stats.null_count, null_pct));
                        ui.end_row();

                        ui.label(egui::RichText::new(i18n.col_stats_distinct()).strong());
                        ui.label(format!("{}", stats.distinct));
                        ui.end_row();

                        if let Some(min) = stats.min_len {
                            ui.label(egui::RichText::new(i18n.col_stats_min_len()).strong());
                            ui.label(format!("{min}"));
                            ui.end_row();
                        }
                        if let Some(max) = stats.max_len {
                            ui.label(egui::RichText::new(i18n.col_stats_max_len()).strong());
                            ui.label(format!("{max}"));
                            ui.end_row();
                        }
                    });

                if !stats.top_values.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new(i18n.col_stats_top_values()).strong());
                    ui.add_space(2.0);
                    for (val, count) in &stats.top_values {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("×{count}"))
                                    .monospace()
                                    .color(egui::Color32::from_rgb(100, 180, 100)),
                            );
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(val).monospace()
                                )
                                .truncate(),
                            );
                        });
                    }
                }

                ui.separator();
                ui.label(
                    egui::RichText::new(i18n.col_stats_source_note())
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });

        if open {
            self.col_stats = Some(stats);
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
            let i18n = I18n::new(self.lang);
            self.push_log(LogEntry::warning(i18n.warn_no_pk(&schema, &table)));
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
fn pick_open_sql_file() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("SQL files", &["sql"])
        .add_filter("All files", &["*"])
        .pick_file()
        .map(|p| p.to_string_lossy().into_owned())
}

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

// ── SQL formatter ─────────────────────────────────────────────────────────────

