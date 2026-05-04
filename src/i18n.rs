use serde::{Deserialize, Serialize};

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    Tr,
}

/// Lightweight i18n helper — zero allocations for static strings.
#[derive(Debug, Clone, Copy)]
pub struct I18n(pub Lang);

impl I18n {
    pub fn new(lang: Lang) -> Self {
        Self(lang)
    }

    #[inline]
    fn t(&self, en: &'static str, tr: &'static str) -> &'static str {
        match self.0 {
            Lang::En => en,
            Lang::Tr => tr,
        }
    }

    // ── Menu — Connection ────────────────────────────────────────────────────

    pub fn menu_connection(&self) -> &'static str { self.t("Connection", "Bağlantı") }
    pub fn menu_new_connection(&self) -> &'static str { self.t("New Connection…", "Yeni Bağlantı…") }
    pub fn menu_disconnect(&self) -> &'static str { self.t("Disconnect", "Bağlantıyı Kes") }

    // ── Menu — Query ─────────────────────────────────────────────────────────

    pub fn menu_query(&self) -> &'static str { self.t("Query", "Sorgu") }
    pub fn menu_safe_mode(&self) -> &'static str { self.t("🛡 Safe Mode", "🛡 Güvenli Mod") }
    pub fn menu_safe_mode_on(&self) -> &'static str { self.t("🛡 Safe Mode  ✓", "🛡 Güvenli Mod  ✓") }
    pub fn menu_join_builder(&self) -> &'static str { self.t("Join Builder…", "Join Oluşturucu…") }
    pub fn menu_dashboard(&self) -> &'static str { self.t("📊 Dashboard", "📊 Gösterge Paneli") }
    pub fn menu_schema_diff(&self) -> &'static str { self.t("⊕ Schema Diff", "⊕ Şema Karşılaştırma") }
    pub fn menu_execute(&self) -> &'static str { self.t("Execute (F5 / Ctrl+Enter)", "Çalıştır (F5 / Ctrl+Enter)") }
    pub fn menu_cancel(&self) -> &'static str { self.t("Cancel (Ctrl+C)", "İptal (Ctrl+C)") }
    pub fn menu_export_csv(&self) -> &'static str { self.t("Export as CSV…", "CSV olarak Dışa Aktar…") }
    pub fn menu_export_json(&self) -> &'static str { self.t("Export as JSON…", "JSON olarak Dışa Aktar…") }
    pub fn menu_language(&self) -> &'static str { self.t("Language", "Dil") }

    // ── Status bar ────────────────────────────────────────────────────────────

    pub fn status_disconnected(&self) -> &'static str { self.t("⬤  Disconnected", "⬤  Bağlı Değil") }
    pub fn status_connecting(&self) -> &'static str { self.t("⬤  Connecting…", "⬤  Bağlanıyor…") }
    pub fn status_error(&self, msg: &str) -> String {
        match self.0 {
            Lang::En => format!("⬤  Error: {msg}"),
            Lang::Tr => format!("⬤  Hata: {msg}"),
        }
    }
    pub fn status_rows(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("{n} rows"),
            Lang::Tr => format!("{n} satır"),
        }
    }

    // ── Safe mode banners ─────────────────────────────────────────────────────

    pub fn safe_mode_tx_banner(&self) -> &'static str {
        self.t(
            "⚠  Safe Mode — transaction open. Changes are not yet saved.",
            "⚠  Güvenli Mod — işlem açık. Değişiklikler henüz kaydedilmedi.",
        )
    }
    pub fn safe_mode_indicator(&self) -> &'static str {
        self.t(
            "🛡 Safe Mode ON — DML statements will be wrapped in BEGIN automatically.",
            "🛡 Güvenli Mod AÇIK — DML ifadeleri otomatik olarak BEGIN ile sarılacak.",
        )
    }
    pub fn btn_commit(&self) -> &'static str { self.t("✓  COMMIT", "✓  ONAYLA") }
    pub fn btn_rollback(&self) -> &'static str { self.t("✕  ROLLBACK", "✕  GERİ AL") }

    // ── App general ───────────────────────────────────────────────────────────

    pub fn btn_connect_dialog(&self) -> &'static str { self.t("Connect…", "Bağlan…") }
    pub fn lbl_no_connection(&self) -> &'static str { self.t("No connection", "Bağlantı yok") }
    pub fn hover_new_connection(&self) -> &'static str { self.t("New connection", "Yeni bağlantı") }
    pub fn window_title_connecting(&self) -> &'static str { self.t("pgclient — connecting…", "pgclient — bağlanıyor…") }
    pub fn err_not_connected(&self) -> &'static str { self.t("Not connected", "Bağlı değil") }
    pub fn window_connect_to_pg(&self) -> &'static str { self.t("Connect to PostgreSQL", "PostgreSQL'e Bağlan") }

    // ── Connection dialog ─────────────────────────────────────────────────────

    pub fn label_name(&self) -> &'static str { self.t("Name:", "Ad:") }
    pub fn label_group(&self) -> &'static str { self.t("Group:", "Grup:") }
    pub fn label_host(&self) -> &'static str { self.t("Host:", "Sunucu:") }
    pub fn label_port(&self) -> &'static str { self.t("Port:", "Port:") }
    pub fn label_database(&self) -> &'static str { self.t("Database:", "Veritabanı:") }
    pub fn label_user(&self) -> &'static str { self.t("User:", "Kullanıcı:") }
    pub fn label_password(&self) -> &'static str { self.t("Password:", "Parola:") }
    pub fn label_ssl_mode(&self) -> &'static str { self.t("SSL mode:", "SSL modu:") }
    pub fn hint_group(&self) -> &'static str {
        self.t("e.g. Production, Staging, Dev (optional)", "örn. Üretim, Test, Geliştirme (opsiyonel)")
    }
    pub fn hint_password(&self) -> &'static str {
        self.t("(leave blank for no password)", "(parola yoksa boş bırakın)")
    }
    pub fn ssh_tunnel(&self) -> &'static str { self.t("SSH Tunnel", "SSH Tüneli") }
    pub fn label_enabled(&self) -> &'static str { self.t("Enabled:", "Etkin:") }
    pub fn label_ssh_host(&self) -> &'static str { self.t("SSH Host:", "SSH Sunucusu:") }
    pub fn label_ssh_port(&self) -> &'static str { self.t("SSH Port:", "SSH Portu:") }
    pub fn label_ssh_user(&self) -> &'static str { self.t("SSH User:", "SSH Kullanıcısı:") }
    pub fn label_auth(&self) -> &'static str { self.t("Auth:", "Kimlik Doğrulama:") }
    pub fn label_password_radio(&self) -> &'static str { self.t("Password", "Parola") }
    pub fn label_private_key(&self) -> &'static str { self.t("Private Key", "Özel Anahtar") }
    pub fn label_ssh_password(&self) -> &'static str { self.t("SSH Password:", "SSH Parolası:") }
    pub fn label_key_path(&self) -> &'static str { self.t("Key Path:", "Anahtar Yolu:") }
    pub fn btn_browse(&self) -> &'static str { self.t("Browse…", "Gözat…") }
    pub fn save_profile(&self) -> &'static str {
        self.t("Save connection profile", "Bağlantı profilini kaydet")
    }
    pub fn btn_connect(&self) -> &'static str { self.t("Connect", "Bağlan") }
    pub fn btn_test_connection(&self) -> &'static str { self.t("Test Connection", "Bağlantıyı Test Et") }
    pub fn btn_testing(&self) -> &'static str { self.t("Testing…", "Test ediliyor…") }
    pub fn test_conn_ok(&self) -> &'static str { self.t("Connection successful!", "Bağlantı başarılı!") }
    pub fn test_conn_fail(&self) -> &'static str { self.t("Connection failed: ", "Bağlantı başarısız: ") }
    pub fn btn_cancel(&self) -> &'static str { self.t("Cancel", "İptal") }
    pub fn hover_close_conn(&self) -> &'static str { self.t("Disconnect and close", "Bağlantıyı kes ve kapat") }
    pub fn err_host_required(&self) -> &'static str {
        self.t("Host is required", "Sunucu adresi gerekli")
    }

    // ── Query panel — toolbar ─────────────────────────────────────────────────

    pub fn btn_run(&self) -> &'static str { self.t("▶ Run", "▶ Çalıştır") }
    pub fn btn_running(&self) -> &'static str { self.t("⏳ Running…", "⏳ Çalışıyor…") }
    pub fn btn_cancel_query(&self) -> &'static str { self.t("■ Cancel", "■ İptal") }
    pub fn btn_explain(&self) -> &'static str { self.t("⚡ Explain", "⚡ Açıkla") }
    pub fn hover_explain(&self) -> &'static str {
        self.t("Run EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)", "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) çalıştır")
    }
    pub fn btn_hist_prev(&self) -> &'static str { self.t("⬆ Hist", "⬆ Geçmiş") }
    pub fn hover_hist_prev(&self) -> &'static str { self.t("Previous query (↑)", "Önceki sorgu (↑)") }
    pub fn hover_hist_next(&self) -> &'static str { self.t("Next query (↓)", "Sonraki sorgu (↓)") }
    pub fn btn_format(&self) -> &'static str { self.t("⇄ Format", "⇄ Biçimle") }
    pub fn hover_format(&self) -> &'static str {
        self.t("Format SQL (Shift+Alt+F)", "SQL'i biçimle (Shift+Alt+F)")
    }
    pub fn hover_export_csv(&self) -> &'static str { self.t("Export as CSV…", "CSV olarak dışa aktar…") }
    pub fn hover_export_json(&self) -> &'static str { self.t("Export as JSON…", "JSON olarak dışa aktar…") }
    pub fn btn_run_file(&self) -> &'static str { self.t("📂 Run File", "📂 Dosya Çalıştır") }
    pub fn hover_run_file(&self) -> &'static str {
        self.t(
            "Execute a SQL file directly without loading it into the editor",
            "Bir SQL dosyasını editöre yüklemeden doğrudan çalıştır",
        )
    }
    pub fn hint_sql_editor(&self) -> &'static str {
        self.t(
            "Enter SQL… (Ctrl+Enter to run, Enter to accept autocomplete)",
            "SQL girin… (Ctrl+Enter çalıştırır, Enter otomatik tamamlamayı kabul eder)",
        )
    }

    // ── Query panel — tabs ────────────────────────────────────────────────────

    pub fn tab_results(&self) -> &'static str { self.t("Results", "Sonuçlar") }
    pub fn tab_history(&self) -> &'static str { self.t("History", "Geçmiş") }
    pub fn tab_plan(&self) -> &'static str { self.t("⚡ Plan", "⚡ Plan") }
    pub fn tab_messages(&self) -> &'static str { self.t("Messages", "Mesajlar") }
    pub fn tab_messages_n(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("Messages ({n})"),
            Lang::Tr => format!("Mesajlar ({n})"),
        }
    }

    // ── Query panel — browse ──────────────────────────────────────────────────

    pub fn browse_prefix(&self) -> &'static str { self.t("Browse:", "Göz At:") }
    pub fn browse_sorted_by(&self) -> &'static str { self.t("sorted by", "sıralama:") }
    pub fn btn_exit_browse(&self) -> &'static str { self.t("✕ Exit browse", "✕ Göz atmayı kapat") }
    pub fn filter_hint(&self) -> &'static str { self.t("🔍 Filter results…", "🔍 Sonuçları filtrele…") }
    pub fn label_search(&self) -> &'static str { self.t("Search:", "Ara:") }
    pub fn lbl_running(&self) -> &'static str { self.t("Running…", "Çalışıyor…") }
    pub fn lbl_running_explain(&self) -> &'static str { self.t("Running EXPLAIN…", "EXPLAIN çalıştırılıyor…") }
    pub fn lbl_no_results_yet(&self) -> &'static str {
        self.t(
            "No results yet. Run a query with F5 or Ctrl+Enter.",
            "Henüz sonuç yok. F5 veya Ctrl+Enter ile sorgu çalıştırın.",
        )
    }
    pub fn lbl_no_messages(&self) -> &'static str {
        self.t("No messages yet.", "Henüz mesaj yok.")
    }
    pub fn btn_clear(&self) -> &'static str { self.t("Clear", "Temizle") }
    pub fn lbl_events(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("{n} events"),
            Lang::Tr => format!("{n} olay"),
        }
    }
    pub fn btn_prev_page(&self) -> &'static str { self.t("← Prev", "← Önceki") }
    pub fn btn_next_page(&self) -> &'static str { self.t("Next →", "Sonraki →") }
    pub fn lbl_page(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!(" Page {} ", n),
            Lang::Tr => format!(" Sayfa {} ", n),
        }
    }
    pub fn lbl_rows_per_page(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("{n} rows/page"),
            Lang::Tr => format!("{n} satır/sayfa"),
        }
    }

    // ── Cell popup ────────────────────────────────────────────────────────────

    pub fn btn_copy(&self) -> &'static str { self.t("Copy", "Kopyala") }
    pub fn btn_edit(&self) -> &'static str { self.t("Edit", "Düzenle") }
    pub fn btn_copy_as_insert(&self) -> &'static str { self.t("Copy as INSERT", "INSERT olarak kopyala") }
    pub fn hover_copy_insert(&self) -> &'static str {
        self.t(
            "Copy the entire row as an INSERT statement",
            "Tüm satırı INSERT ifadesi olarak kopyala",
        )
    }
    pub fn btn_close(&self) -> &'static str { self.t("Close", "Kapat") }
    pub fn lbl_chars(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("{n} chars"),
            Lang::Tr => format!("{n} karakter"),
        }
    }

    // ── Log messages ──────────────────────────────────────────────────────────

    pub fn log_ok_rows(&self, n: i64, ms: f64) -> String {
        match self.0 {
            Lang::En => {
                let s = if n == 1 { "" } else { "s" };
                format!("OK — {n} row{s} affected  ({ms:.1} ms)")
            }
            Lang::Tr => format!("TAMAM — {n} satır etkilendi  ({ms:.1} ms)"),
        }
    }
    pub fn log_exported(&self, path: &str) -> String {
        match self.0 {
            Lang::En => format!("Exported → {path}"),
            Lang::Tr => format!("Dışa aktarıldı → {path}"),
        }
    }
    pub fn log_file_empty(&self, path: &str) -> String {
        match self.0 {
            Lang::En => format!("File is empty: {path}"),
            Lang::Tr => format!("Dosya boş: {path}"),
        }
    }
    pub fn log_running_file(&self, filename: &str) -> String {
        match self.0 {
            Lang::En => format!("Running file: {filename}"),
            Lang::Tr => format!("Dosya çalıştırılıyor: {filename}"),
        }
    }
    pub fn log_file_error(&self, e: &impl std::fmt::Display) -> String {
        match self.0 {
            Lang::En => format!("Could not read file: {e}"),
            Lang::Tr => format!("Dosya okunamadı: {e}"),
        }
    }
    pub fn warn_edit_requires_browse(&self) -> &'static str {
        self.t(
            "Cannot edit: open a table from the sidebar first (browse mode required)",
            "Düzenlenemez: önce kenar çubuğundan bir tablo açın (browse modu gerekli)",
        )
    }
    pub fn warn_no_pk(&self, schema: &str, table: &str) -> String {
        match self.0 {
            Lang::En => format!(
                "Cannot edit: no primary key found on \"{schema}\".\"{table}\""
            ),
            Lang::Tr => format!(
                "Düzenlenemez: \"{schema}\".\"{table}\" tablosunda birincil anahtar bulunamadı"
            ),
        }
    }

    // ── Result table ──────────────────────────────────────────────────────────

    pub fn query_ok_rows(&self, n: u64) -> String {
        match self.0 {
            Lang::En => format!("Query OK — {n} rows affected"),
            Lang::Tr => format!("Sorgu TAMAM — {n} satır etkilendi"),
        }
    }
    pub fn lbl_no_results(&self) -> &'static str { self.t("No results", "Sonuç yok") }

    // ── Sidebar ───────────────────────────────────────────────────────────────

    pub fn schema_browser(&self) -> &'static str { self.t("SCHEMA BROWSER", "ŞEMA TARAYICISI") }
    pub fn sidebar_filter_hint(&self) -> &'static str { self.t("🔍  Filter…", "🔍  Filtrele…") }
    pub fn kind_tables(&self) -> &'static str { self.t("TABLES", "TABLOLAR") }
    pub fn kind_views(&self) -> &'static str { self.t("VIEWS", "GÖRÜNÜMLER") }
    pub fn kind_mat_views(&self) -> &'static str { self.t("MAT VIEWS", "MAT GÖRÜNÜMLER") }
    pub fn kind_foreign_tables(&self) -> &'static str { self.t("FOREIGN TABLES", "YABANCI TABLOLAR") }
    pub fn kind_functions(&self) -> &'static str { self.t("FUNCTIONS", "FONKSİYONLAR") }
    pub fn fn_show_definition(&self) -> &'static str { self.t("Show Definition", "Tanımı Göster") }
    pub fn fn_copy_call(&self) -> &'static str { self.t("Copy Call", "Çağrıyı Kopyala") }
    pub fn schema_menu_new_table(&self) -> &'static str { self.t("＋  New Table…", "＋  Yeni Tablo…") }
    pub fn schema_menu_er(&self) -> &'static str { self.t("📐  View ER Diagram", "📐  ER Diyagramını Gör") }
    pub fn schema_menu_refresh(&self) -> &'static str { self.t("↺  Refresh", "↺  Yenile") }
    pub fn table_menu_generate_script(&self) -> &'static str { self.t("📄  Generate Script", "📄  Script Oluştur") }
    pub fn table_menu_browse(&self) -> &'static str { self.t("▶  Browse rows", "▶  Satırlara Gözat") }
    pub fn table_menu_edit(&self) -> &'static str { self.t("✎  Edit Table…", "✎  Tabloyu Düzenle…") }
    pub fn table_menu_show_ddl(&self) -> &'static str { self.t("{}  Show DDL", "{}  DDL Göster") }
    pub fn table_menu_count(&self) -> &'static str { self.t("∑  Count rows", "∑  Satır Say") }
    pub fn table_menu_show_cols(&self) -> &'static str { self.t("≡  Show columns", "≡  Sütunları Göster") }
    pub fn table_menu_show_indexes(&self) -> &'static str { self.t("⊟  Show indexes", "⊟  İndeksleri Göster") }
    pub fn table_menu_show_fks(&self) -> &'static str { self.t("⊠  Show foreign keys", "⊠  Yabancı Anahtarları Göster") }
    pub fn lbl_not_connected_sidebar(&self) -> &'static str { self.t("Not connected", "Bağlı değil") }

    // ── Tab manager ───────────────────────────────────────────────────────────

    pub fn tab_close(&self) -> &'static str { self.t("Close tab", "Sekmeyi kapat") }
    pub fn tab_close_others(&self) -> &'static str { self.t("Close other tabs", "Diğer sekmeleri kapat") }
    pub fn tab_close_all(&self) -> &'static str { self.t("Close all tabs", "Tüm sekmeleri kapat") }
    pub fn lbl_no_connection_available(&self) -> &'static str {
        self.t("Connection not available", "Bağlantı mevcut değil")
    }

    // ── Dashboard ─────────────────────────────────────────────────────────────

    pub fn btn_refresh(&self) -> &'static str { self.t("↺ Refresh", "↺ Yenile") }
    pub fn dash_tab_table_sizes(&self) -> &'static str { self.t("Table Sizes", "Tablo Boyutları") }
    pub fn dash_tab_connections(&self) -> &'static str { self.t("Connections", "Bağlantılar") }
    pub fn dash_tab_index_stats(&self) -> &'static str { self.t("Index Stats", "İndeks İstatistikleri") }
    pub fn lbl_loading(&self) -> &'static str { self.t("Loading…", "Yükleniyor…") }
    pub fn lbl_loading_dashboard(&self) -> &'static str {
        self.t("Loading dashboard data…", "Gösterge paneli verileri yükleniyor…")
    }
    pub fn col_schema(&self) -> &'static str { self.t("Schema", "Şema") }
    pub fn col_table(&self) -> &'static str { self.t("Table", "Tablo") }
    pub fn col_total(&self) -> &'static str { self.t("Total", "Toplam") }
    pub fn col_indexes(&self) -> &'static str { self.t("Indexes", "İndeksler") }
    pub fn col_pid(&self) -> &'static str { "PID" }
    pub fn col_user(&self) -> &'static str { self.t("User", "Kullanıcı") }
    pub fn col_app(&self) -> &'static str { self.t("App", "Uygulama") }
    pub fn col_state(&self) -> &'static str { self.t("State", "Durum") }
    pub fn col_duration(&self) -> &'static str { self.t("Duration", "Süre") }
    pub fn col_query(&self) -> &'static str { self.t("Query", "Sorgu") }
    pub fn btn_kill(&self) -> &'static str { self.t("Kill", "Sonlandır") }
    pub fn hover_kill(&self, pid: &str) -> String {
        match self.0 {
            Lang::En => format!("Terminate PID {pid}"),
            Lang::Tr => format!("PID {pid}'yi sonlandır"),
        }
    }
    pub fn col_index(&self) -> &'static str { self.t("Index", "İndeks") }
    pub fn col_size(&self) -> &'static str { self.t("Size", "Boyut") }
    pub fn col_scans(&self) -> &'static str { self.t("Scans", "Tarama") }

    // ── Explain (performance suggestions) ────────────────────────────────────

    pub fn suggest_seq_scan(&self, name: &str) -> String {
        match self.0 {
            Lang::En => format!(
                "\"{name}\" full table scan (Seq Scan) — consider adding an index if filtered"
            ),
            Lang::Tr => format!(
                "\"{name}\" tablosunda tam tablo taraması (Seq Scan) — filtreleniyorsa index eklemeyi düşünün"
            ),
        }
    }
    pub fn suggest_bad_estimate(&self, name: &str, plan: i64, actual: i64, ratio: f64) -> String {
        match self.0 {
            Lang::En => format!(
                "\"{name}\" row estimation off (plan: {plan}, actual: {actual}, ratio: {ratio:.1}×) \
                 — run ANALYZE or update stats"
            ),
            Lang::Tr => format!(
                "\"{name}\" için satır tahmini hatalı (plan: {plan}, gerçek: {actual}, oran: {ratio:.1}×) \
                 — ANALYZE çalıştırın veya istatistikleri güncelleyin"
            ),
        }
    }
    pub fn suggest_expensive_sort(&self, key: &str) -> String {
        match self.0 {
            Lang::En => format!(
                "Expensive Sort{key} — consider adding an index on the sort column"
            ),
            Lang::Tr => format!(
                "Pahalı Sort işlemi{key} — sıralama sütununa index eklemeyi düşünün"
            ),
        }
    }
    pub fn suggest_nested_loop(&self) -> &'static str {
        self.t(
            "Nested Loop with large data set — Hash Join may be more efficient",
            "Nested Loop büyük veri setinde çalışıyor — Hash Join daha verimli olabilir",
        )
    }
    pub fn lbl_suggestions(&self) -> &'static str { self.t("⚡ Suggestions", "⚡ Öneriler") }
    pub fn lbl_planning(&self) -> &'static str { self.t("Planning:", "Planlama:") }
    pub fn lbl_execution(&self) -> &'static str { self.t("Execution:", "Çalışma:") }
    pub fn lbl_slowest_node(&self) -> &'static str { self.t("🐢 Slowest Node", "🐢 En Yavaş Node") }

    // ── ER Diagram ────────────────────────────────────────────────────────────

    pub fn er_no_diagram(&self) -> &'static str { self.t("No diagram loaded.", "Diyagram yüklenmedi.") }
    pub fn er_loading(&self, schema: &str) -> String {
        match self.0 {
            Lang::En => format!("Loading ER diagram for \"{schema}\"…"),
            Lang::Tr => format!("\"{}\" için ER diyagramı yükleniyor…", schema),
        }
    }
    pub fn er_error(&self, msg: &str) -> String {
        match self.0 {
            Lang::En => format!("Error: {msg}"),
            Lang::Tr => format!("Hata: {msg}"),
        }
    }
    pub fn er_title(&self, schema: &str) -> String {
        match self.0 {
            Lang::En => format!("ER Diagram — {schema}"),
            Lang::Tr => format!("ER Diyagramı — {schema}"),
        }
    }
    pub fn er_btn_auto_layout(&self) -> &'static str { self.t("Auto Layout", "Otomatik Düzen") }
    pub fn er_btn_reset(&self) -> &'static str { self.t("Reset", "Sıfırla") }

    // ── Join Builder ──────────────────────────────────────────────────────────

    pub fn jb_window_title(&self) -> &'static str { self.t("Join Builder", "Join Oluşturucu") }
    pub fn jb_section_tables(&self) -> &'static str { self.t("TABLES", "TABLOLAR") }
    pub fn jb_section_conditions(&self) -> &'static str { self.t("JOIN CONDITIONS", "JOIN KOŞULLARI") }
    pub fn jb_section_sql(&self) -> &'static str { self.t("GENERATED SQL", "OLUŞTURULAN SQL") }
    pub fn jb_hint_add_tables(&self) -> &'static str {
        self.t("Add at least 2 tables above.", "Yukarıdan en az 2 tablo ekleyin.")
    }
    pub fn jb_btn_add_table(&self) -> &'static str { self.t("＋  Add Table", "＋  Tablo Ekle") }
    pub fn jb_btn_add_condition(&self) -> &'static str {
        self.t("＋  Add Join Condition", "＋  Join Koşulu Ekle")
    }
    pub fn jb_loading_cols(&self) -> &'static str { self.t("(⟳ loading…)", "(⟳ yükleniyor…)") }
    pub fn jb_n_cols(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("({n} cols)"),
            Lang::Tr => format!("({n} sütun)"),
        }
    }
    pub fn jb_btn_run(&self) -> &'static str { self.t("▶  Run", "▶  Çalıştır") }
    pub fn jb_hover_run(&self) -> &'static str { self.t("Execute and show results", "Çalıştır ve sonuçları göster") }
    pub fn jb_btn_send_editor(&self) -> &'static str { self.t("✎  Send to Editor", "✎  Editöre Gönder") }
    pub fn jb_hover_send_editor(&self) -> &'static str {
        self.t("Paste SQL into the active query tab", "SQL'i aktif sorgu sekmesine yapıştır")
    }
    pub fn jb_btn_reset(&self) -> &'static str { self.t("↺  Reset", "↺  Sıfırla") }
    pub fn jb_hover_reset(&self) -> &'static str { self.t("Clear all", "Hepsini temizle") }

    // ── Table dialog ──────────────────────────────────────────────────────────

    pub fn td_title_new(&self) -> &'static str { self.t("New Table", "Yeni Tablo") }
    pub fn td_title_edit(&self) -> &'static str { self.t("Edit Table", "Tabloyu Düzenle") }
    pub fn td_label_schema(&self) -> &'static str { self.t("Schema:", "Şema:") }
    pub fn td_label_table_name(&self) -> &'static str { self.t("Table name:", "Tablo adı:") }
    pub fn td_hint_table_name(&self) -> &'static str { self.t("e.g. users", "örn. users") }
    pub fn td_lbl_columns(&self) -> &'static str { self.t("Columns", "Sütunlar") }
    pub fn td_btn_add_column(&self) -> &'static str { self.t("  + Add column  ", "  + Sütun Ekle  ") }
    pub fn td_lbl_existing_cols(&self) -> &'static str { self.t("Existing Columns", "Mevcut Sütunlar") }
    pub fn td_lbl_table_label(&self) -> &'static str { self.t("Table:", "Tablo:") }
    pub fn td_lbl_no_cols(&self) -> &'static str { self.t("(no columns loaded)", "(sütun yüklenmedi)") }
    pub fn td_col_name(&self) -> &'static str { self.t("Name", "Ad") }
    pub fn td_col_type(&self) -> &'static str { self.t("Type", "Tip") }
    pub fn td_col_drop(&self) -> &'static str { self.t("Drop", "Sil") }
    pub fn td_lbl_add_columns(&self) -> &'static str { self.t("Add Columns", "Sütun Ekle") }
    pub fn td_lbl_no_new_cols(&self) -> &'static str {
        self.t("No new columns to add.", "Eklenecek yeni sütun yok.")
    }
    pub fn td_btn_preview_show(&self) -> &'static str { self.t("▸ Preview SQL", "▸ SQL Önizleme") }
    pub fn td_btn_preview_hide(&self) -> &'static str { self.t("▾ Hide SQL", "▾ SQL Gizle") }
    pub fn td_btn_create(&self) -> &'static str { self.t("  Create Table  ", "  Tablo Oluştur  ") }
    pub fn td_btn_apply(&self) -> &'static str { self.t("  Apply Changes  ", "  Değişiklikleri Uygula  ") }
    pub fn td_err_table_name_required(&self) -> &'static str {
        self.t("Table name is required.", "Tablo adı gerekli.")
    }
    pub fn td_err_col_name_required(&self, i: usize) -> String {
        match self.0 {
            Lang::En => format!("Column {i} name is required."),
            Lang::Tr => format!("{i}. sütun adı gerekli."),
        }
    }
    pub fn td_err_col_no_type(&self, name: &str) -> String {
        match self.0 {
            Lang::En => format!("Column '{name}' has no type."),
            Lang::Tr => format!("'{name}' sütununun tipi yok."),
        }
    }
    pub fn td_err_new_col_name_required(&self) -> &'static str {
        self.t("New column name is required.", "Yeni sütun adı gerekli.")
    }
    pub fn td_err_no_changes(&self) -> &'static str {
        self.t("No changes detected.", "Değişiklik bulunamadı.")
    }
    pub fn td_col_default(&self) -> &'static str { self.t("Default", "Varsayılan") }

    // ── Settings / About ──────────────────────────────────────────────────────
    pub fn menu_settings(&self) -> &'static str { self.t("Settings", "Ayarlar") }
    pub fn menu_about(&self) -> &'static str { self.t("About", "Hakkında") }
    pub fn about_version(&self) -> &'static str { self.t("Version", "Sürüm") }
    pub fn about_repository(&self) -> &'static str { self.t("Repository", "Kaynak Kod") }
    pub fn about_desc(&self) -> &'static str {
        self.t(
            "Blazing-fast lightweight PostgreSQL client built in Rust",
            "Rust ile yazılmış hızlı ve hafif PostgreSQL istemcisi",
        )
    }

    // ── Column Statistics ─────────────────────────────────────────────────────

    pub fn col_stats_menu_item(&self) -> &'static str { self.t("📊 Statistics", "📊 İstatistikler") }
    pub fn col_stats_title(&self, col: &str) -> String {
        match self.0 {
            Lang::En => format!("Column: {col}"),
            Lang::Tr => format!("Sütun: {col}"),
        }
    }
    pub fn col_stats_total(&self) -> &'static str { self.t("Total rows:", "Toplam satır:") }
    pub fn col_stats_null(&self) -> &'static str { self.t("Null:", "Null:") }
    pub fn col_stats_distinct(&self) -> &'static str { self.t("Distinct:", "Farklı değer:") }
    pub fn col_stats_min_len(&self) -> &'static str { self.t("Min length:", "Min uzunluk:") }
    pub fn col_stats_max_len(&self) -> &'static str { self.t("Max length:", "Max uzunluk:") }
    pub fn col_stats_top_values(&self) -> &'static str { self.t("Top values:", "En çok tekrar:") }
    pub fn col_stats_source_note(&self) -> &'static str {
        self.t("(computed from fetched rows)", "(yüklenen satırlardan hesaplandı)")
    }
}
