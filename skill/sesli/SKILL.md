---
name: sesli
description: Use when the user's message starts with $sesli or asks for spoken, panel-friendly output ("sesli", "sesli anlat", "panelden dinleyeceğim"). Formats the start acknowledgement, progress lines and final reply so T3 Panel can read them aloud with local TTS.
---

# sesli

T3 Panel (`~/dev/t3-panel`) T3 thread'lerini kanal kanal dinletir: Cengo bir thread'in sesini açınca, o thread'e yazdığın her yeni asistan mesajının ilk cümlesi Türkçe, lokal sesle okunur. Konuşmayı panel yapar; sen sadece okunabilir biçimde yazarsın. Ses komutu çalıştırma, ek tool çağırma.

## 1. Başlarken (zorunlu)

İlk tool çağrısından ÖNCE tek mesaj yaz: "Tamam, <ne yapacağım>." En fazla 15 kelime.

- Örnek: "Tamam, önce hataları topluyorum, sonra en sık olanı düzeltiyorum."
- Onay bekleme, hemen başla. Cengo durdurmak isterse panelden yazar ya da thread'i açıp durdurur.
- İstek belirsizse ya da geri alınamaz bir iş varsa (silme, push, deploy, para): tek soru sor ve bekle. Bu durumda mesaj "Başlamadan önce: <soru>?" olur.

## 2. Ara adımlar

- Her yeni aşamanın başında ayrı bir mesajla tek cümlelik durum yaz: Türkçe, en fazla 12 kelime, "Şimdi ... yapıyorum" kalıbı.
- Panel her mesajın sadece ilk cümlesini okur. İlk cümlede kod, dosya yolu, hash, URL, komut olmasın; teknik ayrıntı gerekiyorsa sonraki cümlelere koy.
- Aynı aşamayı ikinci kez duyurma. Sorun çıkarsa onu da tek cümleyle söyle: "Testler kırıldı, nedenini arıyorum."
- Bilgisayarı kullanırken (cua-driver gibi computer use araçları): bir uygulamada iş yapmadan önce tek cümle yaz: "TextEdit'i açıp notu yazıyorum." Her tıklamayı ayrı duyurma; uygulama ya da amaç değişince yeni cümle yaz. Panel bu sırada ekranda "Durdur" düğmesi gösterir; geri alınamaz bir adımdan (gönderme, silme, satın alma) önce sor ve bekle.

## 3. Son mesaj

Yazılı raporu her zamanki gibi yaz. En sona şu iki satırı ekle:

```
🔊 <en fazla 2 cümle, ~200 karakter: sonuç, sayılar, gerekiyorsa tek soru>
👀 <ekranda bakılması gerekenler; yoksa bu satırı yazma>
```

🔊 kuralları:

- Panel bu mesajda sadece 🔊 satırını okur.
- Sayıları rakamla yaz: "80 görevin 20'si takıldı".
- Tek tek sayma, grupla: kaçı bitti, takıldı, hata verdi, uyarıyla bitti.
- Karar gerekiyorsa tek soruyla bitir: "Hatalardan mı başlayalım, takılanlardan mı?"
- Onay ya da bilgi bekliyorsan: "Onayını bekliyorum: <ne için>."
- Kod, dosya yolu, URL, emoji, markdown yok.

👀 kuralları:

- Diff, ekran görüntüsü, tablo, PR, HTML rapor gibi gözle bakılması gereken şeyleri yol ya da linkle listele.
- Panel bu satırı okumaz; Cengo thread'e çift tıklayıp T3'te açınca görür.

## Örnek akış

```
Tamam, 80 görevi tarayıp durumlarına göre grupluyorum.
Şimdi takılan görevlerin loglarını okuyorum.
Şimdi hata verenleri nedenine göre ayırıyorum.
…rapor…
🔊 80 görevin 20'si takıldı, 30'u hata verdi, 30'u uyarıyla bitti. Hatalardan mı başlayalım, takılanlardan mı?
👀 ~/dev/proje/report.html
```
