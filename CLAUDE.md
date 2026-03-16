# pgclient — CLAUDE.md

## Proje Özeti
Hafif bir masaüstü PostgreSQL client uygulaması. Rust + egui/eframe ile yazılmış.
Hedef: DBeaver/DataGrip'e alternatif, <50MB RAM, <200ms startup.

## Tech Stack
- **GUI**: egui 0.27 + eframe (immediate-mode, pure Rust)
- **DB Driver**: tokio-postgres 0.7 (async, pure Rust)
- **Async**: tokio (current-thread runtime, ayrı std::thread içinde)
- **TLS**: native-tls + postgres-native-tls
- **Config**: serde + toml
- **Export**: serde_json + manuel CSV
- **File Dialog**: rfd 0.14 (native OS diyaloğu)

## Mimari — Kritik Kural

**UI thread (eframe) ↔ DB thread arası haberleşme SADECE mpsc kanalları ile:**
- `Sender<DbCommand>` → UI'dan DB'ye komut
- `Sender<DbEvent>` → DB'den UI'a sonuç
- DB thread'de `spawn_blocking` ile sync recv, sonra async tokio-postgres

Bu pattern'i ASLA değiştirme. eframe main thread'i bloke eden hiçbir şey yapma.

## Modül Yapısı
```
src/
├── main.rs          — eframe init, tokio runtime bootstrap
├── app.rs           — PgClientApp, eframe::App impl, event loop
├── config.rs        — ConnectionProfile, AppConfig (TOML)
├── history.rs       — QueryHistory (kalıcı, max 500)
├── db/
│   ├── mod.rs
│   ├── connection.rs — DbCommand/DbEvent kanalları, db_worker async loop
│   ├── query.rs      — CellValue enum, QueryResult, extract_cell()
│   └── metadata.rs   — Schema/table/column introspection sorguları
└── ui/
    ├── mod.rs
    ├── sidebar.rs        — SidebarAction enum, schema tree
    ├── query_panel.rs    — SQL editör, BrowseState, sayfalama
    ├── result_table.rs   — egui_extras::TableBuilder, sort
    └── connection_dialog.rs
```

## Önemli Dosyalar
- Bağlantı profilleri: `~/.config/pgclient/config.toml`
- Sorgu geçmişi: `~/.local/share/pgclient/history.txt`

## Build
```bash
cargo build           # dev
cargo build --release # ~8-12MB binary, LTO
```

## Test
```bash
# Gerçek PG instance gerekir:
docker run -d -p 5432:5432 -e POSTGRES_PASSWORD=test postgres:15
cargo test --test integration
```

## Kodlama Kuralları
- `unwrap()` kullanma — `?` veya `anyhow` ile hata yönet
- UI'da blocking çağrı yapma (mpsc::recv hariç, o spawn_blocking içinde)
- Yeni UI widget'ları `src/ui/` altına koy
- Yeni DB sorguları `src/db/metadata.rs` veya `src/db/query.rs`'e ekle
- `DbCommand`/`DbEvent` enum'larını `src/db/connection.rs`'de tut

## Tamamlanan Fazlar
- **Faz 0**: Proje iskeleti, UI/DB thread ayrımı ✓
- **Faz 1**: Bağlantı dialog, SSL/TLS, profil kaydetme ✓
- **Faz 2**: Schema browser (lazy load, filtre, context menu) ✓
- **Faz 3**: Query editor, sonuç tablosu, virtual scrolling, client+DB sort ✓
- **Faz 4**: Data browser, sayfalama, DB-side ORDER BY, native export diyaloğu ✓

## Kalan Fazlar
- **Faz 5**: Uygulama ikonu, platform release build, eksik küçük özellikler (TODO.md'ye bak)

## Syntax Highlighting
`src/ui/syntax.rs` — `highlight_sql(ui, text, wrap_width) -> LayoutJob`
- `SyntaxSet` ve `ThemeSet` `once_cell::Lazy` ile global olarak cache'lenir
- Dark mode → `base16-ocean.dark` / Light mode → `InspiredGitHub`
- `query_panel.rs`'de `TextEdit::layouter` callback'i olarak kullanılır
- Syntect bulamazsa `plain_job()` fallback'e düşer
