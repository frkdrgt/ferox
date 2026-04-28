use std::collections::HashMap;

use anyhow::Result;
use tokio_postgres::Client;

#[derive(Debug, Clone)]
pub struct SchemaInfo {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct TableInfo {
    pub schema: String,
    pub name: String,
    pub kind: TableKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableKind {
    Table,
    View,
    MaterializedView,
    ForeignTable,
}

impl TableKind {
    /// Single-char icon that renders well on Windows/macOS standard fonts.
    pub fn icon(&self) -> &'static str {
        match self {
            TableKind::Table => "■",
            TableKind::View => "◇",
            TableKind::MaterializedView => "◆",
            TableKind::ForeignTable => "○",
        }
    }

    /// Short uppercase label shown in tooltips / badges.
    pub fn label(&self) -> &'static str {
        match self {
            TableKind::Table => "TABLE",
            TableKind::View => "VIEW",
            TableKind::MaterializedView => "MAT VIEW",
            TableKind::ForeignTable => "FOREIGN",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub column_default: Option<String>,
}

pub async fn load_schemas(client: &Client) -> Result<Vec<SchemaInfo>> {
    let rows = client
        .query(
            "SELECT schema_name
             FROM information_schema.schemata
             WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
               AND schema_name NOT LIKE 'pg_temp_%'
               AND schema_name NOT LIKE 'pg_toast_temp_%'
             ORDER BY schema_name",
            &[],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| SchemaInfo {
            name: r.get::<_, String>(0),
        })
        .collect())
}

pub async fn load_tables(client: &Client, schema: &str) -> Result<Vec<TableInfo>> {
    let rows = client
        .query(
            "SELECT table_name, table_type
             FROM information_schema.tables
             WHERE table_schema = $1
             ORDER BY table_name",
            &[&schema],
        )
        .await?;

    let mut tables: Vec<TableInfo> = rows
        .iter()
        .map(|r| {
            let name: String = r.get(0);
            let kind_str: String = r.get(1);
            let kind = match kind_str.as_str() {
                "VIEW" => TableKind::View,
                "FOREIGN" => TableKind::ForeignTable,
                _ => TableKind::Table,
            };
            TableInfo {
                schema: schema.to_owned(),
                name,
                kind,
            }
        })
        .collect();

    // Also fetch materialized views
    let mv_rows = client
        .query(
            "SELECT matviewname
             FROM pg_matviews
             WHERE schemaname = $1
             ORDER BY matviewname",
            &[&schema],
        )
        .await?;

    for r in mv_rows.iter() {
        tables.push(TableInfo {
            schema: schema.to_owned(),
            name: r.get::<_, String>(0),
            kind: TableKind::MaterializedView,
        });
    }

    tables.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(tables)
}

// ── Index & FK info ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub name: String,
    pub is_unique: bool,
    pub definition: String,
}

#[derive(Debug, Clone)]
pub struct ForeignKeyInfo {
    pub name: String,
    pub definition: String,
}

pub async fn load_primary_key(
    client: &Client,
    schema: &str,
    table: &str,
) -> Result<Vec<String>> {
    let rows = client
        .query(
            "SELECT kcu.column_name
             FROM information_schema.table_constraints tc
             JOIN information_schema.key_column_usage kcu
               ON tc.constraint_name = kcu.constraint_name
              AND tc.table_schema = kcu.table_schema
             WHERE tc.constraint_type = 'PRIMARY KEY'
               AND tc.table_schema = $1
               AND tc.table_name = $2
             ORDER BY kcu.ordinal_position",
            &[&schema, &table],
        )
        .await?;
    Ok(rows.iter().map(|r| r.get::<_, String>(0)).collect())
}

pub async fn load_indexes(
    client: &Client,
    schema: &str,
    table: &str,
) -> Result<Vec<IndexInfo>> {
    let rows = client
        .query(
            "SELECT i.relname, ix.indisunique, pg_get_indexdef(ix.indexrelid)
             FROM pg_class t
             JOIN pg_index ix ON t.oid = ix.indrelid
             JOIN pg_class i ON i.oid = ix.indexrelid
             JOIN pg_namespace n ON n.oid = t.relnamespace
             WHERE n.nspname = $1 AND t.relname = $2
             ORDER BY i.relname",
            &[&schema, &table],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| IndexInfo {
            name: r.get(0),
            is_unique: r.get(1),
            definition: r.get(2),
        })
        .collect())
}

pub async fn load_foreign_keys(
    client: &Client,
    schema: &str,
    table: &str,
) -> Result<Vec<ForeignKeyInfo>> {
    let rows = client
        .query(
            "SELECT c.conname, pg_get_constraintdef(c.oid)
             FROM pg_constraint c
             JOIN pg_class t ON t.oid = c.conrelid
             JOIN pg_namespace n ON n.oid = t.relnamespace
             WHERE c.contype = 'f'
               AND n.nspname = $1
               AND t.relname = $2
             ORDER BY c.conname",
            &[&schema, &table],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| ForeignKeyInfo {
            name: r.get(0),
            definition: r.get(1),
        })
        .collect())
}

// ── Functions / procedures ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionKind {
    Function,
    Procedure,
    Aggregate,
    Window,
}

impl FunctionKind {
    pub fn icon(&self) -> &'static str {
        match self {
            FunctionKind::Function  => "ƒ",
            FunctionKind::Procedure => "⚙",
            FunctionKind::Aggregate => "∑",
            FunctionKind::Window    => "⊞",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            FunctionKind::Function  => "FUNCTION",
            FunctionKind::Procedure => "PROCEDURE",
            FunctionKind::Aggregate => "AGGREGATE",
            FunctionKind::Window    => "WINDOW",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    /// Identity arguments, e.g. "amount numeric, rate numeric"
    pub args: String,
    pub return_type: String,
    pub kind: FunctionKind,
}

/// Load all routines in `schema`. Uses `prokind` (PG ≥ 11); on older versions
/// the query will fail and an empty list is returned silently.
pub async fn load_functions(client: &Client, schema: &str) -> Result<Vec<FunctionInfo>> {
    let rows = client
        .query(
            "SELECT p.proname,
                    pg_get_function_identity_arguments(p.oid),
                    COALESCE(pg_get_function_result(p.oid), 'void'),
                    CASE p.prokind
                        WHEN 'p' THEN 'p'
                        WHEN 'a' THEN 'a'
                        WHEN 'w' THEN 'w'
                        ELSE 'f'
                    END
             FROM pg_proc p
             JOIN pg_namespace n ON p.pronamespace = n.oid
             WHERE n.nspname = $1
             ORDER BY p.proname, 2",
            &[&schema],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| {
            let kind_ch: &str = r.get(3);
            FunctionInfo {
                name: r.get(0),
                args: r.get(1),
                return_type: r.get(2),
                kind: match kind_ch {
                    "p" => FunctionKind::Procedure,
                    "a" => FunctionKind::Aggregate,
                    "w" => FunctionKind::Window,
                    _   => FunctionKind::Function,
                },
            }
        })
        .collect())
}

// ── Schema snapshot (for diff) ────────────────────────────────────────────────

/// Load (table_name, column_name, udt_name) for all tables in a schema.
/// Used client-side to compute schema diffs without extra DB round-trips.
pub async fn load_schema_snapshot(
    client: &Client,
    schema: &str,
) -> Vec<(String, String, String)> {
    let escaped = schema.replace('\'', "''");
    let sql = format!(
        "SELECT table_name, column_name, udt_name \
         FROM information_schema.columns \
         WHERE table_schema = '{escaped}' \
         ORDER BY table_name, ordinal_position"
    );
    let mut rows = Vec::new();
    if let Ok(msgs) = client.simple_query(&sql).await {
        for msg in msgs {
            if let tokio_postgres::SimpleQueryMessage::Row(r) = msg {
                let tbl   = r.get(0).unwrap_or("").to_owned();
                let col   = r.get(1).unwrap_or("").to_owned();
                let dtype = r.get(2).unwrap_or("").to_owned();
                rows.push((tbl, col, dtype));
            }
        }
    }
    rows
}

/// Fetch ALL user tables + columns from `information_schema.columns`.
/// Returns a compact multi-line string:
/// `- schema.table (col1 type1, col2 type2, ...)`
/// Excludes system schemas (pg_catalog, information_schema, pg_toast*).
pub async fn load_full_schema_for_ai(client: &Client) -> String {
    let sql = "SELECT table_schema, table_name, column_name, udt_name \
               FROM information_schema.columns \
               WHERE table_schema NOT IN ('pg_catalog','information_schema','pg_toast') \
                 AND table_schema NOT LIKE 'pg_temp_%' \
                 AND table_schema NOT LIKE 'pg_toast_temp_%' \
               ORDER BY table_schema, table_name, ordinal_position";

    let mut map: std::collections::BTreeMap<(String, String), Vec<String>> =
        std::collections::BTreeMap::new();

    if let Ok(msgs) = client.simple_query(sql).await {
        for msg in msgs {
            if let tokio_postgres::SimpleQueryMessage::Row(r) = msg {
                let schema = r.get(0).unwrap_or("").to_owned();
                let table  = r.get(1).unwrap_or("").to_owned();
                let col    = r.get(2).unwrap_or("").to_owned();
                let dtype  = r.get(3).unwrap_or("").to_owned();
                map.entry((schema, table))
                    .or_default()
                    .push(format!("{col} {dtype}"));
            }
        }
    }

    let mut out = String::new();
    for ((schema, table), cols) in &map {
        out.push_str(&format!("- {schema}.{table} ({})\n", cols.join(", ")));
    }
    out
}

// ── Dashboard stats ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TableStat {
    pub schema: String,
    pub table: String,
    pub total_size: String,
    pub table_size: String,
    pub index_size: String,
    pub total_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct ConnInfo {
    pub pid: String,
    pub username: String,
    pub app_name: String,
    pub state: String,
    pub query_preview: String,
    pub duration: String,
}

#[derive(Debug, Clone)]
pub struct IndexStat {
    pub schema: String,
    pub table: String,
    pub index_name: String,
    pub size: String,
    pub scans: i64,
}

pub async fn load_table_stats(client: &Client) -> Result<Vec<TableStat>> {
    use tokio_postgres::SimpleQueryMessage;
    let sql = "SELECT \
        schemaname, \
        tablename, \
        pg_size_pretty(pg_total_relation_size(quote_ident(schemaname)||'.'||quote_ident(tablename))) as total_size, \
        pg_size_pretty(pg_relation_size(quote_ident(schemaname)||'.'||quote_ident(tablename))) as table_size, \
        pg_size_pretty(pg_indexes_size(quote_ident(schemaname)||'.'||quote_ident(tablename))) as index_size, \
        pg_total_relation_size(quote_ident(schemaname)||'.'||quote_ident(tablename)) as total_bytes \
        FROM pg_tables \
        WHERE schemaname NOT IN ('pg_catalog','information_schema') \
        ORDER BY total_bytes DESC NULLS LAST \
        LIMIT 30";
    let msgs = client.simple_query(sql).await?;
    let mut result = Vec::new();
    for msg in msgs {
        if let SimpleQueryMessage::Row(row) = msg {
            result.push(TableStat {
                schema: row.get(0).unwrap_or("").to_owned(),
                table: row.get(1).unwrap_or("").to_owned(),
                total_size: row.get(2).unwrap_or("").to_owned(),
                table_size: row.get(3).unwrap_or("").to_owned(),
                index_size: row.get(4).unwrap_or("").to_owned(),
                total_bytes: row.get(5).and_then(|s| s.parse().ok()).unwrap_or(0),
            });
        }
    }
    Ok(result)
}

pub async fn load_connections(client: &Client) -> Result<Vec<ConnInfo>> {
    use tokio_postgres::SimpleQueryMessage;
    let sql = "SELECT \
        pid::text, \
        COALESCE(usename,'') as usename, \
        COALESCE(application_name,'') as app, \
        COALESCE(state,'') as state, \
        LEFT(COALESCE(query,''),80) as query_preview, \
        COALESCE(EXTRACT(EPOCH FROM (now()-query_start))::bigint::text||'s','') as duration \
        FROM pg_stat_activity \
        WHERE datname = current_database() \
        ORDER BY query_start DESC NULLS LAST";
    let msgs = client.simple_query(sql).await?;
    let mut result = Vec::new();
    for msg in msgs {
        if let SimpleQueryMessage::Row(row) = msg {
            result.push(ConnInfo {
                pid: row.get(0).unwrap_or("").to_owned(),
                username: row.get(1).unwrap_or("").to_owned(),
                app_name: row.get(2).unwrap_or("").to_owned(),
                state: row.get(3).unwrap_or("").to_owned(),
                query_preview: row.get(4).unwrap_or("").to_owned(),
                duration: row.get(5).unwrap_or("").to_owned(),
            });
        }
    }
    Ok(result)
}

pub async fn load_index_stats(client: &Client) -> Result<Vec<IndexStat>> {
    use tokio_postgres::SimpleQueryMessage;
    let sql = "SELECT \
        schemaname, \
        relname as tablename, \
        indexrelname as indexname, \
        pg_size_pretty(pg_relation_size(indexrelid)) as size, \
        idx_scan \
        FROM pg_stat_user_indexes \
        ORDER BY pg_relation_size(indexrelid) DESC NULLS LAST \
        LIMIT 30";
    let msgs = client.simple_query(sql).await?;
    let mut result = Vec::new();
    for msg in msgs {
        if let SimpleQueryMessage::Row(row) = msg {
            result.push(IndexStat {
                schema: row.get(0).unwrap_or("").to_owned(),
                table: row.get(1).unwrap_or("").to_owned(),
                index_name: row.get(2).unwrap_or("").to_owned(),
                size: row.get(3).unwrap_or("").to_owned(),
                scans: row.get(4).and_then(|s| s.parse().ok()).unwrap_or(0),
            });
        }
    }
    Ok(result)
}

// ── ER Diagram types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ParsedForeignKey {
    pub constraint_name: String,
    pub source_columns: Vec<String>,
    pub target_schema: String,
    pub target_table: String,
    pub target_columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ErTableInfo {
    pub schema: String,
    pub table: String,
    pub columns: Vec<ColumnInfo>,
    pub primary_keys: Vec<String>,
    pub foreign_keys: Vec<ParsedForeignKey>,
}

/// Parse a pg_get_constraintdef FK definition like:
///   "FOREIGN KEY (col) REFERENCES [schema.]table(col)"
pub fn parse_foreign_key(fk: &ForeignKeyInfo) -> Option<ParsedForeignKey> {
    let def = &fk.definition;
    let fk_kw = "FOREIGN KEY (";
    let fk_start = def.find(fk_kw)?;
    let after_fk = &def[fk_start + fk_kw.len()..];
    let src_end = after_fk.find(')')?;
    let source_columns: Vec<String> = after_fk[..src_end]
        .split(',')
        .map(|s| s.trim().to_owned())
        .collect();

    let ref_kw = "REFERENCES ";
    let ref_start = def.find(ref_kw)?;
    let after_ref = &def[ref_start + ref_kw.len()..];
    let paren_pos = after_ref.find('(')?;
    let table_part = after_ref[..paren_pos].trim();

    let (target_schema, target_table) = if let Some(dot) = table_part.find('.') {
        (table_part[..dot].to_owned(), table_part[dot + 1..].to_owned())
    } else {
        (String::new(), table_part.to_owned())
    };

    let after_paren = &after_ref[paren_pos + 1..];
    let tgt_end = after_paren.find(')')?;
    let target_columns: Vec<String> = after_paren[..tgt_end]
        .split(',')
        .map(|s| s.trim().to_owned())
        .collect();

    Some(ParsedForeignKey {
        constraint_name: fk.name.clone(),
        source_columns,
        target_schema,
        target_table,
        target_columns,
    })
}

pub async fn load_er_diagram(client: &Client, schema: &str) -> Result<Vec<ErTableInfo>> {
    let tables = load_tables(client, schema).await?;
    let mut result = Vec::new();

    for t in &tables {
        if !matches!(t.kind, TableKind::Table) {
            continue;
        }
        let columns = load_columns(client, schema, &t.name).await.unwrap_or_default();
        let primary_keys = load_primary_key(client, schema, &t.name).await.unwrap_or_default();
        let raw_fks = load_foreign_keys(client, schema, &t.name).await.unwrap_or_default();
        let foreign_keys = raw_fks.iter().filter_map(parse_foreign_key).collect();

        result.push(ErTableInfo {
            schema: schema.to_owned(),
            table: t.name.clone(),
            columns,
            primary_keys,
            foreign_keys,
        });
    }
    Ok(result)
}

pub async fn load_schema_columns(
    client: &Client,
    schema: &str,
) -> Result<HashMap<String, Vec<String>>> {
    let rows = client
        .query(
            "SELECT table_name, column_name
             FROM information_schema.columns
             WHERE table_schema = $1
             ORDER BY table_name, ordinal_position",
            &[&schema],
        )
        .await?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let table: String = row.get(0);
        let col: String = row.get(1);
        map.entry(table).or_default().push(col);
    }
    Ok(map)
}

pub async fn load_columns(
    client: &Client,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>> {
    let rows = client
        .query(
            "SELECT column_name, data_type, is_nullable, column_default
             FROM information_schema.columns
             WHERE table_schema = $1 AND table_name = $2
             ORDER BY ordinal_position",
            &[&schema, &table],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| ColumnInfo {
            name: r.get::<_, String>(0),
            data_type: r.get::<_, String>(1),
            is_nullable: r.get::<_, String>(2) == "YES",
            column_default: r.get::<_, Option<String>>(3),
        })
        .collect())
}
