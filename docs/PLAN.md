# t3-panel: endişeler ve çalışma planı

Kaynak: 10 Kasım 2026 isteği (çift tık, uygulamada açma, Cevapla + Wispr, başlarken onay cümlesi, radyo kanalları + video kontrolleri). Durum sütunu v0.2 (`178fb5d` ve sonrası) itibarıyla.

## Endişeler → kararlar

| # | Endişe | Karar | Durum |
|---|---|---|---|
| 1 | Açma butonu yer kaplıyor; thread'e çift tıklayınca açılsın | `⤢` kaldırıldı. Satıra çift tık açar; tek tık yalnızca seçer. Satırdaki butonlara tıklamak çift tık sayılmaz. Hover'da "çift tık: T3'te aç" ipucu | ✅ |
| 2 | Açınca tarayıcıya gidiyor, "Open in App" adımı fazladan | T3'ün kendi deep link'i `t3code://threads/<env>/<thread>` kullanılıyor (masaüstü uygulamasının kaynağından doğrulandı) | ✅ |
| 3 | Panelden nereden konuşacağım? | Altta cevap kutusu: `→ başlık`, metin kutusu, Gönder / İptal | ✅ |
| 4 | Wispr Flow MCP ile bağlansın, daha kaliteli olur | Wispr'ın resmi MCP'si yalnızca Notetaker (toplantı) verisini açıyor, dikteye erişemiyor. Yerine Wispr'ın kendi `wispr-flow://start-hands-free` / `stop-hands-free` bağlantıları + yerel History veritabanı (salt-okunur). Metin Wispr'ın formatlanmış metni, kalite aynı | ✅ |
| 5 | Açma butonu yerine "Cevapla"; basınca Wispr kendiliğinden dinlesin | `💬 Cevapla` → panel öne gelir, kutu odaklanır, Wispr dinlemeye başlar. Bitirme: Caps Lock (kendi kısayolun) ya da Gönder. Gönder dikte sürerken basılırsa dikte durur, metin gelince kendiliğinden gider | ✅ |
| 6 | Wispr metni yanlış uygulamaya yapıştırabilir | Cevap kutusu açıkken panel öne geçer (key pencere); gönderince/iptalde önceki uygulama geri öne alınır. Yapıştırma olmazsa metin History'den doldurulur; ikisi karşılaştırılıp loglanır | ✅ |
| 7 | İşe başlamadan kısa cevap vermeli: "Tamam, şöyle yapıyorum" | `sesli` skill'inde zorunlu kural: ilk tool çağrısından önce tek cümle. Panelden giden her cevabın başına `$sesli` ekleniyor | ✅ (agent uyumu thread'e göre değişir) |
| 8 | Onay bekleyip beklememe | Beklemez, hemen başlar. Belirsiz ya da geri alınamaz işte (silme, push, deploy, para) tek soru sorup bekler | ✅ skill |
| 9 | İş sırasında da konuşmalı | Dinlenen kanaldaki her yeni asistan mesajı canlı okunur (`🔊` satırı, yoksa ilk cümle). Sorgu saniyede bir | ✅ |
| 10 | Müdahale edebilmeliyim | Çift tıkla T3'te aç ve orada durdur: var. Panelden yazılan cevap çalışan işi kesmez, sıradaki tura düşer (T3 CLI'ın sınırı). Çalışırken araya girme (steer) ve durdurma için T3 MCP + OAuth gerekiyor | ⏳ Faz 2 |
| 11 | Kanallar radyo gibi: tıklamak "play" değil "dinliyorum" | Satırda 🔈/🔊 dinle anahtarı. Açınca bir yakalama cümlesi, sonra canlı. Kapalı kanal konuşmaz, kaçırdıkları `N yeni` rozeti | ✅ |
| 12 | Video gibi: durdurabileyim, "okey" deyip geçebileyim | Kanal başına ⏸/▶ (cümlenin ortasından devam eder, gelenler birikir), ⏭ "okey, geç", `N sırada · canlıya geç` | ✅ |
| 13 | "Arkada durdurma" belirsiz | (a) sesi durdurmak: ⏸ ve `hepsini sustur` ile var. (b) agent'ın işini durdurmak: #10 ile aynı, Faz 2 | ⏳ (b) |
| 14 | "Okey" demek sesle mi? | Şimdilik buton. Sesli komut (sürekli dinleyen mikrofon) Faz 3 | ⏳ Faz 3 |
| 15 | Birden çok kanal aynı anda konuşursa karışır | Tek seferde tek ses, gelen sırasıyla. Konuşan kanal değişince ya da 20 sn sessizlikten sonra önce thread başlığı okunur | ✅ |
| 16 | Hızlı yazan agent'ta kuyruk şişer | Satırda kuyruk sayacı + "canlıya geç" | ✅ |
| 17 | Onay/soru bekleyen thread kaçar | `seni bekliyor` rozeti; kanal dinleniyorsa "senden cevap bekliyor" okunur | ✅ (canlı denenmedi) |
| 18 | Panelin klavye kısayolu yok | Paneli aç/kapat, geç, duraklat için global kısayollar | ⏳ Faz 3 |

## Fazlar

### Faz 1 (v0.2, bu sürüm)
1–9, 11, 12, 13a, 15–17. Doğrulananlar:
- Çift tık → `t3code://` → T3 uygulaması (70 ms, tarayıcı açılmadı).
- Wispr uçtan uca: hoparlörden çalınan Türkçe cümle Wispr ile kutuya yapıştı, History'deki metinle birebir aynı, kendiliğinden gönderildi, odak önceki uygulamaya döndü. Gönder → metin 1.19 sn.
- Cevap → agent'ın ilk mesajı → ses: 832 ms.
- Dinlemeye alınca ilk ses: 259–303 ms (v0.1: 377 ms).
- ⏸ → başka kanal konuşur → ▶ cümlenin ortasından devam (2.1 sn bekletildi), ⏭, canlıya geç, hepsini sustur.

### Faz 2: T3'e doğrudan bağlantı (müdahale)
- T3 MCP'ye OAuth ile bir kerelik bağlanma (onay T3 sayfasında).
- Cevap modu: thread çalışıyorsa "araya gir" (steer), değilse sıradaki tur.
- `⏹ Durdur` (yeniden başlat / kes) satırda, onaylı.
- Liste ve mesajları MCP'den almak: her `t3` komutunun ~330 ms CPU maliyeti biter.

### Faz 3: elleri serbest
- Global kısayollar: paneli aç/kapat, ⏸, ⏭, Cevapla (seçili satır).
- Sesli komutlar: "okey / geç / dur / tekrar".
- Paneli sabitle / kenara gizle.

### Faz 4: ses ve kaynak
- ✅ Daha doğal Türkçe ses: 59 denemelik karşılaştırmadan sonra EMA Lightning (ONNX, Rust içinde) seçildi; RSS ~250–300 MB → ~150–200 MB, tıklama→ses ~300 → 247 ms ([research/tts](../research/tts/README.md)).
- Tek ses olarak EMA kalıyor (11 Kasım: canlı kullanımda RSS ort. 173 / en çok 209 MB, CPU ort. %6, cümle başına üretim p50 144 ms). Kalan tek fikir: tamamlanmamış (streaming) mesajın ilk cümlesini erken okumak; canlıda mesaj→ses gecikmesinin 4–6,5 sn'si T3'ün mesajı bitirmesini beklemekten geliyor, ses üretimi ~0,2 sn.

## Bilinen eksikler (v0.2)
- Denenmedi: `N yeni` rozeti, `seni bekliyor`, "Durduruldu." okunması, panelden ⏸/⏭/canlıya geç butonlarına gerçek tık (aynı işlevler sürücüyle denendi), Cevapla'yı açıkken başka satıra taşımak, IME (marked text).
- Başka uygulama tam ekrandayken panelin görünmesi denenmedi.
