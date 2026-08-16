# 🦊 ferox v0.2.9

> Released: 2026-06-22

---

## ✨ What's New

### 💾 Saved Queries (Snippets)

Save any SQL you use often and recall it instantly.

- Press **Ctrl+Shift+S** (or the 💾 **Save** button) to save the current editor contents under a name
- A searchable **snippets panel** lists every saved query
- Right-click a snippet → **📋 Load into editor** or **🗑 Delete**
- Saving over an existing name shows an overwrite warning
- Snippets persist to a TOML file, so they survive restarts

### ⌨️ Autocomplete Without Expanding Schemas

The SQL editor now autocompletes table and column names **immediately after connecting** — no need to click a schema open first.

- On connect, ferox preloads tables and column names for every user schema (one `LoadTables` + one column query per schema, on the DB thread — the UI never blocks)
- Type `SELECT * FROM u…` and `users` completes right away
- A per-connection completion cache seeds **every** query tab, including tabs opened after the schema was already loaded

### 📊 DB-Side Column Statistics

The column **Statistics** panel now computes against the **full table on the database**, not just the rows already fetched into memory.

- Respects the active browse `WHERE` filter
- Shows a loading indicator while the query runs
- A source note clarifies that the numbers come from the database

---

## 🐛 Fixes

| # | Description |
|---|---|
| 1 | Autocomplete data did not reach tabs opened **after** the schema had already loaded — a per-connection completion cache now seeds any new/empty query tab, so autocomplete works in every tab regardless of when it was opened |

---

## ⚡ Performance

- Completion seeding is guarded by a cheap empty-check per tab — table/column lists are cloned into a panel only **once**, when it is first populated, not every frame
- Schema preloading runs entirely on the DB thread via async `information_schema` queries — the UI thread is never blocked

---

## 🔧 Changed

- Crate renamed to **`ferox-pg`** for crates.io publishing; added package metadata (description, license, repository, keywords, categories)

---

## 📦 Build Info

| | |
|---|---|
| **Version** | 0.2.9 |
| **Binary size** | ~6.9 MB (release LTO) |
| **RAM at idle** | ~45–47 MB |
| **Rust edition** | 2024 |
