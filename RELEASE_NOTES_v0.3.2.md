# 🦊 ferox v0.3.2

> Released: 2026-10-28

---

## ✨ What's New

### 🎨 A refreshed look

ferox has a new coat of paint — without giving up a single megabyte or millisecond.

- **Three-layer dark theme** — content, panels and hover states each get their own surface, with cards, rounded tabs and proper empty states instead of blank areas
- **One consistent icon set** — Phosphor line icons across the sidebar tree, tabs, toolbar, result tabs, message log and dialogs, replacing the old mix of Unicode symbols and emoji
- **Toolbar** — New connection, New query, Dashboard, Join and Schema diff, plus a Safe mode toggle, live in the menu row, so they cost no vertical space
- **Result grid** — row numbers, a clearer header with sort arrows, column rules and right-aligned numbers

### 🏠 Home screen

With no connection open, ferox now shows a home screen instead of an empty window:

- Counters for saved connections, groups, query history and saved queries
- **Quick Start** — every saved profile with a Connect button (or double-click it)
- Shortcuts card, your recent queries (click to copy) and AI assistant status

### 📊 Dashboard cards

The dashboard opens with summary cards: table count, total size, active/all connections and unused/all indexes. Table sizes get proportional bars, never-scanned indexes are highlighted, and connection states show as colored dots.

### 🦊 New logo

A fox head holding a database — crisp at 16 px in the taskbar, with a transparent background so it no longer shows up as a dark square on light taskbars.

---

## ⚡ Performance

This release makes ferox **lighter at idle**, which matters most on older machines:

| | v0.3.1 | v0.3.2 |
|---|---|---|
| Idle CPU | one core busy | **0** |
| Private bytes | ~140 MB | **~95 MB** |
| System fonts read at startup | 15 MB | **none** |
| Binary | ~10.1 MB | ~10.1 MB |

- Fonts were rebuilt — and re-read from disk — on every frame, and the window title was re-sent every frame, so ferox never stopped repainting. Both now happen only when needed.
- All fonts are embedded; Segoe UI Symbol/Emoji are no longer loaded from `C:\Windows\Fonts`.
- The icon font is a 19 KB subset of the glyphs ferox actually uses (the full font is 480 KB).
- The window icon and in-app logo are pre-rendered at their display sizes, so startup only decodes them.

---

## 🐛 Fixes

- **Browse mode filter** — the WHERE filter field was infinitely wide and pushed the Apply button off-screen (and clipped the "rows/page" label)
- **Edit Table / New Table** — name, default and index fields were only a few characters wide
- **Result grid** — now fills the window down to the bottom edge instead of stopping short on tall windows
- **Row context menu** — entries no longer wrap onto two lines
- **ER diagram** — quick drags on a table header are no longer ignored

---

## 📦 Notes

Emoji inside query results are drawn with the bundled Noto Emoji font. Rare symbols outside the bundled fonts may now show as a box, because ferox no longer falls back to Windows' Segoe fonts.
