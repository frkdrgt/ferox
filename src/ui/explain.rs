use egui::{Color32, RichText, Stroke};
use serde_json::Value;

// ── Data model ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ExplainResult {
    pub planning_ms: Option<f64>,
    pub execution_ms: Option<f64>,
    pub root: PlanNode,
    /// Max total_cost in the whole tree — used for relative bar widths.
    pub max_cost: f64,
    /// Max actual_total_time (set only when ANALYZE was used).
    pub max_time: f64,
}

#[derive(Debug, Clone)]
pub struct PlanNode {
    pub node_type: String,
    pub parent_rel: Option<String>, // "Outer" | "Inner"
    pub relation: Option<String>,
    pub alias: Option<String>,
    pub join_type: Option<String>,
    pub startup_cost: f64,
    pub total_cost: f64,
    pub plan_rows: i64,
    pub plan_width: i32,
    // ANALYZE fields
    pub actual_startup_time: Option<f64>,
    pub actual_total_time: Option<f64>,
    pub actual_rows: Option<i64>,
    pub actual_loops: Option<i64>,
    /// Extra predicates / conditions (key, value).
    pub extra: Vec<(String, String)>,
    pub children: Vec<PlanNode>,
}

impl ExplainResult {
    /// Parse the text returned by `EXPLAIN (ANALYZE, FORMAT JSON)`.
    pub fn parse(json_text: &str) -> Option<Self> {
        let arr: Value = serde_json::from_str(json_text).ok()?;
        let obj = arr.as_array()?.first()?.as_object()?;

        let planning_ms = obj.get("Planning Time").and_then(Value::as_f64);
        let execution_ms = obj.get("Execution Time").and_then(Value::as_f64);

        let root = parse_node(obj.get("Plan")?)?;
        let max_cost = root.max_total_cost();
        let max_time = root.max_actual_time();

        Some(Self {
            planning_ms,
            execution_ms,
            root,
            max_cost,
            max_time,
        })
    }

    /// Walk the plan tree and collect human-readable optimization suggestions.
    pub fn collect_suggestions(&self) -> Vec<String> {
        let mut suggestions: Vec<String> = Vec::new();
        collect_node_suggestions(&self.root, &mut suggestions, self.max_time);
        suggestions.dedup();
        suggestions
    }
}

impl PlanNode {
    fn max_total_cost(&self) -> f64 {
        self.children
            .iter()
            .fold(self.total_cost, |m, c| m.max(c.max_total_cost()))
    }

    fn max_actual_time(&self) -> f64 {
        self.children.iter().fold(
            self.actual_total_time.unwrap_or(0.0),
            |m, c| m.max(c.max_actual_time()),
        )
    }

    /// True when this node has the highest actual_total_time in the plan.
    fn is_slowest(&self, max_time: f64) -> bool {
        max_time > 0.0
            && self
                .actual_total_time
                .map(|t| (t - max_time).abs() < 0.001)
                .unwrap_or(false)
    }

    /// Row estimation error ratio: actual / plan.  None if data unavailable.
    fn estimation_ratio(&self) -> Option<f64> {
        if self.plan_rows <= 0 {
            return None;
        }
        let actual = self.actual_rows? * self.actual_loops.unwrap_or(1);
        Some(actual as f64 / self.plan_rows as f64)
    }
}

fn collect_node_suggestions(node: &PlanNode, out: &mut Vec<String>, max_time: f64) {
    // 1. Seq Scan — possible missing index
    if node.node_type == "Seq Scan" {
        let name = node.relation.as_deref().unwrap_or("?");
        out.push(format!(
            "\"{}\" tablosunda tam tablo taraması (Seq Scan) — filtreleniyorsa index eklemeyi düşünün",
            name
        ));
    }

    // 2. Bad row estimation — planner stats may be stale
    if let Some(ratio) = node.estimation_ratio() {
        if ratio > 10.0 || ratio < 0.1 {
            let name = node
                .relation
                .as_deref()
                .unwrap_or(node.node_type.as_str());
            let plan = node.plan_rows;
            let actual = node.actual_rows.unwrap_or(0) * node.actual_loops.unwrap_or(1);
            out.push(format!(
                "\"{}\" için satır tahmini hatalı (plan: {plan}, gerçek: {actual}, oran: {ratio:.1}×) — \
                 ANALYZE çalıştırın veya istatistikleri güncelleyin",
                name
            ));
        }
    }

    // 3. Expensive Sort — sorting without an index
    if (node.node_type == "Sort" || node.node_type == "Incremental Sort")
        && max_time > 0.0
        && node.actual_total_time.map(|t| t / max_time > 0.25).unwrap_or(false)
    {
        let key = node
            .extra
            .iter()
            .find(|(k, _)| k == "Sort Key")
            .map(|(_, v)| format!(" ({v})"))
            .unwrap_or_default();
        out.push(format!(
            "Pahalı Sort işlemi{key} — sıralama sütununa index eklemeyi düşünün"
        ));
    }

    // 4. Nested Loop with many rows — can be O(n²)
    if node.node_type == "Nested Loop" {
        if let Some(actual) = node.actual_rows {
            let loops = node.actual_loops.unwrap_or(1);
            if actual * loops > 10_000 {
                out.push(
                    "Nested Loop büyük veri setinde çalışıyor — Hash Join daha verimli olabilir"
                        .into(),
                );
            }
        }
    }

    for child in &node.children {
        collect_node_suggestions(child, out, max_time);
    }
}

fn parse_node(v: &Value) -> Option<PlanNode> {
    let obj = v.as_object()?;

    let get_str = |key: &str| obj.get(key)?.as_str().map(str::to_owned);
    let get_f64 = |key: &str| obj.get(key)?.as_f64();
    let get_i64 = |key: &str| obj.get(key)?.as_i64();

    let node_type = get_str("Node Type")?;

    // Extra predicate fields
    let extra_keys = [
        "Index Name",
        "Index Cond",
        "Filter",
        "Rows Removed by Filter",
        "Hash Cond",
        "Merge Cond",
        "Join Filter",
        "Sort Key",
        "Sort Method",
        "Group Key",
        "Recheck Cond",
        "TID Cond",
        "Conflict Resolution",
        "One-Time Filter",
    ];
    let mut extra: Vec<(String, String)> = Vec::new();
    for key in &extra_keys {
        if let Some(val) = obj.get(*key) {
            let s = match val {
                Value::String(s) => s.clone(),
                Value::Array(a) => a
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                Value::Number(n) => n.to_string(),
                _ => val.to_string(),
            };
            extra.push((key.to_string(), s));
        }
    }

    let children = obj
        .get("Plans")
        .and_then(Value::as_array)
        .map(|plans| plans.iter().filter_map(parse_node).collect())
        .unwrap_or_default();

    Some(PlanNode {
        node_type,
        parent_rel: get_str("Parent Relationship"),
        relation: get_str("Relation Name"),
        alias: get_str("Alias"),
        join_type: get_str("Join Type"),
        startup_cost: get_f64("Startup Cost").unwrap_or(0.0),
        total_cost: get_f64("Total Cost").unwrap_or(0.0),
        plan_rows: get_i64("Plan Rows").unwrap_or(0),
        plan_width: get_i64("Plan Width").unwrap_or(0) as i32,
        actual_startup_time: get_f64("Actual Startup Time"),
        actual_total_time: get_f64("Actual Total Time"),
        actual_rows: get_i64("Actual Rows"),
        actual_loops: get_i64("Actual Loops"),
        extra,
        children,
    })
}

// ── Renderer ─────────────────────────────────────────────────────────────────

/// Top-level render entry point. Call this inside a panel/frame.
pub fn render_explain(ui: &mut egui::Ui, result: &ExplainResult) {
    // ── Summary bar ──────────────────────────────────────────────────────────
    egui::Frame::none()
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("EXPLAIN ANALYZE").strong().monospace());
                ui.separator();
                if let Some(p) = result.planning_ms {
                    ui.label(format!("Planning: {p:.2} ms"));
                    ui.separator();
                }
                if let Some(e) = result.execution_ms {
                    let color = if e > 1000.0 {
                        Color32::from_rgb(220, 80, 80)
                    } else if e > 100.0 {
                        Color32::from_rgb(220, 180, 60)
                    } else {
                        Color32::from_rgb(80, 200, 120)
                    };
                    ui.colored_label(color, format!("Execution: {e:.2} ms"));
                }
            });
        });

    ui.add_space(4.0);

    // ── Suggestions panel ────────────────────────────────────────────────────
    let suggestions = result.collect_suggestions();
    if !suggestions.is_empty() {
        egui::Frame::none()
            .fill(Color32::from_rgba_premultiplied(60, 40, 10, 220))
            .stroke(Stroke::new(1.0, Color32::from_rgb(180, 130, 40)))
            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
            .rounding(4.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("⚡ Öneriler")
                            .strong()
                            .color(Color32::from_rgb(220, 180, 60)),
                    );
                });
                ui.add_space(2.0);
                for s in &suggestions {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("•")
                                .color(Color32::from_rgb(220, 180, 60)),
                        );
                        ui.label(RichText::new(s).small().color(Color32::from_rgb(220, 200, 140)));
                    });
                }
            });
        ui.add_space(6.0);
    }

    // ── Plan tree ────────────────────────────────────────────────────────────
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut id_counter = 0usize;
            render_node(ui, &result.root, result, 0, &mut id_counter);
        });
}

fn render_node(
    ui: &mut egui::Ui,
    node: &PlanNode,
    result: &ExplainResult,
    depth: usize,
    id: &mut usize,
) {
    let my_id = *id;
    *id += 1;

    let is_slowest = node.is_slowest(result.max_time);
    let is_seq_scan = node.node_type == "Seq Scan";

    let header_text = node_header_text(node);
    let color = if is_slowest && result.max_time > 0.0 {
        Color32::from_rgb(230, 80, 80) // red for slowest
    } else {
        node_color(&node.node_type)
    };

    let default_open = depth < 3;

    let id_source = egui::Id::new("explain_node").with(my_id);
    egui::CollapsingHeader::new(RichText::new(&header_text).color(color).strong())
        .default_open(default_open)
        .id_source(id_source)
        .show(ui, |ui| {
            // ── Badges row ───────────────────────────────────────────────
            ui.horizontal_wrapped(|ui| {
                // "Slowest" badge
                if is_slowest && result.max_time > 0.0 {
                    egui::Frame::none()
                        .fill(Color32::from_rgba_premultiplied(180, 40, 40, 200))
                        .rounding(3.0)
                        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("🐢 En Yavaş Node")
                                    .small()
                                    .color(Color32::WHITE),
                            );
                        });
                }

                // Seq Scan warning badge
                if is_seq_scan {
                    egui::Frame::none()
                        .fill(Color32::from_rgba_premultiplied(180, 100, 20, 200))
                        .rounding(3.0)
                        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("⚠ Full Scan")
                                    .small()
                                    .color(Color32::WHITE),
                            );
                        });
                }
            });

            // ── Stats row ────────────────────────────────────────────────
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!(
                        "cost {:.2}..{:.2}",
                        node.startup_cost, node.total_cost
                    ))
                    .small()
                    .monospace(),
                );

                ui.separator();
                ui.label(
                    RichText::new(format!("rows ≈ {}", node.plan_rows))
                        .small()
                        .monospace(),
                );

                ui.separator();
                ui.label(
                    RichText::new(format!("width {}", node.plan_width))
                        .small()
                        .monospace(),
                );

                if let (Some(t), Some(r)) = (node.actual_total_time, node.actual_rows) {
                    let loops = node.actual_loops.unwrap_or(1);
                    ui.separator();
                    ui.colored_label(
                        time_color(t, result.max_time),
                        RichText::new(format!("time {:.3} ms", t)).small().monospace(),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new(format!("actual rows {}", r * loops))
                            .small()
                            .monospace(),
                    );

                    // Estimation error badge
                    if let Some(ratio) = node.estimation_ratio() {
                        let (badge_color, label) = if ratio > 10.0 || ratio < 0.1 {
                            // Severe mismatch — red badge
                            (
                                Color32::from_rgba_premultiplied(200, 50, 50, 220),
                                format!("est ×{:.1} ⚠", ratio),
                            )
                        } else if ratio > 3.0 || ratio < 0.33 {
                            // Moderate mismatch — yellow badge
                            (
                                Color32::from_rgba_premultiplied(180, 140, 20, 220),
                                format!("est ×{:.1}", ratio),
                            )
                        } else {
                            // OK — plain dim text
                            (Color32::TRANSPARENT, format!("est ×{:.1}", ratio))
                        };

                        if badge_color != Color32::TRANSPARENT {
                            egui::Frame::none()
                                .fill(badge_color)
                                .rounding(3.0)
                                .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(&label)
                                            .small()
                                            .monospace()
                                            .color(Color32::WHITE),
                                    );
                                });
                        } else {
                            ui.label(
                                RichText::new(&label)
                                    .small()
                                    .monospace()
                                    .color(Color32::GRAY),
                            );
                        }
                    }
                }
            });

            // ── Cost / time bar ──────────────────────────────────────────
            let use_time = result.max_time > 0.0 && node.actual_total_time.is_some();
            let bar_ratio = if use_time {
                (node.actual_total_time.unwrap_or(0.0) / result.max_time.max(0.001)) as f32
            } else if result.max_cost > 0.0 {
                (node.total_cost / result.max_cost) as f32
            } else {
                0.0
            }
            .clamp(0.0, 1.0);

            render_bar(ui, bar_ratio, color);

            // ── Extra predicates ─────────────────────────────────────────
            for (key, val) in &node.extra {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(key).small().color(Color32::GRAY));
                    ui.label(RichText::new(val).small().monospace());
                });
            }

            // ── Children ─────────────────────────────────────────────────
            for child in &node.children {
                render_node(ui, child, result, depth + 1, id);
            }
        });
}

fn render_bar(ui: &mut egui::Ui, ratio: f32, color: Color32) {
    let desired_size = egui::vec2(ui.available_width().min(400.0), 6.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    let painter = ui.painter();
    // Background
    painter.rect_filled(rect, 0.0, Color32::from_gray(40));
    // Filled portion
    let fill_rect = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(rect.width() * ratio, rect.height()),
    );
    painter.rect_filled(fill_rect, 0.0, color.linear_multiply(0.8));
    // Border
    painter.rect_stroke(rect, 0.0, Stroke::new(0.5, Color32::from_gray(80)));
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn node_header_text(node: &PlanNode) -> String {
    let mut parts = Vec::new();

    // Prefix: join type for joins
    if let Some(jt) = &node.join_type {
        parts.push(format!("{jt} "));
    }

    parts.push(node.node_type.clone());

    // Relation
    if let Some(rel) = &node.relation {
        parts.push(format!(" on {rel}"));
        if let Some(alias) = &node.alias {
            if alias != rel {
                parts.push(format!(" ({alias})"));
            }
        }
    }

    // Parent role hint
    if let Some(pr) = &node.parent_rel {
        match pr.as_str() {
            "Inner" => parts.push("  [inner]".into()),
            "Outer" => parts.push("  [outer]".into()),
            _ => {}
        }
    }

    parts.concat()
}

fn node_color(node_type: &str) -> Color32 {
    match node_type {
        // Scans
        "Seq Scan" => Color32::from_rgb(220, 120, 50),  // orange — potentially slow
        "Index Scan" | "Index Only Scan" => Color32::from_rgb(80, 200, 120), // green — fast
        "Bitmap Heap Scan" | "Bitmap Index Scan" => Color32::from_rgb(80, 180, 200), // teal
        "Tid Scan" | "Tid Range Scan" => Color32::from_rgb(100, 220, 180),
        "Function Scan" | "Values Scan" | "Subquery Scan" => Color32::from_rgb(180, 160, 80),
        "CTE Scan" | "Named Tuplestore Scan" => Color32::from_rgb(160, 130, 200),
        // Joins
        "Nested Loop" => Color32::from_rgb(180, 100, 220),
        "Hash Join" => Color32::from_rgb(130, 100, 220),
        "Merge Join" => Color32::from_rgb(100, 130, 220),
        // Aggregates
        "Aggregate" | "HashAggregate" | "GroupAggregate" | "MixedAggregate" => {
            Color32::from_rgb(80, 180, 220)
        }
        // Sort / Limit
        "Sort" | "Incremental Sort" => Color32::from_rgb(220, 200, 80),
        "Limit" => Color32::from_gray(160),
        // Materialize / Hash
        "Hash" => Color32::from_rgb(120, 120, 220),
        "Materialize" => Color32::from_rgb(160, 160, 100),
        // Append / Union
        "Append" | "Merge Append" | "Recursive Union" => Color32::from_rgb(180, 120, 100),
        // Modify
        "Insert" | "Update" | "Delete" | "Merge" => Color32::from_rgb(220, 80, 80),
        _ => Color32::from_gray(200),
    }
}

fn time_color(time_ms: f64, max_time: f64) -> Color32 {
    if max_time <= 0.0 {
        return Color32::GRAY;
    }
    let ratio = time_ms / max_time;
    if ratio > 0.7 {
        Color32::from_rgb(230, 80, 80)
    } else if ratio > 0.3 {
        Color32::from_rgb(220, 180, 60)
    } else {
        Color32::from_rgb(100, 200, 100)
    }
}
