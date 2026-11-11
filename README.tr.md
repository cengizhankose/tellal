# t3-panel

[English](README.md) · **Türkçe**

Her uygulamanın üstünde duran, [T3 Code](https://github.com/pingdotgg/t3code) thread'lerini listeleyen ve tıklanınca durumu Türkçe, lokal sesle okuyan panel. Rust + [GPUI](https://gpui.rs) (Zed'in UI framework'ü) + [EMA Lightning](https://huggingface.co/canberkkkkkk/ema-lightning) (8,6M parametreli Türkçe ses modeli, Apache-2.0) üzerinde ONNX Runtime ([`ort`](https://crates.io/crates/ort)). Ücretli API yok; her şey bu Mac'te çalışır.

![t3-panel demo](docs/media/demo.gif)

*Yalnızca demo verisi ([`demo/fixture.json`](demo/fixture.json)). [Sesli video](docs/media/demo.mp4) · [Ekran görüntüleri](docs/media/)*

## Kurulum

Gerekenler: Apple Silicon Mac, T3 Code masaüstü uygulaması ve `t3` CLI'ı, Rust 1.92+, isteğe bağlı Wispr Flow.

```bash
./scripts/fetch-models.sh          # ~34 MB EMA Lightning ONNX modeli → ~/Library/Application Support/t3-panel/models
cargo build --release --workspace
target/release/t3-panel
```

### Demo modu

T3'e, Wispr'a ya da gerçek veriye dokunmadan denemek için (ses modeli yine gerekir):

```bash
T3P_DEMO=demo/fixture.json target/release/t3-panel
```

Thread'ler, mesajlar, durum değişiklikleri ve cevaplar [`demo/fixture.json`](demo/fixture.json)'dan gelir (3 projede 7 thread; Claude, Codex, omp, Cursor). `t3` CLI çalıştırılmaz, T3'ün state DB'si ve Wispr'in DB'si okunmaz, hiçbir bağlantı açılmaz (`t3p-report`: 0 `t3`, 0 `sqlite3` çağrısı). T3 teması, fixture `"theme": "default"` (yerleşik koyu palet) demedikçe yine okunur. Senaryo başlangıçtan ~6–42 sn sonra oynar (`at_s` = başlangıçtan sonraki saniye, `en` = isteğe bağlı İngilizce altyazı): ara adım mesajları, durum değişiklikleri, `🔊` özeti, onay isteği. Cevapla ~2,5 sn sonra fixture'daki dikteyi doldurur, gönderim yalnızca loglanır; agent ~1,5 sn sonra cevap verir ve thread biter. T3'te açma da yalnızca loglanır. Başsız kontrol: `T3P_DEMO=demo/fixture.json T3P_OUTPUT_VOLUME=0 target/release/t3p-bench --e2e`. README'deki demo videosu ve ekran görüntüleri yalnızca bu veriyle çekildi.

## Panel

Tasarım sözleşmesi ve gerekçeler: [`docs/DESIGN.md`](docs/DESIGN.md). Görünüm T3'ün kenar çubuğunu izler; renkler ve yazı tipi T3'te seçili temadan gelir (`Core::theme()`), tema değişince 200 ms'lik geçişle uygulanır.

- **Pencere:** `WindowKind::PopUp` (`NSPanel`, non-activating, seviye 101, tüm Space'ler), Accessory uygulama (Dock ikonu yok), arka plan `Blurred` + canvas %92. Yükseklik içeriğe göre 160–640 px; değişimler `normal` (200 ms), kompakt geçişi `slow` (320 ms). Başlıktan sürüklenir.
- **Başlık:** T3 yazı stili (kalın `T3` + soluk `Panel`). Panel canlıyken yeşil nokta; değilse hap: `bağlanıyor`, `ses yükleniyor`, `hata`. Sağda ara (⌘K), ölçümler, kompakt (⌘⇧C), kapat.
- **Dikkat kartı:** en az bir thread seni bekliyorsa listenin üstünde `N thread seni bekliyor · projeler →`; tıklayınca sıradaki bekleyen satıra gider (bölümü açar, kaydırır, bir kez vurgular).
- **Liste (T3 düzeni):** Sabitlenmiş, Çalışıyor, Aktif, Ertelendi, Bitti bölümleri; son ikisi kapalı başlar, hepsi tıklayınca açılır/kapanır (oturum boyunca hatırlanır). Sıra `Core::threads()` sırasıdır, etkinlik satırı oynatmaz. Satır 36 px: proje işareti (T3 lucide ikonu ve rengi, ikon yoksa T3'ün iki harfli renkli kutusu, projesizse kesikli kare) · başlık · durum glifi (çalışıyor: kesikli halka + `16dk`/`1sa 9dk` sayacı; seni bekliyor: sarı `!`; görülmemiş bitiş: yeşil ✓; durduruldu: soluk ⏸; hiçbiri dönmez) · `N yeni` · provider işareti (14 px, %60; ACP kayıt işaretinde `OM`/`OF`) · dinle düğmesi (her zaman görünür; açıkken accent dolgu, konuşurken ekolayzır) · zaman (`şimdi`, `5dk`, `3sa`, `2g`). Fareyle üstüne gelince ya da seçiliyken zamanın yerine `Cevapla` ve `↗` (T3'te aç) gelir. Satıra **çift tık** thread'i T3'te açar. Dinlenen satırda o anki cümle, bekleyen satırda `seni bekliyor` ikinci satırda görünür.
- **Şimdi çalıyor:** bir kanal konuşurken / duraklatılmışken / sırası varken altta açılır: `proje · başlık`, cümle (2 satır), ilerleme çubuğu (`now_elapsed_ms / now_total_ms`, konuşurken 150 ms'de bir yenilenir), `N sırada`, ⏸/▶ (P), ⏭ okey geç (→), canlıya geç (L, sıra varsa), hepsini sustur (M).
- **Cevap kutusu (`Cevapla`):** v0.2 akışı aynen: açılınca panel key olur, Wispr Flow dikteyi başlatır, Wispr metni kutuya yapıştırır; 600 ms içinde kutu boşsa Wispr geçmişinden doldurulur. Dikte sürerken Gönder → dikte durur, metin gelince kendiliğinden gönderilir. Başarılıysa kutu kapanır, `✓ Gönderildi` bildirimi çıkar ve önceki uygulama öne alınır; hata kutuda kalır, metin korunur. Başka satırda Cevapla hedefi değiştirir. Yazma/IME, ⌘V, ⌘A, Enter, Esc.
- **Durum satırı ve ölçümler:** bağlantı noktası · `N thread · M dinleniyor` · (çalan yokken dinlenen varsa) Sustur · `ölçümler`. Çekmece: RSS, CPU, T3 sorgu, canlı sorgu, TTS, RTF, tık→ses, mesaj→ses, dikte, kuyruk; RSS ve CPU için 60 sn çubuk grafik; run klasörü (tıkla: Finder) ve global kısayol durumu.
- **Durumlar:** yükleniyor (parıltısız 3 yer tutucu satır), boş (`Şu an açık thread yok`), arama sonucu yok, hata bandı (`Tekrar dene` mümkünse, `Kayıtları aç`, kapat).
- **Kompakt:** ⤡ ya da ⌘⇧C ile yalnız başlık + Şimdi çalıyor.
- **Klavye** (panel key iken): ↑/↓ seçim · Space seçili satırı dinle/sustur (seçim yoksa oynatıcıyı duraklat) · Enter/R cevapla · ⌘O T3'te aç · P duraklat · → geç · L canlıya geç · M hepsini sustur · ⌘K ara (Esc temizler) · ⌘⇧C kompakt · Esc sırayla kutu/arama/çekmece/seçim kapatır. Tek harfli kısayollar metin kutusu odaktayken çalışmaz.
- **Global kısayol:** ⌃⌥Space paneli her uygulamadan gizler/gösterir (gösterirken klavyeyi alır ama uygulamayı öne getirmez). `T3P_HOTKEY` (`ctrl+alt+space` biçimi, `off` kapatır).
- **Hareket:** belirteçler `fast` 120 / `normal` 200 / `slow` 320 / `spring` 240 ms. macOS "Hareketi azalt" açıksa hareketler kalkar, solmalar 120 ms'ye iner, ekolayzır yerine sabit hoparlör. Sürekli hareket yalnız ses çalarken (ekolayzır).
- **İkonlar:** `crates/app/assets/**` (Lucide 0.564.0 tam set, provider işaretleri, logo) derleme sırasında binary'ye gömülür (`build.rs`).
- Her etkileşim `ui.*` olarak `events.jsonl`'a yazılır (`ui.click`/`ui.key`/`ui.hotkey` {action, thread, before/after}, `ui.search`, `ui.theme` {renkler, kontrast oranları}, `ui.reduce_motion`, `ui.error_banner`, …).

## Ortam değişkenleri

| Değişken | Varsayılan | Anlamı |
|---|---|---|
| `T3P_DEMO` | — | Demo fixture yolu (ör. `demo/fixture.json`); yukarıdaki [Demo modu](#demo-modu) |
| `T3P_OUTPUT_VOLUME` | `1.0` | Çalma sesi, `0.0`–`1.0`. Sentez, zamanlama ve WAV kaydı değişmez (`0`: sessiz çalıştırma ve kayıt) |
| `T3P_T3_BIN` | `~/.t3/bin/t3` | T3 CLI |
| `T3P_PROJECTS` | T3 state DB'den keşif | İzlenecek proje id/yolları, virgülle |
| `T3P_POLL_MS` | `5000` | Sorgu aralığı |
| `T3P_MODEL_DIR` | `~/Library/Application Support/t3-panel/models/ema-lightning-onnx` | TTS modeli (`meta.json`, `text.onnx`, `sound.onnx`, `decoder.onnx`) |
| `T3P_TTS_THREADS` | `2` | Ses modelinin ONNX Runtime iş parçacığı sayısı |
| `T3P_LEXICON` | `~/Library/Application Support/t3-panel/lexicon.tsv` | Kullanıcı telaffuz sözlüğü (aşağıya bakın) |
| `T3P_LOG_ROOT` | `~/Library/Logs/t3-panel` | Telemetri kökü |
| `T3P_SAVE_AUDIO` | `1` | Her konuşmayı WAV olarak sakla (`0` kapatır) |
| `T3P_SELFTEST` | — | `1`: ilk bitmiş thread'i dinlemeye alır, ses bitince `mute_all` yapıp kapanır. `reply:<thread_id>`: o thread için Cevapla akışı, `T3P_SELFTEST_STOP_AFTER_MS` (varsayılan `8000`) sonra Gönder; `T3P_SELFTEST_NOSEND=1` ise göndermek yerine `ui.selftest_reply {text, source}` yazar; ReplySent'ten 3 sn sonra ya da 60 sn'de kapanır. `shots`: arayüz durumlarını sırayla gösterir (full, selected, dock (Wispr'siz), drawer, compact, empty_search, error), her biri `ui.selftest_state {state, window_number}` yazar; adım süresi `T3P_SELFTEST_STEP_MS` (varsayılan `2500`); sonunda kapanır. `demo`: demo verisiyle zamanlı gezinti (demo videosu bununla çekilir), `ui.selftest_state` / `ui.selftest_action` yazar |
| `T3P_HOTKEY` | `ctrl+alt+space` | Paneli her uygulamadan gizleyen/gösteren global kısayol (`global-hotkey` biçimi, ör. `cmd+shift+KeyP`); `off` kapatır; sonuç `ui.hotkey` olarak loglanır |
| `T3P_LIVE_POLL_MS` | `1000` | Canlı takip aralığı (dinlenen kanalların yeni mesajları + bekleyen istekler) |
| `T3P_REPLY_PREFIX` | `$sesli ` | Panelden gönderilen her cevabın başına eklenir (boş değer kapatır) |
| `T3P_WISPR_DB` | `~/Library/Application Support/Wispr Flow/flow.sqlite` | Wispr Flow geçmişi (yalnızca okunur) |
| `T3P_T3_APP` | en yeni `T3 Code (Nightly).app` / `T3 Code.app` (`/Applications`, `~/Applications`) | Yerleşik tema paletlerinin okunduğu T3 masaüstü uygulaması |
| `T3P_T3_APP_DATA` | `~/Library/Application Support/t3code-v2` | T3 masaüstü uygulamasının Electron userData klasörü (seçili tema `Local Storage`'da; yalnızca okunur) |

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
  - Konuşma: `speak.enqueue` (kaynak `catchup` | `live` | `wait` | `stopped`, metin), `tts.load` (adım adım yükleme ms'si, model MB, sözlük boyutu), `tts.synth` (yazılan metin, sözlükle değişen hali `respelled`, modelin okuduğu hali `spoken`, parça sayısı, ses ms, RTF), `audio.open`, `audio.close`, `audio.play` (utterance başına: `msg_to_audio_ms`, `click_to_audio_ms`, `paused_ms`, `skipped`/`dropped`, `label_spoken`, `prefetched`, wav).
  - Cevap ve dikte: `reply.dictation_start`, `reply.dictation_stop`, `reply.dictation_cancel` (argv, url), `wispr.history` (satır alanları, metin, `e2eLatency`, `latency_from_stop_ms`, `latency_from_start_ms`), `reply.send` (argv, önekli metin, çıkış kodu, stdout + JSON, `queued`).
  - Durum ve arayüz: `state.change`, `thread.open` (`link`, `app_url`), `theme.resolve`, `ui.*`.
- `metrics.jsonl`: saniyede bir RSS, CPU, thread sayısı, kuyruk, child CPU toplamı.
- `audio/<seq>.wav`: okunan her utterance'ın kaydı (etiket dahil; `seq` = `speak.enqueue`).

```bash
target/release/t3p-report latest   # kind başına p50/p95/max, RSS/CPU, live.poll maliyeti, msg→ses, tıklama→ses, dikte gecikmesi, reply.send, kanal eylemleri + report.json + report.html
target/release/t3p-bench           # TTS: soğuk yükleme, cümle başına synth ms / RTF, tepe RSS; her cümle run klasörüne WAV
target/release/t3p-bench --corpus cumleler.txt   # aynısı, kendi cümlelerinle (satır başına bir cümle)
target/release/t3p-bench --t3      # tüm projeleri 3 tur listeleme maliyeti
target/release/t3p-bench --theme   # T3 temasını bir kez çözer: ThemeView JSON'u, çözüm raporu, kontrast tablosu
target/release/t3p-bench --e2e     # başsız uçtan uca: satır dökümü + iki poll arasında sıra karşılaştırması, ilk bitmiş thread'i dinlemeye alır, yakalama cümlesini okurken 250 ms'de bir ilerleme (elapsed/total) yazar, ~1,2 sn'de 1,5 sn duraklatıp devam ettirir
```

## Veri kaynağı

- Thread'ler: `t3 thread list --json` (T3'ün resmi CLI'ı). Cevaplar: `t3 thread send <id> - --json`.
- Proje keşfi, değişiklik tespiti, mesajlar ve bekleyen istekler: `~/.t3/userdata/statev2.sqlite`, yalnızca `/usr/bin/sqlite3 -readonly` ile.
  - Her 5 sn'lik tikte tek sorgu (~10–25 ms, `t3.changes`) hangi projenin `updated_at`'i değiştiğini bulur; `t3 thread list` sadece onlar için çalışır. 120 sn'de bir tam liste. Aynı sorgu (`UNION ALL`) proje başlığı, ikonu (`projection_projects.project_icon_json`, ör. `{"kind":"lucide","name":"cooking-pot","color":"red"}`) ve sabitlenmesini (`pinned_at`), thread'lerin sabitlenmesini (`payload_json` → `$.pinnedAt`, `$.pinOrderKey`), erteleme bitişini (`$.snoozedUntil`) ve aktif run'ın başlangıcını (`orchestration_v2_projection_runs`, `$.startedAt`; T3'ün "Working 16m" sayacının başladığı an → `ThreadView.running_since`) de getirir.
  - Sıra (`Core::threads()`), T3 kenar çubuğu gibi ve kararlı: önce sabitlenmiş thread'ler (T3'ün `pinOrderKey` sırası), sonra aktifler, en sonda bitmiş (settled) ya da ertelenmiş olanlar; her grupta en yeni `created_at` önce. Etkinlik (`updated_at`) sırayı hiç değiştirmez.
  - Canlı takip her saniye tek `sqlite3` çağrısı (~10–25 ms): listelenen thread'lerin `orchestration_v2_projection_messages` tablosundaki yeni, tamamlanmış (`streaming = 0`) asistan mesajları (thread başına `created_at` imleci, `(thread_id, created_at)` indeksi) + `orchestration_v2_projection_turn_items` içindeki `waiting` durumundaki `user_input_request` / `approval_request` / `secret_request` satırları. İmleç `Core::start` anında başlar; geçmiş tekrar okunmaz.
  - Yakalama cümlesi aynı DB'den thread'in son 12 mesajını okur (`t3.messages`, ~6 ms).
- Dikte metni: Wispr Flow `History` tablosu (`flow.sqlite`, yalnızca okunur; `timestamp` UTC metni `YYYY-MM-DD HH:MM:SS.mmm +00:00`).
- Bu tablolar T3'ün ve Wispr'in iç şeması; sürümle değişebilir. Değişiklik sorgusu hata verirse o tik her şeyi listeler ve `t3.changes ok:false` yazar; canlı takip hatası `live.poll ok:false` yazar.

## Tema senkronu

Panelin renkleri T3 Code masaüstü uygulamasında seçili temayı izler (`Core::theme()`, değişince `CoreEvent::ThemeChanged`). Her şey yalnızca okunur; T3'ün ayarlarına ve dosyalarına yazılmaz.

- **Seçim:** T3'ün `localStorage`'ı (`T3P_T3_APP_DATA/Local Storage/leveldb`, origin `t3code://app`): `t3code:theme` (tema id), `t3code:theme-appearance-mode` (`system` | `light` | `dark`), eski `t3code:theme-follow-system`, açık/koyu için ayrı tema seçimi `t3code:theme-halves:v1`, özel temalar `t3code:themes:v1`. LevelDB dosyaları (`.ldb` tablolar + `.log`) her biri tek `read` ile belleğe alınıp okunur (canlı veritabanı açılmaz/kilitlenmez); snappy blokları ve Chromium'un 1 bayt önekli Latin-1 / UTF-16 değerleri çözülür, her anahtarda en yüksek sıra numarası kazanır. IndexedDB okunmaz: T3'ün tema kodu temaları yalnızca `localStorage`'da tutuyor (paket incelendi).
- **Paletler:** yerleşik temalar (T3 Chat, Grove, Ocean, Ember, Iris) ve `system`/`light`/`dark` için nötr paletler çalışma anında kurulu uygulamanın `app.asar`'ından okunur: arşiv başlığı (JSON) ayrıştırılır, yalnızca web istemcisinin `themePalette-*.js` parçası okunur ve içindeki düz nesne literalleri (`{id, label, appearance, colors, variants}`) çıkarılır; T3 nightly güncellense de çalışır (arşiv mtime/boyutu değişince yeniden okunur). Ortam temaları `~/.t3/userdata/themes/*.json` (dosya adı = id; kısa biçimde `canvas` + `accent`'ten paleti T3'ün üreteci gibi türetir). Arama sırası T3'teki gibi: yerleşik → özel → ortam.
- **Görünüm:** `system` modunda macOS görünümü (`AppleInterfaceStyle`, CFPreferences). Çözüm kuralları T3'ün `useTheme` koduyla aynı (eski id'ler, tek görünümlü temalar kendi görünümünde kalır, açık/koyu ayrı seçimler önceliklidir).
- **Eşleme:** panel T3'ün kenar çubuğunu taklit ettiği için kenar çubuğu jetonları kullanılır: `canvas` = `sidebar`, `surface` = `surface`, `surface_hover` = `sidebarRowHover`, `border` = `sidebarBorder`, `text` = `sidebarForeground`, `text_muted` = `sidebarMutedForeground`, `accent` / `accent_text` = `accent` / `accentForeground`, `warning` / `danger` = `warning` / `error`; `success` / `info` T3'ün sabit Tailwind renkleri (emerald / blue, koyuda 400, açıkta 700). Yazı tipi: `client-settings.json` → `fontFamilySans` (boş = sistem), `fontSizeInterface`.
- **Kontrast:** önce T3'ün Görünüm → Kontrast ayarı (`appearanceContrast`, 50–200) T3'ün CSS'i gibi uygulanır (metin beyaza/siyaha, kenarlık ön plana doğru; ikincil metin hiyerarşi için palet değerinde kalır). Sonra OKLCH açıklığı kaydırılarak tabanlar sağlanır: `text` ≥ 7:1 (canvas/surface/hover), `text_muted` ≥ 4,5:1, `accent_text` ≥ 4,5:1 (accent üstünde), `accent` ≥ 3:1, durum renkleri ≥ 4,5:1 (canvas üstünde). Orijinal ve ayarlanmış değerler loglanır.
- **Yenileme:** 2 sn'de bir ucuz kontrol (leveldb ve tema klasörü dosyalarının boyut/mtime'ı, `client-settings.json`, `app.asar`, sistem görünümü). Bir şey değişirse girdiler yeniden okunur; yalnızca temayla ilgili girdiler değiştiyse yeniden çözülür, görünüm farklıysa `ThemeChanged` gönderilir. İlk çözüm `Core::start` içinde (~10–15 ms), ilk karede gerçek renkler var.
- **Telemetri:** `theme.resolve` `{in: {method: startup|refresh, changed}, out: {source, name, dark, colours, font, contrast: [{fg, bg, ratio, required}], original, after_contrast_setting, adjusted: [{token, from, to, ratio_from, ratio_to}], selection: {stored_theme, appearance_mode, halves, resolved, applied, origin, ...}, bundle, settings, system_dark, sources}, dur_ms}`. Okunamayan girdide `ok:false`, `err` = neden; görünüm `ThemeView::default()` (`source:"default"`) + T3'ün yazı tipi olur.
- `source`: `t3:<id>` (yerleşik/özel tema ya da `t3:system`), `t3-env:<id>` (ortam teması) veya `default`.

## Agent tarafı

[`sesli`](skill/sesli/SKILL.md) skill'i ([kurulum](skill/sesli/README.md): `~/.claude/skills`, `~/.codex/skills`, `~/.omp/agent/skills` ya da `~/.agents/skills`): işe başlarken tek cümle, ara adımlar tek cümle, son mesaj `🔊` (okunacak) ve `👀` (okunmaz, ekranda bakılacak) satırlarıyla biter. Panel önce `🔊` satırını okur.

## Ses

- Motor: EMA Lightning ONNX grafikleri ([`ozcancelik/ema-lightning-onnx`](https://huggingface.co/ozcancelik/ema-lightning-onnx), Apache-2.0) `crates/core/src/ema.rs` içinde Rust'a taşındı: [`normalizer-tr`](https://crates.io/crates/normalizer-tr) (rakam, saat, yüzde, kısaltma) → modelin alfabesi → ≤10 sn'lik parçalar → `text.onnx` → kare planı → `sound.onnx` (4 adım) → `decoder.onnx`. 48 kHz, sonunda 15 ms sönümleme (çıtırtıya karşı).
- Telaffuz sözlüğü (`crates/core/src/lexicon.rs`): model İngilizce kelimeleri Türkçe kurallarla okur ("build" → "bu il"). Sık geçen teknik kelimeler normalleştiriciden önce yeniden yazılır ("pull request" → "pul rikuest", "PR'ı" → "pi ar'ı", "T3" → "ti tri"). Yalnızca okunan metin değişir. Kendi eklemelerin `T3P_LEXICON` dosyasına, satır başına `yazılan<TAB>okunan`; `*` ile biten anahtar çekimli halleri de yakalar (`push*` → "pushladım" = "puşladım"). Dosya yerleşik girdileri ezer; panel yeniden açılınca okunur.
- Ölçüm (M4 Pro, 7 panel cümlesi): soğuk yükleme ~0,6 sn, cümle başına 30–250 ms (RTF ~0,04), tepe RSS ~140–200 MB, tıklama→ses 247 ms. Whisper geri okumasında rakamlar, "pull request", "deploy", "iOS", "Durduruldu" doğru. Ayrıntı: [`research/tts/`](research/tts/README.md).
- Sınır: tek ses (kadın). İngilizce thread başlıkları Türkçe kurallarla okunur.

## Kararlar

- Ses için `ort` `AutoDevicePolicy::PreferCPU`: `ort`'un varsayılanı macOS'ta CoreML'i devreye sokuyor; CoreML ile bellek ~700 MB'a, açılış 2,5 sn'ye çıkıyordu. CPU ile ~150 MB ve 0,6 sn, hız neredeyse aynı.
- sherpa-onnx ve Piper kaldırıldı (eski model klasörü `models/vits-piper-tr_TR-dfki-medium` artık kullanılmıyor, silinebilir).
- `gpui` `runtime_shaders`: derleme Xcode'un ayrıca indirilen Metal Toolchain'ine (`xcrun metal`) ihtiyaç duymasın diye shader'lar açılışta derleniyor (açılış ~110 ms).

## Belgeler ve lisans

- [`docs/DESIGN.md`](docs/DESIGN.md) tasarım sözleşmesi, [`docs/PLAN.md`](docs/PLAN.md) kararlar ve fazlar, [`docs/JOURNEY.md`](docs/JOURNEY.md) projenin hikâyesi, [`research/`](research/README.md) ses ve arayüz araştırması, [`tools/`](tools/README.md) demo medyasının kaydı.
- Lisans: [MIT](LICENSE). Üçüncü taraf lisansları: [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
- T3 / Ping Labs ile bağlantısı yoktur.
