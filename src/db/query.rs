use std::fmt;

/// A single cell value in a result set.
#[derive(Debug, Clone)]
pub enum CellValue {
    Null,
    Text(Box<str>),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Bytes(Vec<u8>),
}

impl fmt::Display for CellValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellValue::Null => write!(f, ""),
            CellValue::Text(s) => write!(f, "{s}"),
            CellValue::Integer(i) => write!(f, "{i}"),
            CellValue::Float(v) => write!(f, "{v}"),
            CellValue::Boolean(b) => write!(f, "{b}"),
            CellValue::Bytes(b) => write!(f, "\\x{}", hex::encode(b)),
        }
    }
}

impl CellValue {
    pub fn is_null(&self) -> bool {
        matches!(self, CellValue::Null)
    }
}

/// Parse a PostgreSQL text-protocol cell string into a typed CellValue.
/// Called for every cell returned by simple_query (text format).
pub fn parse_text_cell(s: &str) -> CellValue {
    // PostgreSQL sends booleans as "t" / "f" in text protocol.
    if s == "t" {
        return CellValue::Boolean(true);
    }
    if s == "f" {
        return CellValue::Boolean(false);
    }
    // Try integer, then float.
    if let Ok(i) = s.parse::<i64>() {
        return CellValue::Integer(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return CellValue::Float(f);
    }
    CellValue::Text(s.into())
}

/// Always-quoted SQL identifier (`"name"`, embedded `"` doubled).
pub fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// SQL string literal (`'text'`, embedded `'` doubled). Assumes
/// `standard_conforming_strings = on` (the default since PG 9.1), so backslashes
/// are taken literally.
pub fn quote_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Render a cell as an SQL literal for INSERT export. Every non-NULL value is
/// emitted as a quoted literal — Postgres coerces an untyped literal to the
/// target column's type, so `'42'` works for int columns and `'t'` for bools.
pub fn cell_literal(cell: &CellValue) -> String {
    match cell {
        CellValue::Null => "NULL".to_owned(),
        // Text protocol sends booleans as t/f — keep that form, it round-trips
        // for both bool and text columns better than "true"/"false".
        CellValue::Boolean(b) => if *b { "'t'".to_owned() } else { "'f'".to_owned() },
        other => quote_literal(&other.to_string()),
    }
}

/// Whether a cell filter keeps or drops the rows that match the selected value(s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellFilterKind {
    /// "Filter by Selection" — keep only matching rows.
    Include,
    /// "Exclude by Selection" — drop matching rows.
    Exclude,
}

/// WHERE fragment for "Filter / Exclude by Selection": rows whose `col` equals
/// one of `values` (duplicates ignored). Returns `None` when `values` is empty.
///
/// Values go through [`cell_literal`], so each is a quoted literal that Postgres
/// coerces to the column's type. NULL is matched with `IS [NOT] NULL`, and
/// Exclude keeps rows where `col` is NULL unless NULL itself was selected
/// (a bare `<>` / `NOT IN` would silently drop them).
///
/// Limits: the value is rebuilt from the parsed [`CellValue`], not the raw text
/// the server sent, so text that looks like a number can differ (`007` is shown
/// and matched as `7`; numerics wider than `f64` lose digits). Types with no
/// `=` operator (`json`, `xml`, `point`) get a Postgres error.
pub fn cell_filter_fragment(col: &str, values: &[CellValue], kind: CellFilterKind) -> Option<String> {
    let col = quote_ident(col);
    let has_null = values.iter().any(CellValue::is_null);
    let mut literals: Vec<String> = Vec::new();
    for v in values.iter().filter(|v| !v.is_null()) {
        let lit = cell_literal(v);
        if !literals.contains(&lit) {
            literals.push(lit);
        }
    }
    let list = literals.join(", ");
    let fragment = match (kind, literals.len(), has_null) {
        (_, 0, false) => return None,
        (CellFilterKind::Include, 0, true) => format!("{col} IS NULL"),
        (CellFilterKind::Include, 1, false) => format!("{col} = {list}"),
        (CellFilterKind::Include, 1, true) => format!("({col} = {list} OR {col} IS NULL)"),
        (CellFilterKind::Include, _, false) => format!("{col} IN ({list})"),
        (CellFilterKind::Include, _, true) => format!("({col} IN ({list}) OR {col} IS NULL)"),
        (CellFilterKind::Exclude, 0, true) => format!("{col} IS NOT NULL"),
        (CellFilterKind::Exclude, 1, false) => format!("{col} IS DISTINCT FROM {list}"),
        // NULL was selected too, so NULL rows go — which `<>` / `NOT IN` do by themselves.
        (CellFilterKind::Exclude, 1, true) => format!("{col} <> {list}"),
        (CellFilterKind::Exclude, _, false) => format!("({col} NOT IN ({list}) OR {col} IS NULL)"),
        (CellFilterKind::Exclude, _, true) => format!("{col} NOT IN ({list})"),
    };
    Some(fragment)
}

/// Rows per `INSERT ... VALUES (...), (...)` statement in generated scripts.
pub const INSERT_BATCH_ROWS: usize = 100;

/// Build `INSERT INTO <table> (cols) VALUES ...;` statements for `rows`, batched
/// `INSERT_BATCH_ROWS` rows per statement. `table` is used verbatim, so the
/// caller passes an already-quoted (and optionally schema-qualified) name.
pub fn insert_statements(table: &str, columns: &[String], rows: &[Vec<CellValue>]) -> String {
    let col_list = columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");
    let mut out = String::new();
    for chunk in rows.chunks(INSERT_BATCH_ROWS) {
        out.push_str(&format!("INSERT INTO {table} ({col_list}) VALUES\n"));
        for (i, row) in chunk.iter().enumerate() {
            let vals = row.iter().map(cell_literal).collect::<Vec<_>>().join(", ");
            out.push_str("    (");
            out.push_str(&vals);
            out.push_str(if i + 1 == chunk.len() { ");\n" } else { "),\n" });
        }
    }
    out
}

/// Best-effort guess of the single table a SELECT reads from — used to name the
/// target table of an INSERT export in free-form query mode. Returns `None` for
/// anything that isn't obviously one table (joins, comma joins, subqueries, or
/// more than one FROM), so the caller can fall back to a placeholder.
pub fn guess_single_table(sql: &str) -> Option<String> {
    let tokens: Vec<&str> = sql.split_whitespace().collect();
    let lower: Vec<String> = tokens.iter().map(|t| t.to_lowercase()).collect();
    if lower.iter().filter(|t| *t == "from").count() != 1 || lower.iter().any(|t| t == "join") {
        return None;
    }
    let pos = lower.iter().position(|t| t == "from")?;
    let raw = *tokens.get(pos + 1)?;
    if raw.starts_with('(') {
        return None;
    }
    // Any comma in the FROM clause (before the next clause keyword) is a comma join.
    const CLAUSE_END: [&str; 10] =
        ["where", "group", "order", "limit", "offset", "having", "window", "union", "fetch", "for"];
    if tokens[pos + 1..]
        .iter()
        .zip(&lower[pos + 1..])
        .take_while(|(_, l)| !CLAUSE_END.contains(&l.as_str()))
        .any(|(t, _)| t.contains(','))
    {
        return None;
    }
    let table = raw.trim_end_matches(';').trim_end_matches(')');
    if table.is_empty() { None } else { Some(table.to_owned()) }
}

/// Result of a query execution.
#[derive(Debug, Default, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
    /// Number of rows affected (for INSERT/UPDATE/DELETE).
    pub rows_affected: Option<u64>,
    /// Query execution time in milliseconds.
    pub elapsed_ms: f64,
}

impl QueryResult {
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_escapes_embedded_quotes() {
        assert_eq!(quote_ident(r#"we"ird"#), r#""we""ird""#);
        assert_eq!(quote_literal("it's"), "'it''s'");
        assert_eq!(quote_literal(r"C:\tmp"), r"'C:\tmp'");
    }

    #[test]
    fn insert_statements_batches_and_renders_literals() {
        let cols = vec!["id".to_owned(), "name".to_owned(), "ok".to_owned()];
        let rows = vec![
            vec![CellValue::Integer(1), CellValue::Text("O'Neil".into()), CellValue::Boolean(true)],
            vec![CellValue::Integer(2), CellValue::Null, CellValue::Boolean(false)],
        ];
        let sql = insert_statements("\"public\".\"t\"", &cols, &rows);
        assert_eq!(
            sql,
            "INSERT INTO \"public\".\"t\" (\"id\", \"name\", \"ok\") VALUES\n    ('1', 'O''Neil', 't'),\n    ('2', NULL, 'f');\n"
        );

        let many: Vec<Vec<CellValue>> = (0..INSERT_BATCH_ROWS + 1).map(|i| vec![CellValue::Integer(i as i64)]).collect();
        let sql = insert_statements("t", &["id".to_owned()], &many);
        assert_eq!(sql.matches("INSERT INTO").count(), 2);
    }

    #[test]
    fn cell_filter_single_value() {
        use CellFilterKind::*;
        let v = [CellValue::Integer(42)];
        assert_eq!(cell_filter_fragment("id", &v, Include).as_deref(), Some("\"id\" = '42'"));
        assert_eq!(cell_filter_fragment("id", &v, Exclude).as_deref(), Some("\"id\" IS DISTINCT FROM '42'"));
    }

    #[test]
    fn cell_filter_null() {
        use CellFilterKind::*;
        let v = [CellValue::Null];
        assert_eq!(cell_filter_fragment("x", &v, Include).as_deref(), Some("\"x\" IS NULL"));
        assert_eq!(cell_filter_fragment("x", &v, Exclude).as_deref(), Some("\"x\" IS NOT NULL"));
    }

    #[test]
    fn cell_filter_renders_literals_like_insert_export() {
        use CellFilterKind::Include;
        // Booleans as t/f: valid for bool columns and matches a text column holding "t".
        assert_eq!(cell_filter_fragment("ok", &[CellValue::Boolean(true)], Include).as_deref(), Some("\"ok\" = 't'"));
        assert_eq!(cell_filter_fragment("ok", &[CellValue::Boolean(false)], Include).as_deref(), Some("\"ok\" = 'f'"));
        assert_eq!(cell_filter_fragment("n", &[CellValue::Float(1.5)], Include).as_deref(), Some("\"n\" = '1.5'"));
        assert_eq!(cell_filter_fragment("b", &[CellValue::Bytes(vec![0xde, 0xad])], Include).as_deref(), Some("\"b\" = '\\xdead'"));
        assert_eq!(
            cell_filter_fragment("name", &[CellValue::Text("O'Brien".into())], Include).as_deref(),
            Some("\"name\" = 'O''Brien'")
        );
        assert_eq!(cell_filter_fragment("we\"ird", &[CellValue::Integer(1)], Include).as_deref(), Some("\"we\"\"ird\" = '1'"));
    }

    #[test]
    fn cell_filter_multiple_values() {
        use CellFilterKind::*;
        let v = [CellValue::Integer(1), CellValue::Integer(2), CellValue::Integer(1)];
        assert_eq!(cell_filter_fragment("id", &v, Include).as_deref(), Some("\"id\" IN ('1', '2')"));
        assert_eq!(
            cell_filter_fragment("id", &v, Exclude).as_deref(),
            Some("(\"id\" NOT IN ('1', '2') OR \"id\" IS NULL)")
        );
    }

    #[test]
    fn cell_filter_values_mixed_with_null() {
        use CellFilterKind::*;
        let one = [CellValue::Text("a".into()), CellValue::Null];
        assert_eq!(cell_filter_fragment("c", &one, Include).as_deref(), Some("(\"c\" = 'a' OR \"c\" IS NULL)"));
        assert_eq!(cell_filter_fragment("c", &one, Exclude).as_deref(), Some("\"c\" <> 'a'"));
        let two = [CellValue::Text("a".into()), CellValue::Null, CellValue::Text("b".into())];
        assert_eq!(
            cell_filter_fragment("c", &two, Include).as_deref(),
            Some("(\"c\" IN ('a', 'b') OR \"c\" IS NULL)")
        );
        assert_eq!(cell_filter_fragment("c", &two, Exclude).as_deref(), Some("\"c\" NOT IN ('a', 'b')"));
    }

    #[test]
    fn cell_filter_empty_selection_is_none() {
        assert_eq!(cell_filter_fragment("c", &[], CellFilterKind::Include), None);
    }

    #[test]
    fn guess_single_table_cases() {
        assert_eq!(guess_single_table("SELECT * FROM public.users WHERE id = 1").as_deref(), Some("public.users"));
        assert_eq!(guess_single_table("select a from \"Users\";").as_deref(), Some("\"Users\""));
        assert_eq!(guess_single_table("SELECT * FROM t LIMIT 10").as_deref(), Some("t"));
        assert_eq!(guess_single_table("SELECT * FROM a JOIN b ON a.id = b.id"), None);
        assert_eq!(guess_single_table("SELECT * FROM a x, b y WHERE x.id = y.id"), None);
        assert_eq!(guess_single_table("SELECT * FROM (SELECT 1) s"), None);
        assert_eq!(guess_single_table("SELECT (SELECT max(id) FROM b) FROM a"), None);
        assert_eq!(guess_single_table("SELECT 1"), None);
    }
}
