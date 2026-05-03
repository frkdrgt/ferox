# Changelog

## [Unreleased] — 2026-04-28

### Added
- **DDL viewer in new tab** — "Show DDL" on views/materialised views and "Show Definition" on functions/procedures now open the DDL source in a new query editor tab instead of the result table; the raw definition text is placed directly in the editor for easy editing or copying
- **Sidebar tree sub-sections** — clicking a table now expands collapsible sub-sections (Columns, Indexes, Foreign Keys) with item counts; sections are independently toggled and default to collapsed
- **Autocomplete without schema expand** — on schema expand a single `information_schema.columns` query preloads all column names for the schema; columns are immediately available in the SQL editor autocomplete without needing to manually expand each table
- **Table alias suggestions** — autocomplete appends a short alias derived from the table name (e.g. `tenant_records → tr`, `audit_log → al`); alias hint is shown on the right side of the popup; accepting inserts `table_name alias` ready to use

### Changed
- Section header click detection uses `ui.interact` with stable per-table IDs instead of auto-generated egui IDs, fixing a first-click miss caused by ID drift between frames

### Performance
- `completion_data()` and `update_completion_data_for()` now run only when sidebar data actually changes (dirty flag on `set_tables` / `set_table_details` / `set_schema_columns`), eliminating the per-frame HashSet build and Vec clone across all tabs
- `expanded_sections` key changed from `HashMap<(String,String,String), bool>` to `HashMap<(String,String), [bool;3]>` — reduces 3 HashMap lookups and 9 String clones to 1 lookup and 0 clones per expanded table per frame

---

## [0.2.6]— 2026-04-21

### Added
- **Multi-statement tabs** — when multiple `;`-separated SELECT statements are run, each result opens in its own tab instead of only showing the last one
- **Column Statistics panel** — right-click any column header in the result table → 📊 Statistics popup shows total rows, null count/%, distinct value count, min/max text length, and top-10 most frequent values (computed from fetched rows, no extra DB round-trip)
- **Stored procedure / function browser** — FUNCTIONS section in the sidebar schema tree shows all functions, procedures, aggregates, and window functions with icons, arg signatures, and return type; right-click → Show Definition / Copy Call; lazy-loaded (only on schema expand), cached, refreshable with F5
- **Schema Diff** (Query → Schema Diff) — compare two schemas side-by-side across any two open connections; shows `+` added tables, `-` removed tables, `~` changed tables with per-column diffs (added/removed/type-changed); computed client-side from `information_schema.columns` with no extra DB overhead

---

## [0.2.5] — 2026-04-12

### Changed
- **Tab Manager**: Improved the tab management system for more reliable handling of multiple tabs.
- **Connection Flow**: Updated the connection screen flow for a smoother user experience.
- **Infrastructure**: Ignited MacOS testing workflows to ensure cross-platform stability.
- **Project Structure**: Bumped Rust edition to `2024` in `Cargo.toml`.

---

## [0.2.3] — 2026-03-29

### Added
- **EN/TR localisation** — full bilingual UI; language persists to config (`~/.config/pgclient/config.toml`)
- **Settings menu** — Language submenu + About dialog (logo, version, repo link)
- **App title** renamed to *ferox* (was *pgclient*) across title bar and window chrome

### Changed
- Version string in About dialog is now derived from `Cargo.toml` at compile time (no manual sync needed)

---

## [0.2.2] — (unreleased interim)

### Fixed
- Windows taskbar icon rendering
- Join Builder inner scroll area
- Readable DB error messages (stripped internal driver noise)

### Fixed
- Views and materialised views returning no rows in browse mode
- Show DDL context menu item added for tables, views, and materialised views

---

## [0.2.1] — (unreleased interim)

### Performance
- Replaced `syntect` with a zero-dependency custom SQL tokeniser — saves ~30–45 MB RAM
- Single-thread Tokio runtime; removed unused feature flags — ~47 MB total RAM at idle
- Hard row cap (50 000 rows/result) to prevent unbounded memory growth

---

## [0.2.0] — (unreleased interim)

### Added
- **Multi-statement queries** — semicolon-separated SQL runs sequentially; last SELECT result shown
- **Per-table tabs** — clicking a table in the sidebar opens/reuses a dedicated tab
- **Tab context menu** — Close tab / Close other tabs / Close all tabs (right-click)
- **Safe mode transactions** — DML auto-wrapped in `BEGIN`; Commit / Rollback banner
- **Script generation** — SELECT / INSERT / UPDATE / DELETE scripts from context menu
- **Crash logging** — panic hook writes `~/.local/share/pgclient/crash.log`
- **ER diagram** — visual schema diagram with FK arrows, pan/zoom, drag nodes
- **SSH tunnel** — connect through a jump host (password or private key auth)
- **App icon** — custom ferox logo embedded in binary and taskbar
- **SQL autocomplete** — table and column name suggestions in the query editor
- **Database dashboard** — table sizes, active connections (with Kill), index stats
- **Multiple simultaneous connections** — connection switcher in the sidebar
- **Join Builder** — visual multi-table JOIN query builder
- **Export** — CSV and JSON export via native OS file dialog

### Fixed
- Query results correctly routed to the tab that initiated the query

---

## [0.1.0] — initial release

- Basic PostgreSQL connection (SSL/TLS, saved profiles)
- Schema browser with lazy loading and context menu
- Query editor with SQL syntax highlighting
- Result table with virtual scrolling, client/server sort, inline cell edit
- Data browser with server-side pagination and ORDER BY
- EXPLAIN ANALYZE tree view with performance suggestions
