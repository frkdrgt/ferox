# 🦊 ferox v0.2.8

> Released: 2026-05-23

---

## ✨ What's New

### 🔔 Trigger Browser

Sidebar now shows a **TRIGGERS** section under each expanded table. Displays trigger name, timing (BEFORE / AFTER / INSTEAD OF), events (INSERT / UPDATE / DELETE), the function it calls, and enabled state. Right-click a trigger for context actions.

### 🔢 Sequence Browser

**SEQUENCES** section added to the sidebar per schema. Shows sequence name and current value. Right-click actions:

- **Set Value…** — sets the sequence's next value
- **Show DDL** — opens the sequence DDL in a new query tab

### 🔎 Browse Mode WHERE Filter

A filter bar is now shown in browse mode (table data browser). Type any SQL predicate directly — it is appended as a `WHERE` clause to the browse query.

```
id > 100
name LIKE '%alice%'
created_at > '2025-01-01' AND status = 'active'
```

- Press **Enter** or **Filter** to apply; **✕** clears
- A 🔍 badge appears in the browse banner when a filter is active
- Filter persists across page navigation

### 🖱️ Cell Right-Click Menu

Right-clicking any result table cell now shows a context menu:

- **View Full Value** — opens a resizable popup with the full cell content, selectable text, copy and edit buttons
- **Copy Value** — copies the raw cell value to clipboard

### 🔍 Find in Results (Ctrl+F)

Press **Ctrl+F** (or click the 🔍 button in the results toolbar) to open an inline find bar. Unlike the row filter, find keeps all rows visible and **highlights matching cells** with a yellow background.

- Matching cells highlighted in yellow across all visible rows
- Live match counter — green when matches found, red when none
- **Escape** closes the bar; **✕** clears the search term
- Independent of the row filter — both can be active simultaneously
- Match count recomputed lazily (only when search term or filter changes)

### 📋 Copy Table as Markdown / HTML

Right-click any cell → **Copy table as Markdown** or **Copy table as HTML**.

Copies the current view (respecting active filter and sort order) to the clipboard.

**Markdown output:**

```
| id | name | email |
|---|---|---|
| 1 | Alice | alice@example.com |
| 2 | Bob | bob@example.com |
```

**HTML output:** Full `<table>` with `<thead>` / `<tbody>`, HTML entity–escaped.

### 📐 Column Width Persistence

Column widths resized by dragging the header separator now **persist** correctly:

- **Same query re-run** — user-resized widths are kept across re-runs
- **New query with different columns** — widths reset to content-aware defaults
- **Same columns, different data** — widths stay as resized (e.g., browsing paginated data)

Implemented via a column-name hash as the `TableBuilder` stable Id.

---

## 🐛 Fixes

| # | Description |
|---|---|
| 1 | Right-click context menu now responds when pointer is **over cell text**, not just empty cell space — fixed by reordering egui widget allocation (render content first, allocate full-rect interaction widget last so it wins hover priority) |
| 2 | Cell popup text is now selectable (removed `interactive(false)` guard) |
| 3 | `pg_get_functiondef` now uses OID instead of `regprocedure` cast — eliminates "type does not exist" errors for functions with `text` or complex argument types |

---

## ⚡ Performance

- **Find bar match count** computed lazily — only recalculated when `search_text` or `result_filter` changes, not every frame
- **Column widths** no longer reset on same-schema query re-runs — egui memory handles persistence without recomputation

---

## 📦 Build Info

| | |
|---|---|
| **Version** | 0.2.8 |
| **Binary size** | ~6.9 MB (release LTO) |
| **RAM at idle** | ~45–47 MB |
| **Rust edition** | 2024 |
