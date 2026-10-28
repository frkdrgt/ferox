//! Home screen — shown in the central panel while no connection is open.
//!
//! Everything here comes from state the app already has in memory (profiles,
//! history, snippets, AI config): no DB round-trips, nothing new persisted.

use egui::{Color32, RichText, Sense, Stroke, Vec2};

use crate::config::{AiConfig, ConnectionProfile};
use crate::history::QueryHistory;
use crate::i18n::I18n;
use crate::snippets::Snippets;
use crate::ui::theme::{self as t, icon};

pub enum HomeAction {
    Connect(usize),
    NewConnection,
    ImportDbeaver,
}

const MAX_CONTENT_W: f32 = 1080.0;
const PROFILE_ROW_H: f32 = 46.0;
const RECENT_LIMIT: usize = 6;

pub fn show(
    ui: &mut egui::Ui,
    i18n: &I18n,
    profiles: &[ConnectionProfile],
    history: &QueryHistory,
    snippets: &Snippets,
    ai: &AiConfig,
    logo: Option<&egui::TextureHandle>,
) -> Option<HomeAction> {
    let mut action = None;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Center a max-width column; side gutters absorb wide windows.
            let full_w = ui.available_width();
            let content_w = full_w.min(MAX_CONTENT_W) - 48.0;
            let gutter = ((full_w - content_w) / 2.0).max(16.0);

            ui.horizontal(|ui| {
                ui.add_space(gutter);
                ui.vertical(|ui| {
                    ui.set_width(content_w);
                    ui.add_space(24.0);
                    header(ui, i18n, logo);
                    ui.add_space(20.0);
                    stats_row(ui, i18n, profiles, history, snippets);
                    ui.add_space(14.0);

                    let gap = 14.0;
                    let left_w = ((content_w - gap) * 0.6).floor();
                    let right_w = content_w - gap - left_w;
                    // Children of a horizontal row inherit its layout — give each
                    // card an explicit top-down column of fixed width.
                    let column = egui::Layout::top_down(egui::Align::Min);
                    // Equal card heights: reuse the taller card's height from last frame.
                    let h_id = ui.id().with("home_row2_h");
                    let row_h: f32 = ui.ctx().data(|d| d.get_temp(h_id)).unwrap_or(0.0);
                    let mut tallest = 0.0_f32;
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        let r = ui.allocate_ui_with_layout(Vec2::new(left_w, 0.0), column, |ui| {
                            ui.set_width(left_w);
                            if let Some(a) = quick_start(ui, i18n, profiles, row_h) {
                                action = Some(a);
                            }
                        });
                        tallest = tallest.max(r.response.rect.height());
                        let r = ui.allocate_ui_with_layout(Vec2::new(right_w, 0.0), column, |ui| {
                            ui.set_width(right_w);
                            if let Some(a) = shortcuts(ui, i18n, row_h) {
                                action = Some(a);
                            }
                        });
                        tallest = tallest.max(r.response.rect.height());
                    });
                    if (tallest - row_h).abs() > 0.5 {
                        ui.ctx().data_mut(|d| d.insert_temp(h_id, tallest));
                        ui.ctx().request_repaint();
                    }
                    ui.add_space(14.0);
                    recent_queries(ui, i18n, history);
                    ui.add_space(14.0);
                    ai_card(ui, i18n, ai);
                    ui.add_space(24.0);
                });
            });
        });

    action
}

// ── Sections ──────────────────────────────────────────────────────────────────

fn header(ui: &mut egui::Ui, i18n: &I18n, logo: Option<&egui::TextureHandle>) {
    ui.horizontal(|ui| {
        if let Some(tex) = logo {
            ui.image((tex.id(), Vec2::splat(44.0)));
            ui.add_space(6.0);
        }
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("ferox").size(26.0).strong().color(t::TEXT));
                t::pill(ui, &format!("v{}", env!("CARGO_PKG_VERSION")), t::TEXT_MUTED);
            });
            ui.label(RichText::new(i18n.home_tagline()).color(t::TEXT_MUTED));
        });
    });
}

fn stats_row(
    ui: &mut egui::Ui,
    i18n: &I18n,
    profiles: &[ConnectionProfile],
    history: &QueryHistory,
    snippets: &Snippets,
) {
    let mut groups: Vec<&str> = profiles.iter().filter_map(|p| p.group.as_deref()).collect();
    groups.sort_unstable();
    groups.dedup();

    let stats = [
        (icon::DATABASE, i18n.home_saved_connections(), profiles.len()),
        (icon::FOLDER, i18n.home_groups(), groups.len()),
        (icon::CLOCK_COUNTER_CLOCKWISE, i18n.home_history_count(), history.entries().len()),
        (icon::BOOKMARK_SIMPLE, i18n.home_saved_queries(), snippets.entries.len()),
    ];
    ui.columns(stats.len(), |cols| {
        for (col, (glyph, label, value)) in cols.iter_mut().zip(stats) {
            t::stat_card(col, glyph, label, &value.to_string(), t::TEXT);
        }
    });
}

/// Inner height = outer `row_h` minus the card's inner margins (stroke is drawn outside layout).
fn card_min_height(row_h: f32) -> f32 {
    (row_h - 28.0).max(250.0)
}

fn quick_start(
    ui: &mut egui::Ui,
    i18n: &I18n,
    profiles: &[ConnectionProfile],
    row_h: f32,
) -> Option<HomeAction> {
    let mut action = None;
    t::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(card_min_height(row_h));
        t::card_header(ui, icon::LIGHTNING, i18n.home_quick_start());

        if profiles.is_empty() {
            t::empty_state(ui, icon::PLUGS, i18n.home_no_profiles(), i18n.home_no_profiles_hint());
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    // Center the two buttons as a group.
                    let w = 300.0_f32.min(ui.available_width());
                    ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                    if ui.add(primary_button(&format!("{}  {}", icon::PLUS, i18n.menu_new_connection()))).clicked() {
                        action = Some(HomeAction::NewConnection);
                    }
                    if ui.button(format!("{}  {}", icon::DOWNLOAD_SIMPLE, i18n.menu_import_dbeaver())).clicked() {
                        action = Some(HomeAction::ImportDbeaver);
                    }
                });
            });
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("home_profiles")
            .max_height(PROFILE_ROW_H * 6.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (i, p) in profiles.iter().enumerate() {
                    if profile_row(ui, i18n, p) {
                        action = Some(HomeAction::Connect(i));
                    }
                }
            });
    });
    action
}

/// One saved profile. Returns true when the row or its Connect button is used.
fn profile_row(ui: &mut egui::Ui, i18n: &I18n, p: &ConnectionProfile) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), PROFILE_ROW_H), Sense::click());
    if !ui.is_rect_visible(rect) {
        return false;
    }
    let painter = ui.painter();
    if resp.hovered() {
        painter.rect_filled(rect, 6.0, t::SURFACE_RAISED);
    }

    // Danger-tag / group color as a left bar (same meaning as tabs & switcher).
    let tag = p.resolved_color().map(|[r, g, b]| Color32::from_rgb(r, g, b));
    if let Some(c) = tag {
        let bar = egui::Rect::from_min_size(
            rect.left_top() + Vec2::new(4.0, 10.0),
            Vec2::new(3.0, rect.height() - 20.0),
        );
        painter.rect_filled(bar, 1.5, c);
    }

    painter.text(
        rect.left_center() + Vec2::new(16.0, 0.0),
        egui::Align2::LEFT_CENTER,
        icon::DATABASE,
        egui::FontId::proportional(20.0),
        tag.unwrap_or(t::ACCENT),
    );

    let btn_w = 92.0;
    let text_x = rect.left() + 46.0;
    let text_w = (rect.right() - btn_w - 16.0 - text_x).max(40.0);
    let name = one_line(ui, &p.name, egui::FontId::proportional(13.5), t::TEXT, text_w);
    let sub = format!("{}@{}:{}/{}", p.user, p.host, p.port, p.database);
    let sub = one_line(ui, &sub, egui::FontId::proportional(11.0), t::TEXT_MUTED, text_w);
    painter.galley(egui::pos2(text_x, rect.center().y - 15.0), name, t::TEXT);
    painter.galley(egui::pos2(text_x, rect.center().y + 2.0), sub, t::TEXT_MUTED);

    let btn_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - btn_w / 2.0 - 8.0, rect.center().y),
        Vec2::new(btn_w, 26.0),
    );
    let btn_label = format!("{}  {}", icon::PLUGS_CONNECTED, i18n.home_connect());
    let btn = if resp.hovered() { primary_button(&btn_label) } else { egui::Button::new(btn_label) };
    let clicked = ui.put(btn_rect, btn).clicked();

    clicked || resp.double_clicked()
}

fn shortcuts(ui: &mut egui::Ui, i18n: &I18n, row_h: f32) -> Option<HomeAction> {
    let mut action = None;
    t::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(card_min_height(row_h));
        t::card_header(ui, icon::KEYBOARD, i18n.home_shortcuts());

        if link_row(ui, icon::PLUS, i18n.menu_new_connection()) {
            action = Some(HomeAction::NewConnection);
        }
        if link_row(ui, icon::DOWNLOAD_SIMPLE, i18n.menu_import_dbeaver()) {
            action = Some(HomeAction::ImportDbeaver);
        }
        ui.add_space(8.0);

        let keys: [(&str, &str); 7] = [
            ("Ctrl+Enter", i18n.kb_run()),
            ("Ctrl+T", i18n.kb_new_tab()),
            ("Ctrl+W", i18n.kb_close_tab()),
            ("Ctrl+F", i18n.kb_find()),
            ("Ctrl+Shift+S", i18n.kb_save_query()),
            ("Ctrl+Space", i18n.kb_autocomplete()),
            ("Shift+Alt+F", i18n.kb_format()),
        ];
        egui::Grid::new("home_keys").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            for (k, desc) in keys {
                key_chip(ui, k);
                ui.label(RichText::new(desc).small().color(t::TEXT_MUTED));
                ui.end_row();
            }
        });
    });
    action
}

fn recent_queries(ui: &mut egui::Ui, i18n: &I18n, history: &QueryHistory) {
    t::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        t::card_header(ui, icon::CLOCK_COUNTER_CLOCKWISE, i18n.home_recent_queries());

        let entries = history.entries();
        if entries.is_empty() {
            ui.label(RichText::new(i18n.home_no_history()).color(t::TEXT_FAINT));
            return;
        }
        ui.spacing_mut().item_spacing.y = 2.0;
        for e in entries.iter().rev().take(RECENT_LIMIT) {
            let (rect, resp) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
            if !ui.is_rect_visible(rect) {
                continue;
            }
            if resp.hovered() {
                ui.painter().rect_filled(rect, 5.0, t::SURFACE_RAISED);
            }
            let when = e.executed_at.format("%d.%m %H:%M").to_string();
            ui.painter().text(
                rect.left_center() + Vec2::new(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                when,
                egui::FontId::proportional(11.0),
                t::TEXT_FAINT,
            );
            let flat: String = e.sql.split_whitespace().collect::<Vec<_>>().join(" ");
            let sql_w = rect.width() - 100.0;
            let g = one_line(ui, &flat, egui::FontId::monospace(12.0), t::TEXT, sql_w);
            let y = rect.center().y - g.size().y / 2.0;
            ui.painter().galley(egui::pos2(rect.left() + 92.0, y), g, t::TEXT);
            if resp.on_hover_text(i18n.home_copy_query()).clicked() {
                ui.ctx().copy_text(e.sql.clone());
            }
        }
    });
}

fn ai_card(ui: &mut egui::Ui, i18n: &I18n, ai: &AiConfig) {
    t::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let color = if ai.is_configured() { t::SUCCESS } else { t::TEXT_MUTED };
            ui.label(RichText::new(icon::SPARKLE).size(18.0).color(color));
            ui.vertical(|ui| {
                ui.label(RichText::new(i18n.home_ai_title()).strong().color(t::TEXT));
                let text = if ai.is_configured() {
                    i18n.home_ai_on(&ai.provider, ai.effective_model())
                } else {
                    i18n.home_ai_off().to_owned()
                };
                ui.add(egui::Label::new(RichText::new(text).small().color(t::TEXT_MUTED)).wrap());
            });
        });
    });
}

// ── Small widgets ─────────────────────────────────────────────────────────────

fn primary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned()).color(Color32::WHITE)).fill(t::ACCENT)
}

fn key_chip(ui: &mut egui::Ui, keys: &str) {
    egui::Frame::none()
        .fill(t::BG)
        .stroke(Stroke::new(1.0, t::BORDER_STRONG))
        .rounding(4.0)
        .inner_margin(egui::Margin::symmetric(6.0, 1.0))
        .show(ui, |ui| {
            ui.label(RichText::new(keys).monospace().size(11.0).color(t::TEXT));
        });
}

/// Full-width icon + text row that highlights on hover; true when clicked.
fn link_row(ui: &mut egui::Ui, glyph: &str, text: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if resp.hovered() {
            painter.rect_filled(rect, 5.0, t::SURFACE_RAISED);
        }
        let color = if resp.hovered() { t::ACCENT } else { t::TEXT };
        painter.text(
            rect.left_center() + Vec2::new(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            glyph,
            egui::FontId::proportional(15.0),
            color,
        );
        painter.text(
            rect.left_center() + Vec2::new(30.0, 0.0),
            egui::Align2::LEFT_CENTER,
            text,
            egui::FontId::proportional(13.0),
            color,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// Single-line galley cut with "…" at `max_w`.
fn one_line(
    ui: &egui::Ui,
    text: &str,
    font: egui::FontId,
    color: Color32,
    max_w: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping {
        max_width: max_w,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.fonts(|f| f.layout_job(job))
}
