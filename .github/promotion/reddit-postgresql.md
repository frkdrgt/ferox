# r/PostgreSQL Post

> **Başlık:**
> I got tired of DBeaver slowing down my laptop so I built a minimal Postgres client

---

My daily machine has 8GB RAM. DBeaver at idle was sitting at 400MB, pgAdmin wasn't much better. All I needed was query editor + schema browser + table browsing.

So I built **Ferox** — a small native desktop client for PostgreSQL. Written in Rust, no Electron, no JVM.

**Core features:**
- Query editor with tabs and syntax highlighting
- Schema browser with lazy loading and filter
- Data browser with pagination
- **Join Builder** — visual UI to build multi-table JOINs, generates the SQL for you
- EXPLAIN query plan visualizer
- Inline cell editing
- CSV / JSON export via native file dialog

Runs at ~25MB RAM idle, starts in under 200ms, binary is ~12MB.

Still v0.1.0 and rough around the edges but works well for daily use.

**GitHub:** https://github.com/frkdrgt/ferox

Open to feedback — especially from people who work with Postgres daily and have opinions on what a minimal client should do.
