# t3-panel

Her uygulamanın üstünde duran, T3 Code thread'lerini listeleyen ve tıklanınca durumu Türkçe, lokal sesle okuyan panel. Rust + [GPUI](https://gpui.rs) (Zed'in UI framework'ü) + [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) (Piper `tr_TR-dfki-medium`). Ücretli API yok; her şey bu Mac'te çalışır.

## Kurulum

```bash
./scripts/fetch-models.sh          # ~79 MB Türkçe Piper modeli → ~/Library/Application Support/t3-panel/models
cargo build --release --workspace
target/release/t3-panel
```

## Panel

- `WindowKind::PopUp`: `NSPanel`, odak çalmaz (non-activating), seviye 101, tüm Space'lerde + tam ekran yardımcı penceresi. Uygulama Accessory modunda (Dock ikonu yok).
- Satır: durum noktası (mavi çalışıyor, yeşil bitti/boşta, sarı durduruldu), başlık, proje · zaman · provider. `▶` okur, `⤢` thread'i T3'te açar.
- Başlıktan sürüklenir. `🔇` susturur, `✕` kapatır.
- Alt satır canlı ölçümler: RSS · CPU · son T3 sorgusu · TTS süresi · RTF · tıklama→ses · kuyruk · run klasörü. Tıklayınca run klasörü Finder'da açılır.

## Ortam değişkenleri

| Değişken | Varsayılan | Anlamı |
|---|---|---|
| `T3P_T3_BIN` | `~/.t3/bin/t3` | T3 CLI |
| `T3P_PROJECTS` | T3 state DB'den keşif | İzlenecek proje id/yolları, virgülle |
| `T3P_POLL_MS` | `5000` | Sorgu aralığı |
| `T3P_MODEL_DIR` | `~/Library/Application Support/t3-panel/models/vits-piper-tr_TR-dfki-medium` | TTS modeli |
| `T3P_LOG_ROOT` | `~/Library/Logs/t3-panel` | Telemetri kökü |
| `T3P_SAVE_AUDIO` | `1` | Her konuşmayı WAV olarak sakla (`0` kapatır) |
| `T3P_SELFTEST` | — | `1`: ilk bitmiş thread'i okur, bitince kapanır (otomatik doğrulama) |

## İzleme

Her çalıştırma `~/Library/Logs/t3-panel/<YYYYMMDD-HHMMSS-pid>/` altına yazar; `latest` en yenisini gösterir.

- `events.jsonl`: her girdi/çıktı tek satır: `{ts, run, seq, kind, dur_ms, ok, in, out, err}`.
  - T3: `t3.discover`, `t3.changes`, `t3.list`, `t3.show`, `t3.poll`; her alt süreç için argv, süre, stdout byte, çıkış kodu, child CPU ms, max RSS.
  - Konuşma: `speak.request`, `speak.extract` (okunan metin + kural), `tts.load`, `tts.synth` (karakter, ses ms, RTF, wav), `audio.open`, `audio.play` (`click_to_audio_ms`).
  - Durum ve arayüz: `state.change`, `thread.open`, `ui.*`.
- `metrics.jsonl`: saniyede bir RSS, CPU, thread sayısı, kuyruk, child CPU toplamı.
- `audio/<seq>.wav`: okunan her cümlenin kaydı.

```bash
target/release/t3p-report latest   # kind başına p50/p95/max, RSS/CPU, T3 CLI maliyeti + report.json + report.html
target/release/t3p-bench           # TTS: soğuk yükleme, cümle başına synth ms / RTF, tepe RSS
target/release/t3p-bench --t3      # tüm projeleri 3 tur listeleme maliyeti
target/release/t3p-bench --e2e     # başsız uçtan uca: ilk bitmiş thread'i okur
```

## Veri kaynağı

- Thread'ler: `t3 thread list|show --json` (T3'ün resmi CLI'ı).
- Proje keşfi ve değişiklik tespiti: `~/.t3/userdata/statev2.sqlite`, yalnızca `/usr/bin/sqlite3 -readonly` ile. Her tikte tek sorgu (~14 ms) hangi projenin `updated_at`'i değiştiğini bulur; `t3 thread list` sadece onlar için çalışır. 120 sn'de bir tam liste.
- Bu tablo T3'ün iç şeması; nightly sürümde değişebilir. Sorgu hata verirse o tik her şeyi listeler ve `t3.changes ok:false` yazar.

## Agent tarafı

[`sesli`](../cengo-skills/sesli/SKILL.md) skill'i (`~/.agents/skills`, `~/.claude/skills`, `~/.codex/skills`): ara adımlar tek cümle, son mesaj `🔊` (okunacak) ve `👀` (okunmaz, ekranda bakılacak) satırlarıyla biter. Panel önce `🔊` satırını okur.

## Kararlar

- `sherpa-onnx = "=1.13.8"`: 2.0.1 Piper için espeak-ng fonem adımını içermiyor; Türkçe sentez `phoneme_codepoints is empty` hatası veriyor.
- `gpui` `runtime_shaders`: Metal Toolchain (`xcrun metal`) kurulu değil; shader'lar açılışta derleniyor (açılış ~110 ms).
