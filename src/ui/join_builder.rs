use egui::{Color32, ComboBox, RichText, ScrollArea, TextEdit};

// ── Join type ─────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Default)]
enum JoinType {
    #[default]
    Inner,
    Left,
    Right,
    Full,
}

impl JoinType {
    fn label(&self) -> &'static str {
        match self {
            Self::Inner => "INNER JOIN",
            Self::Left  => "LEFT JOIN",
            Self::Right => "RIGHT JOIN",
            Self::Full  => "FULL JOIN",
        }
    }
}

// ── Internal state ────────────────────────────────────────────────────────────

#[derive(Clone)]
struct TableEntry {
    schema:  String,
    table:   String,
    alias:   String,
    columns: Vec<String>,
}

#[derive(Clone)]
struct Condition {
    left_idx:  usize,
    left_col:  String,
    right_idx: usize,
    right_col: String,
    join_type: JoinType,
}

// ── Public API ────────────────────────────────────────────────────────────────

pub enum JoinAction {
    Run(String),
    SendToEditor(String),
    /// Column metadata was missing — ask the DB worker to load it.
    LoadDetails { schema: String, table: String },
}

/// `(schema_name, [(table_name, [column_name])])`
pub type AvailableData = Vec<(String, Vec<(String, Vec<String>)>)>;

pub struct JoinBuilder {
    pub open: bool,
    tables:     Vec<TableEntry>,
    conditions: Vec<Condition>,
    add_schema_idx: usize,
    add_table_idx:  usize,
    available: AvailableData,
    /// Keys we've already fired a LoadDetails for — avoids repeated requests.
    requested: std::collections::HashSet<(String, String)>,
}

impl Default for JoinBuilder {
    fn default() -> Self {
        Self {
            open: false,
            tables: Vec::new(),
            conditions: Vec::new(),
            add_schema_idx: 0,
            add_table_idx:  0,
            available: Vec::new(),
            requested: std::collections::HashSet::new(),
        }
    }
}

impl JoinBuilder {
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Call every frame with fresh sidebar data.
    /// Also back-fills columns for already-added tables once they arrive.
    pub fn update_available(&mut self, data: AvailableData) {
        for entry in &mut self.tables {
            if entry.columns.is_empty() {
                if let Some((_, tables)) = data.iter().find(|(s, _)| *s == entry.schema) {
                    if let Some((_, cols)) = tables.iter().find(|(t, _)| *t == entry.table) {
                        if !cols.is_empty() {
                            entry.columns = cols.clone();
                        }
                    }
                }
            }
        }
        self.available = data;
    }

    /// Render and return any actions produced this frame.
    pub fn show(&mut self, ctx: &egui::Context) -> Vec<JoinAction> {
        if !self.open {
            return Vec::new();
        }

        let mut actions: Vec<JoinAction> = Vec::new();
        let mut window_open = true;

        egui::Window::new("Join Builder")
            .open(&mut window_open)
            .resizable(true)
            .min_width(720.0)
            .min_height(520.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                self.render(ui, &mut actions);
            });

        if !window_open {
            self.open = false;
        }
        actions
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    fn render(&mut self, ui: &mut egui::Ui, actions: &mut Vec<JoinAction>) {
        ScrollArea::vertical().show(ui, |ui| {

            // ── 1. Tables ─────────────────────────────────────────────────────
            section_header(ui, "TABLES");

            let mut remove_table: Option<usize> = None;
            for (i, t) in self.tables.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&t.alias)
                            .monospace()
                            .color(Color32::from_rgb(86, 156, 214)),
                    );
                    ui.weak("→");
                    ui.label(format!("{}.{}", t.schema, t.table));
                    if t.columns.is_empty() {
                        ui.weak("(⟳ loading…)");
                    } else {
                        ui.weak(format!("({} cols)", t.columns.len()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕").on_hover_text("Remove").clicked() {
                            remove_table = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove_table {
                self.tables.remove(i);
                self.conditions.retain(|c| c.left_idx != i && c.right_idx != i);
                for c in &mut self.conditions {
                    if c.left_idx  > i { c.left_idx  -= 1; }
                    if c.right_idx > i { c.right_idx -= 1; }
                }
                if !self.available.is_empty() {
                    self.add_schema_idx = self.add_schema_idx.min(self.available.len() - 1);
                }
            }

            // Add-table row
            ui.horizontal(|ui| {
                let schema_label = self.available.get(self.add_schema_idx)
                    .map(|(s, _)| s.as_str()).unwrap_or("schema");
                ComboBox::from_id_source("jb_add_schema")
                    .selected_text(schema_label)
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        ScrollArea::vertical()
                            .max_height(300.0)
                            .id_source("jb_add_schema_scroll")
                            .show(ui, |ui| {
                                for (i, (s, _)) in self.available.iter().enumerate() {
                                    if ui.selectable_label(self.add_schema_idx == i, s.as_str()).clicked() {
                                        self.add_schema_idx = i;
                                        self.add_table_idx  = 0;
                                    }
                                }
                            });
                    });

                let tables_in_schema = self.available.get(self.add_schema_idx)
                    .map(|(_, ts)| ts.as_slice()).unwrap_or(&[]);
                let table_label = tables_in_schema.get(self.add_table_idx)
                    .map(|(t, _)| t.as_str()).unwrap_or("table");
                ComboBox::from_id_source("jb_add_table")
                    .selected_text(table_label)
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        ScrollArea::vertical()
                            .max_height(300.0)
                            .id_source("jb_add_table_scroll")
                            .show(ui, |ui| {
                                for (i, (t, _)) in tables_in_schema.iter().enumerate() {
                                    if ui.selectable_label(self.add_table_idx == i, t.as_str()).clicked() {
                                        self.add_table_idx = i;
                                    }
                                }
                            });
                    });

                if ui.button("＋  Add Table").clicked() {
                    if let Some((schema, tables)) = self.available.get(self.add_schema_idx) {
                        if let Some((table, cols)) = tables.get(self.add_table_idx) {
                            let key = (schema.clone(), table.clone());
                            // If columns not cached, request them (once).
                            if cols.is_empty() && !self.requested.contains(&key) {
                                self.requested.insert(key.clone());
                                actions.push(JoinAction::LoadDetails {
                                    schema: schema.clone(),
                                    table:  table.clone(),
                                });
                            }
                            let alias = format!("t{}", self.tables.len());
                            self.tables.push(TableEntry {
                                schema:  schema.clone(),
                                table:   table.clone(),
                                alias,
                                columns: cols.clone(),
                            });
                            if self.tables.len() == 2 && self.conditions.is_empty() {
                                self.conditions.push(Condition {
                                    left_idx:  0, left_col:  String::new(),
                                    right_idx: 1, right_col: String::new(),
                                    join_type: JoinType::Inner,
                                });
                            }
                        }
                    }
                }
            });

            ui.add_space(10.0);

            // ── 2. Join conditions ────────────────────────────────────────────
            section_header(ui, "JOIN CONDITIONS");

            if self.tables.len() < 2 {
                ui.label(
                    RichText::new("Add at least 2 tables above.")
                        .italics()
                        .color(Color32::from_gray(100)),
                );
            } else {
                let tc = self.tables.len();
                let aliases:  Vec<String>       = self.tables.iter().map(|t| t.alias.clone()).collect();
                let all_cols: Vec<Vec<String>>  = self.tables.iter().map(|t| t.columns.clone()).collect();

                let mut remove_cond: Option<usize> = None;

                for (ci, cond) in self.conditions.iter_mut().enumerate() {
                    cond.left_idx  = cond.left_idx.min(tc - 1);
                    cond.right_idx = cond.right_idx.min(tc - 1);

                    let lcols = all_cols.get(cond.left_idx).cloned().unwrap_or_default();
                    let rcols = all_cols.get(cond.right_idx).cloned().unwrap_or_default();

                    ui.horizontal(|ui| {
                        ComboBox::from_id_source(("jb_lt", ci))
                            .selected_text(aliases.get(cond.left_idx).map(|s| s.as_str()).unwrap_or(""))
                            .width(72.0)
                            .show_ui(ui, |ui| {
                                for (i, a) in aliases.iter().enumerate() {
                                    ui.selectable_value(&mut cond.left_idx, i, a.as_str());
                                }
                            });
                        ui.label("·");
                        col_picker(ui, ("jb_lc", ci), &mut cond.left_col, &lcols);

                        ComboBox::from_id_source(("jb_jt", ci))
                            .selected_text(cond.join_type.label())
                            .width(118.0)
                            .show_ui(ui, |ui| {
                                for jt in [JoinType::Inner, JoinType::Left, JoinType::Right, JoinType::Full] {
                                    let lbl = jt.label();
                                    ui.selectable_value(&mut cond.join_type, jt, lbl);
                                }
                            });

                        ComboBox::from_id_source(("jb_rt", ci))
                            .selected_text(aliases.get(cond.right_idx).map(|s| s.as_str()).unwrap_or(""))
                            .width(72.0)
                            .show_ui(ui, |ui| {
                                for (i, a) in aliases.iter().enumerate() {
                                    ui.selectable_value(&mut cond.right_idx, i, a.as_str());
                                }
                            });
                        ui.label("·");
                        col_picker(ui, ("jb_rc", ci), &mut cond.right_col, &rcols);

                        if ui.small_button("✕").clicked() {
                            remove_cond = Some(ci);
                        }
                    });
                }

                if let Some(i) = remove_cond {
                    self.conditions.remove(i);
                }

                if ui.button("＋  Add Join Condition").clicked() {
                    self.conditions.push(Condition {
                        left_idx:  0, left_col:  String::new(),
                        right_idx: (tc - 1).min(1), right_col: String::new(),
                        join_type: JoinType::Inner,
                    });
                }
            }

            ui.add_space(10.0);

            // ── 3. SQL preview ────────────────────────────────────────────────
            section_header(ui, "GENERATED SQL");

            let sql = self.generate_sql();
            let mut sql_display = sql.clone();
            ScrollArea::vertical()
                .max_height(140.0)
                .id_source("jb_sql_scroll")
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut sql_display)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                let has_sql = !sql.trim().is_empty();

                if ui.add_enabled(has_sql, egui::Button::new("▶  Run"))
                    .on_hover_text("Execute and show results").clicked()
                {
                    actions.push(JoinAction::Run(sql.clone()));
                    self.open = false;
                }
                if ui.add_enabled(has_sql, egui::Button::new("✎  Send to Editor"))
                    .on_hover_text("Paste SQL into the active query tab").clicked()
                {
                    actions.push(JoinAction::SendToEditor(sql.clone()));
                    self.open = false;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() { self.open = false; }
                    if ui.button("↺  Reset").on_hover_text("Clear all").clicked() {
                        self.tables.clear();
                        self.conditions.clear();
                        self.requested.clear();
                    }
                });
            });
        });
    }

    // ── SQL generator ─────────────────────────────────────────────────────────

    fn generate_sql(&self) -> String {
        if self.tables.is_empty() { return String::new(); }

        let first  = &self.tables[0];
        let select: Vec<String> = self.tables.iter().map(|t| format!("{}.*", t.alias)).collect();

        let mut sql = format!(
            "SELECT {}\nFROM \"{}\".\"{}\" AS {}",
            select.join(", "),
            first.schema, first.table, first.alias,
        );

        for c in &self.conditions {
            let (Some(lt), Some(rt)) = (self.tables.get(c.left_idx), self.tables.get(c.right_idx))
            else { continue };
            let lc = if c.left_col.is_empty()  { "?" } else { &c.left_col };
            let rc = if c.right_col.is_empty() { "?" } else { &c.right_col };
            sql.push_str(&format!(
                "\n{} \"{}\".\"{}\" AS {}\n    ON {}.{} = {}.{}",
                c.join_type.label(),
                rt.schema, rt.table, rt.alias,
                lt.alias, lc, rt.alias, rc,
            ));
        }
        sql.push(';');
        sql
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn section_header(ui: &mut egui::Ui, label: &str) {
    ui.label(RichText::new(label).small().strong().color(Color32::from_gray(140)));
    ui.separator();
}

fn col_picker(
    ui:    &mut egui::Ui,
    id:    impl std::hash::Hash,
    value: &mut String,
    cols:  &[String],
) {
    if cols.is_empty() {
        ui.add(TextEdit::singleline(value).desired_width(110.0).hint_text("column"));
    } else {
        ComboBox::from_id_source(id)
            .selected_text(if value.is_empty() { "column" } else { value.as_str() })
            .width(130.0)
            .show_ui(ui, |ui| {
                for col in cols {
                    ui.selectable_value(value, col.clone(), col.as_str());
                }
            });
    }
}
