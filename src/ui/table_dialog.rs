use egui::{Color32, RichText};

use crate::db::metadata::{ColumnInfo, IndexInfo};
use crate::db::query::quote_ident;
use crate::i18n::I18n;
use crate::ui::syntax::highlight_sql;

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
    /// The column as loaded — every edit is diffed against this.
    col: ColumnInfo,
    name: String,
    data_type: String,
    not_null: bool,
    default_val: String,
    mark_drop: bool,
}

impl ExistingColEdit {
    fn new(col: ColumnInfo) -> Self {
        Self {
            name: col.name.clone(),
            data_type: col.full_type.clone(),
            not_null: !col.is_nullable,
            default_val: col.column_default.clone().unwrap_or_default(),
            mark_drop: false,
            col,
        }
    }
    fn renamed(&self) -> bool {
        !self.mark_drop && self.name.trim() != self.col.name
    }
    fn type_changed(&self) -> bool {
        !self.mark_drop && self.data_type.trim() != self.col.full_type
    }
    fn default_changed(&self) -> bool {
        !self.mark_drop
            && self.default_val.trim() != self.col.column_default.as_deref().unwrap_or("").trim()
    }
}

#[derive(Debug, Clone)]
struct ExistingIndexEdit {
    idx: IndexInfo,
    mark_drop: bool,
}

#[derive(Debug, Clone)]
struct NewIndex {
    /// Empty = let Postgres pick the name.
    name: String,
    /// Column list or expressions, used verbatim (`email`, `lower(email)`, `a, b DESC`).
    columns: String,
    unique: bool,
    method: String,
}

impl Default for NewIndex {
    fn default() -> Self {
        Self { name: String::new(), columns: String::new(), unique: false, method: "btree".to_owned() }
    }
}

const INDEX_METHODS: &[&str] = &["btree", "hash", "gin", "gist", "brin"];

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
    /// Run the DDL. The dialog stays open (`running`) until the app reports
    /// success (`finish_ddl`) or the error (`ddl_failed`), so a failed ALTER
    /// doesn't throw away the user's edits. `edited` = (schema, final table
    /// name) in edit mode, so the app can refresh that table's cached details.
    ExecuteDdl { sql: String, refresh_schema: String, edited: Option<(String, String)> },
    /// Put the generated DDL into a new query tab instead of running it.
    OpenInEditor(String),
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
    /// Target name for RENAME TO (starts as `table_name`).
    new_table_name: String,
    indexes: Vec<ExistingIndexEdit>,
    new_indexes: Vec<NewIndex>,
    // Shared UI state
    error: String,
    show_preview: bool,
    /// DDL sent, waiting for DdlDone / an error.
    pub running: bool,
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
            new_table_name: String::new(),
            indexes: Vec::new(),
            new_indexes: Vec::new(),
            error: String::new(),
            show_preview: false,
            running: false,
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
        existing_indexes: Vec<IndexInfo>,
        schemas: Vec<String>,
    ) {
        *self = Self::default();
        self.open = true;
        self.mode = Mode::Edit;
        self.schema = schema;
        self.new_table_name = table.clone();
        self.table_name = table;
        self.existing = existing_columns.into_iter().map(ExistingColEdit::new).collect();
        self.indexes = existing_indexes
            .into_iter()
            .map(|idx| ExistingIndexEdit { idx, mark_drop: false })
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
        match &pending {
            Some(TableDialogAction::ExecuteDdl { .. }) => self.running = true,
            Some(TableDialogAction::OpenInEditor(_)) => self.open = false,
            None => {}
        }

        pending
    }

    /// The DDL this dialog sent succeeded — close it.
    pub fn finish_ddl(&mut self) {
        if self.running {
            self.running = false;
            self.open = false;
        }
    }

    /// The DDL this dialog sent failed — keep the edits and show why.
    pub fn ddl_failed(&mut self, msg: &str) {
        if self.running {
            self.running = false;
            self.error = msg.to_owned();
        }
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
        self.render_preview(ui, i18n);

        if !self.error.is_empty() {
            ui.add_space(4.0);
            ui.colored_label(crate::ui::theme::DANGER, &self.error.clone());
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
            if ui
                .add_enabled(!self.running, egui::Button::new(RichText::new(i18n.td_btn_create()).strong()))
                .clicked()
            {
                match self.validate_and_build_create(i18n) {
                    Ok(sql) => {
                        result = Some(TableDialogAction::ExecuteDdl {
                            refresh_schema: self.schema.clone(),
                            sql,
                            edited: None,
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
        let accent = Color32::from_rgb(86, 156, 214);
        let hc = Color32::from_gray(150);

        ui.horizontal(|ui| {
            ui.label(RichText::new(i18n.td_lbl_table_label()).strong());
            ui.label(RichText::new(format!("{}.", self.schema)).monospace().color(accent));
            let changed = self.new_table_name.trim() != self.table_name;
            ui.add(
                egui::TextEdit::singleline(&mut self.new_table_name)
                    .desired_width(200.0)
                    .font(egui::TextStyle::Monospace)
                    .text_color_opt(changed.then_some(accent)),
            )
            .on_hover_text(i18n.td_hover_rename_table());
        });

        // ── Existing columns ─────────────────────────────────────────────────
        ui.add_space(8.0);
        ui.label(RichText::new(i18n.td_lbl_existing_cols()).strong());
        ui.add_space(4.0);
        if self.existing.is_empty() {
            ui.label(RichText::new(i18n.td_lbl_no_cols()).small().color(Color32::from_gray(100)));
        } else {
            egui::ScrollArea::vertical()
                .id_source("ec_scroll")
                .max_height(220.0)
                .show(ui, |ui| {
                    egui::Grid::new("ec_grid")
                        .num_columns(5)
                        .spacing([8.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for h in [i18n.td_col_name(), i18n.td_col_type(), "NOT NULL", i18n.td_col_default()] {
                                ui.label(RichText::new(h).small().strong().color(hc));
                            }
                            ui.label(
                                RichText::new(i18n.td_col_drop())
                                    .small()
                                    .strong()
                                    .color(Color32::from_rgb(180, 70, 70)),
                            );
                            ui.end_row();

                            for (i, ec) in self.existing.iter_mut().enumerate() {
                                let enabled = !ec.mark_drop;
                                let tint = |changed: bool| changed.then_some(accent);
                                let renamed = ec.renamed();
                                let type_changed = ec.type_changed();
                                let default_changed = ec.default_changed();
                                ui.add_enabled_ui(enabled, |ui| {
                                    ui.add_sized(
                                        [130.0, 20.0],
                                        egui::TextEdit::singleline(&mut ec.name)
                                            .font(egui::TextStyle::Monospace)
                                            .text_color_opt(tint(renamed)),
                                    );
                                });
                                ui.add_enabled_ui(enabled, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [170.0, 20.0],
                                            egui::TextEdit::singleline(&mut ec.data_type)
                                                .font(egui::TextStyle::Monospace)
                                                .text_color_opt(tint(type_changed)),
                                        );
                                        type_picker(ui, &format!("ec_type_{i}"), &mut ec.data_type);
                                    });
                                });
                                ui.add_enabled_ui(enabled, |ui| ui.checkbox(&mut ec.not_null, ""));
                                ui.add_enabled_ui(enabled, |ui| {
                                    ui.add_sized(
                                        [190.0, 20.0],
                                        egui::TextEdit::singleline(&mut ec.default_val)
                                            .font(egui::TextStyle::Monospace)
                                            .hint_text("—")
                                            .text_color_opt(tint(default_changed)),
                                    );
                                });
                                ui.checkbox(&mut ec.mark_drop, "");
                                ui.end_row();
                            }
                        });
                });
        }

        // ── New columns ──────────────────────────────────────────────────────
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(i18n.td_lbl_add_columns()).strong());
            if ui.small_button(i18n.td_btn_add_column()).clicked() {
                self.new_cols.push(ColumnDef::default());
                self.error.clear();
            }
        });
        if !self.new_cols.is_empty() {
            ui.add_space(4.0);
            render_column_defs(ui, "ac", &mut self.new_cols, false, i18n);
        }

        // ── Indexes ──────────────────────────────────────────────────────────
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(i18n.td_lbl_indexes()).strong());
            if ui.small_button(i18n.td_btn_add_index()).clicked() {
                self.new_indexes.push(NewIndex::default());
                self.error.clear();
            }
        });
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_source("ix_scroll")
            .max_height(160.0)
            .show(ui, |ui| {
                egui::Grid::new("ix_grid")
                    .num_columns(3)
                    .spacing([8.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for ix in self.indexes.iter_mut() {
                            let name = if let Some(c) = &ix.idx.constraint {
                                format!("{} ({})", ix.idx.name, i18n.td_lbl_constraint(c))
                            } else {
                                ix.idx.name.clone()
                            };
                            let rt = RichText::new(name).monospace().small();
                            ui.label(if ix.mark_drop { rt.strikethrough() } else { rt });
                            let def = index_column_part(&ix.idx.definition);
                            ui.add_sized(
                                [240.0, 16.0],
                                egui::Label::new(
                                    RichText::new(def).monospace().small().color(Color32::from_gray(150)),
                                )
                                .truncate(),
                            )
                            .on_hover_text(&ix.idx.definition);
                            ui.checkbox(&mut ix.mark_drop, i18n.td_col_drop());
                            ui.end_row();
                        }
                    });
            });

        let column_names: Vec<String> = self.existing.iter().filter(|c| !c.mark_drop).map(|c| c.name.trim().to_owned()).collect();
        let mut remove_new_index = None;
        if !self.new_indexes.is_empty() {
            egui::Grid::new("nix_grid")
                .num_columns(5)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    for h in [i18n.td_col_name(), i18n.td_col_index_cols(), "UNIQUE", i18n.td_col_method(), ""] {
                        ui.label(RichText::new(h).small().strong().color(hc));
                    }
                    ui.end_row();
                    for (i, ni) in self.new_indexes.iter_mut().enumerate() {
                        ui.add_sized(
                            [130.0, 20.0],
                            egui::TextEdit::singleline(&mut ni.name).hint_text(i18n.td_hint_index_auto_name()),
                        );
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [190.0, 20.0],
                                egui::TextEdit::singleline(&mut ni.columns)
                                    .font(egui::TextStyle::Monospace)
                                    .hint_text("col1, col2"),
                            );
                            ui.menu_button("+", |ui| {
                                for c in &column_names {
                                    if ui.button(c).clicked() {
                                        if !ni.columns.trim().is_empty() {
                                            ni.columns.push_str(", ");
                                        }
                                        ni.columns.push_str(&quote_ident(c));
                                        ui.close_menu();
                                    }
                                }
                            });
                        });
                        ui.checkbox(&mut ni.unique, "");
                        egui::ComboBox::from_id_source(format!("nix_method_{i}"))
                            .selected_text(&ni.method)
                            .width(70.0)
                            .show_ui(ui, |ui| {
                                for m in INDEX_METHODS {
                                    ui.selectable_value(&mut ni.method, m.to_string(), *m);
                                }
                            });
                        if ui.small_button(crate::ui::theme::icon::X).clicked() {
                            remove_new_index = Some(i);
                        }
                        ui.end_row();
                    }
                });
        }
        if let Some(i) = remove_new_index {
            self.new_indexes.remove(i);
        }

        // ── DDL preview (always visible) ─────────────────────────────────────
        ui.add_space(8.0);
        ui.separator();
        let statements = self.build_alter_statements();
        if statements.is_empty() {
            ui.label(RichText::new(i18n.td_lbl_no_changes_yet()).small().color(Color32::from_gray(110)));
        } else {
            ui.label(RichText::new(i18n.td_lbl_ddl_preview()).strong());
            let mut preview = statements.join("\n");
            let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                let job = highlight_sql(ui, text, wrap_width);
                ui.fonts(|f| f.layout_job(job))
            };
            egui::ScrollArea::vertical()
                .id_source("ddl_preview_scroll")
                .max_height(160.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut preview)
                            .desired_width(f32::INFINITY)
                            .desired_rows(3)
                            .interactive(false)
                            .layouter(&mut layouter),
                    );
                });
            if statements.len() > 1 {
                ui.label(RichText::new(i18n.td_note_single_transaction()).small().color(Color32::from_gray(130)));
            }
            let warnings = self.destructive_changes(i18n);
            if !warnings.is_empty() {
                ui.add_space(4.0);
                ui.colored_label(crate::ui::theme::DANGER, RichText::new(i18n.td_warn_header()).strong());
                for w in warnings {
                    ui.colored_label(crate::ui::theme::DANGER, format!("  • {w}"));
                }
            }
        }

        if !self.error.is_empty() {
            ui.add_space(4.0);
            ui.colored_label(crate::ui::theme::DANGER, &self.error.clone());
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
            let has_changes = !statements.is_empty() && !self.running;
            if ui
                .add_enabled(has_changes, egui::Button::new(i18n.td_btn_open_in_editor()))
                .clicked()
            {
                match self.validate_and_build_alter(i18n) {
                    Ok(sql) => result = Some(TableDialogAction::OpenInEditor(sql)),
                    Err(e) => self.error = e,
                }
            }
            if ui
                .add_enabled(has_changes, egui::Button::new(RichText::new(i18n.td_btn_apply()).strong()))
                .clicked()
            {
                match self.validate_and_build_alter(i18n) {
                    Ok(sql) => {
                        let final_name = self.new_table_name.trim().to_owned();
                        result = Some(TableDialogAction::ExecuteDdl {
                            refresh_schema: self.schema.clone(),
                            sql,
                            edited: Some((self.schema.clone(), final_name)),
                        });
                    }
                    Err(e) => self.error = e,
                }
            }
            if self.running {
                ui.spinner();
            }
        });
        result
    }

    // ── Preview / error rendering ─────────────────────────────────────────────

    fn render_preview(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        ui.add_space(6.0);
        if ui
            .small_button(if self.show_preview { i18n.td_btn_preview_hide() } else { i18n.td_btn_preview_show() })
            .clicked()
        {
            self.show_preview = !self.show_preview;
        }
        if self.show_preview {
            let mut preview = self.build_create_sql();
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
        if self.new_table_name.trim().is_empty() {
            return Err(i18n.td_err_table_name_required().to_owned());
        }
        for ec in &self.existing {
            if !ec.mark_drop && ec.name.trim().is_empty() {
                return Err(i18n.td_err_col_renamed_empty(&ec.col.name));
            }
            if !ec.mark_drop && ec.data_type.trim().is_empty() {
                return Err(i18n.td_err_col_no_type(&ec.col.name));
            }
        }
        for col in &self.new_cols {
            if col.name.trim().is_empty() {
                return Err(i18n.td_err_new_col_name_required().to_owned());
            }
            if col.data_type.trim().is_empty() {
                return Err(i18n.td_err_col_no_type(&col.name));
            }
        }
        if self.new_indexes.iter().any(|ni| ni.columns.trim().is_empty()) {
            return Err(i18n.td_err_index_cols_required().to_owned());
        }
        let statements = self.build_alter_statements();
        if statements.is_empty() {
            return Err(i18n.td_err_no_changes().to_owned());
        }
        Ok(statements.join("\n"))
    }

    /// Human-readable list of changes that lose data or may fail on existing rows.
    fn destructive_changes(&self, i18n: &I18n) -> Vec<String> {
        let mut out = Vec::new();
        for ec in &self.existing {
            if ec.mark_drop {
                out.push(i18n.td_warn_drop_col(&ec.col.name));
            } else if ec.type_changed() {
                out.push(i18n.td_warn_type_change(&ec.col.name, &ec.col.full_type, ec.data_type.trim()));
            } else if ec.not_null && ec.col.is_nullable {
                out.push(i18n.td_warn_set_not_null(&ec.col.name));
            }
        }
        for ix in self.indexes.iter().filter(|ix| ix.mark_drop) {
            out.push(i18n.td_warn_drop_index(ix.idx.constraint.as_deref().unwrap_or(&ix.idx.name)));
        }
        out
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

    /// DDL for every pending edit, in an order that's valid as one batch:
    /// index/constraint drops → one ALTER TABLE with all column subcommands
    /// (on the *old* column names) → column renames → new indexes (which may
    /// use the new names) → table rename last.
    fn build_alter_statements(&self) -> Vec<String> {
        let table = format!("{}.{}", quote_ident(&self.schema), quote_ident(&self.table_name));
        let mut stmts: Vec<String> = Vec::new();

        for ix in self.indexes.iter().filter(|ix| ix.mark_drop) {
            stmts.push(match &ix.idx.constraint {
                Some(c) => format!("ALTER TABLE {table} DROP CONSTRAINT {};", quote_ident(c)),
                None => format!("DROP INDEX {}.{};", quote_ident(&self.schema), quote_ident(&ix.idx.name)),
            });
        }

        let mut ops: Vec<String> = Vec::new();
        for ec in &self.existing {
            let col = quote_ident(&ec.col.name);
            if ec.mark_drop {
                ops.push(format!("    DROP COLUMN {col}"));
                continue;
            }
            if ec.type_changed() {
                let ty = ec.data_type.trim();
                ops.push(format!("    ALTER COLUMN {col} TYPE {ty} USING {col}::{ty}"));
            }
            let was_not_null = !ec.col.is_nullable;
            if ec.not_null && !was_not_null {
                ops.push(format!("    ALTER COLUMN {col} SET NOT NULL"));
            } else if !ec.not_null && was_not_null {
                ops.push(format!("    ALTER COLUMN {col} DROP NOT NULL"));
            }
            if ec.default_changed() {
                let d = ec.default_val.trim();
                if d.is_empty() {
                    ops.push(format!("    ALTER COLUMN {col} DROP DEFAULT"));
                } else {
                    ops.push(format!("    ALTER COLUMN {col} SET DEFAULT {d}"));
                }
            }
        }
        for col in &self.new_cols {
            if col.name.trim().is_empty() {
                continue;
            }
            let mut def = format!("    ADD COLUMN {} {}", quote_ident(col.name.trim()), col.data_type);
            if col.not_null {
                def.push_str(" NOT NULL");
            }
            if !col.default_val.trim().is_empty() {
                def.push_str(&format!(" DEFAULT {}", col.default_val.trim()));
            }
            ops.push(def);
        }
        if !ops.is_empty() {
            stmts.push(format!("ALTER TABLE {table}\n{};", ops.join(",\n")));
        }

        for ec in self.existing.iter().filter(|ec| ec.renamed() && !ec.name.trim().is_empty()) {
            stmts.push(format!(
                "ALTER TABLE {table} RENAME COLUMN {} TO {};",
                quote_ident(&ec.col.name),
                quote_ident(ec.name.trim())
            ));
        }

        for ni in self.new_indexes.iter().filter(|ni| !ni.columns.trim().is_empty()) {
            let unique = if ni.unique { "UNIQUE " } else { "" };
            let name = if ni.name.trim().is_empty() {
                String::new()
            } else {
                format!("{} ", quote_ident(ni.name.trim()))
            };
            stmts.push(format!(
                "CREATE {unique}INDEX {name}ON {table} USING {} ({});",
                ni.method,
                ni.columns.trim()
            ));
        }

        let new_name = self.new_table_name.trim();
        if !new_name.is_empty() && new_name != self.table_name {
            stmts.push(format!("ALTER TABLE {table} RENAME TO {};", quote_ident(new_name)));
        }
        stmts
    }
}

/// Small "▾" menu that fills `data_type` from the common type list — the
/// TextEdit next to it stays free-form for anything not listed.
fn type_picker(ui: &mut egui::Ui, id: &str, data_type: &mut String) {
    ui.push_id(id, |ui| {
        ui.menu_button("▾", |ui| {
            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                for t in PG_TYPES {
                    if ui.button(*t).clicked() {
                        *data_type = t.to_string();
                        ui.close_menu();
                    }
                }
            });
        });
    });
}

/// `CREATE INDEX x ON s.t USING btree (a, b)` → `btree (a, b)` for compact display.
fn index_column_part(definition: &str) -> &str {
    definition.find(" USING ").map_or(definition, |i| &definition[i + 7..])
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
                        ui.add_sized(
                            [130.0, 20.0],
                            egui::TextEdit::singleline(&mut col.name).hint_text("column_name"),
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
                            if ui.small_button(crate::ui::theme::icon::X).clicked() {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, full_type: &str, nullable: bool, default: Option<&str>) -> ColumnInfo {
        ColumnInfo {
            name: name.to_owned(),
            data_type: full_type.to_owned(),
            is_nullable: nullable,
            column_default: default.map(str::to_owned),
            full_type: full_type.to_owned(),
        }
    }

    fn dialog() -> TableDialog {
        let mut d = TableDialog::default();
        d.open_edit(
            "app".into(),
            "users".into(),
            vec![
                col("id", "integer", false, Some("nextval('users_id_seq'::regclass)")),
                col("email", "character varying(255)", true, None),
                col("age", "text", true, Some("'0'::text")),
                col("legacy", "text", true, None),
            ],
            vec![
                IndexInfo {
                    name: "users_pkey".into(),
                    is_unique: true,
                    definition: "CREATE UNIQUE INDEX users_pkey ON app.users USING btree (id)".into(),
                    constraint: Some("users_pkey".into()),
                },
                IndexInfo {
                    name: "users_email_idx".into(),
                    is_unique: false,
                    definition: "CREATE INDEX users_email_idx ON app.users USING btree (email)".into(),
                    constraint: None,
                },
            ],
            vec!["app".into()],
        );
        d
    }

    #[test]
    fn untouched_dialog_has_no_changes() {
        assert!(dialog().build_alter_statements().is_empty());
    }

    #[test]
    fn builds_statements_in_dependency_order() {
        let mut d = dialog();
        d.existing[1].name = "mail".into(); // rename
        d.existing[1].not_null = true;
        d.existing[2].data_type = "integer".into(); // type change
        d.existing[2].default_val = String::new(); // drop default
        d.existing[3].mark_drop = true;
        d.indexes[1].mark_drop = true;
        d.new_indexes.push(NewIndex { columns: "\"mail\"".into(), unique: true, ..NewIndex::default() });
        d.new_table_name = "people".into();

        let s = d.build_alter_statements();
        assert_eq!(
            s,
            vec![
                "DROP INDEX \"app\".\"users_email_idx\";".to_owned(),
                "ALTER TABLE \"app\".\"users\"\n    ALTER COLUMN \"email\" SET NOT NULL,\n    ALTER COLUMN \"age\" TYPE integer USING \"age\"::integer,\n    ALTER COLUMN \"age\" DROP DEFAULT,\n    DROP COLUMN \"legacy\";".to_owned(),
                "ALTER TABLE \"app\".\"users\" RENAME COLUMN \"email\" TO \"mail\";".to_owned(),
                "CREATE UNIQUE INDEX ON \"app\".\"users\" USING btree (\"mail\");".to_owned(),
                "ALTER TABLE \"app\".\"users\" RENAME TO \"people\";".to_owned(),
            ]
        );
        assert_eq!(d.destructive_changes(&I18n::new(crate::i18n::Lang::En)).len(), 4);
    }

    /// Runs the generated batch against a live Postgres (same env vars as
    /// `db::connection::tests`): `cargo test --bin ferox table_dialog -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn generated_ddl_applies_on_postgres() {
        let password = std::env::var("FEROX_TEST_PG_PASSWORD").unwrap_or_else(|_| "postgres".into());
        let (client, conn) = tokio_postgres::connect(
            &format!("host=localhost user=postgres password={password} dbname=postgres"),
            tokio_postgres::NoTls,
        )
        .await
        .expect("connect failed");
        tokio::spawn(conn);
        client
            .batch_execute(
                "DROP SCHEMA IF EXISTS app CASCADE; CREATE SCHEMA app;
                 CREATE TABLE app.users (id serial PRIMARY KEY, email varchar(255), age text DEFAULT '0', legacy text);
                 CREATE INDEX users_email_idx ON app.users (email);
                 INSERT INTO app.users (email, age) VALUES ('a@x', '41');",
            )
            .await
            .unwrap();

        let mut d = dialog();
        d.existing[1].name = "mail".into();
        d.existing[1].not_null = true;
        d.existing[2].data_type = "integer".into();
        d.existing[2].default_val = String::new();
        d.existing[3].mark_drop = true;
        d.indexes[1].mark_drop = true;
        d.new_indexes.push(NewIndex { columns: "\"mail\"".into(), unique: true, ..NewIndex::default() });
        d.new_table_name = "people".into();
        let sql = d.validate_and_build_alter(&I18n::new(crate::i18n::Lang::En)).unwrap();
        client.batch_execute(&sql).await.expect("generated DDL failed");

        let rows = client
            .query("SELECT mail, age + 1 FROM app.people", &[])
            .await
            .unwrap();
        assert_eq!(rows[0].get::<_, String>(0), "a@x");
        assert_eq!(rows[0].get::<_, i32>(1), 42);
        assert!(client.batch_execute("INSERT INTO app.people (mail) VALUES ('a@x')").await.is_err(), "unique index missing");

        // A failing batch leaves nothing half-applied.
        let mut d2 = TableDialog::default();
        d2.open_edit("app".into(), "people".into(),
            vec![col("mail", "character varying(255)", false, None)], vec![], vec!["app".into()]);
        d2.existing[0].data_type = "integer".into(); // 'a@x' can't be cast
        d2.new_table_name = "renamed".into();
        assert!(client.batch_execute(&d2.build_alter_statements().join("
")).await.is_err());
        assert!(client.query("SELECT 1 FROM app.people", &[]).await.is_ok(), "rename applied despite failure");

        client.batch_execute("DROP SCHEMA app CASCADE").await.unwrap();
    }

    #[test]
    fn constraint_backed_index_drops_the_constraint() {
        let mut d = dialog();
        d.indexes[0].mark_drop = true;
        assert_eq!(
            d.build_alter_statements(),
            vec!["ALTER TABLE \"app\".\"users\" DROP CONSTRAINT \"users_pkey\";".to_owned()]
        );
    }
}
