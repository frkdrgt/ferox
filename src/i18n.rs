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
///
/// `\u{E…}` escapes in labels are Phosphor icon codepoints (see
/// `egui_phosphor::regular`, installed by `ui::theme::install_fonts`).
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
    pub fn menu_safe_mode(&self) -> &'static str { self.t("\u{E40C} Safe Mode", "\u{E40C} Güvenli Mod") }
    pub fn menu_safe_mode_on(&self) -> &'static str { self.t("\u{E40C} Safe Mode  \u{E182}", "\u{E40C} Güvenli Mod  \u{E182}") }
    pub fn menu_join_builder(&self) -> &'static str { self.t("Join Builder…", "Join Oluşturucu…") }
    pub fn menu_dashboard(&self) -> &'static str { self.t("\u{E628} Dashboard", "\u{E628} Gösterge Paneli") }
    pub fn menu_schema_diff(&self) -> &'static str { self.t("\u{E27C} Schema Diff", "\u{E27C} Şema Karşılaştırma") }
    pub fn menu_execute(&self) -> &'static str { self.t("Execute (F5 / Ctrl+Enter)", "Çalıştır (F5 / Ctrl+Enter)") }
    pub fn menu_cancel(&self) -> &'static str { self.t("Cancel (Ctrl+C)", "İptal (Ctrl+C)") }
    pub fn menu_export_csv(&self) -> &'static str { self.t("Export as CSV…", "CSV olarak Dışa Aktar…") }
    pub fn menu_export_json(&self) -> &'static str { self.t("Export as JSON…", "JSON olarak Dışa Aktar…") }
    pub fn menu_language(&self) -> &'static str { self.t("Language", "Dil") }

    // ── Status bar ────────────────────────────────────────────────────────────

    pub fn status_disconnected(&self) -> &'static str { self.t("●  Disconnected", "●  Bağlı Değil") }
    pub fn status_connecting(&self) -> &'static str { self.t("●  Connecting…", "●  Bağlanıyor…") }
    pub fn status_error(&self, msg: &str) -> String {
        match self.0 {
            Lang::En => format!("●  Error: {msg}"),
            Lang::Tr => format!("●  Hata: {msg}"),
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
    pub fn btn_commit(&self) -> &'static str { self.t("\u{E182}  COMMIT", "\u{E182}  ONAYLA") }
    pub fn btn_rollback(&self) -> &'static str { self.t("\u{E4F6}  ROLLBACK", "\u{E4F6}  GERİ AL") }

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
    pub fn label_color(&self) -> &'static str { self.t("Color tag:", "Renk etiketi:") }
    pub fn hint_color_clear(&self) -> &'static str { self.t("Clear color tag", "Renk etiketini kaldır") }
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

    pub fn btn_run(&self) -> &'static str { self.t("\u{E3D0} Run", "\u{E3D0} Çalıştır") }
    pub fn btn_running(&self) -> &'static str { self.t("\u{E2B2} Running…", "\u{E2B2} Çalışıyor…") }
    pub fn btn_cancel_query(&self) -> &'static str { self.t("\u{E46C} Cancel", "\u{E46C} İptal") }
    pub fn btn_explain(&self) -> &'static str { self.t("\u{E67C} Explain", "\u{E67C} Açıkla") }
    pub fn hover_explain(&self) -> &'static str {
        self.t("Run EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)", "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) çalıştır")
    }
    pub fn btn_hist_prev(&self) -> &'static str { self.t("\u{E1A0} Hist", "\u{E1A0} Geçmiş") }
    pub fn hover_hist_prev(&self) -> &'static str { self.t("Previous query (↑)", "Önceki sorgu (↑)") }
    pub fn hover_hist_next(&self) -> &'static str { self.t("Next query (↓)", "Sonraki sorgu (↓)") }
    pub fn btn_format(&self) -> &'static str { self.t("\u{EA1E} Format", "\u{EA1E} Biçimle") }
    pub fn hover_format(&self) -> &'static str {
        self.t("Format SQL (Shift+Alt+F)", "SQL'i biçimle (Shift+Alt+F)")
    }
    pub fn hover_export_csv(&self) -> &'static str { self.t("Export as CSV…", "CSV olarak dışa aktar…") }
    pub fn hover_export_json(&self) -> &'static str { self.t("Export as JSON…", "JSON olarak dışa aktar…") }
    pub fn btn_run_file(&self) -> &'static str { self.t("\u{E61E} Run File", "\u{E61E} Dosya Çalıştır") }
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
    pub fn tab_saved(&self) -> &'static str { self.t("Saved", "Kayıtlı") }
    pub fn tab_plan(&self) -> &'static str { self.t("\u{E67C} Plan", "\u{E67C} Plan") }
    pub fn tab_messages(&self) -> &'static str { self.t("Messages", "Mesajlar") }
    pub fn tab_messages_n(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("Messages ({n})"),
            Lang::Tr => format!("Mesajlar ({n})"),
        }
    }

    // ── Saved queries / snippets ──────────────────────────────────────────────

    pub fn btn_save_snippet(&self) -> &'static str { self.t("\u{E248} Save", "\u{E248} Kaydet") }
    pub fn hover_save_snippet(&self) -> &'static str {
        self.t("Save query with a name (Ctrl+Shift+S)", "Sorguyu isimle kaydet (Ctrl+Shift+S)")
    }
    pub fn dlg_save_query(&self) -> &'static str { self.t("\u{E248} Save Query", "\u{E248} Sorguyu Kaydet") }
    pub fn lbl_snippet_name(&self) -> &'static str { self.t("Name:", "İsim:") }
    pub fn btn_save(&self) -> &'static str { self.t("Save", "Kaydet") }
    pub fn snippet_overwrite_warn(&self) -> &'static str {
        self.t(
            "⚠ A snippet with this name exists — it will be overwritten.",
            "⚠ Bu isimde bir kayıt var — üzerine yazılacak.",
        )
    }
    pub fn snippet_ctx_load(&self) -> &'static str { self.t("\u{E1CA}  Load into editor", "\u{E1CA}  Editöre yükle") }
    pub fn snippet_ctx_delete(&self) -> &'static str { self.t("\u{E4A6}  Delete", "\u{E4A6}  Sil") }
    pub fn snippets_empty_hint(&self) -> &'static str {
        self.t(
            "No saved queries yet — press Ctrl+Shift+S to save the current query.",
            "Henüz kayıtlı sorgu yok — geçerli sorguyu Ctrl+Shift+S ile kaydedin.",
        )
    }
    pub fn log_snippet_saved(&self, name: &str) -> String {
        match self.0 {
            Lang::En => format!("Query saved as \"{name}\""),
            Lang::Tr => format!("Sorgu \"{name}\" adıyla kaydedildi"),
        }
    }

    // ── Query panel — browse ──────────────────────────────────────────────────

    pub fn browse_prefix(&self) -> &'static str { self.t("Browse:", "Göz At:") }
    pub fn browse_sorted_by(&self) -> &'static str { self.t("sorted by", "sıralama:") }
    pub fn btn_exit_browse(&self) -> &'static str { self.t("\u{E4F6} Exit browse", "\u{E4F6} Göz atmayı kapat") }
    pub fn filter_hint(&self) -> &'static str { self.t("\u{E266} Filter results…", "\u{E266} Sonuçları filtrele…") }
    pub fn label_search(&self) -> &'static str { self.t("Search:", "Ara:") }
    pub fn lbl_running(&self) -> &'static str { self.t("Running…", "Çalışıyor…") }
    pub fn lbl_running_explain(&self) -> &'static str { self.t("Running EXPLAIN…", "EXPLAIN çalıştırılıyor…") }
    pub fn lbl_no_results_yet(&self) -> &'static str {
        self.t(
            "No results yet",
            "Henüz sonuç yok",
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
    pub fn btn_json_pretty(&self) -> &'static str { self.t("Pretty", "Biçimlendir") }
    pub fn btn_json_raw(&self) -> &'static str { self.t("Raw", "Ham") }
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
    pub fn cell_copy_as_markdown(&self) -> &'static str {
        self.t("Copy table as Markdown", "Tabloyu Markdown Olarak Kopyala")
    }
    pub fn cell_copy_as_html(&self) -> &'static str {
        self.t("Copy table as HTML", "Tabloyu HTML Olarak Kopyala")
    }
    pub fn search_hint(&self) -> &'static str { self.t("Find in results…", "Sonuçlarda ara…") }
    pub fn search_matches(&self, n: usize) -> String {
        match self.0 {
            Lang::En => if n == 1 { "1 match".to_owned() } else { format!("{n} matches") },
            Lang::Tr => format!("{n} eşleşme"),
        }
    }

    // ── Sidebar ───────────────────────────────────────────────────────────────

    pub fn schema_browser(&self) -> &'static str { self.t("SCHEMA BROWSER", "ŞEMA TARAYICISI") }
    pub fn sidebar_filter_hint(&self) -> &'static str { self.t("\u{E30C}  Filter…", "\u{E30C}  Filtrele…") }
    pub fn kind_tables(&self) -> &'static str { self.t("TABLES", "TABLOLAR") }
    pub fn kind_views(&self) -> &'static str { self.t("VIEWS", "GÖRÜNÜMLER") }
    pub fn kind_mat_views(&self) -> &'static str { self.t("MAT VIEWS", "MAT GÖRÜNÜMLER") }
    pub fn kind_foreign_tables(&self) -> &'static str { self.t("FOREIGN TABLES", "YABANCI TABLOLAR") }
    pub fn kind_functions(&self) -> &'static str { self.t("FUNCTIONS", "FONKSİYONLAR") }
    pub fn kind_triggers(&self) -> &'static str { self.t("TRIGGERS", "TETİKLEYİCİLER") }
    pub fn kind_sequences(&self) -> &'static str { self.t("SEQUENCES", "SIRALAR") }
    pub fn fn_show_definition(&self) -> &'static str { self.t("Show Definition", "Tanımı Göster") }
    pub fn fn_copy_call(&self) -> &'static str { self.t("Copy Call", "Çağrıyı Kopyala") }
    pub fn seq_set_value(&self) -> &'static str { self.t("Set Value…", "Değer Ata…") }
    pub fn seq_show_ddl(&self) -> &'static str { self.t("Show DDL", "DDL Göster") }
    pub fn cell_view_full(&self) -> &'static str { self.t("View Full Value", "Tam Değeri Göster") }
    pub fn cell_copy_value(&self) -> &'static str { self.t("Copy Value", "Değeri Kopyala") }
    pub fn bulk_selected_count(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("{n} rows selected"),
            Lang::Tr => format!("{n} satır seçili"),
        }
    }
    pub fn btn_bulk_copy(&self) -> &'static str { self.t("Copy Selected Rows", "Seçili Satırları Kopyala") }
    pub fn btn_bulk_export_csv(&self) -> &'static str { self.t("Export Selected as CSV…", "Seçilenleri CSV Olarak Dışa Aktar…") }
    pub fn btn_bulk_export_json(&self) -> &'static str { self.t("Export Selected as JSON…", "Seçilenleri JSON Olarak Dışa Aktar…") }
    pub fn btn_bulk_delete(&self) -> &'static str { self.t("Delete Selected Rows…", "Seçili Satırları Sil…") }
    pub fn btn_duplicate_row(&self) -> &'static str { self.t("Duplicate Row", "Satırı Çoğalt") }
    pub fn confirm_bulk_delete_title(&self) -> &'static str { self.t("Delete rows?", "Satırlar silinsin mi?") }
    pub fn confirm_bulk_delete_body(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("This will permanently delete {n} row(s). This cannot be undone."),
            Lang::Tr => format!("{n} satır kalıcı olarak silinecek. Bu işlem geri alınamaz."),
        }
    }
    pub fn btn_confirm_delete(&self) -> &'static str { self.t("Delete", "Sil") }
    pub fn browse_filter_hint(&self) -> &'static str {
        self.t("id > 100  OR  name LIKE '%foo%'", "id > 100  VEYA  name LIKE '%foo%'")
    }
    pub fn btn_apply_filter(&self) -> &'static str { self.t("Filter", "Filtrele") }
    pub fn filter_builder_col_hint(&self) -> &'static str { self.t("column", "kolon") }
    pub fn filter_builder_val_hint(&self) -> &'static str { self.t("value", "değer") }
    pub fn btn_add_filter(&self) -> &'static str { self.t("+ Add", "+ Ekle") }
    pub fn hover_add_filter(&self) -> &'static str {
        self.t("Append this condition to the WHERE filter above", "Bu koşulu yukarıdaki WHERE filtresine ekle")
    }
    pub fn schema_menu_new_table(&self) -> &'static str { self.t("\u{E3D4}  New Table…", "\u{E3D4}  Yeni Tablo…") }
    pub fn schema_menu_er(&self) -> &'static str { self.t("\u{EB58}  View ER Diagram", "\u{EB58}  ER Diyagramını Gör") }
    pub fn schema_menu_refresh(&self) -> &'static str { self.t("\u{E094}  Refresh", "\u{E094}  Yenile") }
    pub fn table_menu_generate_script(&self) -> &'static str { self.t("\u{E914}  Generate Script", "\u{E914}  Script Oluştur") }
    pub fn table_menu_browse(&self) -> &'static str { self.t("\u{E476}  Browse rows", "\u{E476}  Satırlara Gözat") }
    pub fn table_menu_edit(&self) -> &'static str { self.t("\u{E3B4}  Edit Table…", "\u{E3B4}  Tabloyu Düzenle…") }
    pub fn table_menu_import_csv(&self) -> &'static str { self.t("\u{E20C}  Import CSV…", "\u{E20C}  CSV İçe Aktar…") }
    pub fn import_csv_title(&self) -> &'static str { self.t("Import CSV", "CSV İçe Aktar") }
    pub fn import_csv_detected_cols(&self) -> &'static str { self.t("Detected columns (from CSV header):", "Algılanan kolonlar (CSV başlığından):") }
    pub fn btn_import(&self) -> &'static str { self.t("Import", "İçe Aktar") }
    pub fn import_csv_running(&self) -> &'static str { self.t("Importing…", "İçe aktarılıyor…") }
    pub fn import_csv_done(&self, rows: u64) -> String {
        match self.0 {
            Lang::En => format!("Imported {rows} row(s)."),
            Lang::Tr => format!("{rows} satır içe aktarıldı."),
        }
    }
    pub fn table_menu_show_ddl(&self) -> &'static str { self.t("{}  Show DDL", "{}  DDL Göster") }
    pub fn table_menu_count(&self) -> &'static str { self.t("\u{EAB8}  Count rows", "\u{EAB8}  Satır Say") }
    pub fn table_menu_show_cols(&self) -> &'static str { self.t("\u{E546}  Show columns", "\u{E546}  Sütunları Göster") }
    pub fn table_menu_show_indexes(&self) -> &'static str { self.t("\u{E2F6}  Show indexes", "\u{E2F6}  İndeksleri Göster") }
    pub fn table_menu_show_fks(&self) -> &'static str { self.t("\u{E2D6}  Show foreign keys", "\u{E2D6}  Yabancı Anahtarları Göster") }
    pub fn lbl_not_connected_sidebar(&self) -> &'static str { self.t("Not connected", "Bağlı değil") }

    // ── Tab manager ───────────────────────────────────────────────────────────

    pub fn tab_close(&self) -> &'static str { self.t("Close tab", "Sekmeyi kapat") }
    pub fn tab_close_others(&self) -> &'static str { self.t("Close other tabs", "Diğer sekmeleri kapat") }
    pub fn tab_close_all(&self) -> &'static str { self.t("Close all tabs", "Tüm sekmeleri kapat") }
    pub fn lbl_no_connection_available(&self) -> &'static str {
        self.t("Connection not available", "Bağlantı mevcut değil")
    }

    // ── Dashboard ─────────────────────────────────────────────────────────────

    pub fn sidebar_counts(&self, schemas: usize, tables: usize) -> String {
        match self.0 {
            Lang::En => format!("{schemas} schemas · {tables} tables loaded"),
            Lang::Tr => format!("{schemas} şema · {tables} tablo yüklü"),
        }
    }
    pub fn tb_new_connection(&self) -> &'static str { self.t("Connection", "Bağlantı") }
    pub fn tb_new_query(&self) -> &'static str { self.t("New Query", "Yeni Sorgu") }
    pub fn tb_dashboard(&self) -> &'static str { self.t("Dashboard", "Gösterge") }
    pub fn tb_join_builder(&self) -> &'static str { self.t("Join Builder", "Join") }
    pub fn tb_schema_diff(&self) -> &'static str { self.t("Schema Diff", "Şema Farkı") }
    pub fn tb_safe_mode(&self) -> &'static str { self.t("Safe Mode", "Güvenli Mod") }
    pub fn dash_card_tables(&self) -> &'static str { self.t("Tables", "Tablolar") }
    pub fn dash_card_total_size(&self) -> &'static str { self.t("Total size", "Toplam boyut") }
    pub fn dash_card_connections(&self) -> &'static str { self.t("Active / all connections", "Aktif / tüm bağlantılar") }
    pub fn dash_card_unused_indexes(&self) -> &'static str { self.t("Unused / all indexes", "Kullanılmayan / tüm indeksler") }
    pub fn btn_refresh(&self) -> &'static str { self.t("\u{E094} Refresh", "\u{E094} Yenile") }
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
    pub fn lbl_suggestions(&self) -> &'static str { self.t("\u{E2DE} Suggestions", "\u{E2DE} Öneriler") }
    pub fn lbl_planning(&self) -> &'static str { self.t("Planning:", "Planlama:") }
    pub fn lbl_execution(&self) -> &'static str { self.t("Execution:", "Çalışma:") }
    pub fn lbl_slowest_node(&self) -> &'static str { self.t("\u{E492} Slowest Node", "\u{E492} En Yavaş Node") }

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
    pub fn jb_btn_add_table(&self) -> &'static str { self.t("\u{E3D4}  Add Table", "\u{E3D4}  Tablo Ekle") }
    pub fn jb_btn_add_condition(&self) -> &'static str {
        self.t("\u{E3D4}  Add Join Condition", "\u{E3D4}  Join Koşulu Ekle")
    }
    pub fn jb_loading_cols(&self) -> &'static str { self.t("(⟳ loading…)", "(⟳ yükleniyor…)") }
    pub fn jb_n_cols(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("({n} cols)"),
            Lang::Tr => format!("({n} sütun)"),
        }
    }
    pub fn jb_btn_run(&self) -> &'static str { self.t("\u{E3D0}  Run", "\u{E3D0}  Çalıştır") }
    pub fn jb_hover_run(&self) -> &'static str { self.t("Execute and show results", "Çalıştır ve sonuçları göster") }
    pub fn jb_btn_send_editor(&self) -> &'static str { self.t("\u{E3B4}  Send to Editor", "\u{E3B4}  Editöre Gönder") }
    pub fn jb_hover_send_editor(&self) -> &'static str {
        self.t("Paste SQL into the active query tab", "SQL'i aktif sorgu sekmesine yapıştır")
    }
    pub fn jb_btn_reset(&self) -> &'static str { self.t("\u{E094}  Reset", "\u{E094}  Sıfırla") }
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
    pub fn td_hover_rename_table(&self) -> &'static str { self.t("Edit to rename the table", "Tabloyu yeniden adlandırmak için düzenleyin") }
    pub fn td_lbl_indexes(&self) -> &'static str { self.t("Indexes", "Index'ler") }
    pub fn td_btn_add_index(&self) -> &'static str { self.t("+ Add index", "+ Index ekle") }
    pub fn td_lbl_constraint(&self, name: &str) -> String {
        match self.0 {
            Lang::En => format!("constraint {name}"),
            Lang::Tr => format!("constraint {name}"),
        }
    }
    pub fn td_col_index_cols(&self) -> &'static str { self.t("Columns / expressions", "Kolonlar / ifadeler") }
    pub fn td_col_method(&self) -> &'static str { self.t("Method", "Yöntem") }
    pub fn td_hint_index_auto_name(&self) -> &'static str { self.t("(auto)", "(otomatik)") }
    pub fn td_lbl_no_changes_yet(&self) -> &'static str {
        self.t("No changes yet — the generated DDL will appear here.", "Henüz değişiklik yok — üretilen DDL burada görünecek.")
    }
    pub fn td_lbl_ddl_preview(&self) -> &'static str { self.t("DDL to run:", "Çalıştırılacak DDL:") }
    pub fn td_note_single_transaction(&self) -> &'static str {
        self.t("Runs as a single transaction — all statements succeed or none do.", "Tek transaction olarak çalışır — ya hepsi uygulanır ya hiçbiri.")
    }
    pub fn td_warn_header(&self) -> &'static str { self.t("⚠ Destructive / risky changes:", "⚠ Yıkıcı / riskli değişiklikler:") }
    pub fn td_warn_drop_col(&self, col: &str) -> String {
        match self.0 {
            Lang::En => format!("column \"{col}\" and all its data will be dropped"),
            Lang::Tr => format!("\"{col}\" kolonu ve tüm verisi silinecek"),
        }
    }
    pub fn td_warn_type_change(&self, col: &str, from: &str, to: &str) -> String {
        match self.0 {
            Lang::En => format!("\"{col}\": {from} → {to} rewrites the table and fails if a value can't be cast"),
            Lang::Tr => format!("\"{col}\": {from} → {to} tabloyu yeniden yazar, dönüştürülemeyen değer varsa başarısız olur"),
        }
    }
    pub fn td_warn_set_not_null(&self, col: &str) -> String {
        match self.0 {
            Lang::En => format!("\"{col}\" SET NOT NULL fails if any row is NULL"),
            Lang::Tr => format!("\"{col}\" SET NOT NULL, NULL değerli satır varsa başarısız olur"),
        }
    }
    pub fn td_warn_drop_index(&self, name: &str) -> String {
        match self.0 {
            Lang::En => format!("index/constraint \"{name}\" will be dropped"),
            Lang::Tr => format!("\"{name}\" index/constraint'i silinecek"),
        }
    }
    pub fn td_btn_open_in_editor(&self) -> &'static str { self.t("Open in Editor", "Editörde Aç") }
    pub fn td_err_col_renamed_empty(&self, col: &str) -> String {
        match self.0 {
            Lang::En => format!("New name for column \"{col}\" is empty."),
            Lang::Tr => format!("\"{col}\" kolonunun yeni adı boş."),
        }
    }
    pub fn td_err_index_cols_required(&self) -> &'static str {
        self.t("Each new index needs at least one column or expression.", "Her yeni index en az bir kolon veya ifade içermeli.")
    }

    // ── Settings / About ──────────────────────────────────────────────────────
    pub fn menu_settings(&self) -> &'static str { self.t("Settings", "Ayarlar") }
    pub fn lbl_null_color(&self) -> &'static str { self.t("NULL cell color", "NULL hücre rengi") }
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

    pub fn col_stats_menu_item(&self) -> &'static str { self.t("\u{E150} Statistics", "\u{E150} İstatistikler") }
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
    pub fn col_stats_source_note_db(&self) -> &'static str {
        self.t("(computed from full table)", "(tüm tablodan hesaplandı)")
    }
    pub fn col_stats_loading(&self) -> &'static str {
        self.t("Loading statistics…", "İstatistikler yükleniyor…")
    }

    // ── Run selection / INSERT export ─────────────────────────────────────────

    pub fn btn_run_selection(&self) -> &'static str { self.t("\u{E3D0} Run Selection", "\u{E3D0} Seçimi Çalıştır") }
    pub fn hover_export_insert(&self) -> &'static str {
        self.t("Export as INSERT statements…", "INSERT ifadeleri olarak dışa aktar…")
    }
    pub fn btn_bulk_export_insert(&self) -> &'static str {
        self.t("Export Selected as INSERT…", "Seçilenleri INSERT Olarak Dışa Aktar…")
    }

    // ── Schema dump ───────────────────────────────────────────────────────────

    pub fn schema_menu_dump_full(&self) -> &'static str {
        self.t("\u{E20C}  Dump schema (structure + data)…", "\u{E20C}  Şemayı dök (yapı + veri)…")
    }
    pub fn schema_menu_dump_structure(&self) -> &'static str {
        self.t("\u{E20C}  Dump schema (structure only)…", "\u{E20C}  Şemayı dök (sadece yapı)…")
    }
    pub fn log_dump_started(&self, schema: &str) -> String {
        match self.0 {
            Lang::En => format!("Dumping schema \"{schema}\"…"),
            Lang::Tr => format!("\"{schema}\" şeması dökülüyor…"),
        }
    }
    pub fn log_dump_done(&self, path: &str, tables: usize, rows: u64) -> String {
        match self.0 {
            Lang::En => format!("Dump written → {path} ({tables} table(s), {rows} row(s))"),
            Lang::Tr => format!("Dump yazıldı → {path} ({tables} tablo, {rows} satır)"),
        }
    }
    pub fn err_dump_in_transaction(&self) -> &'static str {
        self.t(
            "Commit or roll back the open transaction before dumping.",
            "Dump almadan önce açık transaction'ı commit veya rollback edin.",
        )
    }

    // ── AI Explain / Optimize / Fix ───────────────────────────────────────────

    pub fn ai_menu_explain(&self) -> &'static str { self.t("Explain query", "Sorguyu açıkla") }
    pub fn ai_menu_optimize(&self) -> &'static str { self.t("Optimize query", "Sorguyu optimize et") }
    pub fn ai_menu_fix(&self) -> &'static str { self.t("\u{E6A2} Fix error with AI", "\u{E6A2} Hatayı AI ile düzelt") }
    pub fn ai_hover_fix_disabled(&self) -> &'static str {
        self.t("Available after a query fails", "Bir sorgu hata verdikten sonra kullanılabilir")
    }
    pub fn ai_hover_assist_menu(&self) -> &'static str {
        self.t(
            "AI: explain / optimize / fix the query (the selection, if any)",
            "AI: sorguyu açıkla / optimize et / düzelt (varsa seçim)",
        )
    }
    pub fn ai_title_explain(&self) -> &'static str { self.t("\u{E6A2} AI — Explanation", "\u{E6A2} AI — Açıklama") }
    pub fn ai_title_optimize(&self) -> &'static str { self.t("\u{E6A2} AI — Optimized query", "\u{E6A2} AI — Optimize edilmiş sorgu") }
    pub fn ai_title_fix(&self) -> &'static str { self.t("\u{E6A2} AI — Fixed query", "\u{E6A2} AI — Düzeltilmiş sorgu") }
    pub fn ai_warn_new_writes(&self, ops: &str) -> String {
        match self.0 {
            Lang::En => format!("⚠ The suggestion adds operations the original didn't have: {ops} — review before running."),
            Lang::Tr => format!("⚠ Öneri, orijinalde olmayan işlemler ekliyor: {ops} — çalıştırmadan önce inceleyin."),
        }
    }
    pub fn ai_btn_replace(&self) -> &'static str { self.t("Replace in editor", "Editördekiyle değiştir") }
    pub fn ai_btn_copy_sql(&self) -> &'static str { self.t("Copy SQL", "SQL'i kopyala") }
    pub fn ai_btn_copy_text(&self) -> &'static str { self.t("Copy text", "Metni kopyala") }
    pub fn ai_no_change(&self) -> &'static str {
        self.t("No rewrite suggested — the query is fine as is.", "Yeniden yazım önerilmedi — sorgu olduğu gibi uygun.")
    }

    // ── Update check ──────────────────────────────────────────────────────────

    pub fn menu_check_updates_startup(&self) -> &'static str {
        self.t("Check for updates on startup", "Açılışta güncellemeleri denetle")
    }
    pub fn menu_check_updates_now(&self) -> &'static str { self.t("Check for updates now", "Güncellemeleri şimdi denetle") }
    pub fn update_checking(&self) -> &'static str { self.t("Checking for updates…", "Güncellemeler denetleniyor…") }
    pub fn update_up_to_date(&self, current: &str) -> String {
        match self.0 {
            Lang::En => format!("\u{E182} ferox {current} is the latest version."),
            Lang::Tr => format!("\u{E182} ferox {current} en güncel sürüm."),
        }
    }
    pub fn update_available(&self, latest: &str, current: &str) -> String {
        match self.0 {
            Lang::En => format!("⬆ ferox {latest} is available (you have {current})."),
            Lang::Tr => format!("⬆ ferox {latest} yayınlandı (sizdeki: {current})."),
        }
    }
    pub fn update_failed(&self, err: &str) -> String {
        match self.0 {
            Lang::En => format!("Update check failed: {err}"),
            Lang::Tr => format!("Güncelleme denetimi başarısız: {err}"),
        }
    }
    pub fn update_btn_release_notes(&self) -> &'static str { self.t("Release notes / download", "Sürüm notları / indir") }
    pub fn update_btn_skip(&self) -> &'static str { self.t("Skip this version", "Bu sürümü atla") }

    // ── DBeaver import ────────────────────────────────────────────────────────

    pub fn menu_import_dbeaver(&self) -> &'static str { self.t("Import from DBeaver…", "DBeaver'dan İçe Aktar…") }
    pub fn dbeaver_title(&self) -> &'static str { self.t("Import from DBeaver", "DBeaver'dan İçe Aktar") }
    pub fn dbeaver_not_found(&self) -> &'static str {
        self.t(
            "No DBeaver workspace found. Choose a data-sources.json file.",
            "DBeaver çalışma alanı bulunamadı. Bir data-sources.json dosyası seçin.",
        )
    }
    pub fn dbeaver_none_pg(&self) -> &'static str {
        self.t("No PostgreSQL connections found.", "PostgreSQL bağlantısı bulunamadı.")
    }
    pub fn dbeaver_exists(&self) -> &'static str { self.t("already exists", "zaten var") }
    pub fn dbeaver_no_password(&self) -> &'static str { self.t("no saved password", "kayıtlı şifre yok") }
    pub fn dbeaver_skipped_other(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("{n} non-PostgreSQL connection(s) skipped."),
            Lang::Tr => format!("PostgreSQL olmayan {n} bağlantı atlandı."),
        }
    }
    pub fn dbeaver_import_n(&self, n: usize) -> String {
        match self.0 {
            Lang::En => format!("Import {n}"),
            Lang::Tr => format!("{n} bağlantıyı aktar"),
        }
    }
    pub fn dbeaver_choose_file(&self) -> &'static str { self.t("Choose file…", "Dosya seç…") }

    // ── Home screen ───────────────────────────────────────────────────────────

    pub fn home_tagline(&self) -> &'static str {
        self.t("Lightweight PostgreSQL client", "Hafif PostgreSQL istemcisi")
    }
    pub fn home_saved_connections(&self) -> &'static str { self.t("Saved connections", "Kayıtlı bağlantılar") }
    pub fn home_groups(&self) -> &'static str { self.t("Groups", "Gruplar") }
    pub fn home_history_count(&self) -> &'static str { self.t("Queries in history", "Geçmişteki sorgular") }
    pub fn home_saved_queries(&self) -> &'static str { self.t("Saved queries", "Kayıtlı sorgular") }
    pub fn home_quick_start(&self) -> &'static str { self.t("Quick Start", "Hızlı Başlangıç") }
    pub fn home_no_profiles(&self) -> &'static str {
        self.t("No saved connections yet", "Henüz kayıtlı bağlantı yok")
    }
    pub fn home_no_profiles_hint(&self) -> &'static str {
        self.t(
            "Create one, or import your existing DBeaver connections.",
            "Yeni bir tane oluşturun ya da DBeaver bağlantılarınızı içe aktarın.",
        )
    }
    pub fn home_connect(&self) -> &'static str { self.t("Connect", "Bağlan") }
    pub fn home_shortcuts(&self) -> &'static str { self.t("Shortcuts", "Kısayollar") }
    pub fn home_recent_queries(&self) -> &'static str { self.t("Recent Queries", "Son Sorgular") }
    pub fn home_no_history(&self) -> &'static str { self.t("No queries run yet", "Henüz sorgu çalıştırılmadı") }
    pub fn home_copy_query(&self) -> &'static str { self.t("Click to copy", "Kopyalamak için tıklayın") }
    pub fn home_ai_title(&self) -> &'static str { self.t("AI Assistant", "AI Asistanı") }
    pub fn home_ai_on(&self, provider: &str, model: &str) -> String {
        match self.0 {
            Lang::En => format!("Enabled — {provider} / {model}. Use \u{E6A2} AI in the editor toolbar for NL→SQL, explain, optimize and fix."),
            Lang::Tr => format!("Etkin — {provider} / {model}. NL→SQL, açıklama, optimizasyon ve hata düzeltme için editördeki \u{E6A2} AI'ı kullanın."),
        }
    }
    pub fn home_ai_off(&self) -> &'static str {
        self.t(
            "Not configured. Add a Claude, OpenAI, Groq or Ollama key under Settings → AI to write SQL from plain language.",
            "Yapılandırılmadı. Düz metinden SQL yazmak için Ayarlar → AI altından Claude, OpenAI, Groq veya Ollama anahtarı ekleyin.",
        )
    }
    pub fn kb_run(&self) -> &'static str { self.t("Run query / selection", "Sorguyu / seçimi çalıştır") }
    pub fn kb_new_tab(&self) -> &'static str { self.t("New tab", "Yeni sekme") }
    pub fn kb_close_tab(&self) -> &'static str { self.t("Close tab", "Sekmeyi kapat") }
    pub fn kb_find(&self) -> &'static str { self.t("Find in results", "Sonuçlarda ara") }
    pub fn kb_save_query(&self) -> &'static str { self.t("Save query", "Sorguyu kaydet") }
    pub fn kb_autocomplete(&self) -> &'static str { self.t("Autocomplete", "Otomatik tamamlama") }
    pub fn kb_format(&self) -> &'static str { self.t("Format SQL", "SQL'i biçimle") }
    pub fn sidebar_empty_hint(&self) -> &'static str {
        self.t("Connect to browse schemas", "Şemalara göz atmak için bağlanın")
    }
    pub fn lbl_no_results_hint(&self) -> &'static str {
        self.t("Press Ctrl+Enter or F5 to run", "Çalıştırmak için Ctrl+Enter veya F5")
    }
}
