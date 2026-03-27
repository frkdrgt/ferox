# r/rust Post

> **Başlık:**
> Show r/Rust: I built a native PostgreSQL client in Rust because DBeaver was eating my RAM

---

My laptop has 8GB RAM and running DBeaver + a few browser tabs was enough to make everything crawl. pgAdmin wasn't much better. I just wanted to run a query and browse some tables — not launch a spaceship.

So I spent some weekends building **Ferox** — a lightweight desktop PostgreSQL client with egui.

**What it does:**
- Multi-tab query editor with SQL syntax highlighting
- Schema browser (tables, views, mat-views, foreign tables)
- Data browser with pagination and server-side ORDER BY
- Visual Join Builder — pick tables, pick columns, get SQL
- EXPLAIN visualizer
- Inline cell editing
- CSV / JSON export

**The numbers that motivated me:**

| | Ferox | DBeaver |
|--|--|--|
| RAM at idle | ~25 MB | ~400 MB |
| Startup | <200ms | 5–15s |
| Binary size | ~12 MB | ~200 MB |

**Stack:** `egui` + `eframe` for the GUI, `tokio-postgres` for the driver, `syntect` for highlighting. UI and DB run on separate threads communicating through `mpsc` channels — the UI never blocks.

Still early (v0.1.0) but it's been my daily driver for a few weeks now.

**GitHub:** https://github.com/frkdrgt/ferox

Would love feedback, bug reports, or feature ideas. And if anyone's been in the same "my laptop can't run Java GUI apps" situation — this might help.
