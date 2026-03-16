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
