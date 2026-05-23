use egui_extras::{Column, TableBuilder};

use crate::db::query::{CellValue, QueryResult};
use crate::i18n::I18n;

const NULL_COLOR: egui::Color32 = egui::Color32::from_rgb(128, 100, 100);
const NULL_LABEL: &str = "<null>";

// ── Output returned by show() ─────────────────────────────────────────────────

#[derive(Default)]
pub struct TableOutput {
    /// Header click — (col_name, ascending)
    pub sort_changed: Option<(String, bool)>,
    /// Cell double-clicked in browse mode — (display_row, col_idx)
    pub cell_double_clicked: Option<(usize, usize)>,
    /// Cell single-clicked — (display_row, col_idx)
    pub cell_clicked: Option<(usize, usize)>,
    /// Edit committed with Enter — (display_row, col_idx, new_value)
    pub edit_committed: Option<(usize, usize, String)>,
    pub edit_cancelled: bool,
    /// Right-click "Statistics" on a column header → column index
    pub col_stats_requested: Option<usize>,
    /// Right-click "View Full Value" on a cell → (display_row, col_idx)
    pub full_value_requested: Option<(usize, usize)>,
    /// Right-click "Copy table as Markdown" on any cell
    pub copy_as_markdown: bool,
    /// Right-click "Copy table as HTML" on any cell
    pub copy_as_html: bool,
}

// ── ResultTable ───────────────────────────────────────────────────────────────

pub struct ResultTable<'a> {
    result: &'a QueryResult,
    pub selected_row: Option<usize>,
    pub selected_cell: Option<(usize, usize)>,
    pub sort_col: Option<usize>,
    pub sort_asc: bool,
    pub sorted_indices: Vec<usize>,
    /// When true, skip client-side sort; caller re-queries DB.
    pub db_sort_mode: bool,
    /// Initial column widths computed from content (empty = uniform fallback).
    pub col_widths: Vec<f32>,
    // ── Inline edit (set by caller, read back after show()) ──────────────────
    /// Display-row being edited (None = not editing).
    pub edit_row: Option<usize>,
    pub edit_col: Option<usize>,
    /// Current text in the edit box — persisted by caller between frames.
    pub edit_value: String,
    /// Request focus on the TextEdit this frame.
    pub edit_needs_focus: bool,
}

impl<'a> ResultTable<'a> {
    /// Create with externally managed sorted_indices (avoids per-frame allocation).
    /// `col_widths`: content-aware initial widths pre-computed by caller; empty = uniform fallback.
    pub fn with_indices(result: &'a QueryResult, sorted_indices: Vec<usize>, col_widths: Vec<f32>) -> Self {
        Self {
            result,
            selected_row: None,
            selected_cell: None,
            sort_col: None,
            sort_asc: true,
            sorted_indices,
            db_sort_mode: false,
            col_widths,
            edit_row: None,
            edit_col: None,
            edit_value: String::new(),
            edit_needs_focus: false,
        }
    }

    /// `display_indices`: pre-filtered & sorted slice managed by caller (avoids per-frame clone/scan).
    /// `search`: non-empty string causes matching cells to be highlighted (does not filter rows).
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n, display_indices: &[usize], search: &str) -> TableOutput {
        if self.result.columns.is_empty() {
            if let Some(n) = self.result.rows_affected {
                ui.label(i18n.query_ok_rows(n));
            } else {
                ui.label(i18n.lbl_no_results());
            }
            return TableOutput::default();
        }

        let col_count = self.result.columns.len();

        // Use content-aware widths when available, uniform fallback otherwise.
        let uniform_col_width = (ui.available_width() / col_count as f32)
            .max(60.0)
            .min(300.0);

        // Stable Id based on column names — same columns keep user-resized widths across
        // queries; different columns start fresh with content-aware initial widths.
        let col_id: u64 = self.result.columns.iter().fold(0u64, |acc, name| {
            name.bytes().fold(acc, |h, b| h.wrapping_mul(31).wrapping_add(b as u64))
        });

        // Pre-compute lowercase search term once (avoids per-cell reallocation).
        let search_lower: Option<String> = if search.is_empty() {
            None
        } else {
            Some(search.to_lowercase())
        };

        let mut builder = TableBuilder::new(ui)
            .id_source(col_id)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0);

        for i in 0..col_count {
            let w = self.col_widths.get(i).copied().unwrap_or(uniform_col_width);
            builder = builder.column(Column::initial(w).resizable(true));
        }

        // ── Extract fields needed inside closures ─────────────────────────────
        // Copy/clone to avoid capturing `self` by &mut inside the closures,
        // which would conflict with `self.edit_value = …` after the builder.
        let sort_col = self.sort_col;
        let sort_asc = self.sort_asc;
        let selected_row = self.selected_row;
        let _ = self.db_sort_mode; // used below via self.db_sort_mode directly
        let edit_row = self.edit_row;
        let edit_col = self.edit_col;
        let edit_needs_focus = self.edit_needs_focus;
        // Take the edit value out so the closure can mutate it freely.
        let mut edit_val = std::mem::take(&mut self.edit_value);

        let mut sort_changed: Option<(usize, bool)> = None;
        let mut cell_double_clicked: Option<(usize, usize)> = None;
        let mut cell_clicked: Option<(usize, usize)> = None;
        let mut edit_committed_flag = false;
        let mut edit_cancelled_flag = false;
        let mut col_stats_requested: Option<usize> = None;
        let mut full_value_requested: Option<(usize, usize)> = None;
        let mut copy_as_markdown = false;
        let mut copy_as_html = false;

        builder
            .header(24.0, |mut header| {
                for (i, col_name) in self.result.columns.iter().enumerate() {
                    header.col(|ui| {
                        let label = match (sort_col == Some(i), sort_asc) {
                            (true, true) => format!("{col_name} ▲"),
                            (true, false) => format!("{col_name} ▼"),
                            _ => col_name.clone(),
                        };
                        let resp = ui.add(
                            egui::Label::new(egui::RichText::new(label).strong())
                                .sense(egui::Sense::click()),
                        );
                        if resp.clicked() {
                            let asc = if sort_col == Some(i) { !sort_asc } else { true };
                            sort_changed = Some((i, asc));
                        }
                        let col_i = i;
                        resp.context_menu(|ui| {
                            if ui.button("📊 Statistics").clicked() {
                                col_stats_requested = Some(col_i);
                                ui.close_menu();
                            }
                        });
                    });
                }
            })
            .body(|body| {
                body.rows(20.0, display_indices.len(), |mut row| {
                    let display_idx = row.index();
                    let actual_idx = display_indices[display_idx]; // caller-managed, no per-frame alloc
                    let row_data = &self.result.rows[actual_idx];

                    row.set_selected(selected_row == Some(display_idx));

                    for (col_idx, cell) in row_data.iter().enumerate() {
                        let is_editing =
                            edit_row == Some(display_idx) && edit_col == Some(col_idx);

                        row.col(|ui| {
                            if is_editing {
                                // Read keys BEFORE rendering the TextEdit so the
                                // widget cannot consume them first.
                                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                                let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));

                                let te_resp = ui.add(
                                    egui::TextEdit::singleline(&mut edit_val)
                                        .desired_width(f32::INFINITY),
                                );
                                if edit_needs_focus && !te_resp.has_focus() {
                                    te_resp.request_focus();
                                }
                                if te_resp.has_focus() || te_resp.lost_focus() {
                                    if enter {
                                        edit_committed_flag = true;
                                    }
                                    if escape {
                                        edit_cancelled_flag = true;
                                    }
                                }
                            } else {
                                let full_rect = ui.available_rect_before_wrap();
                                let cell_str = cell.to_string();
                                let is_null = matches!(cell, crate::db::query::CellValue::Null);

                                // Highlight cells matching the search term (painter only, no allocation).
                                if let Some(ref s) = search_lower {
                                    if !is_null && cell_str.to_lowercase().contains(s.as_str()) {
                                        ui.painter().rect_filled(
                                            full_rect,
                                            2.0,
                                            egui::Color32::from_rgba_premultiplied(255, 200, 50, 55),
                                        );
                                    }
                                }

                                // Render content FIRST so the allocate_rect below comes last
                                // in egui's widget registry — later allocation wins hover priority,
                                // which makes right-click work even when the pointer is over the text.
                                render_cell(ui, cell);

                                // Claim the full cell rect AFTER render so context_menu responds
                                // to secondary clicks anywhere in the cell (not just empty space).
                                let cell_resp = ui.allocate_rect(full_rect, egui::Sense::click());
                                if cell_resp.double_clicked() {
                                    cell_double_clicked = Some((display_idx, col_idx));
                                } else if cell_resp.clicked() {
                                    cell_clicked = Some((actual_idx, col_idx));
                                }
                                cell_resp.context_menu(|ui| {
                                    if ui.button(i18n.cell_view_full()).clicked() {
                                        full_value_requested = Some((display_idx, col_idx));
                                        ui.close_menu();
                                    }
                                    if !is_null && ui.button(i18n.cell_copy_value()).clicked() {
                                        ui.ctx().copy_text(cell_str.clone());
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button(i18n.cell_copy_as_markdown()).clicked() {
                                        copy_as_markdown = true;
                                        ui.close_menu();
                                    }
                                    if ui.button(i18n.cell_copy_as_html()).clicked() {
                                        copy_as_html = true;
                                        ui.close_menu();
                                    }
                                });
                            }
                        });
                    }
                });
            });

        // ── Write back mutable state ──────────────────────────────────────────
        self.edit_value = edit_val;
        if edit_needs_focus {
            self.edit_needs_focus = false;
        }

        // ── Sort ──────────────────────────────────────────────────────────────
        if let Some((col, asc)) = sort_changed {
            if !self.db_sort_mode {
                self.apply_sort(col, asc);
            } else {
                self.sort_col = Some(col);
                self.sort_asc = asc;
            }
            return TableOutput {
                sort_changed: Some((self.result.columns[col].clone(), asc)),
                cell_clicked,
                col_stats_requested,
                full_value_requested,
                copy_as_markdown,
                copy_as_html,
                ..Default::default()
            };
        }

        // ── Edit commit / cancel ──────────────────────────────────────────────
        let edit_committed = if edit_committed_flag {
            edit_row
                .zip(edit_col)
                .map(|(r, c)| (r, c, self.edit_value.clone()))
        } else {
            None
        };

        if edit_committed_flag || edit_cancelled_flag {
            self.edit_row = None;
            self.edit_col = None;
            self.edit_value.clear();
        }

        TableOutput {
            sort_changed: None,
            cell_double_clicked,
            cell_clicked,
            edit_committed,
            edit_cancelled: edit_cancelled_flag,
            col_stats_requested,
            full_value_requested,
            copy_as_markdown,
            copy_as_html,
        }
    }

    fn apply_sort(&mut self, col_idx: usize, asc: bool) {
        self.sort_col = Some(col_idx);
        self.sort_asc = asc;
        self.sorted_indices.sort_by(|&a, &b| {
            let va = &self.result.rows[a][col_idx];
            let vb = &self.result.rows[b][col_idx];
            let ord = compare_cells(va, vb);
            if asc { ord } else { ord.reverse() }
        });
    }
}

// ── Cell renderers ────────────────────────────────────────────────────────────

fn render_cell(ui: &mut egui::Ui, cell: &CellValue) {
    match cell {
        CellValue::Null => {
            ui.add(egui::Label::new(
                egui::RichText::new(NULL_LABEL).color(NULL_COLOR).italics(),
            ));
        }
        CellValue::Boolean(true) => {
            ui.label(egui::RichText::new("true").color(egui::Color32::GREEN));
        }
        CellValue::Boolean(false) => {
            ui.label(egui::RichText::new("false").color(egui::Color32::RED));
        }
        other => {
            ui.add(egui::Label::new(other.to_string()).truncate());
        }
    }
}

fn compare_cells(a: &CellValue, b: &CellValue) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (CellValue::Null, CellValue::Null) => Ordering::Equal,
        (CellValue::Null, _) => Ordering::Less,
        (_, CellValue::Null) => Ordering::Greater,
        (CellValue::Integer(x), CellValue::Integer(y)) => x.cmp(y),
        (CellValue::Float(x), CellValue::Float(y)) => {
            x.partial_cmp(y).unwrap_or(Ordering::Equal)
        }
        _ => a.to_string().cmp(&b.to_string()),
    }
}
