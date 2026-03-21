use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

use anyhow::Result;
use tokio_postgres::NoTls;

use crate::config::{ConnectionProfile, SslMode};
use crate::db::metadata::{self, ColumnInfo, ConnInfo, ErTableInfo, ForeignKeyInfo, IndexInfo, IndexStat, TableInfo, TableStat};
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
    DashboardData {
        table_stats: Vec<TableStat>,
        connections: Vec<ConnInfo>,
        index_stats: Vec<IndexStat>,
    },
    ErDiagramData { schema: String, tables: Vec<ErTableInfo> },
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
                if let Some(c) = &client {
                    match metadata::load_schemas(c).await {
                        Ok(schemas) => {
                            let _ = evt_tx.send(DbEvent::Schemas(schemas));
                        }
                        Err(e) => {
                            let _ = evt_tx.send(DbEvent::QueryError(e.to_string()));
                        }
                    }
                }
            }

            DbCommand::LoadTables { schema } => {
                if let Some(c) = &client {
                    match metadata::load_tables(c, &schema).await {
                        Ok(tables) => {
                            let _ = evt_tx.send(DbEvent::Tables { schema, tables });
                        }
                        Err(e) => {
                            let _ = evt_tx.send(DbEvent::QueryError(e.to_string()));
                        }
                    }
                }
            }

            DbCommand::LoadDetails { schema, table } => {
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

            DbCommand::LoadPrimaryKey { schema, table } => {
                if let Some(c) = &client {
                    let columns = metadata::load_primary_key(c, &schema, &table)
                        .await
                        .unwrap_or_default();
                    let _ = evt_tx.send(DbEvent::PrimaryKey { schema, table, columns });
                }
            }

            DbCommand::Execute(sql) => {
                if let Some(c) = &mut client {
                    let start = Instant::now();
                    match execute_query(c, &sql).await {
                        Ok(mut result) => {
                            result.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                            let _ = evt_tx.send(DbEvent::QueryResult(result));
                        }
                        Err(e) => {
                            let _ = evt_tx.send(DbEvent::QueryError(e.to_string()));
                        }
                    }
                } else {
                    let _ = evt_tx.send(DbEvent::QueryError("Not connected".into()));
                }
            }

            DbCommand::ExecuteDdl(sql) => {
                if let Some(c) = &mut client {
                    match c.execute(sql.as_str(), &[]).await {
                        Ok(_) => {
                            let _ = evt_tx.send(DbEvent::DdlDone);
                        }
                        Err(e) => {
                            let _ = evt_tx.send(DbEvent::QueryError(e.to_string()));
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
                            let _ = evt_tx.send(DbEvent::QueryError(e.to_string()));
                        }
                    }
                }
            }

            DbCommand::ExportJson { sql, path } => {
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
                            let _ = evt_tx.send(DbEvent::QueryError(e.to_string()));
                        }
                    }
                }
            }

            DbCommand::LoadDashboard => {
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

            DbCommand::LoadErDiagram { schema } => {
                if let Some(c) = &client {
                    match metadata::load_er_diagram(c, &schema).await {
                        Ok(tables) => {
                            let _ = evt_tx.send(DbEvent::ErDiagramData { schema, tables });
                        }
                        Err(e) => {
                            let _ = evt_tx.send(DbEvent::QueryError(e.to_string()));
                        }
                    }
                }
            }
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

async fn execute_query(
    client: &mut tokio_postgres::Client,
    sql: &str,
) -> Result<QueryResult> {
    // Detect if this is a SELECT-like statement.
    let trimmed = sql.trim().to_lowercase();
    let is_query = trimmed.starts_with("select")
        || trimmed.starts_with("with")
        || trimmed.starts_with("table")
        || trimmed.starts_with("values")
        || trimmed.starts_with("show")
        || trimmed.starts_with("explain");

    if is_query {
        // Use simple_query (text protocol) so ALL PostgreSQL types — timestamps,
        // UUIDs, numerics, inet, etc. — arrive as plain strings without needing
        // per-type Rust decoders.
        use tokio_postgres::SimpleQueryMessage;

        let msgs = client.simple_query(sql).await?;
        let mut columns: Vec<String> = vec![];
        let mut rows = vec![];

        for msg in msgs {
            if let SimpleQueryMessage::Row(row) = msg {
                if columns.is_empty() {
                    columns =
                        row.columns().iter().map(|c| c.name().to_owned()).collect();
                }
                let cells = (0..columns.len())
                    .map(|i| match row.get(i) {
                        None => crate::db::query::CellValue::Null,
                        Some(s) => parse_text_cell(s),
                    })
                    .collect();
                rows.push(cells);
            }
        }

        Ok(QueryResult { columns, rows, rows_affected: None, elapsed_ms: 0.0 })
    } else {
        let n = client.execute(sql, &[]).await?;
        Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected: Some(n),
            elapsed_ms: 0.0,
        })
    }
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
                    crate::db::query::CellValue::Text(s) => Value::String(s.clone()),
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
