//! Design tokens + small layout helpers shared by every panel.
//!
//! Three surface layers (JetBrains New UI dark): `BG` behind content, `SURFACE`
//! for chrome (menu bar, sidebar, status bar) and cards, `SURFACE_RAISED` for
//! hovered rows / headers. Keep new colors here instead of hardcoding them.

use egui::{Color32, RichText, Stroke};

pub use egui_phosphor::regular as icon;

// ── Surfaces ──────────────────────────────────────────────────────────────────
pub const BG: Color32 = Color32::from_rgb(30, 31, 34); // #1e1f22
pub const SURFACE: Color32 = Color32::from_rgb(43, 45, 48); // #2b2d30
pub const SURFACE_RAISED: Color32 = Color32::from_rgb(57, 59, 64); // #393b40
pub const EDITOR_BG: Color32 = Color32::from_rgb(25, 26, 28); // #191a1c
pub const BORDER: Color32 = Color32::from_rgb(57, 59, 64); // #393b40
pub const BORDER_STRONG: Color32 = Color32::from_rgb(78, 81, 87); // #4e5157

// ── Text ──────────────────────────────────────────────────────────────────────
pub const TEXT: Color32 = Color32::from_rgb(223, 225, 229); // #dfe1e5
pub const TEXT_MUTED: Color32 = Color32::from_rgb(134, 138, 145); // #868a91
pub const TEXT_FAINT: Color32 = Color32::from_rgb(95, 99, 105); // #5f6369

// ── Accents ───────────────────────────────────────────────────────────────────
pub const ACCENT: Color32 = Color32::from_rgb(78, 159, 222); // #4e9fde
pub const SUCCESS: Color32 = Color32::from_rgb(95, 184, 101); // #5fb865
pub const WARNING: Color32 = Color32::from_rgb(230, 180, 80); // #e6b450
pub const DANGER: Color32 = Color32::from_rgb(199, 84, 80); // #c75450
pub const NUMBER: Color32 = Color32::from_rgb(104, 151, 187); // #6897bb (Darcula number)

pub const CARD_ROUNDING: f32 = 8.0;

/// Panel frame for chrome surfaces (menu bar, sidebar, status bar).
pub fn chrome_frame(margin: egui::Margin) -> egui::Frame {
    egui::Frame::none().fill(SURFACE).inner_margin(margin)
}

/// Rounded, bordered card on top of `BG`.
pub fn card_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(CARD_ROUNDING)
        .inner_margin(egui::Margin::same(14.0))
}

/// Card title row: icon + semibold-looking label, followed by a hairline.
pub fn card_header(ui: &mut egui::Ui, glyph: &str, title: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(glyph).size(15.0).color(TEXT_MUTED));
        ui.label(RichText::new(title).size(14.0).strong().color(TEXT));
    });
    ui.add_space(6.0);
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, BORDER));
    ui.add_space(6.0);
}

/// Centered placeholder for an area with nothing to show yet.
pub fn empty_state(ui: &mut egui::Ui, glyph: &str, title: &str, hint: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() * 0.25).clamp(16.0, 120.0));
        ui.label(RichText::new(glyph).size(34.0).color(TEXT_FAINT));
        ui.add_space(6.0);
        ui.label(RichText::new(title).size(14.0).color(TEXT_MUTED));
        if !hint.is_empty() {
            ui.add_space(2.0);
            ui.label(RichText::new(hint).small().color(TEXT_FAINT));
        }
    });
}

/// Phosphor Regular reduced to the glyphs the app uses (`scripts/subset_icons.py`).
const ICON_FONT: &[u8] = include_bytes!("../../assets/phosphor-subset.ttf");

/// Install fonts once at startup. Everything is embedded in the binary — no
/// system font files are read (Segoe UI Emoji alone is 12 MB of disk I/O + RAM).
/// Not part of the per-frame style pass: `set_fonts` rebuilds the font atlas.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("phosphor".into(), egui::FontData::from_static(ICON_FONT));
    // Proportional: icons right after Ubuntu, then Hack for the arrows/triangles
    // Ubuntu lacks. Monospace: icons for the sidebar/grid. Both are already loaded.
    if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        prop.insert(1, "phosphor".into());
        prop.push("Hack".into());
    }
    if let Some(mono) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        mono.push("phosphor".into());
    }
    ctx.set_fonts(fonts);
}

// ── Shared widgets ────────────────────────────────────────────────────────────

/// Header-cell background for `egui_extras` tables: fills the cell plus half the
/// inter-column gap on each side, with a right rule and a bottom border.
pub fn grid_header_bg(ui: &egui::Ui) {
    let half_gap = ui.spacing().item_spacing.x / 2.0;
    let r = ui.max_rect().expand2(egui::vec2(half_gap, 0.0));
    let painter = ui.painter();
    painter.rect_filled(r, 0.0, SURFACE);
    painter.vline(r.right(), r.y_range(), Stroke::new(1.0, BORDER));
    painter.hline(r.x_range(), r.bottom(), Stroke::new(1.0, BORDER_STRONG));
}

/// `grid_header_bg` + a single-line header label.
pub fn grid_header(ui: &mut egui::Ui, text: &str) {
    grid_header_bg(ui);
    ui.add(egui::Label::new(RichText::new(text).color(TEXT)).truncate());
}

/// Faint vertical rule on a body cell's right edge.
pub fn grid_col_rule(ui: &egui::Ui) {
    let half_gap = ui.spacing().item_spacing.x / 2.0;
    let r = ui.max_rect();
    ui.painter().vline(r.right() + half_gap, r.y_range(), Stroke::new(1.0, BORDER));
}

/// Metric card: icon + muted label on top, big value below.
pub fn stat_card(ui: &mut egui::Ui, glyph: &str, label: &str, value: &str, value_color: Color32) {
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(glyph).color(TEXT_MUTED));
            ui.add(egui::Label::new(RichText::new(label).small().color(TEXT_MUTED)).truncate());
        });
        ui.add_space(2.0);
        ui.add(egui::Label::new(RichText::new(value).size(22.0).strong().color(value_color)).truncate());
    });
}

/// Small rounded outline badge.
pub fn pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::none()
        .stroke(Stroke::new(1.0, BORDER_STRONG))
        .rounding(9.0)
        .inner_margin(egui::Margin::symmetric(7.0, 1.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(color));
        });
}

/// Paint a filled count badge right-aligned at `right_center` (painter-only rows).
pub fn paint_count_badge(painter: &egui::Painter, right_center: egui::Pos2, count: usize) {
    let font = egui::FontId::proportional(10.0);
    let galley = painter.layout_no_wrap(count.to_string(), font, TEXT_MUTED);
    let w = galley.size().x + 10.0;
    let rect = egui::Rect::from_center_size(
        egui::pos2(right_center.x - w / 2.0, right_center.y),
        egui::vec2(w, 15.0),
    );
    painter.rect_filled(rect, 7.5, SURFACE_RAISED);
    painter.galley(rect.center() - galley.size() / 2.0, galley, TEXT_MUTED);
}

/// Flat toolbar row: buttons get no fill/stroke until hovered.
pub fn flat_buttons<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope(|ui| {
        let w = &mut ui.visuals_mut().widgets;
        w.inactive.weak_bg_fill = Color32::TRANSPARENT;
        w.inactive.bg_stroke = Stroke::NONE;
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
        w.hovered.weak_bg_fill = SURFACE_RAISED;
        w.hovered.bg_stroke = Stroke::NONE;
        ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
        add(ui)
    })
    .inner
}

/// Underlined tab toggle (dashboard sub-views, result tabs).
pub fn tab_toggle(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    let color = if selected { TEXT } else { TEXT_MUTED };
    let resp = ui.add(
        egui::Button::new(RichText::new(text).color(color))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE),
    );
    if selected {
        let r = resp.rect;
        ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(2.0, ACCENT));
    }
    resp
}

/// Human-readable byte size (same units as `pg_size_pretty`).
pub fn format_bytes(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["bytes", "kB", "MB", "GB", "TB"];
    let mut v = bytes.max(0) as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{} {}", v as i64, UNITS[0]) } else { format!("{v:.1} {}", UNITS[i]) }
}

#[cfg(test)]
mod tests {
    use ab_glyph::Font;

    /// Every Phosphor glyph referenced in `src/` must be in the embedded subset —
    /// otherwise it renders as an empty box. Fix: `python scripts/subset_icons.py`.
    #[test]
    fn icon_subset_covers_used_glyphs() {
        let font = ab_glyph::FontRef::try_from_slice(super::ICON_FONT).expect("subset font parses");
        let names: std::collections::HashMap<&str, &str> =
            egui_phosphor::regular::ICONS.iter().copied().collect();

        let mut missing = Vec::new();
        let mut stack = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read src dir").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("read source file");
                let mut used = Vec::new();
                // `icon::NAME` / `regular::NAME`
                for prefix in ["icon::", "regular::"] {
                    for (i, _) in text.match_indices(prefix) {
                        let name: String = text[i + prefix.len()..]
                            .chars()
                            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
                            .collect();
                        if let Some(glyph) = names.get(name.as_str()) {
                            used.extend(glyph.chars());
                        }
                    }
                }
                // `\u{E…}` escapes (i18n strings)
                for (i, _) in text.match_indices("\\u{") {
                    let hex: String = text[i + 3..].chars().take_while(|c| *c != '}').collect();
                    if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                        // Private Use Area — where Phosphor lives.
                        if let Some(c) = char::from_u32(cp).filter(|_| (0xE000..=0xF8FF).contains(&cp)) {
                            used.push(c);
                        }
                    }
                }
                for c in used {
                    if font.glyph_id(c).0 == 0 {
                        missing.push(format!("{} U+{:04X}", path.display(), c as u32));
                    }
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "icons missing from assets/phosphor-subset.ttf \
            (run `python scripts/subset_icons.py`):\n{}", missing.join("\n"));
    }
}
