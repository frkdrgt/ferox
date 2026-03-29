use std::collections::HashMap;

use egui::{Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};

use crate::db::metadata::ErTableInfo;
use crate::i18n::I18n;

// ── Constants ─────────────────────────────────────────────────────────────────

const NODE_WIDTH: f32 = 200.0;
const ROW_HEIGHT: f32 = 18.0;
const HEADER_HEIGHT: f32 = 24.0;
const GRID_SPACING_X: f32 = 280.0;
const GRID_SPACING_Y: f32 = 240.0;

const COLOR_HEADER_BG: Color32 = Color32::from_rgb(40, 80, 130);
const COLOR_NODE_BG: Color32 = Color32::from_rgb(28, 32, 40);
const COLOR_NODE_BORDER: Color32 = Color32::from_rgb(60, 80, 110);
const COLOR_PK: Color32 = Color32::from_rgb(255, 210, 80);
const COLOR_FK_COL: Color32 = Color32::from_rgb(120, 200, 255);
const COLOR_COL: Color32 = Color32::from_rgb(180, 185, 195);
const COLOR_TYPE: Color32 = Color32::from_rgb(100, 110, 130);
const COLOR_ARROW: Color32 = Color32::from_rgb(86, 156, 214);

// ── State ─────────────────────────────────────────────────────────────────────

pub enum ErDiagramState {
    Empty,
    Loading { schema: String },
    Loaded { schema: String, tables: Vec<ErTableInfo> },
    Error(String),
}

pub struct ErDiagram {
    pub state: ErDiagramState,
    /// World-space positions for each (schema, table).
    node_positions: HashMap<(String, String), Pos2>,
    pan: Vec2,
    zoom: f32,
    dragging_node: Option<(String, String)>,
    drag_offset: Vec2,
    load_requested: bool,
}

impl Default for ErDiagram {
    fn default() -> Self {
        Self {
            state: ErDiagramState::Empty,
            node_positions: HashMap::new(),
            pan: Vec2::ZERO,
            zoom: 1.0,
            dragging_node: None,
            drag_offset: Vec2::ZERO,
            load_requested: false,
        }
    }
}

impl ErDiagram {
    pub fn start_loading(schema: String) -> Self {
        Self {
            state: ErDiagramState::Loading { schema },
            ..Default::default()
        }
    }

    pub fn set_data(&mut self, schema: String, tables: Vec<ErTableInfo>) {
        self.auto_layout(&schema, &tables);
        self.state = ErDiagramState::Loaded { schema, tables };
        self.load_requested = true;
    }

    pub fn set_error(&mut self, msg: String) {
        self.state = ErDiagramState::Error(msg);
    }

    /// True if we are Loading but haven't yet sent the DB command.
    pub fn needs_load(&self) -> bool {
        matches!(self.state, ErDiagramState::Loading { .. }) && !self.load_requested
    }

    pub fn mark_load_requested(&mut self) {
        self.load_requested = true;
    }

    pub fn schema(&self) -> Option<&str> {
        match &self.state {
            ErDiagramState::Loading { schema } => Some(schema),
            ErDiagramState::Loaded { schema, .. } => Some(schema),
            _ => None,
        }
    }

    fn auto_layout(&mut self, schema: &str, tables: &[ErTableInfo]) {
        self.node_positions.clear();
        let n = tables.len();
        if n == 0 {
            return;
        }
        let cols = (n as f32).sqrt().ceil() as usize;
        for (i, t) in tables.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let x = col as f32 * GRID_SPACING_X;
            let y = row as f32 * GRID_SPACING_Y;
            self.node_positions
                .insert((schema.to_owned(), t.table.clone()), Pos2::new(x, y));
        }
    }

    /// Draw the diagram. Returns true if a repaint is needed.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n) {
        match &self.state {
            ErDiagramState::Empty => {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new(i18n.er_no_diagram()).color(Color32::GRAY));
                });
                return;
            }
            ErDiagramState::Loading { schema } => {
                let schema = schema.clone();
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new(i18n.er_loading(&schema))
                            .color(Color32::GRAY),
                    );
                });
                return;
            }
            ErDiagramState::Error(msg) => {
                let msg = msg.clone();
                ui.centered_and_justified(|ui| {
                    ui.colored_label(Color32::RED, i18n.er_error(&msg));
                });
                return;
            }
            ErDiagramState::Loaded { .. } => {}
        }

        // Extract what we need from state (avoid borrow issues)
        let (schema, tables) = match &self.state {
            ErDiagramState::Loaded { schema, tables } => (schema.clone(), tables.clone()),
            _ => return,
        };

        // ── Toolbar ───────────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(i18n.er_title(&schema))
                    .strong()
                    .color(Color32::from_gray(200)),
            );
            ui.add_space(8.0);
            if ui.small_button(i18n.er_btn_auto_layout()).clicked() {
                self.auto_layout(&schema, &tables);
            }
            if ui.small_button("−").clicked() {
                self.zoom = (self.zoom - 0.15).max(0.2);
            }
            ui.label(format!("{:.0}%", self.zoom * 100.0));
            if ui.small_button("+").clicked() {
                self.zoom = (self.zoom + 0.15).min(4.0);
            }
            if ui.small_button(i18n.er_btn_reset()).clicked() {
                self.pan = Vec2::ZERO;
                self.zoom = 1.0;
            }
        });

        ui.separator();

        // ── Canvas ────────────────────────────────────────────────────────────
        let available = ui.available_rect_before_wrap();
        let (canvas_rect, canvas_resp) =
            ui.allocate_exact_size(available.size(), Sense::click_and_drag());

        // Scroll / zoom
        let scroll_delta = ui.input(|i| i.raw_scroll_delta);
        if canvas_resp.hovered() && scroll_delta.y != 0.0 {
            let old_zoom = self.zoom;
            self.zoom = (self.zoom + scroll_delta.y * 0.001).clamp(0.2, 4.0);
            // Pivot around cursor
            if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
                let cursor_world = (cursor - canvas_rect.min - self.pan) / old_zoom;
                self.pan = cursor - canvas_rect.min - cursor_world * self.zoom;
            }
            ui.ctx().request_repaint();
        }

        // Middle-mouse pan
        if canvas_resp.dragged_by(egui::PointerButton::Middle) {
            self.pan += canvas_resp.drag_delta();
            ui.ctx().request_repaint();
        }

        let zoom = self.zoom;
        let pan = self.pan;
        let origin = canvas_rect.min;

        let world_to_screen = |world: Pos2| -> Pos2 {
            origin + pan + world.to_vec2() * zoom
        };

        // Build FK sets per table for column coloring
        let fk_cols: HashMap<(String, String), std::collections::HashSet<String>> = tables
            .iter()
            .map(|t| {
                let cols: std::collections::HashSet<String> = t
                    .foreign_keys
                    .iter()
                    .flat_map(|fk| fk.source_columns.iter().cloned())
                    .collect();
                ((t.schema.clone(), t.table.clone()), cols)
            })
            .collect();

        // ── Drag handling ─────────────────────────────────────────────────────
        let pointer_pos = ui.input(|i| i.pointer.hover_pos());
        let primary_down = ui.input(|i| i.pointer.primary_down());
        let primary_released = ui.input(|i| i.pointer.primary_released());

        if primary_released {
            self.dragging_node = None;
        }

        if let Some(dragging) = self.dragging_node.clone() {
            if primary_down {
                if let Some(pos) = pointer_pos {
                    let world_pos: Pos2 =
                        ((pos - origin - pan) / zoom - self.drag_offset).to_pos2();
                    self.node_positions.insert(dragging, world_pos);
                    ui.ctx().request_repaint();
                }
            }
        }

        // ── Painter ───────────────────────────────────────────────────────────
        let painter = ui.painter_at(canvas_rect);

        // Background
        painter.rect_filled(canvas_rect, Rounding::ZERO, Color32::from_rgb(15, 17, 22));

        // Draw FK arrows first (behind nodes)
        for t in &tables {
            let src_key = (t.schema.clone(), t.table.clone());
            let Some(&src_pos) = self.node_positions.get(&src_key) else { continue };

            for fk in &t.foreign_keys {
                let tgt_schema = if fk.target_schema.is_empty() {
                    t.schema.clone()
                } else {
                    fk.target_schema.clone()
                };
                let tgt_key = (tgt_schema, fk.target_table.clone());
                let Some(&tgt_pos) = self.node_positions.get(&tgt_key) else { continue };

                // Find approximate row y for source FK column
                let src_col_idx = fk
                    .source_columns
                    .first()
                    .and_then(|c| t.columns.iter().position(|col| &col.name == c))
                    .unwrap_or(0);
                let tgt_col_idx = fk
                    .target_columns
                    .first()
                    .and_then(|c| {
                        // find target table
                        tables
                            .iter()
                            .find(|tt| tt.table == fk.target_table)
                            .and_then(|tt| tt.columns.iter().position(|col| &col.name == c))
                    })
                    .unwrap_or(0);

                let src_y = src_pos.y
                    + HEADER_HEIGHT
                    + src_col_idx as f32 * ROW_HEIGHT
                    + ROW_HEIGHT * 0.5;
                let src_screen = world_to_screen(Pos2::new(src_pos.x + NODE_WIDTH, src_y));

                let tgt_y = tgt_pos.y
                    + HEADER_HEIGHT
                    + tgt_col_idx as f32 * ROW_HEIGHT
                    + ROW_HEIGHT * 0.5;
                let tgt_screen = world_to_screen(Pos2::new(tgt_pos.x, tgt_y));

                // Bezier curve
                let ctrl_offset = ((tgt_screen.x - src_screen.x).abs() * 0.4).max(40.0 * zoom);
                let ctrl1 = src_screen + Vec2::new(ctrl_offset, 0.0);
                let ctrl2 = tgt_screen - Vec2::new(ctrl_offset, 0.0);

                draw_bezier(&painter, src_screen, ctrl1, ctrl2, tgt_screen, COLOR_ARROW, zoom);
            }
        }

        // Draw nodes
        for t in &tables {
            let key = (t.schema.clone(), t.table.clone());
            let Some(&world_pos) = self.node_positions.get(&key) else { continue };

            let node_h =
                HEADER_HEIGHT + t.columns.len() as f32 * ROW_HEIGHT + 4.0;
            let screen_pos = world_to_screen(world_pos);
            let node_rect = Rect::from_min_size(
                screen_pos,
                Vec2::new(NODE_WIDTH * zoom, node_h * zoom),
            );

            if !canvas_rect.intersects(node_rect) {
                continue;
            }

            // Node background + border
            painter.rect_filled(node_rect, Rounding::same(4.0 * zoom), COLOR_NODE_BG);
            painter.rect_stroke(
                node_rect,
                Rounding::same(4.0 * zoom),
                Stroke::new(1.0 * zoom, COLOR_NODE_BORDER),
            );

            // Header
            let header_rect = Rect::from_min_size(
                screen_pos,
                Vec2::new(NODE_WIDTH * zoom, HEADER_HEIGHT * zoom),
            );
            painter.rect_filled(
                header_rect,
                Rounding {
                    nw: 4.0 * zoom,
                    ne: 4.0 * zoom,
                    sw: 0.0,
                    se: 0.0,
                },
                COLOR_HEADER_BG,
            );
            painter.text(
                header_rect.center(),
                egui::Align2::CENTER_CENTER,
                &t.table,
                egui::FontId::proportional(12.0 * zoom),
                Color32::WHITE,
            );

            // Check if header is being dragged
            if canvas_resp.dragged_by(egui::PointerButton::Primary)
                && self.dragging_node.is_none()
            {
                if let Some(pos) = pointer_pos {
                    if header_rect.contains(pos) {
                        self.dragging_node = Some(key.clone());
                        self.drag_offset = (pos - screen_pos) / zoom;
                        ui.ctx().request_repaint();
                    }
                }
            }

            // Columns
            let fk_set = fk_cols.get(&key);
            for (i, col) in t.columns.iter().enumerate() {
                let row_y = screen_pos.y + (HEADER_HEIGHT + i as f32 * ROW_HEIGHT) * zoom;
                let is_pk = t.primary_keys.contains(&col.name);
                let is_fk = fk_set.map(|s| s.contains(&col.name)).unwrap_or(false);

                let prefix = if is_pk { "🔑 " } else if is_fk { "→ " } else { "  " };
                let col_color = if is_pk { COLOR_PK } else if is_fk { COLOR_FK_COL } else { COLOR_COL };

                painter.text(
                    Pos2::new(screen_pos.x + 6.0 * zoom, row_y + ROW_HEIGHT * zoom * 0.5),
                    egui::Align2::LEFT_CENTER,
                    format!("{}{}", prefix, col.name),
                    egui::FontId::monospace(10.0 * zoom),
                    col_color,
                );
                painter.text(
                    Pos2::new(
                        screen_pos.x + (NODE_WIDTH - 4.0) * zoom,
                        row_y + ROW_HEIGHT * zoom * 0.5,
                    ),
                    egui::Align2::RIGHT_CENTER,
                    &col.data_type,
                    egui::FontId::proportional(9.0 * zoom),
                    COLOR_TYPE,
                );
            }
        }
    }
}

// ── Bezier helper ─────────────────────────────────────────────────────────────

fn draw_bezier(
    painter: &egui::Painter,
    p0: Pos2,
    p1: Pos2,
    p2: Pos2,
    p3: Pos2,
    color: Color32,
    zoom: f32,
) {
    let steps = 20;
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let u = 1.0 - t;
        let x = u * u * u * p0.x
            + 3.0 * u * u * t * p1.x
            + 3.0 * u * t * t * p2.x
            + t * t * t * p3.x;
        let y = u * u * u * p0.y
            + 3.0 * u * u * t * p1.y
            + 3.0 * u * t * t * p2.y
            + t * t * t * p3.y;
        pts.push(Pos2::new(x, y));
    }
    for w in pts.windows(2) {
        painter.line_segment([w[0], w[1]], Stroke::new(1.5 * zoom, color));
    }
    // Arrow head at p3
    let dir = (p3 - p2).normalized();
    let perp = Vec2::new(-dir.y, dir.x) * 5.0 * zoom;
    let tip = p3;
    let base = p3 - dir * 8.0 * zoom;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, base + perp, base - perp],
        color,
        Stroke::NONE,
    ));
}
