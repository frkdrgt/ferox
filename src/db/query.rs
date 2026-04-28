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

