use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

use anyhow::Result;
use tokio_postgres::NoTls;

use crate::config::{ConnectionProfile, SslMode};
use crate::db::metadata::{self, ColumnInfo, ConnInfo, ErTableInfo, ForeignKeyInfo, FunctionInfo, IndexInfo, IndexStat, TableInfo, TableStat};
use crate::db::query::{parse_text_cell, QueryResult};

/// Commands sent from the UI thread → DB worker.
#[derive(Debug)]
pub enum DbCommand {
    Connect(ConnectionProfile),
    Disconnect,
    LoadSchemas,
    LoadTables { schema: String },
    LoadDetails { schema: String, table: String },
    LoadPrimaryKey { schema: String, table: String },
    Execute(String),
    /// DDL statement (CREATE/ALTER TABLE etc.). Emits DdlDone on success.
    ExecuteDdl(String),
    CancelQuery,
    ExportCsv { sql: String, path: String },
    ExportJson { sql: String, path: String },
    LoadDashboard,
    LoadErDiagram { schema: String },
    /// Execute DML inside an explicit BEGIN — keeps connection in transaction.
    ExecuteSafe(String),
    /// Commit the open safe-mode transaction.
    Commit,
    /// Roll back the open safe-mode transaction.
    Rollback,
    /// Terminate a backend process by PID (pg_terminate_backend).
    KillConnection { pid: String },
    /// Test a connection profile without storing the client.
    TestConnection(ConnectionProfile),
    /// Execute multiple SQL statements individually; each gets its own QueryResult.
    ExecuteMulti(Vec<String>),
    /// Load functions/procedures for a schema.
    LoadFunctions { schema: String },
    /// Load (table, column, udt_name) snapshot for schema diff.
    LoadSchemaSnapshot { schema: String, request_id: u64 },
    /// Fetch full schema context for AI (all user schemas, tables, columns).
    LoadFullSchemaForAi { request_id: u64 },
}

/// Events sent from the DB worker → UI thread.
#[derive(Debug)]
pub enum DbEvent {
    Connected { host: String, database: String },
    ConnectionError(String),
    Disconnected,
    Schemas(Vec<metadata::SchemaInfo>),
    Tables { schema: String, tables: Vec<TableInfo> },
    TableDetails {
        schema: String,
        table: String,
        columns: Vec<ColumnInfo>,
        indexes: Vec<IndexInfo>,
        foreign_keys: Vec<ForeignKeyInfo>,
    },
    PrimaryKey { schema: String, table: String, columns: Vec<String> },
    QueryResult(QueryResult),
    QueryError(String),
    ExportDone(String),
    /// DDL executed successfully.
    DdlDone,
    /// pg_terminate_backend succeeded for this PID.
    KillDone(String),
    DashboardData {
        table_stats: Vec<TableStat>,
        connections: Vec<ConnInfo>,
        index_stats: Vec<IndexStat>,
    },
    ErDiagramData { schema: String, tables: Vec<ErTableInfo> },
    /// Result of a TestConnection command.
    TestResult { success: bool, message: String },
    /// Results for each statement in an ExecuteMulti command (one per statement).
    MultiQueryResults(Vec<QueryResult>),
    /// Functions/procedures loaded for a schema.
    Functions { schema: String, functions: Vec<FunctionInfo> },
    /// Schema snapshot rows for diff (table, column, udt_name).
    SchemaSnapshot { request_id: u64, rows: Vec<(String, String, String)> },
    /// Full schema context string for AI NL→SQL.
    AiSchemaReady { request_id: u64, context: String },
    /// A safe-mode transaction was opened (BEGIN succeeded).
    TransactionOpen,
    /// The safe-mode transaction was closed (COMMIT or ROLLBACK).
    TransactionClosed,
}

/// Handle that owns the DB background thread.
pub struct DbHandle;

impl DbHandle {
    pub fn spawn(cmd_rx: Receiver<DbCommand>, evt_tx: Sender<DbEvent>) {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio rt");
            rt.block_on(db_worker(cmd_rx, evt_tx));
        });
    }
}

async fn db_worker(cmd_rx: Receiver<DbCommand>, evt_tx: Sender<DbEvent>) {
    use std::sync::{Arc, Mutex};

    let mut client: Option<tokio_postgres::Client> = None;
    let mut cancel_handle: Option<tokio_postgres::CancelToken> = None;
    let mut last_profile: Option<ConnectionProfile> = None;

    // Wrap cmd_rx in Arc<Mutex> so we can move it into spawn_blocking
    let cmd_rx = Arc::new(Mutex::new(cmd_rx));

    loop {
        // Receive next command without blocking the tokio executor
        let rx = Arc::clone(&cmd_rx);
        let cmd = match tokio::task::spawn_blocking(move || rx.lock().unwrap().recv()).await {
            Ok(Ok(c)) => c,
            _ => break, // sender dropped or thread error → exit
        };

        match cmd {
            DbCommand::Connect(profile) => {
                match connect_pg(&profile).await {
                    Ok((c, cancel)) => {
                        let host = profile.host.clone();
                        let database = profile.database.clone();
                        cancel_handle = Some(cancel);
                        client = Some(c);
                        last_profile = Some(profile);
                        let _ = evt_tx.send(DbEvent::Connected { host, database });
                    }
                    Err(e) => {
                        let _ = evt_tx.send(DbEvent::ConnectionError(e.to_string()));
                    }
                }
            }

            DbCommand::Disconnect => {
                client = None;
                cancel_handle = None;
                let _ = evt_tx.send(DbEvent::Disconnected);
            }

            DbCommand::LoadSchemas => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &client {
                        match metadata::load_schemas(c).await {
                            Ok(schemas) => {
                                let _ = evt_tx.send(DbEvent::Schemas(schemas));
                            }
                            Err(e) => {
                                let _ = evt_tx.send(DbEvent::QueryError(fmt_pg_error(&e)));
                            }
                        }
                    }
                }
            }

            DbCommand::LoadTables { schema } => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &client {
                        match metadata::load_tables(c, &schema).await {
                            Ok(tables) => {
                                let _ = evt_tx.send(DbEvent::Tables { schema, tables });
                            }
                            Err(e) => {
                                let _ = evt_tx.send(DbEvent::QueryError(fmt_pg_error(&e)));
                            }
                        }
                    }
                }
            }

            DbCommand::LoadFunctions { schema } => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &client {
                        // Silently return empty on older PG versions that don't have prokind.
                        let functions = metadata::load_functions(c, &schema)
                            .await
                            .unwrap_or_default();
                        let _ = evt_tx.send(DbEvent::Functions { schema, functions });
                    }
                }
            }

            DbCommand::LoadDetails { schema, table } => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &client {
                        let columns = metadata::load_columns(c, &schema, &table)
                            .await
                            .unwrap_or_default();
                        let indexes = metadata::load_indexes(c, &schema, &table)
                            .await
                            .unwrap_or_default();
                        let foreign_keys = metadata::load_foreign_keys(c, &schema, &table)
                            .await
                            .unwrap_or_default();
                        let _ = evt_tx.send(DbEvent::TableDetails {
                            schema,
                            table,
                            columns,
                            indexes,
                            foreign_keys,
                        });
                    }
                }
            }

            DbCommand::LoadPrimaryKey { schema, table } => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &client {
                        let columns = metadata::load_primary_key(c, &schema, &table)
                            .await
                            .unwrap_or_default();
                        let _ = evt_tx.send(DbEvent::PrimaryKey { schema, table, columns });
                    }
                }
            }

            DbCommand::Execute(sql) => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &mut client {
                        let start = Instant::now();
                        match execute_query(c, &sql).await {
                            Ok(mut result) => {
                                result.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                                let _ = evt_tx.send(DbEvent::QueryResult(result));
                            }
                            Err(e) => {
                                let _ = evt_tx.send(DbEvent::QueryError(fmt_pg_error(&e)));
                            }
                        }
                    }
                } else {
                    let _ = evt_tx.send(DbEvent::QueryError("Not connected".into()));
                }
            }

            DbCommand::ExecuteDdl(sql) => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &mut client {
                        match c.execute(sql.as_str(), &[]).await {
                            Ok(_) => {
                                let _ = evt_tx.send(DbEvent::DdlDone);
                            }
                            Err(e) => {
                                let err = anyhow::anyhow!(e);
                                let _ = evt_tx.send(DbEvent::QueryError(fmt_pg_error(&err)));
                            }
                        }
                    }
                } else {
                    let _ = evt_tx.send(DbEvent::QueryError("Not connected".into()));
                }
            }

            DbCommand::CancelQuery => {
                if let Some(cancel) = cancel_handle.take() {
                    tokio::spawn(async move {
                        let _ = cancel.cancel_query(NoTls).await;
                    });
                }
            }

            DbCommand::ExportCsv { sql, path } => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &mut client {
                        match execute_query(c, &sql).await {
                            Ok(result) => match export_csv(&result, &path) {
                                Ok(_) => {
                                    let _ = evt_tx.send(DbEvent::ExportDone(path));
                                }
                                Err(e) => {
                                    let _ = evt_tx.send(DbEvent::QueryError(e.to_string()));
                                }
                            },
                            Err(e) => {
                                let _ = evt_tx.send(DbEvent::QueryError(fmt_pg_error(&e)));
                            }
                        }
                    }
                }
            }

            DbCommand::ExportJson { sql, path } => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &mut client {
                        match execute_query(c, &sql).await {
                            Ok(result) => match export_json(&result, &path) {
                                Ok(_) => {
                                    let _ = evt_tx.send(DbEvent::ExportDone(path));
                                }
                                Err(e) => {
                                    let _ = evt_tx.send(DbEvent::QueryError(e.to_string()));
                                }
                            },
                            Err(e) => {
                                let _ = evt_tx.send(DbEvent::QueryError(fmt_pg_error(&e)));
                            }
                        }
                    }
                }
            }

            DbCommand::LoadDashboard => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &client {
                        let ts = metadata::load_table_stats(c).await.unwrap_or_default();
                        let conns = metadata::load_connections(c).await.unwrap_or_default();
                        let idxs = metadata::load_index_stats(c).await.unwrap_or_default();
                        let _ = evt_tx.send(DbEvent::DashboardData {
                            table_stats: ts,
                            connections: conns,
                            index_stats: idxs,
                        });
                    }
                }
            }

            DbCommand::ExecuteSafe(sql) => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &mut client {
                        // Open transaction
                        if let Err(e) = c.simple_query("BEGIN").await {
                            let _ = evt_tx.send(DbEvent::QueryError(format!("BEGIN failed: {e}")));
                            continue;
                        }
                        let start = Instant::now();
                        match execute_query(c, &sql).await {
                            Ok(mut result) => {
                                result.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                                let _ = evt_tx.send(DbEvent::QueryResult(result));
                                let _ = evt_tx.send(DbEvent::TransactionOpen);
                            }
                            Err(e) => {
                                // Auto-rollback on error to keep connection clean
                                let _ = c.simple_query("ROLLBACK").await;
                                let _ = evt_tx.send(DbEvent::QueryError(fmt_pg_error(&e)));
                                let _ = evt_tx.send(DbEvent::TransactionClosed);
                            }
                        }
                    }
                } else {
                    let _ = evt_tx.send(DbEvent::QueryError("Not connected".into()));
                }
            }

            DbCommand::Commit => {
                if let Some(c) = &mut client {
                    match c.simple_query("COMMIT").await {
                        Ok(_) => { let _ = evt_tx.send(DbEvent::TransactionClosed); }
                        Err(e) => { let _ = evt_tx.send(DbEvent::QueryError(anyhow::anyhow!(e).to_string())); }
                    }
                }
            }

            DbCommand::Rollback => {
                if let Some(c) = &mut client {
                    match c.simple_query("ROLLBACK").await {
                        Ok(_) => { let _ = evt_tx.send(DbEvent::TransactionClosed); }
                        Err(e) => { let _ = evt_tx.send(DbEvent::QueryError(anyhow::anyhow!(e).to_string())); }
                    }
                }
            }

            DbCommand::KillConnection { pid } => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &client {
                        let sql = format!("SELECT pg_terminate_backend({pid}::int)");
                        match c.simple_query(&sql).await {
                            Ok(_) => { let _ = evt_tx.send(DbEvent::KillDone(pid)); }
                            Err(e) => { let _ = evt_tx.send(DbEvent::QueryError(anyhow::anyhow!(e).to_string())); }
                        }
                    }
                }
            }

            DbCommand::TestConnection(profile) => {
                match connect_pg(&profile).await {
                    Ok(_) => {
                        let _ = evt_tx.send(DbEvent::TestResult { success: true, message: String::new() });
                    }
                    Err(e) => {
                        let _ = evt_tx.send(DbEvent::TestResult { success: false, message: fmt_pg_error(&e) });
                    }
                }
            }

            DbCommand::ExecuteMulti(stmts) => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &mut client {
                        let mut results: Vec<QueryResult> = Vec::with_capacity(stmts.len());
                        let mut exec_error: Option<String> = None;
                        for stmt in &stmts {
                            let start = Instant::now();
                            match execute_query(c, stmt).await {
                                Ok(mut r) => {
                                    r.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                                    results.push(r);
                                }
                                Err(e) => {
                                    exec_error = Some(fmt_pg_error(&e));
                                    break;
                                }
                            }
                        }
                        let _ = evt_tx.send(DbEvent::MultiQueryResults(results));
                        if let Some(e) = exec_error {
                            let _ = evt_tx.send(DbEvent::QueryError(e));
                        }
                    }
                } else {
                    let _ = evt_tx.send(DbEvent::QueryError("Not connected".into()));
                }
            }


            DbCommand::LoadSchemaSnapshot { schema, request_id } => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &client {
                        let rows = metadata::load_schema_snapshot(c, &schema).await;
                        let _ = evt_tx.send(DbEvent::SchemaSnapshot { request_id, rows });
                    }
                }
            }

            DbCommand::LoadErDiagram { schema } => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &client {
                        match metadata::load_er_diagram(c, &schema).await {
                            Ok(tables) => {
                                let _ = evt_tx.send(DbEvent::ErDiagramData { schema, tables });
                            }
                            Err(e) => {
                                let _ = evt_tx.send(DbEvent::QueryError(fmt_pg_error(&e)));
                            }
                        }
                    }
                }
            }

            DbCommand::LoadFullSchemaForAi { request_id } => {
                if ensure_connected(&mut client, &mut cancel_handle, &last_profile, &evt_tx).await {
                    if let Some(c) = &client {
                        let context = metadata::load_full_schema_for_ai(c).await;
                        let _ = evt_tx.send(DbEvent::AiSchemaReady { request_id, context });
                    }
                } else {
                    let _ = evt_tx.send(DbEvent::AiSchemaReady { request_id, context: String::new() });
                }
            }
        }
    }
}

/// Checks if the current client is alive; if not, attempts to reconnect using
/// the last known profile. Returns `true` if a usable connection is available.
/// Sends `DbEvent::Connected` on successful reconnect (triggers sidebar reload).
async fn ensure_connected(
    client: &mut Option<tokio_postgres::Client>,
    cancel_handle: &mut Option<tokio_postgres::CancelToken>,
    last_profile: &Option<ConnectionProfile>,
    evt_tx: &Sender<DbEvent>,
) -> bool {
    // If client exists and connection is still alive, nothing to do.
    if let Some(c) = client.as_ref() {
        if !c.is_closed() {
            return true;
        }
    }

    // Connection is gone. Try to reconnect if we have a profile.
    let profile = match last_profile {
        Some(p) => p,
        None => return false,
    };

    match connect_pg(profile).await {
        Ok((c, cancel)) => {
            let host = profile.host.clone();
            let database = profile.database.clone();
            *cancel_handle = Some(cancel);
            *client = Some(c);
            let _ = evt_tx.send(DbEvent::Connected { host, database });
            true
        }
        Err(e) => {
            let _ = evt_tx.send(DbEvent::QueryError(format!("Reconnect failed: {e}")));
            false
        }
    }
}

async fn connect_pg(
    profile: &ConnectionProfile,
) -> Result<(tokio_postgres::Client, tokio_postgres::CancelToken)> {
    let (effective_host, effective_port) = match &profile.ssh_tunnel {
        Some(ssh) if ssh.enabled => {
            let port = super::ssh::establish_tunnel(
                &ssh.host,
                ssh.port,
                &ssh.user,
                &ssh.auth,
                &profile.host,
                profile.port,
            )
            .await?;
            ("127.0.0.1".to_owned(), port)
        }
        _ => (profile.host.clone(), profile.port),
    };

    let connstr = if effective_host != profile.host || effective_port != profile.port {
        let ssl = match profile.ssl {
            SslMode::Disable => "disable",
            SslMode::Prefer => "prefer",
            SslMode::Require => "require",
        };
        format!(
            "host={} port={} user={} password={} dbname={} sslmode={}",
            effective_host, effective_port, profile.user, profile.password, profile.database, ssl
        )
    } else {
        profile.connection_string()
    };

    match profile.ssl {
        SslMode::Disable => {
            let (client, connection) = tokio_postgres::connect(&connstr, NoTls).await?;
            let cancel = client.cancel_token();
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("[pgclient] DB connection error: {e}");
                }
            });
            Ok((client, cancel))
        }
        SslMode::Prefer | SslMode::Require => {
            let mut builder = native_tls::TlsConnector::builder();
            // For Prefer, accept self-signed certs; for Require, enforce server cert.
            if profile.ssl == SslMode::Prefer {
                builder.danger_accept_invalid_certs(true);
            }
            let connector = builder.build()?;
            let tls = postgres_native_tls::MakeTlsConnector::new(connector);
            let (client, connection) = tokio_postgres::connect(&connstr, tls).await?;
            let cancel = client.cancel_token();
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("[pgclient] DB connection error (TLS): {e}");
                }
            });
            Ok((client, cancel))
        }
    }
}

/// Maximum rows kept in memory per query result. Rows beyond this are dropped
/// and a warning is appended so the user knows the result was truncated.
const MAX_RESULT_ROWS: usize = 50_000;

async fn execute_query(
    client: &mut tokio_postgres::Client,
    sql: &str,
) -> Result<QueryResult> {
    // Use simple_query (text protocol) for ALL statements:
    // - Supports semicolons and multiple statements in one call.
    // - All PostgreSQL types arrive as plain strings without per-type decoders.
    // - Works for SELECT, DML, DDL alike.
    use tokio_postgres::SimpleQueryMessage;

    let msgs = client.simple_query(sql).await?;

    // Buffers for the statement currently being accumulated.
    let mut cur_cols: Vec<String> = vec![];
    let mut cur_rows: Vec<Vec<crate::db::query::CellValue>> = vec![];

    // Keep the last SELECT result; accumulate row counts for DML/DDL.
    let mut last_columns: Vec<String> = vec![];
    let mut last_rows: Vec<Vec<crate::db::query::CellValue>> = vec![];
    let mut rows_affected: Option<u64> = None;
    let mut had_select = false;

    for msg in msgs {
        match msg {
            SimpleQueryMessage::Row(row) => {
                if cur_cols.is_empty() {
                    cur_cols = row.columns().iter().map(|c| c.name().to_owned()).collect();
                }
                if cur_rows.len() < MAX_RESULT_ROWS {
                    let cells = (0..cur_cols.len())
                        .map(|i| match row.get(i) {
                            None => crate::db::query::CellValue::Null,
                            Some(s) => parse_text_cell(s),
                        })
                        .collect();
                    cur_rows.push(cells);
                }
            }
            SimpleQueryMessage::CommandComplete(tag) => {
                if !cur_rows.is_empty() || !cur_cols.is_empty() {
                    // This statement returned rows — save as the latest result.
                    last_columns = std::mem::take(&mut cur_cols);
                    last_rows = std::mem::take(&mut cur_rows);
                    had_select = true;
                } else {
                    cur_cols.clear();
                    cur_rows.clear();
                    // CommandComplete carries the affected-row count directly.
                    rows_affected = Some(rows_affected.unwrap_or(0) + tag);
                }
            }
            _ => {}
        }
    }

    if had_select {
        return Ok(QueryResult { columns: last_columns, rows: last_rows, rows_affected: None, elapsed_ms: 0.0 });
    }

    // No rows were received. If the SQL was a single SELECT-like statement that
    // returned 0 rows, simple_query never emits a RowDescription message, so we
    // lost the column names.  Use prepare() to recover them without re-executing.
    let stmts: Vec<&str> = sql.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if stmts.len() == 1 {
        let lower = stmts[0].to_lowercase();
        let is_select_like = lower.starts_with("select")
            || lower.starts_with("with")
            || lower.starts_with("table")
            || lower.starts_with("values")
            || lower.starts_with("show")
            || lower.starts_with("explain");

        if is_select_like {
            if let Ok(stmt) = client.prepare(stmts[0]).await {
                let columns: Vec<String> =
                    stmt.columns().iter().map(|c| c.name().to_owned()).collect();
                if !columns.is_empty() {
                    return Ok(QueryResult { columns, rows: vec![], rows_affected: None, elapsed_ms: 0.0 });
                }
            }
            // SELECT-like but prepare() failed or returned no columns (e.g. a view
            // that references a dropped object). Still report as SELECT (no
            // rows_affected) so the caller doesn't mistake this for DML.
            return Ok(QueryResult { columns: vec![], rows: vec![], rows_affected: None, elapsed_ms: 0.0 });
        }
    }

    Ok(QueryResult { columns: vec![], rows: vec![], rows_affected, elapsed_ms: 0.0 })
}

/// Format an anyhow error as a human-readable string.
/// For tokio-postgres `DbError`s the raw string looks like
/// `db error: ERROR: relation "x" does not exist` — we strip the redundant
/// prefix and append DETAIL / HINT lines when available.
fn fmt_pg_error(e: &anyhow::Error) -> String {
    if let Some(pg) = e.downcast_ref::<tokio_postgres::Error>() {
        if let Some(db) = pg.as_db_error() {
            let mut s = db.message().to_owned();
            if let Some(d) = db.detail() { s.push_str(&format!("\nDetail: {d}")); }
            if let Some(h) = db.hint()   { s.push_str(&format!("\nHint: {h}")); }
            return s;
        }
    }
    e.to_string()
}

fn export_csv(result: &QueryResult, path: &str) -> Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;

    // Header
    writeln!(f, "{}", result.columns.join(","))?;

    // Rows
    for row in &result.rows {
        let cells: Vec<String> = row
            .iter()
            .map(|cell| {
                let s = cell.to_string();
                if s.contains(',') || s.contains('"') || s.contains('\n') {
                    format!("\"{}\"", s.replace('"', "\"\""))
                } else {
                    s
                }
            })
            .collect();
        writeln!(f, "{}", cells.join(","))?;
    }
    Ok(())
}

fn export_json(result: &QueryResult, path: &str) -> Result<()> {
    use serde_json::{json, Value};

    let rows: Vec<Value> = result
        .rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (col, cell) in result.columns.iter().zip(row.iter()) {
                let v = match cell {
                    crate::db::query::CellValue::Null => Value::Null,
                    crate::db::query::CellValue::Text(s) => Value::String(s.to_string()),
                    crate::db::query::CellValue::Integer(i) => json!(i),
                    crate::db::query::CellValue::Float(f) => json!(f),
                    crate::db::query::CellValue::Boolean(b) => Value::Bool(*b),
                    crate::db::query::CellValue::Bytes(b) => {
                        Value::String(format!("\\x{}", hex::encode(b)))
                    }
                };
                obj.insert(col.clone(), v);
            }
            Value::Object(obj)
        })
        .collect();

    let json_str = serde_json::to_string_pretty(&rows)?;
    std::fs::write(path, json_str)?;
    Ok(())
}
