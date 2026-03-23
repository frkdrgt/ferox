use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};

// ── Keyword sets ──────────────────────────────────────────────────────────────

/// Returns true if `word` (already upper-cased) is a SQL keyword.
fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "SELECT" | "FROM" | "WHERE" | "JOIN" | "LEFT" | "RIGHT" | "INNER" | "OUTER"
        | "FULL" | "CROSS" | "ON" | "AS" | "ORDER" | "BY" | "GROUP" | "HAVING"
        | "LIMIT" | "OFFSET" | "UNION" | "INTERSECT" | "EXCEPT" | "DISTINCT"
        | "ALL" | "WITH" | "RECURSIVE" | "INSERT" | "INTO" | "VALUES" | "UPDATE"
        | "SET" | "DELETE" | "MERGE" | "RETURNING" | "CREATE" | "ALTER" | "DROP"
        | "TABLE" | "VIEW" | "INDEX" | "SEQUENCE" | "SCHEMA" | "DATABASE"
        | "TRIGGER" | "FUNCTION" | "PROCEDURE" | "BEGIN" | "COMMIT" | "ROLLBACK"
        | "SAVEPOINT" | "RELEASE" | "TRANSACTION" | "NOT" | "AND" | "OR" | "IN"
        | "LIKE" | "ILIKE" | "IS" | "BETWEEN" | "EXISTS" | "CASE" | "WHEN"
        | "THEN" | "ELSE" | "END" | "CAST" | "OVER" | "PARTITION" | "WINDOW"
        | "FILTER" | "LATERAL" | "NATURAL" | "USING" | "UNIQUE" | "PRIMARY"
        | "FOREIGN" | "KEY" | "REFERENCES" | "CONSTRAINT" | "DEFAULT"
        | "NOT NULL" | "NULL" | "TRUE" | "FALSE" | "ASC" | "DESC" | "NULLS"
        | "FIRST" | "LAST" | "IF" | "DO" | "LANGUAGE" | "VOLATILE" | "STABLE"
        | "IMMUTABLE" | "RETURNS" | "DECLARE" | "EXPLAIN" | "ANALYZE" | "VERBOSE"
        | "BUFFERS" | "FORMAT" | "COPY" | "GRANT" | "REVOKE" | "TRUNCATE"
        | "VACUUM" | "REINDEX" | "CLUSTER" | "SHOW" | "SET"
    )
}

/// Returns true if `word` is a built-in SQL type name.
fn is_type(word: &str) -> bool {
    matches!(
        word,
        "INTEGER" | "INT" | "INT2" | "INT4" | "INT8" | "BIGINT" | "SMALLINT"
        | "TEXT" | "VARCHAR" | "CHAR" | "CHARACTER" | "VARYING" | "BOOLEAN"
        | "BOOL" | "FLOAT" | "FLOAT4" | "FLOAT8" | "DOUBLE" | "PRECISION"
        | "DECIMAL" | "NUMERIC" | "REAL" | "DATE" | "TIME" | "TIMESTAMP"
        | "TIMESTAMPTZ" | "TIMETZ" | "INTERVAL" | "UUID" | "JSON" | "JSONB"
        | "BYTEA" | "SERIAL" | "BIGSERIAL" | "SMALLSERIAL" | "OID" | "VOID"
        | "MONEY" | "BIT" | "VARBIT" | "MACADDR" | "INET" | "CIDR" | "POINT"
        | "LINE" | "LSEG" | "BOX" | "PATH" | "POLYGON" | "CIRCLE" | "TSQUERY"
        | "TSVECTOR" | "ARRAY" | "RECORD" | "TRIGGER" | "REGTYPE" | "REGCLASS"
    )
}

/// Returns true if `word` is a common SQL function name.
fn is_function(word: &str) -> bool {
    matches!(
        word,
        "COUNT" | "SUM" | "AVG" | "MIN" | "MAX" | "COALESCE" | "NULLIF"
        | "NOW" | "CURRENT_TIMESTAMP" | "CURRENT_DATE" | "CURRENT_TIME"
        | "EXTRACT" | "DATE_PART" | "DATE_TRUNC" | "AGE" | "TO_CHAR"
        | "TO_DATE" | "TO_TIMESTAMP" | "TO_NUMBER" | "LOWER" | "UPPER"
        | "TRIM" | "LTRIM" | "RTRIM" | "LENGTH" | "SUBSTR" | "SUBSTRING"
        | "REPLACE" | "SPLIT_PART" | "CONCAT" | "CONCAT_WS" | "FORMAT"
        | "LPAD" | "RPAD" | "POSITION" | "STRPOS" | "REGEXP_REPLACE"
        | "REGEXP_MATCH" | "ARRAY_AGG" | "STRING_AGG" | "JSON_AGG"
        | "JSONB_AGG" | "ROW_NUMBER" | "RANK" | "DENSE_RANK" | "LAG"
        | "LEAD" | "FIRST_VALUE" | "LAST_VALUE" | "NTH_VALUE" | "NTILE"
        | "GENERATE_SERIES" | "UNNEST" | "ARRAY_LENGTH" | "CARDINALITY"
        | "PG_SLEEP" | "PG_TERMINATE_BACKEND" | "PG_CANCEL_BACKEND"
        | "GREATEST" | "LEAST" | "ABS" | "CEIL" | "FLOOR" | "ROUND"
        | "TRUNC" | "POWER" | "SQRT" | "RANDOM" | "MD5" | "ENCODE" | "DECODE"
    )
}

// ── Colour palettes ───────────────────────────────────────────────────────────

struct Palette {
    keyword:  Color32,
    type_:    Color32,
    function: Color32,
    string:   Color32,
    number:   Color32,
    comment:  Color32,
    operator: Color32,
    default:  Color32,
}

const DARK: Palette = Palette {
    keyword:  Color32::from_rgb(0xb4, 0x8e, 0xad), // purple
    type_:    Color32::from_rgb(0x8f, 0xa1, 0xb3), // steel blue
    function: Color32::from_rgb(0x96, 0xb5, 0xb4), // teal
    string:   Color32::from_rgb(0xa3, 0xbe, 0x8c), // green
    number:   Color32::from_rgb(0xd0, 0x87, 0x70), // orange
    comment:  Color32::from_rgb(0x65, 0x73, 0x7e), // muted
    operator: Color32::from_rgb(0xc0, 0xc5, 0xce), // light grey
    default:  Color32::from_rgb(0xc0, 0xc5, 0xce),
};

const LIGHT: Palette = Palette {
    keyword:  Color32::from_rgb(0xa7, 0x1d, 0x5d), // dark pink
    type_:    Color32::from_rgb(0x00, 0x86, 0xb3), // blue
    function: Color32::from_rgb(0x79, 0x5d, 0xa3), // purple
    string:   Color32::from_rgb(0x18, 0x36, 0x91), // dark blue
    number:   Color32::from_rgb(0x09, 0x69, 0x9e), // blue
    comment:  Color32::from_rgb(0x96, 0x98, 0x96), // grey
    operator: Color32::from_rgb(0x33, 0x33, 0x33),
    default:  Color32::from_rgb(0x33, 0x33, 0x33),
};

// ── Tokeniser ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Tok {
    Keyword,
    Type,
    Function,
    String,
    Number,
    Comment,
    Operator,
    Default,
}

/// Tokenise `text` into (token_kind, slice) pairs.
fn tokenise(text: &str) -> Vec<(Tok, &str)> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut out = Vec::with_capacity(64);
    let mut i = 0;

    while i < len {
        // ── Line comment --
        if i + 1 < len && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            let start = i;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            out.push((Tok::Comment, &text[start..i]));
            continue;
        }

        // ── Block comment /* … */
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2; // consume */
            out.push((Tok::Comment, &text[start..i.min(len)]));
            continue;
        }

        // ── Single-quoted string '…'
        if bytes[i] == b'\'' {
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\'' {
                    i += 1;
                    if i < len && bytes[i] == b'\'' { i += 1; continue; } // escaped ''
                    break;
                }
                i += 1;
            }
            out.push((Tok::String, &text[start..i]));
            continue;
        }

        // ── Dollar-quoted string $tag$…$tag$
        if bytes[i] == b'$' {
            let start = i;
            i += 1;
            let tag_start = i;
            while i < len && bytes[i] != b'$' { i += 1; }
            if i < len {
                i += 1; // closing $
                let tag = &text[tag_start..i - 1];
                let close = format!("${}$", tag);
                if let Some(pos) = text[i..].find(close.as_str()) {
                    i += pos + close.len();
                } else {
                    i = len;
                }
                out.push((Tok::String, &text[start..i]));
                continue;
            }
        }

        // ── Number (integer or float, optional leading sign handled elsewhere)
        if bytes[i].is_ascii_digit()
            || (bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'e' || bytes[i] == b'E' || bytes[i] == b'x' || bytes[i] == b'X') {
                i += 1;
            }
            out.push((Tok::Number, &text[start..i]));
            continue;
        }

        // ── Identifier / keyword  (ASCII letters, digits, underscore, $)
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$') {
                i += 1;
            }
            let word = &text[start..i];
            let upper = word.to_ascii_uppercase();
            let tok = if is_keyword(&upper) {
                Tok::Keyword
            } else if is_type(&upper) {
                Tok::Type
            } else if is_function(&upper) {
                // Only treat as function if immediately followed by '('
                if i < len && bytes[i] == b'(' { Tok::Function } else { Tok::Default }
            } else {
                Tok::Default
            };
            out.push((tok, word));
            continue;
        }

        // ── Quoted identifier "…"
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            while i < len && bytes[i] != b'"' { i += 1; }
            if i < len { i += 1; }
            out.push((Tok::Default, &text[start..i]));
            continue;
        }

        // ── Operator / punctuation (single char)
        let ch_len = text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        let chunk = &text[i..i + ch_len];
        let tok = match bytes[i] {
            b'=' | b'<' | b'>' | b'!' | b'+' | b'-' | b'*' | b'/' | b'%'
            | b'|' | b'&' | b'^' | b'~' | b'@' | b'#' => Tok::Operator,
            _ => Tok::Default,
        };
        out.push((tok, chunk));
        i += ch_len;
    }

    out
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Build a syntax-highlighted `LayoutJob` for a SQL string.
///
/// Called by egui's `TextEdit::layouter` callback on every repaint where the
/// text or wrap width changed — egui caches the resulting `Galley` otherwise.
pub fn highlight_sql(ui: &egui::Ui, text: &str, wrap_width: f32) -> LayoutJob {
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
    let pal = if ui.visuals().dark_mode { &DARK } else { &LIGHT };

    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;

    for (tok, chunk) in tokenise(text) {
        let color = match tok {
            Tok::Keyword  => pal.keyword,
            Tok::Type     => pal.type_,
            Tok::Function => pal.function,
            Tok::String   => pal.string,
            Tok::Number   => pal.number,
            Tok::Comment  => pal.comment,
            Tok::Operator => pal.operator,
            Tok::Default  => pal.default,
        };
        job.append(chunk, 0.0, TextFormat {
            font_id: font_id.clone(),
            color,
            ..Default::default()
        });
    }

    if job.sections.is_empty() {
        job.append(text, 0.0, TextFormat {
            font_id,
            color: if ui.visuals().dark_mode { DARK.default } else { LIGHT.default },
            ..Default::default()
        });
    }

    job
}
