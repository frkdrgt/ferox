# 🦊 ferox v0.3.0

> Released: 2026-09-08

---

## ✨ What's New

### 🔍 Structured Filter Builder

Browse mode's filter bar was raw-SQL-only. Now there's a column/operator/value picker next to it.

- Pick a column, an operator (`=, !=, >, <, >=, <=, LIKE, ILIKE, IS NULL, IS NOT NULL, IN`), and a value
- **+ Add** appends a properly quoted `WHERE` fragment to the existing filter box — numeric values stay unquoted, everything else is quoted/escaped
- The raw filter box is still there for anything the picker doesn't cover; both feed the same `WHERE` clause

### ☑️ Multi-Row Selection & Bulk Actions

- **Shift+Click** selects a range, **Ctrl+Click** toggles individual rows
- Right-click a selection → **Copy Selected Rows**, **Export Selected as CSV…**, **Export Selected as JSON…**, or **Delete Selected Rows…**
- Bulk delete shows a confirmation dialog first, then runs one `DELETE` built from the table's primary key
- Bulk export writes the rows already in memory straight to disk — no re-query

### 📋 Duplicate Row

Right-click any row → **Duplicate Row** runs an `INSERT` copying every column except the primary key, so serial/identity columns get a fresh value from the database instead of colliding on a unique-constraint violation.

### ⇩ CSV Import

Right-click a table → **Import CSV…**, pick a file. Column names come straight from the CSV's own header row (Postgres skips that line itself via `HEADER true`) — the file is streamed into the table with `COPY ... FROM STDIN` in 64KB chunks, so even a large import never sits fully in memory.

### 🎨 Connection Color Tags

Give any saved connection a color (red/yellow/green/blue) in the connection dialog. It shows up as an accent stripe on the tab bar and in the connection switcher — a quick visual guard against running destructive SQL against the wrong database. Untagged connections still get the old name-based guess (e.g. a group called "Production" turns red).

### 🔐 Credentials Moved to the OS Keychain

Connection passwords, SSH tunnel passwords, and your AI provider API key no longer sit in `config.toml` as plain text.

- Stored via **Windows Credential Manager** / **macOS Keychain** / **Linux Secret Service** (the `keyring` crate)
- If no keychain backend is available, ferox falls back to plain text automatically — nothing breaks, you just don't get the extra protection
- Migration happens the next time you hit **Save** on a connection or your AI settings — nothing changes for existing profiles until then

### 🧾 JSON/JSONB Pretty-Print

The "View Full Value" popup now recognizes JSON-shaped cell values, pretty-prints them, and syntax-highlights keys/strings/numbers/literals — with a **Pretty / Raw** toggle if you want the exact original text.

---

## 🐛 Fixes

| # | Description |
|---|---|
| 1 | Settings → NULL cell color picker closed the whole Settings menu the instant you touched it — replaced the popup color wheel with a fixed row of swatch buttons |
| 2 | Cell value popup: the button row could overlap the Close button / character count on long JSON payloads — buttons now wrap, and Close/count moved to their own line |

---

## ⚡ Performance

- Bulk row export runs on the DB worker thread with data already in memory — no extra query, and the UI thread never touches the filesystem
- CSV import streams the file in 64KB chunks — memory use doesn't scale with file size
- **Binary size grew from ~6.9 MB to ~9.9 MB** (release, LTO). Almost all of it is the `keyring` crate's Windows backend, which pulls in a regex engine — a trade-off accepted deliberately for real credential encryption. RAM at idle (~45 MB) and cold start (<200 ms) are unaffected, since keychain I/O only happens on connect/save, never per-frame.

---

## 🧪 Testing

- 19 new unit tests covering the filter-operator SQL builder, string quoting, the JSON/CSV tokenizers, and connection-color resolution — run on every `cargo test`
- 2 new `#[ignore]`-gated integration tests exercising the riskiest additions — the `COPY FROM STDIN` protocol path and the real OS keychain — against a live local Postgres (`cargo test -- --ignored`)

---

## 📦 Build Info

| | |
|---|---|
| **Version** | 0.3.0 |
| **Binary size** | ~9.9 MB (release LTO) |
| **RAM at idle** | ~45 MB |
| **Rust edition** | 2024 |
