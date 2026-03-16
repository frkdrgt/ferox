# pgclient — Yapılacaklar

## Faz 5: Polish + Release

### SQL Syntax Highlighting ✓
- [x] `syntect` + `once_cell` Cargo.toml'a eklendi
- [x] `src/ui/syntax.rs` yazıldı — `HighlightLines` + `LayoutJob` dönüşümü
- [x] `query_panel.rs`'de `TextEdit::layouter` ile entegre edildi
- [x] Dark mode → `base16-ocean.dark`, Light mode → `InspiredGitHub`
- [x] SyntaxSet / ThemeSet `once_cell::Lazy` ile global cache (ilk kullanımda ~5ms yükle)

### Uygulama İkonu
- [ ] 32x32 ve 256x256 PNG ikon tasarla/bul
- [ ] `main.rs`'deki `load_icon()` fonksiyonunu gerçek PNG bytes ile doldur
- [ ] Windows için `.ico` dosyası oluştur (resource compiler)
- [ ] macOS için `Info.plist` + `.icns`

### Eksik Küçük Özellikler
- [ ] Bağlantı dialog'unda "Test Connection" butonu (bağlan + ping + kapat)
- [ ] Profil silme (Connection menüsünden)
- [ ] Schema tree'de F5 ile yenileme (cache temizle + reload)
- [ ] Sorgu editöründe Ctrl+A ile tümünü seç
- [ ] Sonuç tablosunda hücreye çift tık → tam değeri popup'ta göster (uzun text için)
- [ ] NULL değerlere özel renk tercihini config'e kaydet
- [x] Tab başlığında aktif DB adını göster ✓

### Performans
- [x] Schema tree'de TTL-based cache (60 saniye) — F5 ile manuel refresh de ✓
- [ ] Büyük sonuç setlerinde (>10k satır) column width hesabını lazy yap
- [ ] Release build boyutunu ölç: `cargo build --release && ls -lh target/release/pgclient`

### Dağıtım / CI-CD
- [x] `.github/workflows/release.yml` — tag push'unda otomatik build + GitHub Release ✓
- [x] `pgclient.exe.manifest` — DPI awareness, UTF-8, Windows compat ✓
- [x] `build.rs` — manifest + ikon embed (winresource) ✓
- [x] macOS: `lipo` universal binary + `.app` bundle + `.dmg` (workflow içinde) ✓
- [ ] 32x32 + 256x256 PNG ikon → `assets/icon.png` + `assets/icon.ico`
- [ ] Windows: msvc toolchain ile yerel test (`rustup default stable-x86_64-pc-windows-msvc`)
- [ ] GitHub repo oluştur + ilk release: `git tag v0.1.0 && git push --tags`
- [ ] Her iki platformda startup süresi ve RAM ölçümü

---

## Gelecek Fikirler (Scope dışı şimdilik)

- [x] Çoklu sekme (tab) — aynı anda birden fazla sorgu editörü
- [ ] Otomatik tamamlama (tablo/sütun adları)
- [ ] ER diyagramı görünümü
- [ ] SSH tunnel desteği
- [x] Sorgu planı görselleştirme (EXPLAIN ANALYZE) ✓
- [x] Tablo verisi düzenleme (inline edit + UPDATE) ✓
