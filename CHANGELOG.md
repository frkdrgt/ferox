# Changelog

## [Unreleased]

### Added
- **Multi-statement tabs** — when multiple `;`-separated SELECT statements are run, each result opens in its own tab instead of only showing the last one
- **Column Statistics panel** — right-click any column header in the result table → 📊 Statistics popup shows total rows, null count/%, distinct value count, min/max text length, and top-10 most frequent values (computed from fetched rows, no extra DB round-trip)

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
