use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};
use once_cell::sync::Lazy;
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

/// Cached syntect globals — loaded once on first use (~5ms).
static SS: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
static TS: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

/// Build a syntax-highlighted `LayoutJob` for a SQL string.
///
/// Called by egui's `TextEdit::layouter` callback on every repaint where the
/// text or wrap width changed — egui caches the resulting `Galley` otherwise.
pub fn highlight_sql(ui: &egui::Ui, text: &str, wrap_width: f32) -> LayoutJob {
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
    let dark = ui.visuals().dark_mode;

    let mut job = build_job(text, font_id, dark);
    job.wrap.max_width = wrap_width;
    job
}

fn build_job(text: &str, font_id: FontId, dark: bool) -> LayoutJob {
    let theme_name = if dark { "base16-ocean.dark" } else { "InspiredGitHub" };

    let Some(syntax) = SS.find_syntax_by_extension("sql") else {
        return plain_job(text, font_id);
    };

    let Some(theme) = TS.themes.get(theme_name) else {
        return plain_job(text, font_id);
    };

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut job = LayoutJob::default();

    for line in LinesWithEndings::from(text) {
        let spans = match highlighter.highlight_line(line, &SS) {
            Ok(s) => s,
            Err(_) => {
                append_plain(&mut job, line, font_id.clone());
                continue;
            }
        };

        for (style, chunk) in spans {
            let fg = style.foreground;
            let color = Color32::from_rgba_unmultiplied(fg.r, fg.g, fg.b, fg.a);
            let italics = style.font_style.contains(FontStyle::ITALIC);
            let underline = if style.font_style.contains(FontStyle::UNDERLINE) {
                egui::Stroke::new(1.0, color)
            } else {
                egui::Stroke::NONE
            };

            job.append(
                chunk,
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color,
                    italics,
                    underline,
                    ..Default::default()
                },
            );
        }
    }

    job
}

fn plain_job(text: &str, font_id: FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    append_plain(&mut job, text, font_id);
    job
}

fn append_plain(job: &mut LayoutJob, text: &str, font_id: FontId) {
    job.append(
        text,
        0.0,
        TextFormat {
            font_id,
            color: Color32::LIGHT_GRAY,
            ..Default::default()
        },
    );
}
