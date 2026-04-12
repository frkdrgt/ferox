use egui::{Color32, RichText};

use crate::db::metadata::ColumnInfo;
use crate::i18n::I18n;

const PG_TYPES: &[&str] = &[
    "integer",
    "bigint",
    "smallint",
    "serial",
    "bigserial",
    "text",
    "varchar(255)",
    "char(1)",
    "boolean",
    "numeric",
    "real",
    "double precision",
    "timestamp",
    "timestamptz",
    "date",
    "time",
    "uuid",
    "json",
    "jsonb",
    "bytea",
];

// ── Column definition (for new/add columns) ───────────────────────────────────

#[derive(Debug, Clone)]
struct ColumnDef {
    name: String,
    data_type: String,
    not_null: bool,
    default_val: String,
    is_primary_key: bool,
}

impl Default for ColumnDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            data_type: "text".to_owned(),
            not_null: false,
            default_val: String::new(),
            is_primary_key: false,
        }
    }
}

// ── Existing column edit state (for Edit mode) ────────────────────────────────

#[derive(Debug, Clone)]
struct ExistingColEdit {
    col: ColumnInfo,
    mark_drop: bool,
    not_null: bool,
}

// ── Dialog mode ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    New,
    Edit,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Action the dialog emits back to the app when the user commits changes.
#[derive(Debug)]
pub enum TableDialogAction {
    ExecuteDdl { sql: String, refresh_schema: String },
}

#[derive(Debug)]
pub struct TableDialog {
    pub open: bool,
    mode: Mode,
    schema: String,
    table_name: String,
    schemas: Vec<String>,
    // New table mode
    columns: Vec<ColumnDef>,
    // Edit table mode
    existing: Vec<ExistingColEdit>,
    new_cols: Vec<ColumnDef>,
    // Shared UI state
    error: String,
    show_preview: bool,
}

impl Default for TableDialog {
    fn default() -> Self {
        Self {
            open: false,
            mode: Mode::New,
            schema: String::new(),
            table_name: String::new(),
            schemas: Vec::new(),
            columns: vec![ColumnDef::default()],
            existing: Vec::new(),
            new_cols: Vec::new(),
            error: String::new(),
            show_preview: false,
        }
    }
}

impl TableDialog {
    pub fn open_new(&mut self, schema: String, schemas: Vec<String>) {
        *self = Self::default();
        self.open = true;
        self.mode = Mode::New;
        self.schema = schema;
        self.schemas = schemas;
    }

    pub fn open_edit(
        &mut self,
        schema: String,
        table: String,
        existing_columns: Vec<ColumnInfo>,
        schemas: Vec<String>,
    ) {
        *self = Self::default();
        self.open = true;
        self.mode = Mode::Edit;
        self.schema = schema;
        self.table_name = table;
        self.existing = existing_columns
            .into_iter()
            .map(|col| {
                let not_null = !col.is_nullable;
                ExistingColEdit { col, mark_drop: false, not_null }
            })
            .collect();
        self.schemas = schemas;
    }

    /// Call once per frame from the egui update loop.
    pub fn show(&mut self, ctx: &egui::Context, i18n: &I18n) -> Option<TableDialogAction> {
        if !self.open {
            return None;
        }

        let title = if self.mode == Mode::New { i18n.td_title_new() } else { i18n.td_title_edit() };
        let mut open = self.open;
        let mut pending: Option<TableDialogAction> = None;

        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .min_width(560.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                pending = match self.mode {
                    Mode::New => self.show_new(ui, i18n),
                    Mode::Edit => self.show_edit(ui, i18n),
                };
            });

        if !open {
            self.open = false;
        }
        if pending.is_some() {
            self.open = false;
        }

        pending
    }

    // ── New Table ─────────────────────────────────────────────────────────────

    fn show_new(&mut self, ui: &mut egui::Ui, i18n: &I18n) -> Option<TableDialogAction> {
        egui::Grid::new("nt_meta")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label(i18n.td_label_schema());
                egui::ComboBox::from_id_source("nt_schema")
                    .selected_text(&self.schema)
                    .show_ui(ui, |ui| {
                        for s in self.schemas.clone() {
                            ui.selectable_value(&mut self.schema, s.clone(), s);
                        }
                    });
                ui.end_row();

                ui.label(i18n.td_label_table_name());
                ui.add(
                    egui::TextEdit::singleline(&mut self.table_name)
                        .desired_width(220.0)
                        .hint_text(i18n.td_hint_table_name()),
                );
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(i18n.td_lbl_columns()).strong());
            if ui.small_button(i18n.td_btn_add_column()).clicked() {
                self.columns.push(ColumnDef::default());
                self.error.clear();
            }
        });
        ui.add_space(4.0);

        render_column_defs(ui, "nc", &mut self.columns, true, i18n);
        self.render_preview(ui, true, i18n);

        if !self.error.is_empty() {
            ui.add_space(4.0);
            ui.colored_label(Color32::from_rgb(230, 80, 80), &self.error.clone());
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        let mut result = None;
        ui.horizontal(|ui| {
            if ui.button(i18n.btn_cancel()).clicked() {
                self.open = false;
            }
            ui.add_space(8.0);
            if ui.button(RichText::new(i18n.td_btn_create()).strong()).clicked() {
                match self.validate_and_build_create(i18n) {
                    Ok(sql) => {
                        result = Some(TableDialogAction::ExecuteDdl {
                            refresh_schema: self.schema.clone(),
                            sql,
                        });
                    }
                    Err(e) => {
                        self.error = e;
                    }
                }
            }
        });
        result
    }

    // ── Edit Table ────────────────────────────────────────────────────────────

    fn show_edit(&mut self, ui: &mut egui::Ui, i18n: &I18n) -> Option<TableDialogAction> {
        ui.horizontal(|ui| {
            ui.label(RichText::new(i18n.td_lbl_table_label()).strong());
            ui.label(
                RichText::new(format!("{}.{}", self.schema, self.table_name))
                    .monospace()
                    .color(Color32::from_rgb(86, 156, 214)),
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new(i18n.td_lbl_existing_cols()).strong());
        ui.add_space(4.0);

        if self.existing.is_empty() {
            ui.label(
                RichText::new(i18n.td_lbl_no_cols())
                    .small()
                    .color(Color32::from_gray(100)),
            );
        } else {
            // Header + data in ONE grid so columns align.
            egui::ScrollArea::vertical()
                .id_source("ec_scroll")
                .max_height(200.0)
                .show(ui, |ui| {
                    egui::Grid::new("ec_grid")
                        .num_columns(4)
                        .spacing([8.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // Header row
                            let hc = Color32::from_gray(150);
                            ui.add(egui::Label::new(
                                RichText::new(i18n.td_col_name()).small().strong().color(hc),
                            ).truncate());
                            ui.add_sized(
                                [140.0, 14.0],
                                egui::Label::new(
                                    RichText::new(i18n.td_col_type()).small().strong().color(hc),
                                ),
                            );
                            ui.add_sized(
                                [58.0, 14.0],
                                egui::Label::new(
                                    RichText::new("NOT NULL").small().strong().color(hc),
                                ),
                            );
                            ui.add_sized(
                                [36.0, 14.0],
                                egui::Label::new(
                                    RichText::new(i18n.td_col_drop())
                                        .small()
                                        .strong()
                                        .color(Color32::from_rgb(180, 70, 70)),
                                ),
                            );
                            ui.end_row();

                            // Data rows
                            for ec in self.existing.iter_mut() {
                                let name_color = if ec.mark_drop {
                                    Color32::from_gray(90)
                                } else {
                                    Color32::from_gray(210)
                                };
                                let type_color = if ec.mark_drop {
                                    Color32::from_gray(70)
                                } else {
                                    Color32::from_gray(160)
                                };

                                let name_rt = {
                                    let rt = RichText::new(&ec.col.name)
                                        .monospace()
                                        .small()
                                        .color(name_color);
                                    if ec.mark_drop { rt.strikethrough() } else { rt }
                                };
                                ui.label(name_rt);

                                ui.label(
                                    RichText::new(&ec.col.data_type)
                                        .monospace()
                                        .small()
                                        .color(type_color),
                                );

                                if ec.mark_drop {
                                    ui.centered_and_justified(|ui| {
                                        ui.label(
                                            RichText::new("—")
                                                .small()
                                                .color(Color32::from_gray(70)),
                                        );
                                    });
                                } else {
                                    ui.centered_and_justified(|ui| {
                                        ui.checkbox(&mut ec.not_null, "");
                                    });
                                }

                                ui.centered_and_justified(|ui| {
                                    ui.checkbox(&mut ec.mark_drop, "");
                                });
                                ui.end_row();
                            }
                        });
                });
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new(i18n.td_lbl_add_columns()).strong());
            if ui.small_button(i18n.td_btn_add_column()).clicked() {
                self.new_cols.push(ColumnDef::default());
                self.error.clear();
            }
        });

        if self.new_cols.is_empty() {
            ui.add_space(2.0);
            ui.label(
                RichText::new(i18n.td_lbl_no_new_cols())
                    .small()
                    .color(Color32::from_gray(100)),
            );
        } else {
            ui.add_space(4.0);
            render_column_defs(ui, "ac", &mut self.new_cols, false, i18n);
        }

        self.render_preview(ui, false, i18n);

        if !self.error.is_empty() {
            ui.add_space(4.0);
            ui.colored_label(Color32::from_rgb(230, 80, 80), &self.error.clone());
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        let mut result = None;
        ui.horizontal(|ui| {
            if ui.button(i18n.btn_cancel()).clicked() {
                self.open = false;
            }
            ui.add_space(8.0);
            if ui.button(RichText::new(i18n.td_btn_apply()).strong()).clicked() {
                match self.validate_and_build_alter(i18n) {
                    Ok(sql) => {
                        result = Some(TableDialogAction::ExecuteDdl {
                            refresh_schema: self.schema.clone(),
                            sql,
                        });
                    }
                    Err(e) => {
                        self.error = e;
                    }
                }
            }
        });
        result
    }

    // ── Preview / error rendering ─────────────────────────────────────────────

    fn render_preview(&mut self, ui: &mut egui::Ui, is_new: bool, i18n: &I18n) {
        ui.add_space(6.0);
        if ui
            .small_button(if self.show_preview { i18n.td_btn_preview_hide() } else { i18n.td_btn_preview_show() })
            .clicked()
        {
            self.show_preview = !self.show_preview;
        }
        if self.show_preview {
            let sql =
                if is_new { self.build_create_sql() } else { self.build_alter_sql() };
            let mut preview = sql;
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::multiline(&mut preview)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(5)
                    .interactive(false),
            );
        }
    }

    // ── Validation ────────────────────────────────────────────────────────────

    fn validate_and_build_create(&mut self, i18n: &I18n) -> Result<String, String> {
        self.error.clear();
        if self.table_name.trim().is_empty() {
            return Err(i18n.td_err_table_name_required().to_owned());
        }
        for (i, col) in self.columns.iter().enumerate() {
            if col.name.trim().is_empty() {
                return Err(i18n.td_err_col_name_required(i + 1));
            }
            if col.data_type.trim().is_empty() {
                return Err(i18n.td_err_col_no_type(&col.name));
            }
        }
        Ok(self.build_create_sql())
    }

    fn validate_and_build_alter(&mut self, i18n: &I18n) -> Result<String, String> {
        self.error.clear();
        for col in &self.new_cols {
            if col.name.trim().is_empty() {
                return Err(i18n.td_err_new_col_name_required().to_owned());
            }
            if col.data_type.trim().is_empty() {
                return Err(i18n.td_err_col_no_type(&col.name));
            }
        }
        let sql = self.build_alter_sql();
        if sql.trim().is_empty() {
            return Err(i18n.td_err_no_changes().to_owned());
        }
        Ok(sql)
    }

    // ── SQL builders ──────────────────────────────────────────────────────────

    fn build_create_sql(&self) -> String {
        let mut col_defs: Vec<String> = Vec::new();
        let mut pk_cols: Vec<String> = Vec::new();

        for col in &self.columns {
            if col.name.trim().is_empty() {
                continue;
            }
            let mut def = format!("    \"{}\" {}", col.name.trim(), col.data_type);
            if col.not_null {
                def.push_str(" NOT NULL");
            }
            if !col.default_val.trim().is_empty() {
                def.push_str(&format!(" DEFAULT {}", col.default_val.trim()));
            }
            col_defs.push(def);
            if col.is_primary_key {
                pk_cols.push(format!("\"{}\"", col.name.trim()));
            }
        }

        if !pk_cols.is_empty() {
            col_defs.push(format!("    PRIMARY KEY ({})", pk_cols.join(", ")));
        }

        format!(
            "CREATE TABLE \"{}\".\"{}\" (\n{}\n);",
            self.schema,
            self.table_name.trim(),
            col_defs.join(",\n")
        )
    }

    fn build_alter_sql(&self) -> String {
        let mut ops: Vec<String> = Vec::new();

        for ec in &self.existing {
            if ec.mark_drop {
                ops.push(format!("    DROP COLUMN \"{}\"", ec.col.name));
            } else {
                let was_not_null = !ec.col.is_nullable;
                if ec.not_null && !was_not_null {
                    ops.push(format!(
                        "    ALTER COLUMN \"{}\" SET NOT NULL",
                        ec.col.name
                    ));
                } else if !ec.not_null && was_not_null {
                    ops.push(format!(
                        "    ALTER COLUMN \"{}\" DROP NOT NULL",
                        ec.col.name
                    ));
                }
            }
        }

        for col in &self.new_cols {
            if col.name.trim().is_empty() {
                continue;
            }
            let mut def =
                format!("    ADD COLUMN \"{}\" {}", col.name.trim(), col.data_type);
            if col.not_null {
                def.push_str(" NOT NULL");
            }
            if !col.default_val.trim().is_empty() {
                def.push_str(&format!(" DEFAULT {}", col.default_val.trim()));
            }
            ops.push(def);
        }

        if ops.is_empty() {
            return String::new();
        }

        format!(
            "ALTER TABLE \"{}\".\"{}\"\n{};",
            self.schema,
            self.table_name,
            ops.join(",\n")
        )
    }
}

// ── Free function: column definition grid ────────────────────────────────────

/// Renders an editable list of column definitions.
/// Header and data rows share **one** Grid so columns always align.
/// `show_pk` = true for New Table mode (shows PK checkbox).
/// `show_pk` = false for Edit Table "add columns" panel.
fn render_column_defs(
    ui: &mut egui::Ui,
    id_prefix: &str,
    columns: &mut Vec<ColumnDef>,
    show_pk: bool,
    i18n: &I18n,
) {
    let num_cols = if show_pk { 6 } else { 5 };
    let mut to_delete: Option<usize> = None;
    // Collect "PK just turned on" indices to apply not_null next frame.
    let mut set_not_null: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_source(format!("{id_prefix}_scroll"))
        .max_height(220.0)
        .show(ui, |ui| {
            egui::Grid::new(format!("{id_prefix}_grid"))
                .num_columns(num_cols)
                .spacing([8.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    // ── Header row ────────────────────────────────────────
                    let hc = Color32::from_gray(150);
                    ui.add(
                        egui::Label::new(
                            RichText::new(i18n.td_col_name()).small().strong().color(hc),
                        )
                        .truncate(),
                    );
                    // Force Type column to match ComboBox minimum width
                    ui.add_sized(
                        [140.0, 14.0],
                        egui::Label::new(
                            RichText::new(i18n.td_col_type()).small().strong().color(hc),
                        ),
                    );
                    ui.add_sized(
                        [58.0, 14.0],
                        egui::Label::new(
                            RichText::new("NOT NULL").small().strong().color(hc),
                        ),
                    );
                    if show_pk {
                        ui.add_sized(
                            [28.0, 14.0],
                            egui::Label::new(
                                RichText::new("PK").small().strong().color(hc),
                            ),
                        );
                    }
                    ui.add_sized(
                        [88.0, 14.0],
                        egui::Label::new(
                            RichText::new(i18n.td_col_default()).small().strong().color(hc),
                        ),
                    );
                    ui.label(""); // delete col
                    ui.end_row();

                    // ── Data rows ─────────────────────────────────────────
                    let col_count = columns.len();
                    for (i, col) in columns.iter_mut().enumerate() {
                        ui.add(
                            egui::TextEdit::singleline(&mut col.name)
                                .desired_width(108.0)
                                .hint_text("column_name"),
                        );

                        egui::ComboBox::from_id_source(format!("{id_prefix}_type_{i}"))
                            .selected_text(&col.data_type)
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                for t in PG_TYPES {
                                    ui.selectable_value(
                                        &mut col.data_type,
                                        t.to_string(),
                                        *t,
                                    );
                                }
                            });

                        ui.centered_and_justified(|ui| {
                            ui.checkbox(&mut col.not_null, "");
                        });

                        if show_pk {
                            ui.centered_and_justified(|ui| {
                                let was_pk = col.is_primary_key;
                                ui.checkbox(&mut col.is_primary_key, "");
                                if col.is_primary_key && !was_pk {
                                    set_not_null = Some(i);
                                }
                            });
                        }

                        ui.add(
                            egui::TextEdit::singleline(&mut col.default_val)
                                .desired_width(88.0)
                                .hint_text("default"),
                        );

                        if !show_pk || col_count > 1 {
                            if ui.small_button("✕").clicked() {
                                to_delete = Some(i);
                            }
                        } else {
                            ui.label("");
                        }

                        ui.end_row();
                    }
                });
        });

    if let Some(idx) = set_not_null {
        columns[idx].not_null = true;
    }
    if let Some(idx) = to_delete {
        columns.remove(idx);
    }
}
