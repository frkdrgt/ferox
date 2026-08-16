# Ferox — Yapılacaklar

## Aktif

Şu an açık madde yok.

---

## Tamamlanan

### Faz 13 — Küçük UX Tamamlamaları ✓
- [x] Sorgu editöründe Ctrl+A ile tümünü seç
- [x] NULL değerlere özel renk tercihi Settings menüsünden config'e kaydediliyor (`AppConfig.null_color`)
- [x] Büyük sonuç setlerinde column width hesabı zaten lazy — `compute_col_widths()` sadece ilk 200 satırı örnekliyor, satır sayısından bağımsız sabit maliyetli; ayrıca sonuç geldiğinde bir kez çalışıyor (frame başına değil)

### v0.2.9 — Saved Queries, Autocomplete Preload, DB-side Stats ✓
- [x] Saved queries (snippets) — Ctrl+Shift+S, TOML-persisted, aranabilir panel
- [x] Bağlantıda tablo/sütun ön-yükleme — schema açmadan autocomplete çalışır
- [x] DB-side column statistics — tam tablo üzerinden hesaplanır (browse filtresine duyarlı)
- [x] Crate adı `ferox-pg`'ye çevrildi (crates.io yayını için)

### v0.2.8 — Trigger/Sequence Browser, Browse Filter ✓
- [x] Trigger browser (TRIGGERS bölümü, sağ-tık aksiyonlar)
- [x] Sequence browser (SEQUENCES bölümü, Set Value / Show DDL)
- [x] Browse-mode WHERE filtresi (persist, 🔍 badge)
- [x] Cell sağ-tık menüsü — View Full Value / Copy Value
- [x] Ctrl+F ile sonuçlarda arama (Find in Results)
- [x] Sonucu Markdown/HTML olarak kopyalama
- [x] Sütun genişliği kalıcılığı (aynı sorgu tekrar çalıştırıldığında korunur)

### v0.2.7 — Inline Cell Editing, DDL Viewer ✓
- [x] Inline cell editing — tüm tipler için, çift tık ile (query-mode dahil)
- [x] DDL viewer yeni tab'da açılıyor (Show DDL / Show Definition)
- [x] Sidebar collapsible alt-bölümler (Columns/Indexes/FK)
- [x] Schema-expand olmadan autocomplete (schema açılınca tek sorgu ile tüm sütunlar)
- [x] Tablo alias önerisi autocomplete'te
- [x] Bağlantı dialog'unda "Test Connection" butonu

### Faz 8 — Multi-statement Tabs + Sütun İstatistikleri ✓
- [x] Multi-statement sorgu çalıştırınca her SELECT sonucu ayrı tab'da açılıyor
- [x] Sütun başlığına sağ-tık → 📊 Statistics popup (null %, distinct, min/max uzunluk, top-10 değer)
- [x] `split_sql_statements()` — single-quote ve `--` comment'i doğru atlayan SQL bölücü
- [x] `DbCommand::ExecuteMulti` + `DbEvent::MultiQueryResults` (yeni kanal mesajları)
- [x] `TabManager::set_multi_results_for` — ilk sonuç mevcut tab, kalanlar yeni tab
- [x] `ColumnStats::compute()` — in-memory O(n) hesaplama, sıfır DB yükü
- [x] `ResultTable` column header context menu + `TableOutput::col_stats_requested`

### Faz 7 — i18n, Settings Menüsü, v0.2.3 ✓
- [x] Tam EN/TR lokalizasyon — tüm UI string'leri çift dil; seçim config'e kaydediliyor
- [x] `src/i18n.rs` — sıfır bağımlılık, compile-time safe, `I18n(Lang)` newtype
- [x] Tüm UI modülleri güncellendi: app, sidebar, query_panel, result_table, explain, tab_manager, dashboard, er_diagram, join_builder, table_dialog, connection_dialog
- [x] Settings menüsü — Language alt menüsü + About dialog
- [x] About dialog — logo, sürüm (`env!("CARGO_PKG_VERSION")`), açıklama, repo linki
- [x] Uygulama başlığı `pgclient` → `ferox` (title bar, taskbar, window chrome)
- [x] `Cargo.toml` versiyonu 0.1.0 → 0.2.3
- [x] `CHANGELOG.md` oluşturuldu
- [x] `v0.2.3` annotated tag oluşturuldu

### Faz 6 — Multi-statement, Tab UX, RAM Optimizasyonları ✓
- [x] Sorgu sonuna `;` koyunca hata veriyordu — `simple_query` protokolüne geçildi
- [x] Birden fazla `;` ayrılmış statement sırayla çalışır, son SELECT gösterilir
- [x] Tablo ismine tıklayınca her tablo ayrı tab'da açılıyor
- [x] Tab sağ-tık menüsü: Close tab / Close other tabs / Close all tabs
- [x] Boş tablo/view sütun başlıklarını göstermiyor — `prepare()` fallback eklendi
- [x] View/materialized view browse sonuç dönmüyordu — DML detection fix
- [x] View/MatView context menüsüne "Show DDL" eklendi
- [x] syntect kaldırıldı → sıfırdan SQL tokenizer (~30-45 MB RAM tasarrufu)
- [x] `new_multi_thread(2)` → `new_current_thread()` (2 idle worker thread kaldırıldı)
- [x] `accesskit`, `rt-multi-thread`, gereksiz tokio-postgres features kaldırıldı
- [x] Sorgu sonucu 50.000 satır hard cap

### Faz 5 — Polish + Release ✓
- [x] SQL syntax highlighting (dark/light tema)
- [x] Uygulama ikonu (PNG + Windows .ico embed)
- [x] CI/CD: tag push'unda otomatik GitHub Release
- [x] JetBrains Darcula renk paleti
- [x] Tab bar modernizasyonu (aktif tab mavi çizgi, hover rengi)
- [x] Profil silme (Connection menüsünden `×`)
- [x] Bağlantı dialog Cancel butonu fix
- [x] Schema tree F5 ile yenileme
- [x] Crash log (`~/.local/share/pgclient/crash.log`)
- [x] Script generation (SELECT/INSERT/UPDATE/DELETE)
- [x] Safe mode transactions (explicit BEGIN/COMMIT/ROLLBACK)

### Faz 4 ✓
- [x] Data browser (tablo çift tık → sayfalama)
- [x] DB-side ORDER BY (sütun başlığına tık)
- [x] Inline cell editing (çift tık → UPDATE)
- [x] CSV & JSON export (native OS file dialog)

### Faz 3 ✓
- [x] Query editor (SQL editör + sonuç tablosu)
- [x] Virtual scrolling
- [x] Client-side sort
- [x] Sorgu geçmişi (max 500, kalıcı)

### Faz 2 ✓
- [x] Schema browser (lazy load, filtre)
- [x] Context menu (Browse, Scripts, Count, Show columns/indexes/FK)
- [x] ER diyagramı görünümü

### Faz 1 ✓
- [x] Bağlantı dialog (host/port/user/pass/db/ssl)
- [x] SSL/TLS desteği
- [x] Profil kaydetme (TOML)
- [x] Çoklu eş zamanlı bağlantı

### Faz 0 ✓
- [x] Proje iskeleti
- [x] UI/DB thread ayrımı (mpsc kanalları)
- [x] Tokio async runtime (DB thread'inde)

---

## Gelecek Fikirler (Scope dışı şimdilik)
- [ ] Bookmarked queries (kayıtlı sorgular)
- [ ] Dark/light tema runtime switch
- [ ] Result diff (iki sonucu yan yana karşılaştır)
- [ ] CSV/JSON import (drag-and-drop)
- [ ] Stored procedure / function browser
- [ ] Query formatter için klavye kısayolu
- [ ] Code signing sertifikası (antivirüs false positive çözümü)
