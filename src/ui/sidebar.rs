use std::collections::HashMap;
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Sense, Vec2};

use crate::db::metadata::{ColumnInfo, ForeignKeyInfo, FunctionInfo, FunctionKind, IndexInfo, SchemaInfo, TableInfo, TableKind};
use crate::i18n::I18n;

/// Script kinds for the Generate Script menu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScriptKind {
    Select,
    Insert,
    Update,
    Delete,
}

/// Actions the sidebar can request from the rest of the app.
#[derive(Debug)]
pub enum SidebarAction {
    LoadTables(String),
    LoadFunctions(String),
    LoadDetails { schema: String, table: String },
    BrowseTable { schema: String, table: String },
    /// Paste SQL into the active editor without executing.
    SetSql(String),
    RunSql(String),
    NewTable { schema: String },
    EditTable { schema: String, table: String },
    ViewErDiagram { schema: String },
    /// Generate a script for schema.table — app.rs resolves columns if needed.
    GenerateScript { schema: String, table: String, kind: ScriptKind },
}

// ── Script generation helpers (pub — used by app.rs when details arrive) ─────

pub fn placeholder(data_type: &str) -> &'static str {
    let t = data_type.to_lowercase();
    if t.contains("int") || t.contains("serial") || t.contains("numeric")
        || t.contains("float") || t.contains("double") || t.contains("decimal")
        || t.contains("real") || t.contains("money")
    {
        "0"
    } else if t.contains("bool") {
        "false"
    } else if t.contains("timestamp") || t.contains("date") || t.contains("time") {
        "NOW()"
    } else if t.contains("uuid") {
        "gen_random_uuid()"
    } else if t.contains("json") {
        "'{}'::jsonb"
    } else {
        "''"
    }
}

pub fn pk_from_indexes(indexes: &[IndexInfo]) -> Vec<String> {
    indexes
        .iter()
        .find(|i| i.name.ends_with("_pkey") || i.name.to_lowercase().contains("primary"))
        .and_then(|i| {
            let start = i.definition.rfind('(')?;
            let end = i.definition.rfind(')')?;
            if end > start {
                Some(
                    i.definition[start + 1..end]
                        .split(',')
                        .map(|s| s.trim().to_owned())
                        .collect(),
                )
            } else {
                None
            }
        })
        .unwrap_or_default()
}

pub fn script_select(schema: &str, table: &str, cols: &[ColumnInfo]) -> String {
    if cols.is_empty() {
        return format!("SELECT *\nFROM \"{schema}\".\"{table}\"\nWHERE 1=1\nLIMIT 100;");
    }
    let col_list = cols
        .iter()
        .map(|c| format!("    \"{}\"", c.name))
        .collect::<Vec<_>>()
        .join(",\n");
    format!("SELECT\n{col_list}\nFROM \"{schema}\".\"{table}\"\nWHERE 1=1\nLIMIT 100;")
}

pub fn script_insert(schema: &str, table: &str, cols: &[ColumnInfo]) -> String {
    let insertable: Vec<&ColumnInfo> = cols
        .iter()
        .filter(|c| {
            !c.column_default
                .as_deref()
                .map(|d| d.contains("nextval"))
                .unwrap_or(false)
        })
        .collect();
    if insertable.is_empty() {
        return format!("INSERT INTO \"{schema}\".\"{table}\" DEFAULT VALUES;");
    }
    let col_list = insertable
        .iter()
        .map(|c| format!("    \"{}\"", c.name))
        .collect::<Vec<_>>()
        .join(",\n");
    let val_list = insertable
        .iter()
        .map(|c| {
            format!(
                "    {}  -- {}: {}",
                placeholder(&c.data_type),
                c.name,
                c.data_type
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    format!("INSERT INTO \"{schema}\".\"{table}\" (\n{col_list}\n) VALUES (\n{val_list}\n);")
}

pub fn script_update(schema: &str, table: &str, cols: &[ColumnInfo], pk: &[String]) -> String {
    let pk_set: std::collections::HashSet<&str> = pk.iter().map(|s| s.as_str()).collect();
    let set_cols: Vec<&ColumnInfo> = cols.iter().filter(|c| !pk_set.contains(c.name.as_str())).collect();
    let set_clause = if set_cols.is_empty() {
        "    -- no non-PK columns to update".to_owned()
    } else {
        set_cols
            .iter()
            .map(|c| format!("    \"{}\" = {}  -- {}", c.name, placeholder(&c.data_type), c.data_type))
            .collect::<Vec<_>>()
            .join(",\n")
    };
    let where_clause = if pk.is_empty() {
        "    1=1  -- ⚠️ replace with actual condition".to_owned()
    } else {
        pk.iter()
            .map(|p| {
                let dt = cols.iter().find(|c| &c.name == p).map(|c| c.data_type.as_str()).unwrap_or("?");
                format!("    \"{}\" = {}  -- {}", p, placeholder(dt), dt)
            })
            .collect::<Vec<_>>()
            .join("\n    AND ")
    };
    format!("UPDATE \"{schema}\".\"{table}\"\nSET\n{set_clause}\nWHERE\n{where_clause};")
}

pub fn script_delete(schema: &str, table: &str, cols: &[ColumnInfo], pk: &[String]) -> String {
    let where_clause = if pk.is_empty() {
        "    1=1  -- ⚠️ replace with actual condition".to_owned()
    } else {
        pk.iter()
            .map(|p| {
                let dt = cols.iter().find(|c| &c.name == p).map(|c| c.data_type.as_str()).unwrap_or("?");
                format!("    \"{}\" = {}  -- {}", p, placeholder(dt), dt)
            })
            .collect::<Vec<_>>()
            .join("\n    AND ")
    };
    format!(
        "-- ⚠️ Review WHERE clause carefully before running\nDELETE FROM \"{schema}\".\"{table}\"\nWHERE\n{where_clause};"
    )
}

// ── Colour palette ────────────────────────────────────────────────────────────

const COLOR_TABLE: Color32 = Color32::from_rgb(86, 156, 214);
const COLOR_VIEW: Color32 = Color32::from_rgb(180, 130, 220);
const COLOR_MATVIEW: Color32 = Color32::from_rgb(220, 160, 60);
const COLOR_FOREIGN: Color32 = Color32::from_rgb(78, 201, 176);
const COLOR_SCHEMA: Color32 = Color32::from_rgb(206, 206, 206);
const COLOR_SCHEMA_ARROW: Color32 = Color32::from_rgb(130, 130, 130);
const COLOR_KIND_HEADER: Color32 = Color32::from_rgb(80, 80, 80);
const COLOR_SUBITEM: Color32 = Color32::from_rgb(150, 150, 150);
const COLOR_INDEX: Color32 = Color32::from_rgb(100, 180, 100);
const COLOR_FK: Color32 = Color32::from_rgb(220, 160, 80);

fn kind_color(kind: &TableKind) -> Color32 {
    match kind {
        TableKind::Table => COLOR_TABLE,
        TableKind::View => COLOR_VIEW,
        TableKind::MaterializedView => COLOR_MATVIEW,
        TableKind::ForeignTable => COLOR_FOREIGN,
    }
}

// ── Detail cache ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct TableDetailCache {
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub loaded: bool,
}

// ── Sidebar state ─────────────────────────────────────────────────────────────

const TABLE_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub struct Sidebar {
    schemas: Vec<SchemaInfo>,
    tables: HashMap<String, Vec<TableInfo>>,
    /// When tables for a schema were last loaded — used for TTL expiry.
    tables_loaded_at: HashMap<String, Instant>,
    expanded: HashMap<String, bool>,
    expanded_tables: HashMap<(String, String), bool>,
    /// Per-table sub-section expand state: (schema, table) → [cols_open, idx_open, fk_open].
    expanded_sections: HashMap<(String, String), [bool; 3]>,
    table_details: HashMap<(String, String), TableDetailCache>,
    /// Column names per table loaded in bulk for autocomplete: schema → table → [col].
    schema_columns: HashMap<String, HashMap<String, Vec<String>>>,
    filter: String,
    selected: Option<(String, String)>,
    /// Functions/procedures per schema (lazy-loaded alongside tables).
    functions: HashMap<String, Vec<FunctionInfo>>,
    /// Whether the FUNCTIONS section is expanded per schema.
    expanded_functions: HashMap<String, bool>,
    /// Schemas currently being refreshed in the background (stale data still shown).
    refreshing: std::collections::HashSet<String>,
    /// Set when tables/columns change — triggers a single autocomplete rebuild.
    completion_dirty: bool,
}

impl Sidebar {
    pub fn clear(&mut self) {
        self.schemas.clear();
        self.tables.clear();
        self.tables_loaded_at.clear();
        self.expanded.clear();
        self.expanded_tables.clear();
        self.expanded_sections.clear();
        self.table_details.clear();
        self.schema_columns.clear();
        self.functions.clear();
        self.expanded_functions.clear();
        self.refreshing.clear();
        self.selected = None;
        self.completion_dirty = true;
    }

    /// Returns true (and resets the flag) when completion data needs rebuilding.
    pub fn take_completion_dirty(&mut self) -> bool {
        std::mem::replace(&mut self.completion_dirty, false)
    }

    pub fn set_functions(&mut self, schema: &str, functions: Vec<FunctionInfo>) {
        self.functions.insert(schema.to_owned(), functions);
    }

    pub fn set_schemas(&mut self, schemas: Vec<SchemaInfo>) {
        self.schemas = schemas;
    }

    pub fn set_tables(&mut self, schema: &str, tables: Vec<TableInfo>) {
        self.tables.insert(schema.to_owned(), tables);
        self.tables_loaded_at.insert(schema.to_owned(), Instant::now());
        self.refreshing.remove(schema);
        self.completion_dirty = true;
    }

    pub fn schema_names(&self) -> Vec<String> {
        self.schemas.iter().map(|s| s.name.clone()).collect()
    }

    /// Build a compact schema summary for the Claude AI prompt.
    /// Includes table names and, when already loaded, column names.
    pub fn schema_context_for_ai(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        let mut schemas: Vec<&str> = self.tables.keys().map(|s| s.as_str()).collect();
        schemas.sort();
        for schema in schemas {
            if let Some(tables) = self.tables.get(schema) {
                for t in tables {
                    let key = (t.schema.clone(), t.name.clone());
                    if let Some(details) = self.table_details.get(&key) {
                        let cols: Vec<String> = details
                            .columns
                            .iter()
                            .map(|c| {
                                if c.data_type.is_empty() {
                                    c.name.clone()
                                } else {
                                    format!("{} {}", c.name, c.data_type)
                                }
                            })
                            .collect();
                        lines.push(format!("- {}.{} ({})", schema, t.name, cols.join(", ")));
                    } else {
                        lines.push(format!("- {}.{}", schema, t.name));
                    }
                }
            }
        }
        lines.join("\n")
    }

    pub fn set_schema_columns(&mut self, schema: &str, columns: HashMap<String, Vec<String>>) {
        self.schema_columns.insert(schema.to_owned(), columns);
        self.completion_dirty = true;
    }

    /// Returns (table_names, column_names) for autocomplete.
    pub fn completion_data(&self) -> (Vec<String>, Vec<String>) {
        let mut tables = Vec::new();
        let mut col_set: std::collections::HashSet<String> = Default::default();

        for schema_tables in self.tables.values() {
            for t in schema_tables {
                tables.push(t.name.clone());
                let key = (t.schema.clone(), t.name.clone());
                if let Some(details) = self.table_details.get(&key) {
                    for col in &details.columns {
                        col_set.insert(col.name.clone());
                    }
                }
            }
        }

        // Bulk-loaded column names (faster path — populated on schema expand)
        for table_map in self.schema_columns.values() {
            for cols in table_map.values() {
                for col in cols {
                    col_set.insert(col.clone());
                }
            }
        }

        tables.dedup();
        (tables, col_set.into_iter().collect())
    }

    /// Returns all known tables with their loaded column names,
    /// grouped by schema — used by the Join Builder.
    pub fn all_tables_with_columns(&self) -> Vec<(String, Vec<(String, Vec<String>)>)> {
        self.schemas.iter().filter_map(|s| {
            let tables = self.tables.get(&s.name)?;
            let entries: Vec<(String, Vec<String>)> = tables.iter().map(|t| {
                let cols = self.table_details
                    .get(&(s.name.clone(), t.name.clone()))
                    .map(|d| d.columns.iter().map(|c| c.name.clone()).collect())
                    .unwrap_or_default();
                (t.name.clone(), cols)
            }).collect();
            Some((s.name.clone(), entries))
        }).collect()
    }

    pub fn get_table_details(
        &self,
        schema: &str,
        table: &str,
    ) -> Option<&TableDetailCache> {
        self.table_details.get(&(schema.to_owned(), table.to_owned()))
    }

    pub fn set_table_details(
        &mut self,
        schema: &str,
        table: &str,
        columns: Vec<ColumnInfo>,
        indexes: Vec<IndexInfo>,
        foreign_keys: Vec<ForeignKeyInfo>,
    ) {
        self.table_details.insert(
            (schema.to_owned(), table.to_owned()),
            TableDetailCache { columns, indexes, foreign_keys, loaded: true },
        );
        self.completion_dirty = true;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n) -> Vec<SidebarAction> {
        let mut actions: Vec<SidebarAction> = Vec::new();

        // ── F5: force-refresh all expanded schemas ─────────────────────────────
        // Stale data stays visible during reload; `refreshing` marks them as in-progress.
        if ui.input(|i| i.key_pressed(egui::Key::F5)) {
            let to_reload: Vec<String> = self
                .expanded
                .iter()
                .filter(|(_, v)| **v)
                .map(|(k, _)| k.clone())
                .collect();
            for schema_name in to_reload {
                self.tables_loaded_at.remove(&schema_name);
                self.refreshing.insert(schema_name.clone());
                actions.push(SidebarAction::LoadTables(schema_name.clone()));
                actions.push(SidebarAction::LoadFunctions(schema_name));
            }
        }

        // ── TTL: auto-reload stale table lists for expanded schemas ────────────
        let stale: Vec<String> = self
            .tables_loaded_at
            .iter()
            .filter(|(schema, loaded_at)| {
                self.expanded.get(*schema).copied().unwrap_or(false)
                    && loaded_at.elapsed() > TABLE_TTL
            })
            .map(|(k, _)| k.clone())
            .collect();
        for schema_name in stale {
            self.tables_loaded_at.remove(&schema_name);
            actions.push(SidebarAction::LoadTables(schema_name));
        }

        // ── Header ────────────────────────────────────────────────────────────
        egui::Frame::none()
            .inner_margin(egui::Margin { left: 8.0, right: 8.0, top: 8.0, bottom: 4.0 })
            .show(ui, |ui| {
                ui.label(
                    RichText::new(i18n.schema_browser())
                        .small()
                        .strong()
                        .color(Color32::from_gray(120)),
                );
                ui.add_space(4.0);

                let filter_resp = ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .hint_text(i18n.sidebar_filter_hint())
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Small),
                );
                if filter_resp.changed() && !self.filter.is_empty() {
                    for s in &self.schemas {
                        self.expanded.entry(s.name.clone()).or_insert(true);
                    }
                }
            });

        ui.separator();

        // ── Tree ──────────────────────────────────────────────────────────────
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;

                let schemas: Vec<SchemaInfo> = self.schemas.clone();
                let filter = self.filter.to_lowercase();

                for schema in &schemas {
                    let is_expanded = *self.expanded.get(&schema.name).unwrap_or(&false);
                    let tables = self
                        .tables
                        .get(&schema.name)
                        .cloned()
                        .unwrap_or_default();

                    let visible: Vec<&TableInfo> = tables
                        .iter()
                        .filter(|t| {
                            filter.is_empty()
                                || t.name.to_lowercase().contains(&filter)
                        })
                        .collect();

                    // ── Schema header row ─────────────────────────────────
                    let schema_resp = {
                        let arrow = if is_expanded { "▾" } else { "▸" };
                        let available_w = ui.available_width();
                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::new(available_w, 22.0),
                            Sense::click(),
                        );

                        if ui.is_rect_visible(rect) {
                            let painter = ui.painter();
                            let bg = if resp.hovered() {
                                Color32::from_rgb(65, 69, 73)
                            } else {
                                Color32::TRANSPARENT
                            };
                            painter.rect_filled(rect, 2.0, bg);
                            painter.text(
                                rect.left_center() + Vec2::new(6.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                arrow,
                                egui::FontId::proportional(10.0),
                                COLOR_SCHEMA_ARROW,
                            );
                            painter.text(
                                rect.left_center() + Vec2::new(20.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                &schema.name,
                                egui::FontId::proportional(13.0),
                                COLOR_SCHEMA,
                            );
                            if self.refreshing.contains(&schema.name) {
                                painter.text(
                                    rect.right_center() + Vec2::new(-8.0, 0.0),
                                    egui::Align2::RIGHT_CENTER,
                                    "↻",
                                    egui::FontId::proportional(11.0),
                                    Color32::from_rgb(120, 160, 200),
                                );
                            } else if filter.is_empty() && !tables.is_empty() {
                                let badge = format!("{}", tables.len());
                                painter.text(
                                    rect.right_center() + Vec2::new(-8.0, 0.0),
                                    egui::Align2::RIGHT_CENTER,
                                    &badge,
                                    egui::FontId::proportional(10.0),
                                    Color32::from_gray(100),
                                );
                            }
                        }
                        resp
                    };

                    if schema_resp.clicked() {
                        let entry = self
                            .expanded
                            .entry(schema.name.clone())
                            .or_insert(false);
                        *entry = !*entry;
                        if *entry {
                            if !self.tables.contains_key(&schema.name) {
                                actions.push(SidebarAction::LoadTables(schema.name.clone()));
                            }
                            if !self.functions.contains_key(&schema.name) {
                                actions.push(SidebarAction::LoadFunctions(schema.name.clone()));
                            }
                        }
                    }

                    let schema_name_for_menu = schema.name.clone();
                    schema_resp.context_menu(|ui| {
                        ui.label(
                            RichText::new(&schema_name_for_menu).strong().small(),
                        );
                        ui.separator();
                        if ui.button(i18n.schema_menu_new_table()).clicked() {
                            actions.push(SidebarAction::NewTable {
                                schema: schema_name_for_menu.clone(),
                            });
                            ui.close_menu();
                        }
                        if ui.button(i18n.schema_menu_er()).clicked() {
                            actions.push(SidebarAction::ViewErDiagram {
                                schema: schema_name_for_menu.clone(),
                            });
                            ui.close_menu();
                        }
                        if ui.button(i18n.schema_menu_refresh()).clicked() {
                            self.functions.remove(&schema_name_for_menu);
                            actions.push(SidebarAction::LoadTables(schema_name_for_menu.clone()));
                            actions.push(SidebarAction::LoadFunctions(schema_name_for_menu.clone()));
                            ui.close_menu();
                        }
                    });

                    let visible_funcs: Vec<&FunctionInfo> = self
                        .functions
                        .get(&schema.name)
                        .map(|fns| {
                            fns.iter()
                                .filter(|f| {
                                    filter.is_empty()
                                        || f.name.to_lowercase().contains(&filter)
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    if !is_expanded || (visible.is_empty() && visible_funcs.is_empty()) {
                        continue;
                    }

                    // ── Table rows grouped by kind ─────────────────────────
                    ui.indent(egui::Id::new(&schema.name), |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;

                        let kinds = [
                            (TableKind::Table, i18n.kind_tables()),
                            (TableKind::View, i18n.kind_views()),
                            (TableKind::MaterializedView, i18n.kind_mat_views()),
                            (TableKind::ForeignTable, i18n.kind_foreign_tables()),
                        ];

                        for (kind, kind_label) in &kinds {
                            let kind_tables: Vec<&TableInfo> = visible
                                .iter()
                                .copied()
                                .filter(|t| t.kind == *kind)
                                .collect();

                            if kind_tables.is_empty() {
                                continue;
                            }

                            // Kind group header
                            render_kind_header(ui, kind_label, kind_tables.len());

                            for table in &kind_tables {
                                let key = (schema.name.clone(), table.name.clone());
                                let is_selected =
                                    self.selected == Some(key.clone());
                                let is_table_expanded =
                                    *self.expanded_tables.get(&key).unwrap_or(&false);
                                let details = self.table_details.get(&key).cloned();

                                let resp = render_table_row_ui(
                                    ui,
                                    table,
                                    is_selected,
                                    is_table_expanded,
                                );

                                if resp.clicked() {
                                    self.selected = Some(key.clone());
                                    let entry = self
                                        .expanded_tables
                                        .entry(key.clone())
                                        .or_insert(false);
                                    *entry = !*entry;
                                    if *entry && details.is_none() {
                                        actions.push(SidebarAction::LoadDetails {
                                            schema: schema.name.clone(),
                                            table: table.name.clone(),
                                        });
                                        self.table_details.insert(
                                            key.clone(),
                                            TableDetailCache::default(),
                                        );
                                    }
                                }

                                if resp.double_clicked() {
                                    actions.push(SidebarAction::BrowseTable {
                                        schema: schema.name.clone(),
                                        table: table.name.clone(),
                                    });
                                }

                                let schema_name = schema.name.clone();
                                let table_name = table.name.clone();
                                let icon_color = kind_color(&table.kind);
                                let icon = table.kind.icon();

                                let resp = resp.on_hover_ui(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(icon_color, icon);
                                        ui.label(
                                            RichText::new(table.kind.label())
                                                .small()
                                                .color(Color32::from_gray(160)),
                                        );
                                        ui.separator();
                                        ui.label(
                                            RichText::new(format!(
                                                "{}.{}",
                                                schema_name, table_name
                                            ))
                                            .monospace()
                                            .small(),
                                        );
                                    });
                                });
                                let details_for_menu = details.clone();
                                resp.context_menu(|ui| {
                                    ui.label(
                                        RichText::new(format!("{schema_name}.{table_name}"))
                                            .strong()
                                            .small(),
                                    );
                                    ui.separator();

                                    // ── Script generation ─────────────────
                                    ui.menu_button(i18n.table_menu_generate_script(), |ui| {
                                        for (label, kind) in [
                                            ("SELECT", ScriptKind::Select),
                                            ("INSERT", ScriptKind::Insert),
                                            ("UPDATE", ScriptKind::Update),
                                            ("DELETE", ScriptKind::Delete),
                                        ] {
                                            if ui.button(label).clicked() {
                                                actions.push(SidebarAction::GenerateScript {
                                                    schema: schema_name.clone(),
                                                    table: table_name.clone(),
                                                    kind,
                                                });
                                                ui.close_menu();
                                            }
                                        }
                                    });
                                    ui.separator();

                                    if ui.button(i18n.table_menu_browse()).clicked() {
                                        actions.push(SidebarAction::BrowseTable {
                                            schema: schema_name.clone(),
                                            table: table_name.clone(),
                                        });
                                        ui.close_menu();
                                    }
                                    if matches!(table.kind, TableKind::Table) {
                                        if ui.button(i18n.table_menu_edit()).clicked() {
                                            actions.push(SidebarAction::EditTable {
                                                schema: schema_name.clone(),
                                                table: table_name.clone(),
                                            });
                                            ui.close_menu();
                                        }
                                    }
                                    if matches!(table.kind, TableKind::View | TableKind::MaterializedView) {
                                        if ui.button(i18n.table_menu_show_ddl()).clicked() {
                                            let sql = match table.kind {
                                                TableKind::MaterializedView => format!(
                                                    "SELECT 'CREATE MATERIALIZED VIEW \"{schema_name}\".\"{table_name}\" AS' || chr(10) \
                                                     || definition AS ddl \
                                                     FROM pg_matviews \
                                                     WHERE schemaname = '{schema_name}' \
                                                       AND matviewname = '{table_name}';"
                                                ),
                                                _ => format!(
                                                    "SELECT 'CREATE OR REPLACE VIEW \"{schema_name}\".\"{table_name}\" AS' || chr(10) \
                                                     || pg_get_viewdef('\"{}\".\"{table_name}\"'::regclass, true) AS ddl;",
                                                    schema_name
                                                ),
                                            };
                                            actions.push(SidebarAction::RunSql(sql));
                                            ui.close_menu();
                                        }
                                    }
                                    if ui.button(i18n.table_menu_count()).clicked() {
                                        actions.push(SidebarAction::RunSql(format!(
                                            "SELECT COUNT(*) AS total FROM \"{schema_name}\".\"{table_name}\";"
                                        )));
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button(i18n.table_menu_show_cols()).clicked() {
                                        actions.push(SidebarAction::RunSql(format!(
                                            "SELECT column_name, data_type, is_nullable, column_default \
                                             FROM information_schema.columns \
                                             WHERE table_schema = '{schema_name}' \
                                               AND table_name = '{table_name}' \
                                             ORDER BY ordinal_position;"
                                        )));
                                        ui.close_menu();
                                    }
                                    if ui.button(i18n.table_menu_show_indexes()).clicked() {
                                        actions.push(SidebarAction::RunSql(format!(
                                            "SELECT indexname, indexdef \
                                             FROM pg_indexes \
                                             WHERE schemaname = '{schema_name}' \
                                               AND tablename = '{table_name}';"
                                        )));
                                        ui.close_menu();
                                    }
                                    if ui.button(i18n.table_menu_show_fks()).clicked() {
                                        actions.push(SidebarAction::RunSql(format!(
                                            "SELECT c.conname, pg_get_constraintdef(c.oid) \
                                             FROM pg_constraint c \
                                             JOIN pg_class t ON t.oid = c.conrelid \
                                             JOIN pg_namespace n ON n.oid = t.relnamespace \
                                             WHERE c.contype = 'f' \
                                               AND n.nspname = '{schema_name}' \
                                               AND t.relname = '{table_name}' \
                                             ORDER BY c.conname;"
                                        )));
                                        ui.close_menu();
                                    }
                                });

                                // Sub-items when expanded
                                if is_table_expanded {
                                    // Single lookup: [cols_open, idx_open, fk_open]
                                    let [sec_cols, sec_idx, sec_fk] =
                                        *self.expanded_sections.get(&key).unwrap_or(&[false; 3]);

                                    let id_cols = egui::Id::new((&key.0, &key.1, "sec_cols"));
                                    let id_idx  = egui::Id::new((&key.0, &key.1, "sec_idx"));
                                    let id_fk   = egui::Id::new((&key.0, &key.1, "sec_fk"));

                                    let toggled = ui.indent(egui::Id::new(&key), |ui| {
                                        render_table_details(
                                            ui, &details,
                                            sec_cols, sec_idx, sec_fk,
                                            id_cols, id_idx, id_fk,
                                        )
                                    }).inner;

                                    for sec_str in toggled {
                                        let idx = match sec_str.as_str() {
                                            "cols" => 0,
                                            "idx"  => 1,
                                            _      => 2,
                                        };
                                        let secs = self.expanded_sections
                                            .entry(key.clone())
                                            .or_insert([false; 3]);
                                        secs[idx] = !secs[idx];
                                    }
                                }
                            }
                        }

                        // ── FUNCTIONS section ──────────────────────────────
                        if !visible_funcs.is_empty() {
                            let fn_exp_key = format!("__fn__{}", schema.name);
                            let fn_expanded = *self
                                .expanded_functions
                                .get(&schema.name)
                                .unwrap_or(&false);

                            ui.add_space(4.0);
                            let hdr_resp = render_kind_header_clickable(
                                ui,
                                i18n.kind_functions(),
                                visible_funcs.len(),
                                fn_expanded,
                            );
                            if hdr_resp.clicked() {
                                let e = self
                                    .expanded_functions
                                    .entry(schema.name.clone())
                                    .or_insert(false);
                                *e = !*e;
                            }

                            if fn_expanded {
                                let schema_name = schema.name.clone();
                                for func in &visible_funcs {
                                    let resp = render_function_row(
                                        ui,
                                        func,
                                        egui::Id::new((&fn_exp_key, &func.name, &func.args)),
                                    );
                                    let func_name = func.name.clone();
                                    let func_args = func.args.clone();
                                    let func_ret  = func.return_type.clone();
                                    let func_kind = func.kind.clone();
                                    let sn = schema_name.clone();
                                    resp.context_menu(|ui| {
                                        ui.label(
                                            RichText::new(format!("{sn}.{func_name}"))
                                                .strong()
                                                .small(),
                                        );
                                        ui.separator();
                                        if ui.button(i18n.fn_show_definition()).clicked() {
                                            // Paste a query that retrieves the source.
                                            let sql = if func_kind == FunctionKind::Aggregate {
                                                format!(
                                                    "-- Aggregate functions do not have a pg_get_functiondef\n\
                                                     SELECT p.proname, p.prokind\n\
                                                     FROM pg_proc p\n\
                                                     JOIN pg_namespace n ON p.pronamespace = n.oid\n\
                                                     WHERE n.nspname = '{sn}' AND p.proname = '{func_name}';"
                                                )
                                            } else {
                                                format!(
                                                    "SELECT pg_get_functiondef('{sn}.{func_name}({func_args})'::regprocedure);"
                                                )
                                            };
                                            actions.push(SidebarAction::SetSql(sql));
                                            ui.close_menu();
                                        }
                                        if func_kind != FunctionKind::Aggregate
                                            && func_kind != FunctionKind::Window
                                        {
                                            if ui.button(i18n.fn_copy_call()).clicked() {
                                                let call = build_call_template(
                                                    &sn, &func_name, &func_args,
                                                    &func_ret, &func_kind,
                                                );
                                                actions.push(SidebarAction::SetSql(call));
                                                ui.close_menu();
                                            }
                                        }
                                    });
                                }
                            }
                        }
                    });

                    ui.add_space(2.0);
                }

                if self.schemas.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(i18n.lbl_not_connected_sidebar())
                                .color(Color32::from_gray(90))
                                .italics(),
                        );
                    });
                }
            });

        actions
    }
}

// ── Widget helpers ────────────────────────────────────────────────────────────

fn render_kind_header(ui: &mut egui::Ui, label: &str, count: usize) {
    ui.add_space(4.0);
    let available_w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(available_w, 18.0), Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Label text
        painter.text(
            rect.left_center() + Vec2::new(4.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label.to_uppercase(),
            egui::FontId::proportional(9.5),
            Color32::from_rgb(110, 123, 139), // TEXT_DIM
        );

        // Count badge — pill background
        let badge_str = format!("{count}");
        let badge_font = egui::FontId::proportional(9.5);
        let badge_galley = painter.layout_no_wrap(
            badge_str.clone(),
            badge_font.clone(),
            Color32::from_rgb(110, 123, 139),
        );
        let badge_w = badge_galley.rect.width() + 8.0;
        let badge_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - badge_w / 2.0 - 4.0, rect.center().y),
            egui::vec2(badge_w, 13.0),
        );
        painter.rect_filled(badge_rect, egui::Rounding::same(6.0), Color32::from_rgb(76, 80, 82));
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            &badge_str,
            badge_font,
            Color32::from_rgb(169, 183, 198),
        );
    }
}

fn render_table_row_ui(
    ui: &mut egui::Ui,
    table: &TableInfo,
    is_selected: bool,
    is_expanded: bool,
) -> egui::Response {
    let icon = table.kind.icon();
    let icon_color = kind_color(&table.kind);
    let available_w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(available_w, 20.0), Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        let bg = if is_selected {
            Color32::from_rgba_premultiplied(86, 156, 214, 35)
        } else if resp.hovered() {
            Color32::from_rgb(65, 69, 73)
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, 2.0, bg);

        if is_selected {
            let bar = egui::Rect::from_min_size(
                rect.left_top(),
                Vec2::new(3.0, rect.height()),
            );
            painter.rect_filled(bar, 0.0, Color32::from_rgb(78, 159, 222)); // #4e9fde
        }

        // Expand arrow
        let arrow = if is_expanded { "▾" } else { "▸" };
        painter.text(
            rect.left_center() + Vec2::new(2.0, 0.0),
            egui::Align2::LEFT_CENTER,
            arrow,
            egui::FontId::proportional(9.0),
            Color32::from_gray(90),
        );

        // Type icon
        painter.text(
            rect.left_center() + Vec2::new(14.0, 0.0),
            egui::Align2::LEFT_CENTER,
            icon,
            egui::FontId::proportional(9.0),
            icon_color,
        );

        // Table name
        let name_color = if is_selected {
            Color32::WHITE
        } else {
            Color32::from_gray(210)
        };
        painter.text(
            rect.left_center() + Vec2::new(26.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &table.name,
            egui::FontId::monospace(12.0),
            name_color,
        );
    }

    resp
}

fn render_table_details(
    ui: &mut egui::Ui,
    details: &Option<TableDetailCache>,
    sec_cols: bool,
    sec_idx: bool,
    sec_fk: bool,
    id_cols: egui::Id,
    id_idx: egui::Id,
    id_fk: egui::Id,
) -> Vec<String> {
    let mut toggled: Vec<String> = Vec::new();

    let Some(d) = details else {
        return toggled;
    };

    if !d.loaded {
        ui.label(RichText::new("⟳ Loading…").small().color(Color32::from_gray(90)));
        return toggled;
    }

    // Columns section
    if !d.columns.is_empty() {
        let resp = render_detail_section_header(ui, "Columns", d.columns.len(), sec_cols, id_cols);
        if resp.clicked() { toggled.push("cols".to_owned()); }
        if sec_cols {
            ui.indent(id_cols.with("content"), |ui| {
                for col in &d.columns {
                    ui.label(
                        RichText::new(format!("{} : {}", col.name, col.data_type))
                            .small()
                            .monospace()
                            .color(COLOR_SUBITEM),
                    );
                }
            });
        }
    }

    // Indexes section
    if !d.indexes.is_empty() {
        let resp = render_detail_section_header(ui, "Indexes", d.indexes.len(), sec_idx, id_idx);
        if resp.clicked() { toggled.push("idx".to_owned()); }
        if sec_idx {
            ui.indent(id_idx.with("content"), |ui| {
                for idx in &d.indexes {
                    let unique = if idx.is_unique { " ◈" } else { "" };
                    ui.label(
                        RichText::new(format!("{}{}", idx.name, unique))
                            .small()
                            .monospace()
                            .color(COLOR_INDEX),
                    )
                    .on_hover_text(&idx.definition);
                }
            });
        }
    }

    // Foreign Keys section
    if !d.foreign_keys.is_empty() {
        let resp = render_detail_section_header(ui, "Foreign Keys", d.foreign_keys.len(), sec_fk, id_fk);
        if resp.clicked() { toggled.push("fk".to_owned()); }
        if sec_fk {
            ui.indent(id_fk.with("content"), |ui| {
                for fk in &d.foreign_keys {
                    ui.label(
                        RichText::new(&fk.name)
                            .small()
                            .monospace()
                            .color(COLOR_FK),
                    )
                    .on_hover_text(&fk.definition);
                }
            });
        }
    }

    if d.columns.is_empty() && d.indexes.is_empty() && d.foreign_keys.is_empty() {
        ui.label(RichText::new("(empty)").small().color(Color32::from_gray(70)));
    }

    toggled
}

fn render_detail_section_header(
    ui: &mut egui::Ui,
    label: &str,
    count: usize,
    expanded: bool,
    id: egui::Id,
) -> egui::Response {
    let available_w = ui.available_width();
    // Allocate rect without sense, then interact with a stable ID
    let (rect, _) = ui.allocate_exact_size(Vec2::new(available_w, 16.0), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let bg = if resp.hovered() { Color32::from_rgb(55, 59, 63) } else { Color32::TRANSPARENT };
        painter.rect_filled(rect, 2.0, bg);

        let arrow = if expanded { "▾" } else { "▸" };
        painter.text(
            rect.left_center() + Vec2::new(2.0, 0.0),
            egui::Align2::LEFT_CENTER,
            arrow,
            egui::FontId::proportional(8.5),
            Color32::from_gray(100),
        );
        painter.text(
            rect.left_center() + Vec2::new(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(130, 140, 150),
        );
        let badge = format!("{count}");
        painter.text(
            rect.right_center() + Vec2::new(-4.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            &badge,
            egui::FontId::proportional(9.0),
            Color32::from_gray(90),
        );
    }

    resp
}

/// Like `render_kind_header` but clickable (toggles expand) and shows an arrow.
fn render_kind_header_clickable(
    ui: &mut egui::Ui,
    label: &str,
    count: usize,
    expanded: bool,
) -> egui::Response {
    let available_w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(available_w, 18.0), Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        let bg = if resp.hovered() {
            Color32::from_rgb(55, 59, 63)
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, 2.0, bg);

        let arrow = if expanded { "▾" } else { "▸" };
        painter.text(
            rect.left_center() + Vec2::new(4.0, 0.0),
            egui::Align2::LEFT_CENTER,
            arrow,
            egui::FontId::proportional(9.0),
            Color32::from_rgb(110, 123, 139),
        );
        painter.text(
            rect.left_center() + Vec2::new(14.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label.to_uppercase(),
            egui::FontId::proportional(9.5),
            Color32::from_rgb(110, 123, 139),
        );

        let badge_str = format!("{count}");
        let badge_font = egui::FontId::proportional(9.5);
        let badge_galley = painter.layout_no_wrap(
            badge_str.clone(),
            badge_font.clone(),
            Color32::from_rgb(110, 123, 139),
        );
        let badge_w = badge_galley.rect.width() + 8.0;
        let badge_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - badge_w / 2.0 - 4.0, rect.center().y),
            egui::vec2(badge_w, 13.0),
        );
        painter.rect_filled(badge_rect, egui::Rounding::same(6.0), Color32::from_rgb(76, 80, 82));
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            &badge_str,
            badge_font,
            Color32::from_rgb(169, 183, 198),
        );
    }
    resp
}

const COLOR_FUNC: Color32 = Color32::from_rgb(78, 201, 176); // teal — distinct from tables

fn render_function_row(
    ui: &mut egui::Ui,
    func: &FunctionInfo,
    id: egui::Id,
) -> egui::Response {
    let available_w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(available_w, 20.0), Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let bg = if resp.hovered() {
            Color32::from_rgb(55, 59, 63)
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, 0.0, bg);

        // Icon
        painter.text(
            rect.left_center() + Vec2::new(6.0, 0.0),
            egui::Align2::LEFT_CENTER,
            func.kind.icon(),
            egui::FontId::proportional(11.0),
            COLOR_FUNC,
        );

        // name(args) → return_type  (truncated)
        let sig = if func.args.is_empty() {
            format!("{}()", func.name)
        } else {
            format!("{}(…)", func.name)
        };
        painter.text(
            rect.left_center() + Vec2::new(20.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &sig,
            egui::FontId::monospace(11.0),
            Color32::from_rgb(169, 183, 198),
        );

        // Return type badge on the right
        if !func.return_type.is_empty() && func.return_type != "void" {
            let ret_short: String = func.return_type.chars().take(16).collect();
            painter.text(
                rect.right_center() + Vec2::new(-6.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                &ret_short,
                egui::FontId::monospace(9.5),
                Color32::from_gray(90),
            );
        }
    }

    // Hover tooltip: full signature
    resp.on_hover_ui(|ui| {
        ui.horizontal(|ui| {
            ui.colored_label(COLOR_FUNC, func.kind.icon());
            ui.label(
                RichText::new(func.kind.label())
                    .small()
                    .color(Color32::from_gray(160)),
            );
        });
        ui.add(egui::Label::new(
            RichText::new(format!("{}({})", func.name, func.args)).monospace().small(),
        ));
        if !func.return_type.is_empty() {
            ui.label(
                RichText::new(format!("→ {}", func.return_type))
                    .small()
                    .color(Color32::from_gray(140)),
            );
        }
    })
}

/// Generate a call template for the editor (SET SQL).
fn build_call_template(
    schema: &str,
    name: &str,
    args: &str,
    return_type: &str,
    kind: &FunctionKind,
) -> String {
    // Build placeholder args: each param "name type" → just the name or a placeholder.
    let arg_placeholders: String = args
        .split(',')
        .map(|a| a.trim())
        .filter(|a| !a.is_empty())
        .map(|a| {
            // "param_name type" → keep param_name, else use placeholder
            let parts: Vec<&str> = a.splitn(2, ' ').collect();
            if parts.len() == 2 { format!("NULL /* {} */", a) } else { "NULL".to_owned() }
        })
        .collect::<Vec<_>>()
        .join(", ");

    match kind {
        FunctionKind::Procedure => {
            format!("CALL \"{schema}\".\"{name}\"({arg_placeholders});")
        }
        _ => {
            let _ = return_type;
            format!("SELECT \"{schema}\".\"{name}\"({arg_placeholders});")
        }
    }
}
