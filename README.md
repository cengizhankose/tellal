# t3-panel

Her uygulamanın üstünde duran, T3 Code thread'lerini listeleyen ve tıklanınca durumu Türkçe, lokal sesle okuyan panel. Rust + [GPUI](https://gpui.rs) (Zed'in UI framework'ü) + [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) (Piper `tr_TR-dfki-medium`). Ücretli API yok; her şey bu Mac'te çalışır.

## Kurulum

```bash
./scripts/fetch-models.sh          # ~79 MB Türkçe Piper modeli → ~/Library/Application Support/t3-panel/models
cargo build --release --workspace
target/release/t3-panel
```

## Panel

- `WindowKind::PopUp`: `NSPanel`, normalde odak çalmaz (non-activating), seviye 101, tüm Space'lerde + tam ekran yardımcı penceresi. Uygulama Accessory modunda (Dock ikonu yok). Cevap kutusu açıkken panel key pencere olur (uygulama öne gelir); kapanınca önceki uygulama geri öne alınır. `hidesOnDeactivate` kapalı, panel hep görünür.
- Satır: durum noktası (mavi çalışıyor, yeşil bitti/boşta, sarı durduruldu), başlık, proje · zaman · provider. Sağda dinle düğmesi (sessiz: soluk 🔈, dinleniyor: 🔊, konuşurken vurgulu) ve `💬 Cevapla`. Satıra **çift tık** thread'i T3 masaüstü uygulamasında açar (tek tık sadece seçer; ipucu tooltip'te). Sessiz kanalda gelen mesajlar `N yeni`, kullanıcıyı bekleyen thread sarı `seni bekliyor` çipi gösterir. Dinlenen kanal konuşurken / duraklatılmışken / sırada mesaj varken üçüncü satır: o anki cümle, `⏸`/`▶` (duraklat/devam), `⏭` (okey, geç), sırada varsa `N sırada · canlıya geç`.
- Başlıktan sürüklenir. `🔇 hepsini sustur` tüm kanalları susturur (hiçbiri dinlenmiyorsa soluk), `✕` kapatır.
- Cevap kutusu (`Cevapla`): `→ başlık`, metin kutusu (yazma/IME, Cmd+V, Cmd+A, ok tuşları, Enter gönderir, Esc iptal; uzun metin yatay kayar), durum çipi, `Gönder` / `İptal`. Açılınca Wispr Flow dikteyi başlatır (`🎙 Wispr dinliyor`); Wispr metni kutuya yapıştırır. 600 ms içinde kutu hâlâ boşsa metin Wispr geçmişinden doldurulur. Dikte sürerken Gönder → dikte durur, `⏳ metin bekleniyor`, metin gelince kendiliğinden gönderilir. Başarılıysa `✓ gönderildi` (meşgul thread'de `· sıraya alındı`), 1,2 sn sonra kutu kapanır. Hata olursa kutu ve metin kalır, tekrar denenebilir. Boş metin gönderilmez. Başka satırda Cevapla eski dikteyi iptal edip hedefi değiştirir.
- Alt satır canlı ölçümler (iki satır): RSS · CPU · son T3 sorgusu · canlı takip sorgusu · TTS süresi · RTF / tıklama→ses · mesaj→ses · dikte · dinlenen kanal sayısı · kuyruk. Tıklayınca run klasörü Finder'da açılır (yolu tooltip'te).

## Ortam değişkenleri

| Değişken | Varsayılan | Anlamı |
|---|---|---|
| `T3P_T3_BIN` | `~/.t3/bin/t3` | T3 CLI |
| `T3P_PROJECTS` | T3 state DB'den keşif | İzlenecek proje id/yolları, virgülle |
| `T3P_POLL_MS` | `5000` | Sorgu aralığı |
| `T3P_MODEL_DIR` | `~/Library/Application Support/t3-panel/models/vits-piper-tr_TR-dfki-medium` | TTS modeli |
| `T3P_LOG_ROOT` | `~/Library/Logs/t3-panel` | Telemetri kökü |
| `T3P_SAVE_AUDIO` | `1` | Her konuşmayı WAV olarak sakla (`0` kapatır) |
| `T3P_SELFTEST` | — | `1`: ilk bitmiş thread'i dinlemeye alır, ses bitince `mute_all` yapıp kapanır. `reply:<thread_id>`: o thread için Cevapla akışı, `T3P_SELFTEST_STOP_AFTER_MS` (varsayılan `8000`) sonra Gönder; `T3P_SELFTEST_NOSEND=1` ise göndermek yerine `ui.selftest_reply {text, source}` yazar; ReplySent'ten 3 sn sonra ya da 60 sn'de kapanır |
| `T3P_LIVE_POLL_MS` | `1000` | Canlı takip aralığı (dinlenen kanalların yeni mesajları + bekleyen istekler) |
| `T3P_REPLY_PREFIX` | `$sesli ` | Panelden gönderilen her cevabın başına eklenir (boş değer kapatır) |
| `T3P_WISPR_DB` | `~/Library/Application Support/Wispr Flow/flow.sqlite` | Wispr Flow geçmişi (yalnızca okunur) |

## Kanallar

Her thread bir kanal. Radyo gibi: sessiz kanal okunmaz, gelen asistan mesajları yalnızca `unheard` sayacını artırır. Dinlemeye alınınca (`set_listening`) tek bir yakalama cümlesi okunur (son `🔊` satırı, yoksa son mesajın ilk cümlesi, yoksa durum), sonra her yeni asistan mesajı canlı okunur: `🔊` satırı varsa o, yoksa ilk cümle (en çok 160 karakter). Thread kullanıcıyı beklemeye geçerse "senden cevap bekliyor.", durdurulursa "Durduruldu." okunur.

Video gibi: aynı anda tek ses çalar; dinlenen, duraklatılmamış kanallar arasında geliş sırası (FIFO). Duraklatma (`toggle_pause`) cümlenin ortasında tutar, kuyruk dolmaya devam eder, bu arada başka kanallar konuşabilir; devam edilince tutulan ses ilk sıradadır. `skip` çalanı atar, `jump_to_live` çalanı ve birikmişi atar, `mute_all` her şeyi susturur. Kanal değişince veya 20 sn sessizlikten sonra başa thread başlığından kısa bir etiket eklenir (≤30 karakterse başlık, değilse ilk 4 kelime). Ses, çalınacağı an sentezlenir; bir sonraki utterance önceden hazırlanır, cihaz açılırken ilk cümle sentezlenir.

Cevap: `send_reply` metni `T3P_REPLY_PREFIX` ile `t3 thread send <id> - --json`'a stdin'den verir (meşgul thread'de sıradaki tur olur, hiçbir şey kesilmez). Göndermeden hemen önce kanal yakalama cümlesi olmadan dinlemeye geçer; yalnızca gönderimden sonra yazılan mesajlar okunur. Dikte: `dictation_start` `open -g wispr-flow://start-hands-free` çalıştırır ve Wispr'in History tablosunu 250 ms'de bir izler (başlangıçtan yeni ilk final satır → `DictationTranscript`; boş/iptal/5 dk → `DictationFailed`). `dictation_stop` / `dictation_cancel` `stop-hands-free` açar; iptal metni yok sayar.

`open_thread` `https://app.t3.codes/threads/<env>/<thread>` linkini `t3code://threads/<env>/<thread>`'e çevirip `open` eder; tarayıcı değil T3 masaüstü uygulaması açılır.

## İzleme

Her çalıştırma `~/Library/Logs/t3-panel/<YYYYMMDD-HHMMSS-pid>/` altına yazar; `latest` en yenisini gösterir.

- `events.jsonl`: her girdi/çıktı tek satır: `{ts, run, seq, kind, dur_ms, ok, in, out, err}`.
  - T3: `t3.discover`, `t3.changes`, `t3.list`, `t3.messages`, `t3.poll`; her alt süreç için argv, süre, stdout byte, çıkış kodu, child CPU ms, max RSS.
  - Canlı takip: `live.poll` (`threads`, `rows`, `new_msgs`, `waiting`, child CPU), `live.message` (`thread`, `message_id`, `created_at`, `rule`, `action`: `enqueued` | `unheard` | `before_listen` | `empty`, okunacak metin), `wait.detect`.
  - Kanallar: `channel.listen`, `channel.pause`, `channel.skip`, `channel.live`, `channel.mute_all`.
  - Konuşma: `speak.enqueue` (kaynak `catchup` | `live` | `wait` | `stopped`, metin), `tts.load`, `tts.synth` (karakter, ses ms, RTF), `audio.open`, `audio.close`, `audio.play` (utterance başına: `msg_to_audio_ms`, `click_to_audio_ms`, `paused_ms`, `skipped`/`dropped`, `label_spoken`, `prefetched`, wav).
  - Cevap ve dikte: `reply.dictation_start`, `reply.dictation_stop`, `reply.dictation_cancel` (argv, url), `wispr.history` (satır alanları, metin, `e2eLatency`, `latency_from_stop_ms`, `latency_from_start_ms`), `reply.send` (argv, önekli metin, çıkış kodu, stdout + JSON, `queued`).
  - Durum ve arayüz: `state.change`, `thread.open` (`link`, `app_url`), `ui.*`.
- `metrics.jsonl`: saniyede bir RSS, CPU, thread sayısı, kuyruk, child CPU toplamı.
- `audio/<seq>.wav`: okunan her utterance'ın kaydı (etiket dahil; `seq` = `speak.enqueue`).

```bash
target/release/t3p-report latest   # kind başına p50/p95/max, RSS/CPU, live.poll maliyeti, msg→ses, tıklama→ses, dikte gecikmesi, reply.send, kanal eylemleri + report.json + report.html
target/release/t3p-bench           # TTS: soğuk yükleme, cümle başına synth ms / RTF, tepe RSS
target/release/t3p-bench --t3      # tüm projeleri 3 tur listeleme maliyeti
target/release/t3p-bench --e2e     # başsız uçtan uca: ilk bitmiş thread'i dinlemeye alır, yakalama cümlesini okur
```

## Veri kaynağı

- Thread'ler: `t3 thread list --json` (T3'ün resmi CLI'ı). Cevaplar: `t3 thread send <id> - --json`.
- Proje keşfi, değişiklik tespiti, mesajlar ve bekleyen istekler: `~/.t3/userdata/statev2.sqlite`, yalnızca `/usr/bin/sqlite3 -readonly` ile.
  - Her 5 sn'lik tikte tek sorgu (~14 ms) hangi projenin `updated_at`'i değiştiğini bulur; `t3 thread list` sadece onlar için çalışır. 120 sn'de bir tam liste.
  - Canlı takip her saniye tek `sqlite3` çağrısı (~10–25 ms): listelenen thread'lerin `orchestration_v2_projection_messages` tablosundaki yeni, tamamlanmış (`streaming = 0`) asistan mesajları (thread başına `created_at` imleci, `(thread_id, created_at)` indeksi) + `orchestration_v2_projection_turn_items` içindeki `waiting` durumundaki `user_input_request` / `approval_request` / `secret_request` satırları. İmleç `Core::start` anında başlar; geçmiş tekrar okunmaz.
  - Yakalama cümlesi aynı DB'den thread'in son 12 mesajını okur (`t3.messages`, ~6 ms).
- Dikte metni: Wispr Flow `History` tablosu (`flow.sqlite`, yalnızca okunur; `timestamp` UTC metni `YYYY-MM-DD HH:MM:SS.mmm +00:00`).
- Bu tablolar T3'ün ve Wispr'in iç şeması; sürümle değişebilir. Değişiklik sorgusu hata verirse o tik her şeyi listeler ve `t3.changes ok:false` yazar; canlı takip hatası `live.poll ok:false` yazar.

## Agent tarafı

[`sesli`](../cengo-skills/sesli/SKILL.md) skill'i (`~/.agents/skills`, `~/.claude/skills`, `~/.codex/skills`): ara adımlar tek cümle, son mesaj `🔊` (okunacak) ve `👀` (okunmaz, ekranda bakılacak) satırlarıyla biter. Panel önce `🔊` satırını okur.

## Kararlar

- `sherpa-onnx = "=1.13.8"`: 2.0.1 Piper için espeak-ng fonem adımını içermiyor; Türkçe sentez `phoneme_codepoints is empty` hatası veriyor.
- `gpui` `runtime_shaders`: Metal Toolchain (`xcrun metal`) kurulu değil; shader'lar açılışta derleniyor (açılış ~110 ms).
