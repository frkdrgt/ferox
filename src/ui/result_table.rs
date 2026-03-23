use egui_extras::{Column, TableBuilder};

use crate::db::query::{CellValue, QueryResult};

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
    /// Client-side text filter applied to all cell values.
    pub filter_text: String,
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
    pub fn new(result: &'a QueryResult) -> Self {
        let sorted_indices = (0..result.rows.len()).collect();
        Self {
            result,
            selected_row: None,
            selected_cell: None,
            sort_col: None,
            sort_asc: true,
            sorted_indices,
            db_sort_mode: false,
            filter_text: String::new(),
            edit_row: None,
            edit_col: None,
            edit_value: String::new(),
            edit_needs_focus: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> TableOutput {
        if self.result.columns.is_empty() {
            if let Some(n) = self.result.rows_affected {
                ui.label(format!("Query OK — {n} rows affected"));
            } else {
                ui.label("No results");
            }
            return TableOutput::default();
        }

        let col_count = self.result.columns.len();

        let col_width = (ui.available_width() / col_count as f32)
            .max(60.0)
            .min(300.0);

        let mut builder = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0);

        for _ in 0..col_count {
            builder = builder.column(Column::initial(col_width).resizable(true));
        }

        // ── Extract fields needed inside closures ─────────────────────────────
        // Copy/clone to avoid capturing `self` by &mut inside the closures,
        // which would conflict with `self.edit_value = …` after the builder.
        let sorted_indices = self.sorted_indices.clone();
        let sort_col = self.sort_col;
        let sort_asc = self.sort_asc;
        let selected_row = self.selected_row;
        let _ = self.db_sort_mode; // used below via self.db_sort_mode directly
        let edit_row = self.edit_row;
        let edit_col = self.edit_col;
        let edit_needs_focus = self.edit_needs_focus;
        // Take the edit value out so the closure can mutate it freely.
        let mut edit_val = std::mem::take(&mut self.edit_value);

        // Apply client-side filter on sorted_indices (does not mutate sorted_indices)
        let display_indices: Vec<usize> = if !self.filter_text.is_empty() {
            let f = self.filter_text.to_lowercase();
            sorted_indices
                .iter()
                .copied()
                .filter(|&i| {
                    self.result.rows[i]
                        .iter()
                        .any(|cell| cell.to_string().to_lowercase().contains(&f))
                })
                .collect()
        } else {
            sorted_indices.clone()
        };

        let mut sort_changed: Option<(usize, bool)> = None;
        let mut cell_double_clicked: Option<(usize, usize)> = None;
        let mut cell_clicked: Option<(usize, usize)> = None;
        let mut edit_committed_flag = false;
        let mut edit_cancelled_flag = false;

        builder
            .header(24.0, |mut header| {
                for (i, col_name) in self.result.columns.iter().enumerate() {
                    header.col(|ui| {
                        let label = match (sort_col == Some(i), sort_asc) {
                            (true, true) => format!("{col_name} ▲"),
                            (true, false) => format!("{col_name} ▼"),
                            _ => col_name.clone(),
                        };
                        if ui
                            .add(
                                egui::Label::new(egui::RichText::new(label).strong())
                                    .sense(egui::Sense::click()),
                            )
                            .clicked()
                        {
                            let asc = if sort_col == Some(i) { !sort_asc } else { true };
                            sort_changed = Some((i, asc));
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(20.0, display_indices.len(), |mut row| {
                    let display_idx = row.index();
                    let actual_idx = display_indices[display_idx];
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
                                // egui_extras TableRow::col returns Sense::hover only,
                                // so we allocate an interactive rect ourselves.
                                let rect = ui.available_rect_before_wrap();
                                let cell_resp = ui.interact(
                                    rect,
                                    ui.id().with((display_idx, col_idx)),
                                    egui::Sense::click(),
                                );
                                render_cell(ui, cell);
                                if cell_resp.double_clicked() {
                                    cell_double_clicked = Some((display_idx, col_idx));
                                } else if cell_resp.clicked() {
                                    cell_clicked = Some((actual_idx, col_idx));
                                }
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
            ui.add(egui::Label::new(other.to_string()).truncate(true));
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
