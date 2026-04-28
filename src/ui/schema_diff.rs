use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

use egui::{Color32, RichText, ScrollArea};

use crate::db::DbCommand;
use crate::i18n::I18n;

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn next_request_id() -> u64 {
    NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
}

// ── Types ──────────────────────────────────────────────────────────────────────

/// (table_name, column_name, data_type) rows from information_schema.columns
pub type SchemaRows = Vec<(String, String, String)>;

#[derive(Debug, Clone)]
pub enum ColDiff {
    Added { col: String, dtype: String },
    Removed { col: String, dtype: String },
    TypeChanged { col: String, old: String, new: String },
}

#[derive(Debug, Clone)]
pub enum TableDiff {
    /// Table exists only in B (added relative to A).
    Added(String),
    /// Table exists only in A (removed relative to A → B).
    Removed(String),
    Changed { table: String, cols: Vec<ColDiff> },
}

fn compute_diff(a_rows: &SchemaRows, b_rows: &SchemaRows) -> Vec<TableDiff> {
    let mut a_map: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut b_map: HashMap<String, HashMap<String, String>> = HashMap::new();

    for (tbl, col, dtype) in a_rows {
        a_map.entry(tbl.clone()).or_default().insert(col.clone(), dtype.clone());
    }
    for (tbl, col, dtype) in b_rows {
        b_map.entry(tbl.clone()).or_default().insert(col.clone(), dtype.clone());
    }

    let mut a_tables: Vec<_> = a_map.keys().cloned().collect();
    a_tables.sort();

    let mut b_tables: Vec<_> = b_map.keys().cloned().collect();
    b_tables.sort();

    let mut result: Vec<TableDiff> = Vec::new();

    // Removed tables (in A but not B)
    for tbl in &a_tables {
        if !b_map.contains_key(tbl) {
            result.push(TableDiff::Removed(tbl.clone()));
        }
    }

    // Added tables (in B but not A)
    for tbl in &b_tables {
        if !a_map.contains_key(tbl) {
            result.push(TableDiff::Added(tbl.clone()));
        }
    }

    // Changed tables (in both)
    for tbl in &a_tables {
        if let Some(b_cols) = b_map.get(tbl) {
            let a_cols = &a_map[tbl];
            let mut col_diffs: Vec<ColDiff> = Vec::new();

            // Check A cols
            let mut a_col_names: Vec<_> = a_cols.keys().cloned().collect();
            a_col_names.sort();
            for col in &a_col_names {
                match b_cols.get(col) {
                    None => col_diffs.push(ColDiff::Removed {
                        col: col.clone(),
                        dtype: a_cols[col].clone(),
                    }),
                    Some(b_dtype) if b_dtype != &a_cols[col] => col_diffs.push(ColDiff::TypeChanged {
                        col: col.clone(),
                        old: a_cols[col].clone(),
                        new: b_dtype.clone(),
                    }),
                    _ => {}
                }
            }

            // Added cols (in B but not A)
            let mut b_col_names: Vec<_> = b_cols.keys().cloned().collect();
            b_col_names.sort();
            for col in &b_col_names {
                if !a_cols.contains_key(col) {
                    col_diffs.push(ColDiff::Added {
                        col: col.clone(),
                        dtype: b_cols[col].clone(),
                    });
                }
            }

            if !col_diffs.is_empty() {
                result.push(TableDiff::Changed {
                    table: tbl.clone(),
                    cols: col_diffs,
                });
            }
        }
    }

    // Sort: removed → added → changed
    result.sort_by_key(|d| match d {
        TableDiff::Removed(_) => 0u8,
        TableDiff::Added(_) => 1,
        TableDiff::Changed { .. } => 2,
    });

    result
}

// ── State machine ──────────────────────────────────────────────────────────────

enum DiffState {
    Idle,
    Loading {
        req_a: u64,
        req_b: u64,
        a_done: bool,
        b_done: bool,
        rows_a: SchemaRows,
        rows_b: SchemaRows,
    },
    Done(Vec<TableDiff>),
    Error(String),
}

impl Default for DiffState {
    fn default() -> Self {
        DiffState::Idle
    }
}

// ── SchemaDiff ─────────────────────────────────────────────────────────────────

pub struct SchemaDiff {
    pub conn_id_a: usize,
    pub schema_a: String,
    pub conn_id_b: usize,
    pub schema_b: String,
    state: DiffState,
}

impl SchemaDiff {
    pub fn new(conn_id_a: usize, conn_id_b: usize) -> Self {
        Self {
            conn_id_a,
            schema_a: "public".to_owned(),
            conn_id_b,
            schema_b: "public".to_owned(),
            state: DiffState::Idle,
        }
    }

    /// True if this diff is waiting for the given request_id.
    pub fn has_request(&self, request_id: u64) -> bool {
        match &self.state {
            DiffState::Loading { req_a, req_b, .. } => request_id == *req_a || request_id == *req_b,
            _ => false,
        }
    }

    /// Deliver a snapshot result from the DB. Returns true if this was consumed.
    pub fn deliver(&mut self, request_id: u64, rows: SchemaRows) -> bool {
        let DiffState::Loading { req_a, req_b, a_done, b_done, rows_a, rows_b } = &mut self.state
        else {
            return false;
        };

        if request_id == *req_a {
            *rows_a = rows;
            *a_done = true;
        } else if request_id == *req_b {
            *rows_b = rows;
            *b_done = true;
        } else {
            return false;
        }

        if *a_done && *b_done {
            let diff = compute_diff(rows_a, rows_b);
            self.state = DiffState::Done(diff);
        }
        true
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        conns: &[(usize, &str, &Sender<DbCommand>)],
        _i18n: &I18n,
    ) {
        ui.vertical(|ui| {
            // ── Controls ──────────────────────────────────────────────────────
            ui.horizontal(|ui| {
                // Side A
                ui.label("A:");
                egui::ComboBox::from_id_source("diff_conn_a")
                    .selected_text(conn_name(conns, self.conn_id_a))
                    .show_ui(ui, |ui| {
                        for (id, name, _) in conns {
                            ui.selectable_value(&mut self.conn_id_a, *id, *name);
                        }
                    });
                ui.add(
                    egui::TextEdit::singleline(&mut self.schema_a)
                        .desired_width(100.0)
                        .hint_text("schema"),
                );

                ui.label(" → ");

                // Side B
                ui.label("B:");
                egui::ComboBox::from_id_source("diff_conn_b")
                    .selected_text(conn_name(conns, self.conn_id_b))
                    .show_ui(ui, |ui| {
                        for (id, name, _) in conns {
                            ui.selectable_value(&mut self.conn_id_b, *id, *name);
                        }
                    });
                ui.add(
                    egui::TextEdit::singleline(&mut self.schema_b)
                        .desired_width(100.0)
                        .hint_text("schema"),
                );

                let is_loading = matches!(self.state, DiffState::Loading { .. });
                ui.add_enabled_ui(!is_loading && !conns.is_empty(), |ui| {
                    if ui.button("Compare").clicked() {
                        self.start_compare(conns);
                    }
                });

                if is_loading {
                    ui.spinner();
                }
            });

            ui.separator();

            // ── Results ───────────────────────────────────────────────────────
            match &self.state {
                DiffState::Idle => {
                    ui.add_space(16.0);
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("Select connections and schemas, then click Compare.")
                                .color(Color32::GRAY),
                        );
                    });
                }
                DiffState::Loading { .. } => {
                    ui.add_space(16.0);
                    ui.label(RichText::new("Loading snapshots…").color(Color32::GRAY));
                }
                DiffState::Error(e) => {
                    ui.label(RichText::new(e).color(Color32::from_rgb(207, 84, 80)));
                }
                DiffState::Done(diffs) => {
                    if diffs.is_empty() {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("✓  Schemas are identical.")
                                .color(Color32::from_rgb(73, 156, 84))
                                .strong(),
                        );
                    } else {
                        let n_added =
                            diffs.iter().filter(|d| matches!(d, TableDiff::Added(_))).count();
                        let n_removed =
                            diffs.iter().filter(|d| matches!(d, TableDiff::Removed(_))).count();
                        let n_changed = diffs
                            .iter()
                            .filter(|d| matches!(d, TableDiff::Changed { .. }))
                            .count();

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("+{n_added} added"))
                                    .color(Color32::from_rgb(73, 156, 84))
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(format!("~{n_changed} changed"))
                                    .color(Color32::from_rgb(229, 192, 123))
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(format!("-{n_removed} removed"))
                                    .color(Color32::from_rgb(207, 84, 80))
                                    .strong(),
                            );
                        });
                        ui.separator();

                        let diffs = diffs.clone();
                        ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                            for diff in &diffs {
                                match diff {
                                    TableDiff::Added(tbl) => {
                                        ui.label(
                                            RichText::new(format!("+ {tbl}"))
                                                .color(Color32::from_rgb(73, 156, 84))
                                                .monospace()
                                                .strong(),
                                        );
                                    }
                                    TableDiff::Removed(tbl) => {
                                        ui.label(
                                            RichText::new(format!("- {tbl}"))
                                                .color(Color32::from_rgb(207, 84, 80))
                                                .monospace()
                                                .strong(),
                                        );
                                    }
                                    TableDiff::Changed { table, cols } => {
                                        ui.label(
                                            RichText::new(format!("~ {table}"))
                                                .color(Color32::from_rgb(229, 192, 123))
                                                .monospace()
                                                .strong(),
                                        );
                                        ui.indent(egui::Id::new(("diff_tbl", table.as_str())), |ui| {
                                            for col in cols {
                                                match col {
                                                    ColDiff::Added { col, dtype } => {
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "+ {col}  ({dtype})"
                                                            ))
                                                            .color(Color32::from_rgb(73, 156, 84))
                                                            .monospace(),
                                                        );
                                                    }
                                                    ColDiff::Removed { col, dtype } => {
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "- {col}  ({dtype})"
                                                            ))
                                                            .color(Color32::from_rgb(207, 84, 80))
                                                            .monospace(),
                                                        );
                                                    }
                                                    ColDiff::TypeChanged { col, old, new } => {
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "~ {col}  ({old} → {new})"
                                                            ))
                                                            .color(Color32::from_rgb(229, 192, 123))
                                                            .monospace(),
                                                        );
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                        });
                    }
                }
            }
        });
    }

    fn start_compare(&mut self, conns: &[(usize, &str, &Sender<DbCommand>)]) {
        let req_a = next_request_id();
        let req_b = next_request_id();

        if let Some((_, _, tx)) = conns.iter().find(|(id, _, _)| *id == self.conn_id_a) {
            let _ = tx.send(DbCommand::LoadSchemaSnapshot {
                schema: self.schema_a.clone(),
                request_id: req_a,
            });
        }
        if let Some((_, _, tx)) = conns.iter().find(|(id, _, _)| *id == self.conn_id_b) {
            let _ = tx.send(DbCommand::LoadSchemaSnapshot {
                schema: self.schema_b.clone(),
                request_id: req_b,
            });
        }

        self.state = DiffState::Loading {
            req_a,
            req_b,
            a_done: false,
            b_done: false,
            rows_a: Vec::new(),
            rows_b: Vec::new(),
        };
    }
}

fn conn_name<'a>(conns: &'a [(usize, &'a str, &'a Sender<DbCommand>)], conn_id: usize) -> &'a str {
    conns
        .iter()
        .find(|(id, _, _)| *id == conn_id)
        .map(|(_, name, _)| *name)
        .unwrap_or("—")
}
