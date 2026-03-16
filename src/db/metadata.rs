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
