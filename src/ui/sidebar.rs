use std::collections::HashMap;
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Sense, Vec2};

use crate::db::metadata::{ColumnInfo, ForeignKeyInfo, IndexInfo, SchemaInfo, TableInfo, TableKind};

/// Actions the sidebar can request from the rest of the app.
#[derive(Debug)]
pub enum SidebarAction {
    LoadTables(String),
    LoadDetails { schema: String, table: String },
    BrowseTable { schema: String, table: String },
    RunSql(String),
    NewTable { schema: String },
    EditTable { schema: String, table: String },
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
    table_details: HashMap<(String, String), TableDetailCache>,
    filter: String,
    selected: Option<(String, String)>,
}

impl Sidebar {
    pub fn clear(&mut self) {
        self.schemas.clear();
        self.tables.clear();
        self.tables_loaded_at.clear();
        self.expanded.clear();
        self.expanded_tables.clear();
        self.table_details.clear();
        self.selected = None;
    }

    pub fn set_schemas(&mut self, schemas: Vec<SchemaInfo>) {
        self.schemas = schemas;
    }

    pub fn set_tables(&mut self, schema: &str, tables: Vec<TableInfo>) {
        self.tables.insert(schema.to_owned(), tables);
        self.tables_loaded_at.insert(schema.to_owned(), Instant::now());
    }

    pub fn schema_names(&self) -> Vec<String> {
        self.schemas.iter().map(|s| s.name.clone()).collect()
    }

    /// Returns (table_names, column_names) for autocomplete.
    pub fn completion_data(&self) -> (Vec<String>, Vec<String>) {
        let mut tables = Vec::new();
        let mut columns = Vec::new();
        for schema_tables in self.tables.values() {
            for t in schema_tables {
                tables.push(t.name.clone());
                let key = (t.schema.clone(), t.name.clone());
                if let Some(details) = self.table_details.get(&key) {
                    for col in &details.columns {
                        if !columns.contains(&col.name) {
                            columns.push(col.name.clone());
                        }
                    }
                }
            }
        }
        tables.dedup();
        (tables, columns)
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
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Vec<SidebarAction> {
        let mut actions: Vec<SidebarAction> = Vec::new();

        // ── F5: force-refresh all expanded schemas ─────────────────────────────
        if ui.input(|i| i.key_pressed(egui::Key::F5)) {
            let to_reload: Vec<String> = self
                .expanded
                .iter()
                .filter(|(_, v)| **v)
                .map(|(k, _)| k.clone())
                .collect();
            for schema_name in to_reload {
                self.tables.remove(&schema_name);
                self.tables_loaded_at.remove(&schema_name);
                actions.push(SidebarAction::LoadTables(schema_name));
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
                    RichText::new("SCHEMA BROWSER")
                        .small()
                        .strong()
                        .color(Color32::from_gray(120)),
                );
                ui.add_space(4.0);

                let filter_resp = ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .hint_text("🔍  Filter…")
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
                                Color32::from_rgb(32, 44, 68)
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
                            if filter.is_empty() && !tables.is_empty() {
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
                        if *entry && !self.tables.contains_key(&schema.name) {
                            actions.push(SidebarAction::LoadTables(schema.name.clone()));
                        }
                    }

                    let schema_name_for_menu = schema.name.clone();
                    schema_resp.context_menu(|ui| {
                        ui.label(
                            RichText::new(&schema_name_for_menu).strong().small(),
                        );
                        ui.separator();
                        if ui.button("＋  New Table…").clicked() {
                            actions.push(SidebarAction::NewTable {
                                schema: schema_name_for_menu.clone(),
                            });
                            ui.close_menu();
                        }
                        if ui.button("↺  Refresh").clicked() {
                            actions.push(SidebarAction::LoadTables(
                                schema_name_for_menu.clone(),
                            ));
                            ui.close_menu();
                        }
                    });

                    if !is_expanded || visible.is_empty() {
                        continue;
                    }

                    // ── Table rows grouped by kind ─────────────────────────
                    ui.indent(egui::Id::new(&schema.name), |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;

                        let kinds = [
                            (TableKind::Table, "TABLES"),
                            (TableKind::View, "VIEWS"),
                            (TableKind::MaterializedView, "MAT VIEWS"),
                            (TableKind::ForeignTable, "FOREIGN TABLES"),
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
                                resp.context_menu(|ui| {
                                    ui.label(
                                        RichText::new(format!("{schema_name}.{table_name}"))
                                            .strong()
                                            .small(),
                                    );
                                    ui.separator();
                                    if ui.button("▶  Browse rows").clicked() {
                                        actions.push(SidebarAction::BrowseTable {
                                            schema: schema_name.clone(),
                                            table: table_name.clone(),
                                        });
                                        ui.close_menu();
                                    }
                                    if matches!(table.kind, TableKind::Table) {
                                        if ui.button("✎  Edit Table…").clicked() {
                                            actions.push(SidebarAction::EditTable {
                                                schema: schema_name.clone(),
                                                table: table_name.clone(),
                                            });
                                            ui.close_menu();
                                        }
                                    }
                                    if ui.button("∑  Count rows").clicked() {
                                        actions.push(SidebarAction::RunSql(format!(
                                            "SELECT COUNT(*) AS total FROM \"{schema_name}\".\"{table_name}\";"
                                        )));
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button("≡  Show columns").clicked() {
                                        actions.push(SidebarAction::RunSql(format!(
                                            "SELECT column_name, data_type, is_nullable, column_default \
                                             FROM information_schema.columns \
                                             WHERE table_schema = '{schema_name}' \
                                               AND table_name = '{table_name}' \
                                             ORDER BY ordinal_position;"
                                        )));
                                        ui.close_menu();
                                    }
                                    if ui.button("⊟  Show indexes").clicked() {
                                        actions.push(SidebarAction::RunSql(format!(
                                            "SELECT indexname, indexdef \
                                             FROM pg_indexes \
                                             WHERE schemaname = '{schema_name}' \
                                               AND tablename = '{table_name}';"
                                        )));
                                        ui.close_menu();
                                    }
                                    if ui.button("⊠  Show foreign keys").clicked() {
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
                                    ui.indent(egui::Id::new(&key), |ui| {
                                        render_table_details(ui, &details);
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
                            RichText::new("Not connected")
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
    let (rect, _) = ui.allocate_exact_size(Vec2::new(available_w, 14.0), Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let text = format!("{label} ({count})");
        // Draw text
        painter.text(
            rect.left_center() + Vec2::new(2.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &text,
            egui::FontId::proportional(9.5),
            COLOR_KIND_HEADER,
        );
        // Draw line to the right
        let line_x = 2.0 + (text.len() as f32 * 5.8).min(available_w - 10.0);
        painter.line_segment(
            [
                rect.left_center() + Vec2::new(line_x + 4.0, 0.0),
                rect.right_center() + Vec2::new(-4.0, 0.0),
            ],
            egui::Stroke::new(0.5, Color32::from_gray(45)),
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
            Color32::from_rgb(32, 44, 68)
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, 2.0, bg);

        if is_selected {
            let bar = egui::Rect::from_min_size(
                rect.left_top(),
                Vec2::new(2.0, rect.height()),
            );
            painter.rect_filled(bar, 0.0, COLOR_TABLE);
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

fn render_table_details(ui: &mut egui::Ui, details: &Option<TableDetailCache>) {
    let Some(d) = details else {
        return;
    };

    if !d.loaded {
        ui.label(RichText::new("⟳ Loading…").small().color(Color32::from_gray(90)));
        return;
    }

    // Columns
    if !d.columns.is_empty() {
        ui.label(
            RichText::new("COLUMNS")
                .small()
                .strong()
                .color(COLOR_KIND_HEADER),
        );
        for col in &d.columns {
            let not_null = if !col.is_nullable { " !" } else { "" };
            ui.label(
                RichText::new(format!("  {} : {}{}", col.name, col.data_type, not_null))
                    .small()
                    .monospace()
                    .color(COLOR_SUBITEM),
            );
        }
    }

    // Indexes
    if !d.indexes.is_empty() {
        ui.add_space(2.0);
        ui.label(
            RichText::new("INDEXES")
                .small()
                .strong()
                .color(COLOR_KIND_HEADER),
        );
        for idx in &d.indexes {
            let unique = if idx.is_unique { " ◈" } else { "" };
            ui.label(
                RichText::new(format!("  {}{}", idx.name, unique))
                    .small()
                    .monospace()
                    .color(COLOR_INDEX),
            )
            .on_hover_text(&idx.definition);
        }
    }

    // Foreign Keys
    if !d.foreign_keys.is_empty() {
        ui.add_space(2.0);
        ui.label(
            RichText::new("FOREIGN KEYS")
                .small()
                .strong()
                .color(COLOR_KIND_HEADER),
        );
        for fk in &d.foreign_keys {
            ui.label(
                RichText::new(format!("  {}", fk.name))
                    .small()
                    .monospace()
                    .color(COLOR_FK),
            )
            .on_hover_text(&fk.definition);
        }
    }

    if d.columns.is_empty() && d.indexes.is_empty() && d.foreign_keys.is_empty() {
        ui.label(RichText::new("(empty)").small().color(Color32::from_gray(70)));
    }
}
