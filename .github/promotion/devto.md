# Dev.to Article

> **Başlık:**
> I built a PostgreSQL client in Rust because DBeaver was eating my RAM

> **Tags:** rust, postgres, opensource, showdev

---

My daily laptop has 8GB of RAM. Not a lot by modern standards, but enough — until I open DBeaver.

DBeaver at idle: ~400MB. Add a browser, a terminal, maybe a local Docker container running Postgres, and half my RAM is gone before I've written a single query. pgAdmin wasn't much better.

So I did what any reasonable developer would do: I spent my weekends building a replacement.

## Meet Ferox

**Ferox** is a lightweight native desktop PostgreSQL client written entirely in Rust.

No Electron. No JVM. No web engine. Just a native binary that starts in under 200ms and sits at ~25MB RAM at idle.

→ https://github.com/frkdrgt/ferox

---

## Features

- **Multi-tab query editor** — SQL syntax highlighting, Ctrl+T for new tab, persistent query history
- **Schema browser** — lazy-loaded tree with filter: schemas, tables, views, materialized views, foreign tables
- **Data browser** — double-click any table to browse with server-side pagination and ORDER BY
- **Visual Join Builder** — pick your tables, pick your columns, get ready-to-run SQL
- **EXPLAIN visualizer** — tree view of query plans with cost and timing per node
- **Inline editing** — double-click a cell, edit, Enter to commit an UPDATE
- **Export** — CSV and JSON via native OS file dialog

---

## The Architecture

The design is intentionally simple. Two threads, one channel each:

```
┌─────────────────────────────┐
│     UI Thread (egui)        │
│  immediate-mode rendering   │
└────────┬──────────┬─────────┘
         │ DbCommand│ DbEvent
         ▼          ▼
┌─────────────────────────────┐
│     DB Thread (tokio)       │
│  tokio-postgres + native-tls│
└─────────────────────────────┘
```

The UI thread sends `DbCommand` variants (Execute, LoadSchemas, LoadDetails, etc.) and receives `DbEvent` variants back (QueryResult, Tables, Error, etc.). The UI never blocks — it just renders whatever state it has and updates when events arrive.

```rust
// Sending a query from UI thread
let _ = self.db_tx.send(DbCommand::Execute(sql));

// Receiving results in the same UI update loop
while let Ok(event) = self.db_rx.try_recv() {
    match event {
        DbEvent::QueryResult(result) => { /* update state */ }
        DbEvent::QueryError(msg)     => { /* show error   */ }
        _                            => {}
    }
}
```

This pattern keeps egui's immediate-mode rendering smooth — no await, no blocking, no Arc<Mutex<>> gymnastics.

---

## Why egui?

I wanted a pure-Rust GUI with no system dependencies beyond what Rust itself provides. `egui` is immediate-mode (like Dear ImGui), which means the entire UI is re-rendered every frame from application state. No widget tree to manage, no data binding, no reactivity framework.

For a database client this works surprisingly well — tables, scroll areas, and text inputs are all first-class citizens in egui.

---

## Tech Stack

| Role | Crate |
|------|-------|
| GUI | `egui` + `eframe` |
| PostgreSQL driver | `tokio-postgres` |
| Async runtime | `tokio` |
| TLS | `native-tls` + `postgres-native-tls` |
| Syntax highlighting | `syntect` |
| Config | `serde` + `toml` |
| File dialogs | `rfd` |

---

## Numbers

| Metric | Ferox | DBeaver |
|--------|-------|---------|
| RAM at idle | **~25 MB** | ~400 MB |
| Startup | **<200ms** | 5–15s |
| Binary size | **~12 MB** | ~200 MB |

---

## What's Next

- Auto-complete (table names, column names, keywords)
- SSH tunnel support
- ER diagram view
- Multiple simultaneous connections
- Dark/light theme toggle

---

It's v0.1.0 and rough around the edges, but it's been my daily driver for a few weeks now. Pre-built binaries for Windows, macOS (universal), and Linux are on the releases page.

If you've ever been frustrated by heavy database tools on modest hardware, give it a try.

**GitHub:** https://github.com/frkdrgt/ferox

Feedback and contributions welcome.
