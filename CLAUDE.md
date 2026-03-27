# pgclient — CLAUDE.md

## Proje Özeti
Hafif bir masaüstü PostgreSQL client uygulaması. Rust + egui/eframe ile yazılmış.
Hedef: DBeaver/DataGrip'e alternatif, <50MB RAM, <200ms startup.

**Gerçek ölçümler (v0.2.x):** ~45–47 MB RAM, 6.9 MB binary (release LTO)

## Tech Stack
- **GUI**: egui 0.27 + eframe (immediate-mode, pure Rust; `accesskit` devre dışı)
- **DB Driver**: tokio-postgres 0.7 (async, pure Rust; `simple_query` text protokol)
- **Async**: tokio (current-thread runtime, ayrı std::thread içinde)
- **TLS**: native-tls + postgres-native-tls
- **SSH**: russh 0.44
- **Config**: serde + toml
- **Export**: serde_json + manuel CSV
- **File Dialog**: rfd 0.14 (native OS diyaloğu)
- **SQL Highlighting**: sıfırdan yazılmış tokenizer — `src/ui/syntax.rs` (syntect yok)

## Mimari — Kritik Kural

**UI thread (eframe) ↔ DB thread arası haberleşme SADECE mpsc kanalları ile:**
- `Sender<DbCommand>` → UI'dan DB'ye komut
- `Sender<DbEvent>` → DB'den UI'a sonuç
- DB thread kendi `current_thread` tokio runtime'ını `std::thread::spawn` içinde oluşturur
- `spawn_blocking` ile sync recv, sonra async tokio-postgres

Bu pattern'i ASLA değiştirme. eframe main thread'i bloke eden hiçbir şey yapma.

## Modül Yapısı
```
src/
├── main.rs           — eframe init, current_thread tokio runtime
├── app.rs            — PgClientApp, eframe::App impl, event loop
├── config.rs         — ConnectionProfile, AppConfig (TOML)
├── history.rs        — QueryHistory (kalıcı, max 500)
├── logger.rs         — crash log (panic hook)
├── db/
│   ├── mod.rs
│   ├── connection.rs — DbCommand/DbEvent, db_worker, execute_query (simple_query)
│   ├── query.rs      — CellValue enum, QueryResult
│   ├── metadata.rs   — Schema/table/column/index/FK introspection
│   └── ssh.rs        — SSH tunnel (russh)
└── ui/
    ├── mod.rs
    ├── sidebar.rs        — SidebarAction, schema tree, context menu
    ├── query_panel.rs    — SQL editör, BrowseState, sayfalama
    ├── result_table.rs   — egui_extras::TableBuilder, sort, inline edit
    ├── tab_manager.rs    — Tab lifecycle, yönlendirme, sağ-tık menüsü
    ├── connection_dialog.rs
    ├── dashboard.rs      — DB dashboard (tablo boyutları, bağlantılar, index)
    ├── explain.rs        — EXPLAIN ANALYZE ağaç görünümü
    ├── er_diagram.rs     — ER diyagramı
    ├── autocomplete.rs   — Tablo/sütun otomatik tamamlama
    └── syntax.rs         — Sıfırdan SQL tokenizer (keyword/type/string/comment)
```

## Önemli Dosyalar
- Bağlantı profilleri: `~/.config/pgclient/config.toml`
- Sorgu geçmişi: `~/.local/share/pgclient/history.txt`
- Crash log: `~/.local/share/pgclient/crash.log`

## Build
```bash
cargo build           # dev
cargo build --release # ~6.9MB binary, LTO
```

## Kodlama Kuralları
- `unwrap()` kullanma — `?` veya `anyhow` ile hata yönet
- UI'da blocking çağrı yapma (mpsc::recv hariç, o spawn_blocking içinde)
- Yeni UI widget'ları `src/ui/` altına koy
- Yeni DB sorguları `src/db/metadata.rs` veya `src/db/query.rs`'e ekle
- `DbCommand`/`DbEvent` enum'larını `src/db/connection.rs`'de tut
- Tüm sorgular `execute_query()` üzerinden geçmeli → `simple_query` protokol

## execute_query Kritik Notlar
- Tüm SQL'ler (SELECT, DML, DDL) `simple_query` ile çalışır — noktalı virgül ve çoklu statement desteklenir
- `;` ile ayrılmış birden fazla statement sırayla çalışır; son SELECT sonucu gösterilir
- Boş tablo/view (0 satır): `prepare()` ile sütun isimleri kurtarılır
- SELECT-like sorgular hiçbir zaman `rows_affected` set etmez — `set_result()` DML detection'ı bu sayede doğru çalışır
- Hard cap: 50.000 satır/sonuç (unbounded memory büyümesini önler)

## Tab Yönetimi (tab_manager.rs)
- Tabloya tıklanınca: mevcut tab bulunursa switch, aktif tab boşsa reuse, doluysa yeni tab
- Sağ-tık context menu: Close tab / Close other tabs / Close all tabs
- `running_tabs: HashMap<conn_id, tab_idx>` — sonuçları doğru taba yönlendirir

## Tamamlanan Fazlar
- **Faz 0**: Proje iskeleti, UI/DB thread ayrımı ✓
- **Faz 1**: Bağlantı dialog, SSL/TLS, profil kaydetme ✓
- **Faz 2**: Schema browser (lazy load, filtre, context menu) ✓
- **Faz 3**: Query editor, sonuç tablosu, virtual scrolling, client+DB sort ✓
- **Faz 4**: Data browser, sayfalama, DB-side ORDER BY, native export diyaloğu ✓
- **Faz 5**: Uygulama ikonu, release CI/CD, UI modernizasyon ✓
- **Faz 6**: Multi-statement queries, per-table tabs, view/matview browse fix, RAM optimizasyonları ✓

## Kalan İşler
- Test Connection butonu (bağlantı dialog'unda)
- Ctrl+A ile tümünü seç (sorgu editörü)
- NULL renk tercihi config'e kaydet
- Büyük sonuç setlerinde column width hesabı lazy yap

## UI Tema Notları
`src/app.rs:configure_style()` — JetBrains Darcula paleti:
- Panel: `#2b2b2b`, Window: `#3c3f41`, Faint: `#313335`
- Accent/seçim: `#4e9fde` (mavi), Hover: `#5c6164`
- Run butonu: `#499c54` (yeşil), Cancel: `#c75450` (kırmızı)
- Yeni renk/spacing eklerken bu fonksiyonu güncelle, hardcode etme

## Syntax Highlighting
`src/ui/syntax.rs` — sıfırdan yazılmış SQL tokenizer
- Syntect/once_cell yok — sıfır bağımlılık, ~0 MB overhead
- Byte-level scanner: keyword, type, function, string (single-quote + dollar-quote), number, comment (--, /* */), operator
- `highlight_sql(ui, text, wrap_width) -> LayoutJob`
- Dark: `base16-ocean.dark` renk paleti, Light: `InspiredGitHub` renk paleti
- `query_panel.rs`'de `TextEdit::layouter` callback'i olarak kullanılır
