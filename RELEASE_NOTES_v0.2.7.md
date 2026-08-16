# 🦊 ferox v0.2.7

> Released: 2026-05-04

---

## ✨ What's New

### ✏️ Inline Cell Editing — All Data Types
Double-click any cell in the result table to edit it directly. No type restrictions — PostgreSQL handles the cast implicitly, so booleans, integers, JSONB, arrays, and more all just work.

- `NULL` cells show a styled `<null>` label instead of an empty box
- Editor starts with the literal `NULL` string — always visible, always editable
- Works in both **browse mode** and **query results**
- Committing outside browse mode shows a warning in the Messages tab

### 📂 DDL in a New Tab
**Show DDL** (views, materialised views) and **Show Definition** (functions, procedures) now open the source directly in a new query editor tab — ready to copy, tweak, or run.

### 🗂️ Collapsible Sidebar Sub-sections
Expanding a table now reveals three independently togglable sub-sections:

| Section | Contents |
|---|---|
| 📋 Columns | Name + type for each column |
| 🔑 Indexes | All indexes on the table |
| 🔗 Foreign Keys | FK relationships |

Each section shows an item count and is collapsed by default.

### 🔮 Smarter Autocomplete

**Preloaded column names** — schema expand fires a single `information_schema.columns` query that preloads every column. Suggestions appear immediately in the SQL editor without needing to expand each table first.

**Table alias hints** — autocomplete now suggests a short alias alongside the table name:

```
tenant_records   →   tr
audit_log        →   al
user_sessions    →   us
```

Accepting a suggestion inserts `table_name alias` — ready to use.

---

## 🐛 Fixes

- **Edit button missing** — double-clicking a cell in query results now correctly shows the edit popup (the `is_browse` guard was removed)
- **NULL edit box empty** — NULL cells no longer open a blank editor; the value starts as `NULL`

---

## ⚡ Performance

- **Completion data** rebuilds only when sidebar data actually changes via a dirty flag on `set_tables` / `set_table_details` / `set_schema_columns` — eliminates per-frame HashSet build and Vec clone across all tabs
- **Expanded sections** lookup optimised: `HashMap<(String,String,String), bool>` → `HashMap<(String,String), [bool;3]>` — from 3 lookups + 9 String clones down to 1 lookup + 0 clones per expanded table per frame

---

## 📦 Build Info

| | |
|---|---|
| **Version** | 0.2.7 |
| **Binary size** | ~6.9 MB (release LTO) |
| **RAM at idle** | ~45–47 MB |
| **Rust edition** | 2024 |
