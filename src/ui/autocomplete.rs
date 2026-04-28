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

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub display: String,
    pub insert: String,
    pub kind: SuggestionKind,
    pub hint: Option<String>,
}

fn table_alias(name: &str) -> String {
    let parts: Vec<&str> = name.split('_').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 1 {
        name.chars().next()
            .and_then(|c| c.to_lowercase().next())
            .map(|c| c.to_string())
            .unwrap_or_default()
    } else {
        parts.iter()
            .filter_map(|p| p.chars().next())
            .flat_map(|c| c.to_lowercase())
            .collect()
    }
}

// ── Autocomplete ──────────────────────────────────────────────────────────────

pub struct Autocomplete {
    pub visible: bool,
    pub suggestions: Vec<Suggestion>,
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

        if word.is_empty() {
            self.visible = false;
            self.suggestions.clear();
            return;
        }

        let lower = word.to_lowercase();
        let mut suggestions: Vec<Suggestion> = Vec::new();

        // Keywords
        for kw in SQL_KEYWORDS {
            if kw.to_lowercase().starts_with(&lower) {
                suggestions.push(Suggestion {
                    display: kw.to_string(),
                    insert: kw.to_string(),
                    kind: SuggestionKind::Keyword,
                    hint: None,
                });
            }
        }

        // Tables — insert includes short alias
        for table in tables {
            if table.to_lowercase().starts_with(&lower) {
                let alias = table_alias(table);
                suggestions.push(Suggestion {
                    display: table.clone(),
                    insert: format!("{} {}", table, alias),
                    kind: SuggestionKind::Table,
                    hint: Some(alias),
                });
            }
        }

        // Columns
        for col in columns {
            if col.to_lowercase().starts_with(&lower) {
                if !suggestions.iter().any(|s| s.display == *col) {
                    suggestions.push(Suggestion {
                        display: col.clone(),
                        insert: col.clone(),
                        kind: SuggestionKind::Column,
                        hint: None,
                    });
                }
            }
        }

        // Prefer exact prefix match at top, then alphabetical
        suggestions.sort_by(|a, b| {
            let a_exact = a.display.to_lowercase() == lower;
            let b_exact = b.display.to_lowercase() == lower;
            b_exact.cmp(&a_exact).then(a.display.cmp(&b.display))
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
        let result = self.suggestions.get(self.selected).map(|s| s.insert.clone());
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

        let visible: Vec<Suggestion> = self.suggestions.iter().take(8).cloned().collect();
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
                        ui.set_min_width(240.0);
                        for (i, suggestion) in visible.iter().enumerate() {
                            let is_selected = i == self.selected;

                            let bg = if is_selected {
                                egui::Color32::from_rgb(38, 56, 90)
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width().max(240.0), 20.0),
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
                                    suggestion.kind.icon(),
                                    egui::FontId::monospace(10.0),
                                    suggestion.kind.color(),
                                );

                                // Display text
                                painter.text(
                                    rect.left_center() + egui::vec2(18.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    &suggestion.display,
                                    egui::FontId::monospace(12.0),
                                    if is_selected {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::from_gray(200)
                                    },
                                );

                                // Alias hint on the right
                                if let Some(hint) = &suggestion.hint {
                                    painter.text(
                                        rect.right_center() + egui::vec2(-6.0, 0.0),
                                        egui::Align2::RIGHT_CENTER,
                                        hint,
                                        egui::FontId::monospace(10.0),
                                        egui::Color32::from_gray(100),
                                    );
                                }
                            }

                            if resp.clicked() {
                                accepted = Some(suggestion.insert.clone());
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
