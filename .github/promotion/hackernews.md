# Hacker News Post

> **Başlık:**
> Show HN: Ferox – a native PostgreSQL client in Rust (<30MB RAM, no JVM)

---

My laptop has 8GB RAM. DBeaver at idle sat at ~400MB, pgAdmin wasn't much better. I just wanted to run queries and browse tables without my machine grinding to a halt.

So I built Ferox — a native desktop PostgreSQL client written in Rust.

**GitHub:** https://github.com/frkdrgt/ferox

**What it does:**

- Multi-tab SQL editor with syntax highlighting
- Schema browser with lazy loading (tables, views, mat-views, foreign tables)
- Data browser with server-side pagination and ORDER BY
- Visual Join Builder — select tables and columns, get ready-to-run SQL
- EXPLAIN query plan visualizer
- Inline cell editing with UPDATE support
- CSV / JSON export via native OS file dialog
- Persistent query history (last 500 queries)

**Stack:**

- GUI: `egui` + `eframe` (immediate-mode, pure Rust, no webview)
- DB driver: `tokio-postgres` (async, pure Rust)
- Syntax highlighting: `syntect`
- UI ↔ DB communication: `mpsc` channels — the UI thread never blocks

**Numbers:**

| | Ferox | DBeaver |
|--|--|--|
| RAM at idle | ~25 MB | ~400 MB |
| Startup | <200ms | 5–15s |
| Binary | ~12 MB | ~200 MB |

Still v0.1.0. Works on Windows and macOS, Linux binary also available.
Pre-built binaries on the releases page.

Happy to answer questions about the egui architecture or the tokio-postgres async setup.
