# Panel için yerel Türkçe ses modeli araştırması (11 Kasım 2026)

Soru: Piper `tr_TR-dfki` yerine bu Mac'te çalışan, daha güzel Türkçe okuyan bir model var mı?

Kaynaklar:
- [landscape.md](landscape.md): 21 Türkçe destekli model, lisans, boyut, Rust yolu, Cactus.
- [community.md](community.md): Reddit, HF, GitHub, Türkçe forum yorumları; ajan seslendirme projeleri.
- [bakeoff.md](bakeoff.md) ve [bakeoff/index.html](bakeoff/index.html): bu Mac'te 59 çalıştırma, 7 panel cümlesi, 413 ses dosyası.
- [compare.html](compare.html): öne çıkan adayların grafiği.
- [listen/](listen/): aday başına 3 cümlelik dinleme dosyası (baştaki sessizlik kesildi, ses seviyesi eşitlendi).

## Karar (11 Kasım 2026)

Cengo 5 (EMA Lightning), 7 (Pocket TTS female_1) ve 8 (ZONOS3-Mini klon) numaralı sesleri beğendi. Panelde **EMA Lightning** kullanılıyor: en küçük (34 MB), Rust içinde ONNX Runtime ile çalışıyor, yan süreç yok. Pocket 1,2 GB ve Python yan süreç, ZONOS 1,8 GB ve rakam okuyamıyor. Canlı kullanımda performansı yeterli bulundu; EMA tek ses olarak kalıyor.

Rust'taki ölçüm (aynı 7 cümle, M4 Pro): soğuk yükleme 0,58 sn, cümle başına 32–252 ms, RTF 0,044, tepe RSS 143 MB, tıklama→ses 247 ms. Whisper geri okumasında 7 cümlenin 6'sı birebir doğru; "Build" yazımı sözlükle "bilt" yapıldıktan sonra o da doğru. Bake-off'taki Python ONNX koşusunda İngilizce kelimeler 3/5'ti; telaffuz sözlüğüyle "pull request", "deploy", "iOS", "PR", "commit", "push", "merge", "backend" doğru duyuluyor.

Dinle: [listen/10-ema-panel-rust.mp3](listen/10-ema-panel-rust.mp3) (panelin kendi Rust motoru).

## Sonuç

| Sıra | Model | Neden | Bedeli |
|---|---|---|---|
| 1 | **Supertonic 3, sherpa-onnx int8** | En az okuma hatası (WER %0–1,8), bu testte rakamları ve PR/deploy/build/iOS'u doğru okudu, 10 hazır ses. Panelin zaten kullandığı `sherpa-onnx =1.13.8` crate'i destekliyor: yeni bağımlılık yok, model 129 MB | Akış (streaming) yok. İlk kelime ~1,4 sn (8 adım) / ~1,0 sn (4 adım); bunun ~0,5 sn'si baştaki sessizlik, kesilebilir. 4 adımda hata artıyor (%9,1). Bellek ~430 MB. Supertone projeyi Temmuz 2026'da arşivledi: ağırlıklar duruyor ama geliştirme yok. Topluluk uzun rakamları bozduğunu bildiriyor ([#31](https://github.com/supertone-oss-archive/supertonic/issues/31), [#144](https://github.com/supertone-oss-archive/supertonic/issues/144)); bizim kısa rakamlarımız doğru çıktı |
| 2 | **EMA Lightning / Ahenk Cep (ONNX, `ort`)** | 8,6M parametre, 35 MB. İlk ses 40–180 ms. Türkçe'ye özel, rakamlar doğru. [Hitabet](https://github.com/mesutde/Hitabet-Turkce-TTS) hazır Rust kodu var | İngilizce kelimeler zayıf ("Build" → "Bu il"). Birer kadın sesi, başka ses yok. WER %5,5. EMA'da "ç"den sonra çıtırtı bildiriliyor ([HF #7](https://huggingface.co/canberkkkkkk/ema-lightning/discussions/7)) |
| 3 | **Pocket TTS Turkish, female_1** | Gerçek akış: ilk ses 25 ms. WER %3,6, rakamlar doğru, 6 ses + duygu etiketi | 1,2 GB bellek, bugün yalnızca Python yan süreçle çalışır |
| Kendi sesin | **ZONOS3-Mini** (Zyphra, 9 Kasım 2026, Apache-2.0) | Sesini klonluyor, C++/Metal sunucu, ilk ses ~0,4 sn | 1,8 GB; Türkçe rakam okuyamıyor (önce normalleştirici şart); 2 günlük model |

Elenenler:
- **Cactus (Cactus Compute):** TTS modeli yok, Türkçe konuşma yok. Ürünleri dil modeli, konuşma tanıma (7 dil, Türkçe yok) ve görüntü. Panelde işe yaramaz.
- **Chatterbox (senin klon sesin dahil):** 5 GB bellek, cümle başına ~11 sn. Videoda iyi, panelde değil. Rakamları okuyamıyor; Resemble Ağustos 2026'da bakımı bıraktı.
- **XTTS-v2:** 18–23 sn açılış, 4 GB, ticari kullanım yok.
- **Anka, VoxCPM2, Audio8, OmniVoice, MOSS-TTS-Nano:** ya çok yavaş/ağır ya Türkçe'si bozuk.

## Ölçülmeyen: doğallık

Kimse sesleri dinlemedi; WER yalnızca "doğru okudu mu" ölçer. Topluluk da aynı uyarıyı yapıyor: Piper otomatik doğallık puanında iyi çıkıyor ama Türk kullanıcılar onu "robotik" buluyor. Karar `listen/` klasöründeki dosyalarla kulakla verilmeli.

## Modelden bağımsız ders: metin normalleştirme

Topluluktaki ortak tavsiye ve ölçümler aynı yeri gösteriyor: rakamlar, kesme işaretli ekler ("20'si") ve İngilizce kelimeler modelden çok ön işlemeye bağlı. Hangi model seçilirse seçilsin panele Türkçe bir normalleştirici eklenmeli (`normalizer-tr`'nin Rust kaynağı ya da Hitabet'teki Rust kodu) ve teknik kelimeler için küçük bir telaffuz sözlüğü tutulmalı.

## Panele geçiş planı (Supertonic seçilirse)

1. `speech.rs`: `OfflineTtsVitsModelConfig` yerine `OfflineTtsSupertonicModelConfig`, `lang="tr"`, ses `sid` ayarı (`T3P_VOICE`).
2. Baştaki sessizliği kes (eşik −45 dB) ve ilk yan cümleyi ayrı üret: hedef ilk ses < 500 ms.
3. Canlı ara adımlar 4–6 adım, 🔊 özetleri 8 adım; telemetriye `steps` ve `lead_silence_ms` ekle.
4. Türkçe normalleştirici + telaffuz sözlüğü.
5. `t3p-bench` ile önce/sonra ölçümü.
