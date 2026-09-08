# ferox — Gelecek Özellik Fikirleri

## 🛠 Orta Öncelik

### Trigger browser
Sidebar'da TRIGGERS alt bölümü. `pg_trigger` + `pg_proc` join — trigger adı, event, timing (BEFORE/AFTER), fonksiyon.

### Sequence browser
`information_schema.sequences` — adı, current value, increment, min/max. Browse + "Set Value" aksiyonu.

### Bağlantı ping / latency göstergesi
Sidebar altında veya tab başlığında `● 4ms`. `SELECT 1` ile periyodik ölçüm.

### Sorgu çalışma süresi & satır sayısı statusbar'ı
Sorgu bitince altta `✓ 1.243 satır — 87ms`. Şu an Messages tab'ında var ama gömülü.

---

## 💡 Düşük Öncelik / Uzun Vadeli

### Şema anlık görüntüsü (Snapshot)
"Şu anki şemayı kaydet" → daha sonra Schema Diff ile geçmiş snapshot'a karşılaştır. `DbEvent::SchemaSnapshot` zaten mevcut, UI eksik.

### Kayıtlı sorgular / snippet
Ctrl+Shift+S ile sorguyu isimle kaydet. Sidebar veya ayrı panelde liste. History var ama isimsiz.

### Split view — yan yana iki editör
Aynı connection'da iki sorguyu aynı anda çalıştır. Büyük mimari değişiklik gerektirir.

### Extension browser
`SELECT * FROM pg_extension` — yüklü extension'lar ve versiyonları. Read-only liste.

---

## 🐛 Küçük Düzeltmeler

| Sorun | Çözüm |
|---|---|
| ER diyagramında çok kolonlu tablo taşıyor | Node max-height + scroll veya `...` truncate |
| Dashboard tablolarında sıralama yok | `sort_by` state ekle |

✓ Çözüldü: Ctrl+A (v-next), NULL rengi config'e kaydedilmiyor (v-next), column genişlik hesabı — zaten sample-based ve tek seferlik olduğu doğrulandı.

✓ Çözüldü (Faz 14): çoklu satır seçimi & toplu işlem (kopyala/export/sil), gelişmiş
operatör tabanlı filtre çubuğu, satır kopyalama/duplicate, CSV import (`COPY FROM STDIN`).
Ayrıca kod incelemesiyle bulunan, bu dosyada hiç yer almamış üç boşluk da kapatıldı:
DB/SSH şifreleri + AI API key artık OS keychain'de (`keyring`), bağlantılar için
prod/staging renk etiketi (tab + switcher), JSON/JSONB pretty-print (View Full Value popup).
