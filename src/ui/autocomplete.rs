use egui::{Color32, Id, Rect, Ui};

// ── SQL keywords ──────────────────────────────────────────────────────────────

static SQL_KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "FULL",
    "CROSS", "ON", "AND", "OR", "NOT", "IN", "EXISTS", "BETWEEN", "LIKE", "ILIKE",
    "IS", "NULL", "TRUE", "FALSE", "AS", "DISTINCT", "ALL", "UNION", "INTERSECT",
    "EXCEPT", "ORDER", "BY", "GROUP", "HAVING", "LIMIT", "OFFSET", "FETCH",
    "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "TRUNCATE",
    "CREATE", "TABLE", "VIEW", "INDEX", "SEQUENCE", "SCHEMA", "DATABASE",
    "ALTER", "DROP", "ADD", "COLUMN", "CONSTRAINT", "PRIMARY", "KEY",
    "FOREIGN", "REFERENCES", "UNIQUE", "CHECK", "DEFAULT", "NOT", "NULL",
    "WITH", "RECURSIVE", "CASE", "WHEN", "THEN", "ELSE", "END",
    "CAST", "COALESCE", "NULLIF", "GREATEST", "LEAST",
    "COUNT", "SUM", "AVG", "MIN", "MAX", "STRING_AGG", "ARRAY_AGG",
    "NOW", "CURRENT_TIMESTAMP", "CURRENT_DATE", "CURRENT_TIME",
    "EXTRACT", "DATE_TRUNC", "DATE_PART",
    "UPPER", "LOWER", "TRIM", "LTRIM", "RTRIM", "LENGTH", "SUBSTRING",
    "REPLACE", "REGEXP_REPLACE", "SPLIT_PART", "CONCAT", "FORMAT",
    "TO_CHAR", "TO_DATE", "TO_TIMESTAMP", "TO_NUMBER",
    "ROW_NUMBER", "RANK", "DENSE_RANK", "OVER", "PARTITION",
    "RETURNING", "CONFLICT", "DO", "NOTHING", "EXCLUDED",
    "BEGIN", "COMMIT", "ROLLBACK", "TRANSACTION", "SAVEPOINT",
    "EXPLAIN", "ANALYZE", "VERBOSE", "BUFFERS", "FORMAT", "JSON", "TEXT",
    "INTEGER", "BIGINT", "SMALLINT", "NUMERIC", "DECIMAL", "REAL",
    "DOUBLE", "PRECISION", "BOOLEAN", "VARCHAR", "CHARACTER", "VARYING",
    "CHAR", "TEXT", "BYTEA", "DATE", "TIME", "TIMESTAMP", "INTERVAL",
    "UUID", "JSON", "JSONB", "ARRAY", "SERIAL", "BIGSERIAL",
    "ASC", "DESC", "NULLS", "FIRST", "LAST",
    "LATERAL", "NATURAL", "USING", "IF",
];

// ── SuggestionKind ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionKind {
    Keyword,
    Table,
    Column,
}

impl SuggestionKind {
    pub fn color(&self) -> Color32 {
        match self {
            SuggestionKind::Keyword => Color32::from_rgb(86, 156, 214),
            SuggestionKind::Table   => Color32::from_rgb(78, 201, 176),
            SuggestionKind::Column  => Color32::from_rgb(220, 160, 60),
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            SuggestionKind::Keyword => "K",
            SuggestionKind::Table   => "T",
            SuggestionKind::Column  => "C",
        }
    }
}

// ── Autocomplete ──────────────────────────────────────────────────────────────

pub struct Autocomplete {
    pub visible: bool,
    pub suggestions: Vec<(String, SuggestionKind)>,
    pub selected: usize,
    pub word_start: usize,
    pub current_word: String,
}

impl Default for Autocomplete {
    fn default() -> Self {
        Self {
            visible: false,
            suggestions: Vec::new(),
            selected: 0,
            word_start: 0,
            current_word: String::new(),
        }
    }
}

impl Autocomplete {
    pub fn is_visible(&self) -> bool {
        self.visible && !self.suggestions.is_empty()
    }

    pub fn update(
        &mut self,
        sql: &str,
        cursor_idx: usize,
        tables: &[String],
        columns: &[String],
    ) {
        let (word_start, word) = extract_word(sql, cursor_idx);
        self.word_start = word_start;
        self.current_word = word.to_owned();

        if word.len() < 1 {
            self.visible = false;
            self.suggestions.clear();
            return;
        }

        let lower = word.to_lowercase();
        let mut suggestions: Vec<(String, SuggestionKind)> = Vec::new();

        // Keywords
        for kw in SQL_KEYWORDS {
            if kw.to_lowercase().starts_with(&lower) {
                suggestions.push((kw.to_string(), SuggestionKind::Keyword));
            }
        }

        // Tables
        for table in tables {
            if table.to_lowercase().starts_with(&lower) {
                suggestions.push((table.clone(), SuggestionKind::Table));
            }
        }

        // Columns
        for col in columns {
            if col.to_lowercase().starts_with(&lower) {
                // Avoid duplicates with table list
                if !suggestions.iter().any(|(s, _)| s == col) {
                    suggestions.push((col.clone(), SuggestionKind::Column));
                }
            }
        }

        // Limit to 8 items, prefer exact prefix match at top
        suggestions.sort_by(|a, b| {
            let a_exact = a.0.to_lowercase() == lower;
            let b_exact = b.0.to_lowercase() == lower;
            b_exact.cmp(&a_exact).then(a.0.cmp(&b.0))
        });

        if suggestions.is_empty() {
            self.visible = false;
        }

        self.suggestions = suggestions;

        // Clamp selection
        if self.selected >= self.suggestions.len() {
            self.selected = 0;
        }
    }

    pub fn force_show(&mut self) {
        if !self.suggestions.is_empty() {
            self.visible = true;
        }
    }

    pub fn dismiss(&mut self) {
        self.visible = false;
    }

    pub fn accept(&mut self) -> Option<String> {
        if !self.is_visible() {
            return None;
        }
        let result = self.suggestions.get(self.selected).map(|(s, _)| s.clone());
        self.visible = false;
        self.suggestions.clear();
        result
    }

    pub fn select_next(&mut self) {
        if !self.suggestions.is_empty() {
            self.selected = (self.selected + 1) % self.suggestions.len().min(8);
        }
    }

    pub fn select_prev(&mut self) {
        let max = self.suggestions.len().min(8);
        if max == 0 { return; }
        if self.selected == 0 {
            self.selected = max - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Render the autocomplete popup. Returns Some(accepted_word) if user clicked an item.
    pub fn show(&mut self, ui: &Ui, editor_rect: Rect) -> Option<String> {
        if !self.is_visible() {
            return None;
        }

        let visible = self.suggestions.iter().take(8).cloned().collect::<Vec<_>>();
        if visible.is_empty() {
            return None;
        }

        let mut accepted: Option<String> = None;
        let popup_pos = egui::pos2(editor_rect.left(), editor_rect.bottom() + 2.0);

        egui::Area::new(Id::new("autocomplete_popup"))
            .fixed_pos(popup_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .fill(egui::Color32::from_rgb(28, 32, 40))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 70, 90)))
                    .inner_margin(egui::Margin::same(2.0))
                    .show(ui, |ui| {
                        ui.set_min_width(220.0);
                        for (i, (text, kind)) in visible.iter().enumerate() {
                            let is_selected = i == self.selected;

                            let bg = if is_selected {
                                egui::Color32::from_rgb(38, 56, 90)
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width().max(220.0), 20.0),
                                egui::Sense::click(),
                            );

                            if resp.hovered() {
                                self.selected = i;
                            }

                            if ui.is_rect_visible(rect) {
                                let painter = ui.painter();
                                painter.rect_filled(rect, 2.0, if resp.hovered() {
                                    egui::Color32::from_rgb(38, 56, 90)
                                } else {
                                    bg
                                });

                                // Kind badge
                                painter.text(
                                    rect.left_center() + egui::vec2(4.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    kind.icon(),
                                    egui::FontId::monospace(10.0),
                                    kind.color(),
                                );

                                // Suggestion text
                                painter.text(
                                    rect.left_center() + egui::vec2(18.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    text,
                                    egui::FontId::monospace(12.0),
                                    if is_selected {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::from_gray(200)
                                    },
                                );
                            }

                            if resp.clicked() {
                                accepted = Some(text.clone());
                            }
                        }
                    });
            });

        if accepted.is_some() {
            self.visible = false;
            self.suggestions.clear();
        }

        accepted
    }
}

// ── Helper ────────────────────────────────────────────────────────────────────

/// Find the start and content of the word at `cursor` in `text`.
/// `cursor` is a **char index** (from egui `CCursor::index`).
/// Returns `(word_start_byte, word_slice)` where `word_start_byte` is a byte offset.
pub fn extract_word(text: &str, cursor_char: usize) -> (usize, &str) {
    // Convert char index → byte offset (safe even with multi-byte UTF-8).
    let cursor_byte = text
        .char_indices()
        .nth(cursor_char)
        .map(|(i, _)| i)
        .unwrap_or(text.len());

    let before = &text[..cursor_byte];

    // rfind returns byte offsets; advance past the delimiter char.
    let word_start = before
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + before[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1))
        .unwrap_or(0);

    let word = &text[word_start..cursor_byte];
    (word_start, word)
}
