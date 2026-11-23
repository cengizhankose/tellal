/* ==========================================================================
   TELLAL LANDING PAGE — Interactive Audio Engine, Panel Simulator,
   Live T3 Theme Sync, Bilingual EN/TR Switcher & Acoustic Canvas
   ========================================================================== */

const I18N = {
  en: {
    'nav.demo': 'Demo',
    'nav.voices': 'Voices',
    'nav.features': 'Capabilities',
    'nav.architecture': 'Architecture',
    'nav.install': 'Install',
    'nav.theme': 'Theme',
    'nav.download': 'Download',

    'hero.eyebrow': 'Native Rust + GPUI panel for T3 Code · Apple Silicon',
    'hero.titleStart': 'A',
    'hero.titleAccent': 'voice',
    'hero.titleEnd': 'for your T3 Code agents.',
    'hero.subtitle':
      'Run nine coding agents at once without babysitting tabs. Tellal floats above every macOS Space, narrates live steps and spoken summaries using local neural voices, and lets you reply or stop computer use in one keystroke.',
    'hero.ctaDownload': 'Download Tellal 0.4.0',
    'hero.ctaListen': 'Hear real voice',
    'hero.ctaFilm': '70s walkthrough',
    'hero.specApi': 'cloud API cost',
    'hero.specLatency': 'click → audio',
    'hero.specCpu': 'idle CPU',
    'hero.specVoices': 'local ONNX',
    'hero.floatHint': 'Always on top · Click any channel',

    'panel.screenPill': 'screen',
    'panel.phonePill': 'on phone',
    'panel.cuTitle': 'An agent is using your screen',
    'panel.cuSessions': '3 sessions',
    'panel.cuSub': 'omp · last action: now',
    'panel.cuStop': 'Stop',
    'panel.attnCount': '1 thread waiting for you',
    'panel.cancel': 'Cancel',
    'panel.send': 'Send',
    'panel.readyBadge': 'real audio',
    'panel.playingBadge': 'playing',
    'panel.pausedBadge': 'paused',
    'panel.drawerVoices': 'Voices',
    'panel.ready': 'ready',
    'panel.metrics': 'metrics',
    'panel.replyBtn': 'Reply',
    'panel.secPinned': 'Pinned',
    'panel.secWorking': 'Working',
    'panel.secActive': 'Active',
    'panel.secWaiting': 'waiting for you: needs an approval or an answer',
    'panel.wisprListening': 'Wispr Flow hands-free…',
    'panel.wisprReady': "Wispr's text is in · Enter sends",
    'panel.sentToast': '✓ Sent to T3 thread',
    'panel.stoppedToast': 'Screen control stopped (3 revoked)',
    'panel.mutedToast': 'All channels muted',
    'panel.newTaskLabel': '→ New task · No project',
    'panel.replyLabel': '→ Reply ·',
    'panel.statusThreads': '6 threads',
    'panel.statusListening': 'listening',

    'sim.listen': 'Play channel audio',
    'sim.reply': 'Voice reply',
    'sim.computer': 'Screen-use alert',
    'sim.drawer': 'Metrics',
    'sim.compact': 'Compact',

    'etymology.pos': 'n. · Ottoman',
    'etymology.def':
      'The town crier who walked the streets announcing the news out loud — and to English ears, a <em>tell-all</em>. Now, a quiet floating voice for parallel coding agents.',
    'pillars.p1Title': 'Radio channels, not chat tabs',
    'pillars.p1Body':
      'Unmute any thread for one catch-up line, live step updates, and a crisp spoken summary while full diffs stay in T3.',
    'pillars.p2Title': '100% local on Apple Silicon',
    'pillars.p2Body':
      'Tiny ONNX neural voices run in-process on your CPU. No ElevenLabs keys, no proxies, no per-character billing, nothing leaves your Mac.',
    'pillars.p3Title': 'Calm, native, focus-safe',
    'pillars.p3Body':
      "Built with Rust and Zed's GPUI inside a non-activating NSPanel. Floats over every Space and fullscreen window without stealing focus.",

    'reel.kicker': 'Real macOS captures · Real local voice',
    'reel.titleStart': 'Every workflow, recorded',
    'reel.titleAccent': 'live.',
    'reel.sub':
      'Zero motion-graphics fakes. Seven unedited captures of Tellal running on Apple Silicon — click any chapter and toggle sound to hear the actual local TTS engine.',
    'reel.unmute': 'Unmute audio',
    'reel.muted': 'Audio on',
    'reel.fullDemo': 'Full 70s walkthrough (1080p)',

    'voices.kicker': 'In-process ONNX Runtime · CPU Provider',
    'voices.titleStart': 'Two neural voices.',
    'voices.titleAccent': 'Zero cloud bills.',
    'voices.sub':
      'Selected from a 59-run Whisper read-back bake-off across 21 open models. Both engines are ported straight to Rust, SHA-256 verified, and auto-switched per utterance with TELLAL_VOICE=auto.',
    'voices.audition': 'Audition',
    'voices.stop': 'Stop',

    'bento.kicker': 'Crafted down to the last pixel & packet',
    'bento.titleStart': 'Every detail built for',
    'bento.titleAccent': 'deep flow.',
    'bento.cuBadge': 'cua-driver · Safety First',
    'bento.cuTitle': 'See when an agent touches your screen. Kill it from anywhere.',
    'bento.cuDesc':
      'When an MCP agent drives macOS via cua-driver, a persistent crimson banner tracks live sessions and last action time — even in compact mode. Hit Stop or press the global shortcut from any app to revoke every session instantly.',
    'bento.cuHint': 'Click the keycap combo to test instant revocation (Carbon hotkey — zero Accessibility permission required).',
    'bento.mobBadge': 'Tailscale Tailnet · HTTPS / WSS',
    'bento.mobTitle': 'Walk away from your desk. Take the voice on your phone.',
    'bento.mobDesc':
      "Publish Tellal's loopback server with one Tailscale command. Tap the speaker on your phone and your Mac goes silent automatically. Reply with keyboard dictation or push-to-talk over BlackHole → Wispr Flow.",
    'bento.phoneOn': 'On phone',
    'bento.phoneOff': 'On Mac',
    'bento.ptt': 'Tap and talk · WSS → BlackHole',
    'bento.macSilent': 'Mac muted · Tailscale-User-Login verified',
    'bento.macActive': 'Playing on Mac · Phone standing by',
    'bento.skillBadge': 'Agent Protocol · $aloud & $sesli',
    'bento.skillTitle': 'Agents write for the ear, not a wall of logs.',
    'bento.withSkill': 'With $aloud',
    'bento.withoutSkill': 'Without',
    'bento.themeBadge': 'app.asar + LevelDB · 200ms Cross-fade',
    'bento.themeTitle': 'Looks like a detached piece of T3 Code.',
    'bento.themeDesc':
      "Reads your active built-in, custom, or environment theme directly from T3's localStorage and app.asar, then enforces OKLCH WCAG contrast floors automatically. Click any theme below — this entire page follows live.",
    'bento.kbBadge': 'Zero mouse required · Global Hotkeys',
    'bento.kbTitle': 'Every action lives under your fingers.',
    'bento.kbDefault': 'Hover or tap any keycap to inspect its command.',

    'kb.hotkey': 'Show / hide panel',
    'kb.listen': 'Listen / mute',
    'kb.reply': 'Voice reply',
    'kb.newtask': 'New voice task',
    'kb.open': 'Open in T3',
    'kb.pause': 'Pause voice',
    'kb.skip': 'Okay, next',
    'kb.live': 'Jump to live',
    'kb.mute': 'Mute all',
    'kb.search': 'Filter threads',
    'kb.compact': 'Compact mode',
    'kb.stop': 'Kill screen use',

    'arch.kicker': 'Measured on Apple M4 Pro · Full local telemetry',
    'arch.titleStart': 'Pure Rust under the hood.',
    'arch.titleAccent': 'Measured to the millisecond.',
    'arch.inputs': 'Read-Only Inputs',
    'arch.surfaces': 'Surfaces',

    'bench.b1Title': 'Click → first audio',
    'bench.b1Sub': 'Synthesizes while opening audio device',
    'bench.b2Title': 'Synthesis RTF',
    'bench.b2Sub': '30–250 ms per sentence on CPU',
    'bench.b3Title': '60s idle CPU',
    'bench.b3Sub': 'Down 75% via read-only SQLite change ticks',
    'bench.b4Title': 'Memory while speaking',
    'bench.b4Sub': 'Forced CPU provider vs ~700 MB CoreML',

    'story.badge': 'Open Source · MIT License',
    'story.title': 'Built in 48 hours inside a single T3 Code thread.',
    'story.desc':
      'Dictated in Turkish with Wispr Flow, planned and built by coding agents, backed by a 59-run TTS bake-off, 28 T3 sidebar findings, and evidence-based UI rules. Open-sourced with its full design spec and benchmark reports.',
    'story.journey': 'How it was built →',
    'story.design': 'UI contract & measurements →',
    'story.bakeoff': '59-run TTS bake-off →',

    'install.kicker': 'Apple Silicon · macOS 13.4+ · Developer ID Signed & Notarized',
    'install.titleStart': 'Up and listening in',
    'install.titleAccent': 'thirty seconds.',
    'install.tabDmg': 'Signed DMG',
    'install.tabSource': 'Build / Demo Mode',
    'install.tabMobile': 'Phone via Tailscale',
    'install.dmgMeta': 'Apple Silicon · macOS 13.4+ · Signed & Notarized by Apple · MIT',
    'install.downloadLatest': 'Download Latest Release',
    'install.s1Title': 'Drag to Applications & launch',
    'install.s1Body': 'Tellal floats above every Space with no Dock icon. Toggle anytime with ⌃⌥Space.',
    'install.s2Title': 'Voice downloads automatically',
    'install.s2Body': 'Downloads your language voice once (35 MB Turkish, 96 MB English), SHA-256 verified.',
    'install.s3Title': 'Unmute a thread & install $aloud / $sesli',
    'install.s3Body': 'Add the skill to Claude Code, Codex, or omp so agents emit crisp spoken summaries.',
    'install.copy': 'Copy',
    'install.copied': 'Copied ✓',
    'install.mobileFoot':
      'Open the metrics drawer in Tellal → Mobile → click your https://<mac>.<tailnet>.ts.net:8455/ link to copy it. Requests without your Tailscale-User-Login are rejected with 403.',

    'footer.tagline': 'A voice for your T3 Code agents. Open source, local-first, built in Rust + GPUI.',
    'footer.releases': 'Releases',
    'footer.design': 'Design Spec',
    'footer.journey': 'Journey',
    'footer.credits': 'Built with GPUI, ONNX Runtime, EMA Lightning & Supertonic 2 · Not affiliated with T3 / Ping Labs.',
    'modal.title': 'Tellal — Full 70s Walkthrough (with local voice audio)',
  },

  tr: {
    'nav.demo': 'Demo',
    'nav.voices': 'Sesler',
    'nav.features': 'Özellikler',
    'nav.architecture': 'Mimari',
    'nav.install': 'Kurulum',
    'nav.theme': 'Tema',
    'nav.download': 'İndir',

    'hero.eyebrow': 'T3 Code için Rust + GPUI yerel macOS paneli · Apple Silicon',
    'hero.titleStart': 'T3 Code ajanlarına',
    'hero.titleAccent': 'yerel bir ses.',
    'hero.titleEnd': '',
    'hero.subtitle':
      'Sekmeler arasında boğulmadan dokuz ajanı aynı anda yönet. Tellal tüm macOS ekranlarının üstünde süzülür, canlı adımları ve sesli özetlerini yerel nöral seslerle okur; tek tuşla sesli cevap ver ya da ekran kontrolünü durdur.',
    'hero.ctaDownload': "Tellal 0.4.0'ı İndir",
    'hero.ctaListen': 'Gerçek sesi dinle',
    'hero.ctaFilm': '70 sn tam tur',
    'hero.specApi': 'bulut API ücreti',
    'hero.specLatency': 'tıklama → ses',
    'hero.specCpu': 'boşta CPU',
    'hero.specVoices': 'yerel ONNX',
    'hero.floatHint': 'Her zaman üstte · Bir kanala tıkla',

    'panel.screenPill': 'ekran',
    'panel.phonePill': 'telefonda',
    'panel.cuTitle': 'Ajan ekranını kullanıyor',
    'panel.cuSessions': '3 oturum',
    'panel.cuSub': 'omp · son işlem: şimdi',
    'panel.cuStop': 'Durdur',
    'panel.attnCount': '1 thread seni bekliyor',
    'panel.cancel': 'İptal',
    'panel.send': 'Gönder',
    'panel.readyBadge': 'gerçek ses',
    'panel.playingBadge': 'çalıyor',
    'panel.pausedBadge': 'duraklatıldı',
    'panel.drawerVoices': 'Sesler',
    'panel.ready': 'hazır',
    'panel.metrics': 'ölçümler',
    'panel.replyBtn': 'Cevapla',
    'panel.secPinned': 'Sabitlenmiş',
    'panel.secWorking': 'Çalışıyor',
    'panel.secActive': 'Aktif',
    'panel.secWaiting': 'seni bekliyor: onay ya da cevap gerekli',
    'panel.wisprListening': 'Wispr Flow dinliyor…',
    'panel.wisprReady': 'Wispr metni geldi · Enter gönderir',
    'panel.sentToast': '✓ T3 thread’ine gönderildi',
    'panel.stoppedToast': 'Ekran kontrolü durduruldu (3 oturum)',
    'panel.mutedToast': 'Tüm kanallar susturuldu',
    'panel.newTaskLabel': '→ Yeni görev · Projesiz',
    'panel.replyLabel': '→ Cevapla ·',
    'panel.statusThreads': '6 thread',
    'panel.statusListening': 'dinleniyor',

    'sim.listen': 'Kanal sesini çal',
    'sim.reply': 'Sesle cevapla',
    'sim.computer': 'Ekran uyarısı',
    'sim.drawer': 'Ölçümler',
    'sim.compact': 'Kompakt',

    'etymology.pos': 'i. · Osmanlıca',
    'etymology.def':
      'Sokak sokak dolaşıp haberleri yüksek sesle duyuran münadi — İngilizce okuyanlar içinse <em>tell-all</em>. Şimdi paralel çalışan kodlama ajanların için sakin, yüzen bir ses.',
    'pillars.p1Title': 'Sekme değil, radyo kanalı',
    'pillars.p1Body':
      'İstediğin thread’in sesini aç: tek yakalama cümlesi, canlı adım anlatımı ve iş bitince tek cümlelik sesli özet; uzun raporlar T3’te kalır.',
    'pillars.p2Title': 'Apple Silicon’da %100 yerel',
    'pillars.p2Body':
      'Küçük ONNX ses modelleri doğrudan CPU’da çalışır. ElevenLabs anahtarı yok, bulut maliyeti yok, Mac’inden tek bayt çıkmaz.',
    'pillars.p3Title': 'Sakin, native, odak dostu',
    'pillars.p3Body':
      'Rust ve Zed’in GPUI kütüphanesiyle odak çalmayan NSPanel olarak yazıldı. Tam ekran dahil tüm Space’lerin üstünde süzülür.',

    'reel.kicker': 'Gerçek macOS kayıtları · Gerçek yerel ses',
    'reel.titleStart': 'Her özellik, canlı',
    'reel.titleAccent': 'kaydedildi.',
    'reel.sub':
      'Sahte animasyon yok. Apple Silicon üzerinde çalışan gerçek Tellal’ın 7 kaydı — herhangi bir sahneyi seç ve yerel EMA Lightning Türkçe sesini duymak için sesi aç.',
    'reel.unmute': 'Sesi aç',
    'reel.muted': 'Ses açık',
    'reel.fullDemo': '70 sn tam gezinti (1080p)',

    'voices.kicker': 'Gömülü ONNX Runtime · CPU Motoru',
    'voices.titleStart': 'İki nöral ses.',
    'voices.titleAccent': 'Sıfır API faturası.',
    'voices.sub':
      '21 açık kaynak modelde 59 turluk Whisper geri okuma testinden seçildi. İki motor da Rust’a taşındı, SHA-256 doğrulamalı ve TELLAL_VOICE=auto ile her cümlede dilini kendi seçer.',
    'voices.audition': 'Dinle',
    'voices.stop': 'Durdur',

    'bento.kicker': 'Her piksel, her paket özenle tasarlandı',
    'bento.titleStart': 'Akışını bozmayan',
    'bento.titleAccent': 'enstrüman.',
    'bento.cuBadge': 'cua-driver · Önce Güvenlik',
    'bento.cuTitle': 'Ajan ekranına dokunduğunda gör. Her uygulamadan durdur.',
    'bento.cuDesc':
      'Bir MCP ajanı cua-driver ile macOS ekranını yönetirken kırmızı güvenlik bandı canlı oturumları gösterir — kompakt modda bile. Durdur’a tıkla ya da her uygulamadan ⌃⌥⎋ ile tüm oturumları anında kes.',
    'bento.cuHint': 'Anında iptali test etmek için tuş kombinasyonuna tıkla (Carbon kısayolu — Erişilebilirlik izni gerektirmez).',
    'bento.mobBadge': 'Tailscale Tailnet · HTTPS / WSS',
    'bento.mobTitle': 'Masadan kalk. Ajanların sesini telefonuna al.',
    'bento.mobDesc':
      'Tellal’ın loopback sunucusunu tek Tailscale komutuyla yayınla. Telefondaki hoparlöre dokun, Mac bilerek sussun. Klavye diktesiyle ya da BlackHole → Wispr Flow üzerinden bas-konuşla cevap ver.',
    'bento.phoneOn': 'Telefonda',
    'bento.phoneOff': 'Mac’te',
    'bento.ptt': 'Dokun ve konuş · WSS → BlackHole',
    'bento.macSilent': 'Mac susturuldu · Tailscale-User-Login doğrulandı',
    'bento.macActive': 'Mac’te çalıyor · Telefon beklemede',
    'bento.skillBadge': 'Ajan Protokolü · $sesli & $aloud',
    'bento.skillTitle': 'Ajanlar ekran duvarı değil, kulak için yazar.',
    'bento.withSkill': '$sesli ile',
    'bento.withoutSkill': 'Normal',
    'bento.themeBadge': 'app.asar + LevelDB · 200ms Geçiş',
    'bento.themeTitle': 'T3 Code’dan kopmuş bir parça gibi.',
    'bento.themeDesc':
      'T3’te seçili yerleşik, özel veya ortam temanı doğrudan localStorage ve app.asar’dan okur; OKLCH WCAG kontrast tabanlarını garanti eder. Aşağıdan bir tema seç — tüm bu sayfa canlı dönüşsün.',
    'bento.kbBadge': 'Faresiz kullanım · Global Kısayollar',
    'bento.kbTitle': 'Her komut parmaklarının altında.',
    'bento.kbDefault': 'Komutunu görmek için bir tuşun üzerine gel ya da tıkla.',

    'kb.hotkey': 'Paneli göster / gizle',
    'kb.listen': 'Dinle / sustur',
    'kb.reply': 'Sesle cevapla',
    'kb.newtask': 'Yeni sesli görev',
    'kb.open': 'T3’te aç',
    'kb.pause': 'Sesi duraklat',
    'kb.skip': 'Okey, geç',
    'kb.live': 'Canlıya geç',
    'kb.mute': 'Hepsini sustur',
    'kb.search': 'Thread ara',
    'kb.compact': 'Kompakt mod',
    'kb.stop': 'Ekranı durdur',

    'arch.kicker': 'Apple M4 Pro üzerinde ölçüldü · Tam yerel telemetri',
    'arch.titleStart': 'Kaputun altında saf Rust.',
    'arch.titleAccent': 'Milisaniyesine kadar ölçülü.',
    'arch.inputs': 'Salt-Okunur Kaynaklar',
    'arch.surfaces': 'Yüzeyler',

    'bench.b1Title': 'Tıklama → ilk ses',
    'bench.b1Sub': 'Ses aygıtı açılırken ilk cümle sentezlenir',
    'bench.b2Title': 'Sentez RTF',
    'bench.b2Sub': 'CPU’da cümle başına 30–250 ms',
    'bench.b3Title': '60 sn boşta CPU',
    'bench.b3Sub': 'Salt-okunur SQLite sorgusuyla %75 daha hafif',
    'bench.b4Title': 'Konuşurken bellek',
    'bench.b4Sub': 'CoreML (~700 MB) yerine zorunlu CPU motoru',

    'story.badge': 'Açık Kaynak · MIT Lisansı',
    'story.title': 'Tek bir T3 Code thread’inde 48 saatte üretildi.',
    'story.desc':
      'Her istek Wispr Flow ile Türkçe dikte edildi; kodlama ajanları planlayıp parçalara böldü. 59 turluk TTS karşılaştırması, 28 T3 kenar çubuğu bulgusu ve kanıta dayalı arayüz kurallarıyla tüm geçmişiyle açık kaynak paylaşıldı.',
    'story.journey': 'Nasıl üretildi →',
    'story.design': 'Tasarım sözleşmesi & ölçümler →',
    'story.bakeoff': '59 turluk TTS testi →',

    'install.kicker': 'Apple Silicon · macOS 13.4+ · Developer ID İmzalı & Notarize',
    'install.titleStart': 'Otuz saniyede kur,',
    'install.titleAccent': 'dinlemeye başla.',
    'install.tabDmg': 'İmzalı DMG',
    'install.tabSource': 'Kaynak / Demo Modu',
    'install.tabMobile': 'Tailscale ile Telefon',
    'install.dmgMeta': 'Apple Silicon · macOS 13.4+ · Apple Notarize & İmzalı · MIT',
    'install.downloadLatest': 'Son Sürümü İndir',
    'install.s1Title': 'Uygulamalar’a sürükle ve aç',
    'install.s1Body': 'Dock ikonu olmadan tüm Space’lerin üstünde süzülür. ⌃⌥Space ile aç/kapa.',
    'install.s2Title': 'Ses otomatik iner',
    'install.s2Body': 'Dilinin ses modeli tek seferde iner (Türkçe 35 MB, İngilizce 96 MB), SHA-256 doğrulamalı.',
    'install.s3Title': 'Kanalı aç & $sesli / $aloud ekle',
    'install.s3Body': 'Skill’i Claude Code, Codex veya omp’ye ekle; ajanlar net sesli özetler yazsın.',
    'install.copy': 'Kopyala',
    'install.copied': 'Kopyalandı ✓',
    'install.mobileFoot':
      'Tellal’da ölçümler çekmecesi → Mobil bölümündeki https://<mac>.<tailnet>.ts.net:8455/ linkine tıkla. Tailscale-User-Login taşımayan her istek 403 ile reddedilir.',

    'footer.tagline': 'T3 Code ajanlarına yerel bir ses. Açık kaynak, gizlilik öncelikli, Rust + GPUI.',
    'footer.releases': 'Sürümler',
    'footer.design': 'Tasarım Spec',
    'footer.journey': 'Hikâye',
    'footer.credits': 'GPUI, ONNX Runtime, EMA Lightning & Supertonic 2 ile · T3 / Ping Labs ile bağlantısı yoktur.',
    'modal.title': 'Tellal — 70 sn Tam Gezinti (gerçek yerel ses ile)',
  },
};

/* Thread data mirrored directly from demo/fixture.json */
const SIM_THREADS = [
  {
    id: 't-release',
    section: 'pinned',
    project: 'field-app',
    icon: 'phone',
    color: 'emerald',
    title: { en: 'Prepare release notes', tr: 'Prepare release notes' },
    status: 'done',
    time: { en: '50m', tr: '50dk' },
    audioKey: 'new-task',
    caption: {
      en: 'The release notes are ready: 9 new features and 23 bug fixes.',
      tr: 'Yeni sürümün notları hazır: 9 yeni özellik ve 23 hata düzeltmesi var.',
    },
    replyDictation: {
      en: 'Great, tag v0.4.0 and publish the draft release.',
      tr: 'Harika, v0.4.0 etiketini bas ve taslak sürümü yayınla.',
    },
  },
  {
    id: 't-backfill',
    section: 'working',
    project: 'billing-api',
    icon: 'server',
    color: 'violet',
    title: { en: 'Backfill currency on legacy invoices', tr: 'Backfill currency on legacy invoices' },
    status: 'waiting',
    unheard: { en: '1 new', tr: '1 yeni' },
    time: { en: 'now', tr: 'şimdi' },
    audioKey: 'attention',
    caption: {
      en: "The dry run is clean: no conflicts across 18 thousand invoices. Waiting for your approval.",
      tr: 'Deneme çalıştırması temiz, 18 bin faturada çakışma yok. Göç için onayını bekliyorum.',
    },
    replyDictation: {
      en: 'Approved. Take a backup first, then run the migration.',
      tr: 'Onaylıyorum. Önce yedek al, sonra göçü çalıştır.',
    },
  },
  {
    id: 't-notes',
    section: 'working',
    project: 'No project',
    icon: 'np',
    color: 'amber',
    title: { en: 'Write meeting notes in TextEdit', tr: 'Write meeting notes in TextEdit' },
    status: 'working',
    time: { en: '15m', tr: '15dk' },
    audioKey: 'new-task',
    caption: {
      en: 'Okay, moving 37 screenshots into 9 date folders on the desktop.',
      tr: 'Tamam, masaüstündeki ekran görüntülerini tarihlerine göre klasörlere taşıyacağım.',
    },
    replyDictation: {
      en: 'Add the launch checklist at the bottom too.',
      tr: 'En alta lansman kontrol listesini de ekle.',
    },
  },
  {
    id: 't-checkout',
    section: 'active',
    project: 'acme-web',
    icon: 'globe',
    color: 'sky',
    title: { en: 'Fix flaky checkout tests', tr: 'Fix flaky checkout tests' },
    status: 'done',
    time: { en: 'now', tr: 'şimdi' },
    audioKey: 'listen',
    caption: {
      en: 'Found it: the test clicks Pay before the cart loads. Fixed — 48 of 48 passing.',
      tr: 'Sebebi buldum: test sepet yüklenmeden ödeme butonuna tıklıyor. 48 testin hepsi geçti.',
    },
    replyDictation: {
      en: 'Great, push the change and open a PR.',
      tr: 'Süper, değişikliği gönder ve bir PR aç.',
    },
  },
  {
    id: 't-batch',
    section: 'active',
    project: 'acme-web',
    icon: 'globe',
    color: 'sky',
    title: { en: 'Run 80 refactor tasks in a batch', tr: 'Run 80 refactor tasks in a batch' },
    status: 'done',
    unheard: { en: 'summary', tr: 'özet' },
    time: { en: '2m', tr: '2dk' },
    audioKey: 'summary',
    caption: {
      en: 'Of the 80 tasks, 20 got stuck, 30 failed, the rest finished with warnings. Shall we start with the errors?',
      tr: "80 görevin 20'si takıldı, 30'u hata verdi, kalanı uyarıyla bitti. Hatalardan mı başlayalım?",
    },
    replyDictation: {
      en: 'Start with the 30 failed ones and group by error.',
      tr: 'Hata veren 30 görevle başla, sebeplerine göre grupla.',
    },
  },
  {
    id: 't-onboarding',
    section: 'active',
    project: 'field-app',
    icon: 'phone',
    color: 'emerald',
    title: { en: 'Redesign onboarding screens', tr: 'Redesign onboarding screens' },
    status: 'idle',
    time: { en: '6m', tr: '6dk' },
    audioKey: 'reply',
    caption: {
      en: 'Okay, backing up the database first. Once done I will run the migration and report back.',
      tr: 'Tamam, önce veritabanının yedeğini alıyorum. Bitince sonucu sana haber vereceğim.',
    },
    replyDictation: {
      en: 'Looks super clean — open a PR with the dark mode screenshots.',
      tr: 'Çok temiz olmuş — karanlık mod ekran görüntüleriyle PR aç.',
    },
  },
];

const REEL_CLIPS = [
  {
    id: 'listen',
    num: '01',
    title: { en: 'Listen to a thread', tr: 'Bir thread’i dinle' },
    short: { en: 'Catch-up line, then live steps as the agent works', tr: 'Yakalama cümlesi, sonra canlı adımlar' },
    desc: {
      en: 'Unmute any thread like a radio channel: first one catch-up sentence, then one concise sentence per step while the agent works.',
      tr: 'İstediğin thread’i radyo kanalı gibi aç: önce tek yakalama cümlesi, sonra ajan çalıştıkça her adım tek cümle.',
    },
  },
  {
    id: 'summary',
    num: '02',
    title: { en: 'Hear the spoken outcome', tr: 'Sesli özeti duy' },
    short: { en: '80-task batch condensed to one spoken sentence', tr: '80 görevlik iş tek cümlelik sesli özette' },
    desc: {
      en: 'When a run finishes you hear its one-sentence spoken summary (“Of the 80 tasks, 20 got stuck, 30 failed…”); the full report stays in T3.',
      tr: 'İş bitince tek cümlelik sesli özetini duyarsın (“80 görevin 20’si takıldı, 30’u hata verdi…”); uzun rapor T3’te kalır.',
    },
  },
  {
    id: 'attention',
    num: '03',
    title: { en: 'Know when an agent needs you', tr: 'Ajan seni beklediğinde bil' },
    short: { en: 'Attention card raises & jumps straight to approvals', tr: 'Dikkat kartı açılır ve onay bekleyene gider' },
    desc: {
      en: 'Any thread waiting for an answer or approval raises the amber attention card at the top; one click jumps and highlights it.',
      tr: 'Cevap ya da onay bekleyen thread üstte dikkat kartını açar; tek tıkla bölümü açıp satıra gider.',
    },
  },
  {
    id: 'reply',
    num: '04',
    title: { en: 'Reply by voice', tr: 'Sesle cevap ver' },
    short: { en: 'Wispr Flow hands-free dictation → t3 thread send', tr: 'Wispr Flow eller serbest dikte → t3 thread send' },
    desc: {
      en: 'Reply opens the dock, triggers Wispr Flow hands-free dictation, queues your message into T3, and reads back the agent’s acknowledgement.',
      tr: 'Cevapla kutuyu açar, Wispr Flow dikteyi başlatır, metni T3 thread’ine iletir ve ajanın sesli onayını okur.',
    },
  },
  {
    id: 'new-task',
    num: '05',
    title: { en: 'Start a task by voice', tr: 'Sesle görev başlat' },
    short: { en: 'Press N, dictate, Send — already tuned in', tr: 'N’ye bas, dikte et, Gönder — zaten dinliyor' },
    desc: {
      en: 'Hit + (or N), dictate a new task, and send: Tellal creates the thread via t3 thread start and tunes into it automatically.',
      tr: '+ (veya N), dikte et, Gönder: yeni T3 thread’i başlar ve panel onu kendiliğinden dinlemeye alır.',
    },
  },
  {
    id: 'computer',
    num: '06',
    title: { en: 'See and stop computer use', tr: 'Bilgisayar kullanımını durdur' },
    short: { en: 'Live cua-driver banner + ⌃⌥⎋ global kill switch', tr: 'Canlı cua-driver bandı + ⌃⌥⎋ acil durdurma' },
    desc: {
      en: 'While any MCP agent drives your screen through cua-driver, a crimson banner shows live sessions; Stop or ⌃⌥⎋ revokes all of them instantly.',
      tr: 'Bir ajan cua-driver ile ekranı kullanırken bant canlı oturumları gösterir; Durdur veya her uygulamadan ⌃⌥⎋ hepsini anında keser.',
    },
  },
  {
    id: 'theme',
    num: '07',
    title: { en: 'Live T3 theme sync', tr: 'Canlı T3 tema senkronu' },
    short: { en: 'Dark, light, and custom T3 themes follow in 200ms', tr: 'Açık, koyu ve özel T3 temaları 200ms’de izlenir' },
    desc: {
      en: 'Colors and typography mirror whatever theme you pick in T3 Code — light or dark, built-in or custom — with enforced WCAG contrast floors.',
      tr: 'Renkler ve yazı tipi T3 Code’da seçili temayı açık ya da koyu, canlı izler; WCAG kontrast tabanları otomatik korunur.',
    },
  },
];

const KB_DESCRIPTIONS = {
  en: {
    hotkey: 'Global ⌃⌥Space — Show or hide Tellal from any macOS app or fullscreen Space.',
    listen: 'Space — Toggle radio listening on the selected thread (or pause/resume player).',
    reply: 'Enter / R — Open the Reply dock and trigger Wispr Flow hands-free dictation.',
    newtask: 'N — Open New Task dock, dictate a prompt, and start listening immediately.',
    open: '⌘O — Jump directly into the thread inside the T3 Code desktop app (t3code://).',
    pause: 'P — Pause mid-sentence while incoming agent steps keep queuing cleanly.',
    skip: '→ — Skip the current utterance (“okay, next”) and move to the next queued item.',
    live: 'L — Jump to live, clearing older queued messages to hear the latest update.',
    mute: 'M — Mute all channels immediately and return to silence.',
    search: '⌘K — Instant search across thread titles and projects (matches T3’s shortcut).',
    compact: '⌘⇧C — Collapse to a whisper-thin header + Now Playing bar.',
    stop: 'Global ⌃⌥⎋ (or ⌘. inside panel) — Immediately run cua-driver revoke --all.',
  },
  tr: {
    hotkey: 'Global ⌃⌥Space — Her uygulamadan ya da tam ekran Space’ten paneli göster/gizle.',
    listen: 'Space — Seçili thread’i dinlemeye al/sustur (seçim yoksa oynatıcıyı duraklat).',
    reply: 'Enter / R — Cevap kutusunu aç ve Wispr Flow eller serbest dikteyi başlat.',
    newtask: 'N — Yeni görev kutusunu aç, sesle söyle, thread oluşur oluşmaz dinle.',
    open: '⌘O — Thread’i doğrudan T3 Code masaüstü uygulamasında aç (t3code://).',
    pause: 'P — Cümle ortasında duraklat; gelen adımlar kuyrukta birikmeye devam eder.',
    skip: '→ — Çalan cümleyi atla (“okey, geç”) ve sıradakine geç.',
    live: 'L — Canlıya geç; biriken eski mesajları atlayıp en son adımı duy.',
    mute: 'M — Tüm kanalları anında sustur.',
    search: '⌘K — Thread başlıkları ve projelerde anında filtrele (T3 kısayoluyla aynı).',
    compact: '⌘⇧C — Sadece başlık + Şimdi çalıyor çubuğuna küçült.',
    stop: 'Global ⌃⌥⎋ (panelde ⌘.) — Her uygulamadan cua-driver revoke --all çalıştır.',
  },
};

/* ==================== APP STATE ==================== */
const state = {
  lang: 'en',
  theme: 't3-chat-dark',
  activeChannelId: 't-checkout',
  isPlaying: false,
  auditionLang: null,
  computerAlert: false,
  compact: false,
  drawerOpen: false,
  dockMode: null, // null | 'reply' | 'new_task'
  dockTargetId: 't-backfill',
  skillView: 'aloud',
  reelIndex: 0,
  reelUnmuted: false,
  phoneHandoff: true,
  ksRevoked: false,
};

/* ==================== SHARED WEB AUDIO ENGINE ==================== */
const audioEl = new Audio();
audioEl.crossOrigin = 'anonymous';
audioEl.preload = 'auto';

let audioCtx = null;
let analyser = null;
let freqData = null;
let audioSourceConnected = false;

function ensureAudioAnalyser() {
  if (audioSourceConnected) {
    if (audioCtx && audioCtx.state === 'suspended') audioCtx.resume();
    return;
  }
  try {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    audioCtx = new AC();
    const src = audioCtx.createMediaElementSource(audioEl);
    analyser = audioCtx.createAnalyser();
    analyser.fftSize = 128;
    analyser.smoothingTimeConstant = 0.82;
    freqData = new Uint8Array(analyser.frequencyBinCount);
    src.connect(analyser);
    analyser.connect(audioCtx.destination);
    audioSourceConnected = true;
  } catch {
    // Graceful fallback if AudioContext is restricted
  }
}

function getAudioEnergy() {
  if (!analyser || !freqData || audioEl.paused) return 0;
  analyser.getByteFrequencyData(freqData);
  let sum = 0;
  const count = Math.min(28, freqData.length);
  for (let i = 1; i < count; i++) sum += freqData[i];
  return sum / (count * 255);
}

/* ==================== HELPER: TRANSLATION ==================== */
function t(key) {
  const dict = I18N[state.lang];
  if (dict && Object.prototype.hasOwnProperty.call(dict, key)) return dict[key];
  if (Object.prototype.hasOwnProperty.call(I18N.en, key)) return I18N.en[key];
  return key;
}

function applyTranslations() {
  document.documentElement.lang = state.lang;
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const key = el.getAttribute('data-i18n');
    const val = t(key);
    if (val.includes('<em>')) el.innerHTML = val;
    else el.textContent = val;
  });

  // Update lang toggle pills
  document.querySelectorAll('[data-set-lang]').forEach((btn) => {
    btn.classList.toggle('is-active', btn.dataset.setLang === state.lang);
  });

  // Re-render dynamic components
  renderSimList();
  updatePlayerDisplay();
  renderReelChapters();
  selectReel(state.reelIndex, false);
  renderSkillPreview();
  updatePhoneUI();
  updateAuditionButtons();

  // Update modal video source
  const modalVideo = document.getElementById('modal-video');
  if (modalVideo) {
    const src = `media/${state.lang}/demo.mp4`;
    const sourceEl = modalVideo.querySelector('source');
    if (sourceEl && sourceEl.getAttribute('src') !== src) {
      sourceEl.setAttribute('src', src);
      modalVideo.load();
    }
  }
}

/* ==================== LIVE T3 THEME SYNC ==================== */
function applyTheme(themeId) {
  state.theme = themeId;
  document.documentElement.setAttribute('data-theme', themeId);
  document.querySelectorAll('[data-set-theme]').forEach((el) => {
    el.classList.toggle('is-active', el.dataset.setTheme === themeId);
  });
  // Update theme-color meta
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) {
    requestAnimationFrame(() => {
      const bg = getComputedStyle(document.documentElement).getPropertyValue('--bg').trim();
      if (bg) meta.setAttribute('content', bg);
    });
  }
}

/* ==================== ACOUSTIC SOUNDFIELD CANVAS ==================== */
function initSoundfieldCanvas() {
  const canvas = document.getElementById('soundfield');
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  let w = 0;
  let h = 0;
  let dpr = Math.min(window.devicePixelRatio || 1, 2);
  let phase = 0;
  let smoothEnergy = 0;

  function resize() {
    const rect = canvas.getBoundingClientRect();
    w = rect.width;
    h = rect.height;
    canvas.width = Math.max(1, Math.floor(w * dpr));
    canvas.height = Math.max(1, Math.floor(h * dpr));
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }
  resize();
  window.addEventListener('resize', resize);

  function getAccentRGB() {
    const raw = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim();
    if (raw.startsWith('#') && raw.length === 7) {
      return [
        parseInt(raw.slice(1, 3), 16),
        parseInt(raw.slice(3, 5), 16),
        parseInt(raw.slice(5, 7), 16),
      ];
    }
    return [232, 62, 140];
  }

  function draw() {
    ctx.clearRect(0, 0, w, h);
    const liveEnergy = getAudioEnergy();
    // Subtle synthetic activity when playing even if AudioContext blocked
    const fallbackEnergy = !audioEl.paused ? 0.24 + Math.sin(phase * 3.1) * 0.08 : 0;
    const targetEnergy = Math.max(liveEnergy, fallbackEnergy);
    smoothEnergy += (targetEnergy - smoothEnergy) * 0.14;

    const [r, g, b] = getAccentRGB();
    const isLight = getComputedStyle(document.documentElement).getPropertyValue('--is-light').trim() === '1';
    const baseY = h * 0.54;
    const ribbons = 5;

    for (let i = 0; i < ribbons; i++) {
      const ratio = i / (ribbons - 1);
      const amp = (14 + i * 7) * (1 + smoothEnergy * 3.4);
      const freq = 0.0032 + i * 0.00085;
      const speed = (0.7 + i * 0.22) * (1 + smoothEnergy * 1.6);
      const alpha = (isLight ? 0.11 : 0.14) + smoothEnergy * 0.22 - ratio * 0.05;

      ctx.beginPath();
      const step = 10;
      for (let x = 0; x <= w + step; x += step) {
        // Horizontal Gaussian envelope so waves swell dramatically behind the interactive panel & center
        const normX = x / w;
        const envelope = Math.sin(normX * Math.PI);
        const harmonic =
          Math.sin(x * freq + phase * speed + i * 1.1) * 0.65 +
          Math.cos(x * freq * 1.9 - phase * speed * 0.7 + i) * 0.35;
        const y = baseY + (i - 2) * 26 + harmonic * amp * envelope;
        if (x === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }

      ctx.strokeStyle = `rgba(${r}, ${g}, ${b}, ${Math.max(0.03, alpha).toFixed(3)})`;
      ctx.lineWidth = i === 2 ? 1.7 : 1.05;
      ctx.stroke();
    }

    if (!reduced) {
      phase += 0.016;
      requestAnimationFrame(draw);
    }
  }

  draw();
}

/* ==================== INTERACTIVE TELLAL PANEL SIMULATOR ==================== */
function projectIconSVG(icon) {
  if (icon === 'np') return `<span class="tp-proj-np">NP</span>`;
  if (icon === 'globe') {
    return `<span class="tp-proj-icon sky"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/></svg></span>`;
  }
  if (icon === 'server') {
    return `<span class="tp-proj-icon violet"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect width="20" height="8" x="2" y="2" rx="2"/><rect width="20" height="8" x="2" y="14" rx="2"/><line x1="6" x2="6.01" y1="6" y2="6"/><line x1="6" x2="6.01" y1="18" y2="18"/></svg></span>`;
  }
  return `<span class="tp-proj-icon emerald"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect width="14" height="20" x="5" y="2" rx="2"/><path d="M12 18h.01"/></svg></span>`;
}

function statusMarkup(th) {
  if (th.status === 'waiting') {
    return `<span class="tp-status-txt is-waiting">! ${th.time[state.lang]}</span>`;
  }
  if (th.status === 'working') {
    return `<span class="tp-status-txt is-working">◌ ${th.time[state.lang]}</span>`;
  }
  if (th.status === 'done') {
    return `<span class="tp-status-txt is-done">✓ ${th.time[state.lang]}</span>`;
  }
  return `<span class="tp-status-txt">${th.time[state.lang]}</span>`;
}

function renderSimList() {
  const container = document.getElementById('tp-list');
  if (!container) return;

  const groups = [
    { key: 'pinned', label: t('panel.secPinned') },
    { key: 'working', label: t('panel.secWorking') },
    { key: 'active', label: t('panel.secActive') },
  ];

  let html = '';
  for (const g of groups) {
    const items = SIM_THREADS.filter((th) => th.section === g.key);
    html += `<div class="tp-sec-head"><span>${g.label}<span class="tp-sec-count">${items.length}</span></span><span>▾</span></div>`;
    for (const th of items) {
      const isListening = state.activeChannelId === th.id;
      const isSpeaking = isListening && state.isPlaying && !state.auditionLang;
      const speakerIcon = isSpeaking
        ? `<div class="tp-eq is-active"><span></span><span></span><span></span></div>`
        : `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/></svg>`;

      const showCaption = isListening || th.status === 'waiting';
      const captionClass = th.status === 'waiting' && !isListening ? 'tp-row-caption is-warn' : 'tp-row-caption';
      const captionText = th.status === 'waiting' && !isListening ? t('panel.secWaiting') : th.caption[state.lang];

      html += `
        <div class="tp-row ${isListening ? 'is-listening' : ''}" data-thread-id="${th.id}" role="option" aria-selected="${isListening}">
          <div class="tp-row-main">
            ${projectIconSVG(th.icon)}
            <span class="tp-row-title">${th.title[state.lang]}</span>
            <div class="tp-row-right">
              ${th.unheard && !isListening ? `<span class="tp-unheard">${th.unheard[state.lang]}</span>` : ''}
              <button type="button" class="tp-listen-btn ${isListening ? 'is-on' : ''}" data-action="listen" data-id="${th.id}" title="Listen / mute (Space)" aria-label="Listen to ${th.title.en}">
                ${speakerIcon}
              </button>
              <button type="button" class="tp-reply-inline" data-action="reply" data-id="${th.id}">${t('panel.replyBtn')}</button>
              ${statusMarkup(th)}
            </div>
          </div>
          ${showCaption ? `<div class="${captionClass}">${captionText}</div>` : ''}
        </div>
      `;
    }
  }

  container.innerHTML = html;

  const statusEl = document.getElementById('tp-status-text');
  if (statusEl) {
    const count = state.activeChannelId ? 1 : 0;
    statusEl.textContent = `${t('panel.statusThreads')} · ${count} ${t('panel.statusListening')}`;
  }
}

function updatePlayerDisplay() {
  const th = SIM_THREADS.find((x) => x.id === state.activeChannelId) || SIM_THREADS[3];
  const meta = document.getElementById('tp-player-meta');
  const cap = document.getElementById('tp-player-caption');
  const eq = document.getElementById('tp-eq');
  const badge = document.getElementById('tp-player-queue');
  const playIcon = document.getElementById('tp-play-icon');
  const pauseIcon = document.getElementById('tp-pause-icon');

  if (meta) meta.textContent = `${th.project} · ${th.title[state.lang]}`;
  if (cap) cap.textContent = th.caption[state.lang];
  const activePlaying = state.isPlaying && !state.auditionLang;
  if (eq) eq.classList.toggle('is-active', activePlaying);
  if (badge) {
    badge.textContent = activePlaying ? t('panel.playingBadge') : state.activeChannelId ? t('panel.readyBadge') : t('panel.pausedBadge');
  }
  if (playIcon) playIcon.hidden = activePlaying;
  if (pauseIcon) pauseIcon.hidden = !activePlaying;
}

function showToast(msg) {
  const el = document.getElementById('tp-toast');
  if (!el) return;
  el.textContent = msg;
  el.hidden = false;
  clearTimeout(showToast._timer);
  showToast._timer = setTimeout(() => {
    el.hidden = true;
  }, 2400);
}

function playChannelAudio(threadId, overrideKey = null) {
  const th = SIM_THREADS.find((x) => x.id === threadId) || SIM_THREADS[3];
  state.activeChannelId = th.id;
  state.auditionLang = null;
  updateAuditionButtons();

  // Remove initial hero pulse once user engages
  document.querySelectorAll('.sim-chip.is-pulse').forEach((c) => c.classList.remove('is-pulse'));

  const key = overrideKey || th.audioKey;
  const src = `media/audio/${state.lang}/${key}.mp3`;
  ensureAudioAnalyser();
  audioEl.src = src;
  audioEl.currentTime = 0;
  audioEl
    .play()
    .then(() => {
      state.isPlaying = true;
      renderSimList();
      updatePlayerDisplay();
    })
    .catch(() => {
      state.isPlaying = false;
      renderSimList();
      updatePlayerDisplay();
    });
}

function stopAudio() {
  audioEl.pause();
  state.isPlaying = false;
  state.auditionLang = null;
  renderSimList();
  updatePlayerDisplay();
  updateAuditionButtons();
}

/* Reply / New Task Dock Simulation */
let dictationTimer = null;

function openDock(mode, threadId = 't-backfill') {
  const dock = document.getElementById('tp-dock');
  const label = document.getElementById('tp-dock-label');
  const input = document.getElementById('tp-dock-input');
  const chipText = document.getElementById('tp-dock-chip-text');
  if (!dock || !input) return;

  if (state.compact) toggleCompact(false);
  state.dockMode = mode;
  state.dockTargetId = threadId;
  dock.hidden = false;

  const th = SIM_THREADS.find((x) => x.id === threadId) || SIM_THREADS[1];
  const fullText =
    mode === 'new_task'
      ? state.lang === 'tr'
        ? 'Masaüstündeki ekran görüntülerini tarihe göre klasörle.'
        : 'Sort the screenshots on the desktop into folders by date.'
      : th.replyDictation[state.lang];

  label.textContent = mode === 'new_task' ? t('panel.newTaskLabel') : `${t('panel.replyLabel')} ${th.title[state.lang]}`;
  input.value = '';
  chipText.textContent = t('panel.wisprListening');

  clearInterval(dictationTimer);
  let i = 0;
  dictationTimer = setInterval(() => {
    i += 2;
    input.value = fullText.slice(0, i);
    if (i >= fullText.length) {
      clearInterval(dictationTimer);
      chipText.textContent = t('panel.wisprReady');
    }
  }, 34);
}

function closeDock() {
  clearInterval(dictationTimer);
  state.dockMode = null;
  const dock = document.getElementById('tp-dock');
  if (dock) dock.hidden = true;
}

function sendDock() {
  const mode = state.dockMode;
  const targetId = state.dockTargetId;
  closeDock();
  showToast(t('panel.sentToast'));
  if (mode === 'new_task') {
    playChannelAudio('t-notes', 'new-task');
  } else {
    playChannelAudio(targetId, 'reply');
  }
}

/* Computer-Use Alert Simulation */
function setComputerAlert(show) {
  state.computerAlert = show;
  const banner = document.getElementById('tp-computer-banner');
  const pill = document.getElementById('tp-screen-pill');
  if (banner) banner.hidden = !show;
  if (pill) pill.hidden = !show;
  document.querySelector('[data-sim="computer"]')?.classList.toggle('is-active', show);
}

function toggleCompact(force) {
  state.compact = typeof force === 'boolean' ? force : !state.compact;
  document.getElementById('sim-panel')?.classList.toggle('is-compact', state.compact);
  document.getElementById('tp-compact-btn')?.classList.toggle('is-active', state.compact);
  document.querySelector('[data-sim="compact"]')?.classList.toggle('is-active', state.compact);
  if (state.compact && state.drawerOpen) toggleDrawer(false);
}

function toggleDrawer(force) {
  state.drawerOpen = typeof force === 'boolean' ? force : !state.drawerOpen;
  const drawer = document.getElementById('tp-drawer');
  if (drawer) drawer.hidden = !state.drawerOpen;
  document.getElementById('tp-metrics-btn')?.classList.toggle('is-active', state.drawerOpen);
  document.querySelector('[data-sim="drawer"]')?.classList.toggle('is-active', state.drawerOpen);
  if (state.drawerOpen && state.compact) toggleCompact(false);
}

/* Sparklines inside Metrics Drawer */
function initSparklines() {
  const rssEl = document.getElementById('spark-rss');
  const cpuEl = document.getElementById('spark-cpu');
  if (!rssEl || !cpuEl) return;

  const rssVals = [28, 34, 48, 62, 65, 64, 66, 65, 63, 66, 68, 64, 65, 66, 62, 64, 65, 63, 64, 65];
  const cpuVals = [8, 12, 62, 18, 14, 10, 44, 15, 11, 9, 13, 35, 12, 10, 14, 18, 9, 12, 11, 14];

  rssEl.innerHTML = rssVals.map((v) => `<i style="height:${v}%"></i>`).join('');
  cpuEl.innerHTML = cpuVals.map((v) => `<i style="height:${v}%"></i>`).join('');
}

/* ==================== VOICE AUDITION CARDS ==================== */
function initVoiceWaves() {
  ['wave-en', 'wave-tr'].forEach((id, idx) => {
    const el = document.getElementById(id);
    if (!el) return;
    const bars = 38;
    let html = '';
    for (let i = 0; i < bars; i++) {
      const h = Math.round(22 + Math.abs(Math.sin(i * 0.45 + idx * 1.4)) * 68);
      html += `<i style="height:${h}%"></i>`;
    }
    el.innerHTML = html;
  });

  // Live animation loop when auditioning
  setInterval(() => {
    if (!state.auditionLang || audioEl.paused) return;
    const el = document.getElementById(`wave-${state.auditionLang}`);
    if (!el) return;
    const energy = Math.max(0.25, getAudioEnergy());
    el.querySelectorAll('i').forEach((bar, i) => {
      const wave = 18 + Math.abs(Math.sin(Date.now() * 0.009 + i * 0.55)) * 78 * Math.min(1.25, energy * 2.4);
      bar.style.height = `${Math.min(96, Math.round(wave))}%`;
    });
  }, 90);
}

function updateAuditionButtons() {
  document.querySelectorAll('[data-audition]').forEach((btn) => {
    const lang = btn.dataset.audition;
    const playing = state.auditionLang === lang && state.isPlaying;
    btn.querySelector('.v-play').hidden = playing;
    btn.querySelector('.v-pause').hidden = !playing;
    btn.querySelector('span').textContent = playing ? t('voices.stop') : t('voices.audition');
    btn.closest('.voice-card')?.classList.toggle('is-playing', playing);
  });
}

function toggleVoiceAudition(lang) {
  if (state.auditionLang === lang && state.isPlaying) {
    stopAudio();
    return;
  }
  state.auditionLang = lang;
  ensureAudioAnalyser();
  audioEl.src = `media/audio/${lang}/summary.mp3`;
  audioEl.currentTime = 0;
  audioEl
    .play()
    .then(() => {
      state.isPlaying = true;
      updateAuditionButtons();
      renderSimList();
      updatePlayerDisplay();
    })
    .catch(() => {});
}

/* ==================== REEL THEATRE ==================== */
function renderReelChapters() {
  const list = document.getElementById('reel-chapters');
  if (!list) return;
  list.innerHTML = REEL_CLIPS.map(
    (c, idx) => `
    <button type="button" class="reel-ch ${idx === state.reelIndex ? 'is-active' : ''}" data-reel-idx="${idx}" role="tab" aria-selected="${idx === state.reelIndex}">
      <span class="reel-ch-num">${c.num}</span>
      <span class="reel-ch-body">
        <strong>${c.title[state.lang]}</strong>
        <span>${c.short[state.lang]}</span>
      </span>
    </button>
  `
  ).join('');
}

function selectReel(idx, shouldPlay = true) {
  state.reelIndex = idx;
  const clip = REEL_CLIPS[idx];
  const video = document.getElementById('reel-video');
  const titleEl = document.getElementById('reel-active-title');
  const descEl = document.getElementById('reel-active-desc');

  if (titleEl) titleEl.textContent = `${clip.num} · ${clip.title[state.lang]}`;
  if (descEl) descEl.textContent = clip.desc[state.lang];

  document.querySelectorAll('.reel-ch').forEach((el, i) => {
    el.classList.toggle('is-active', i === idx);
    el.setAttribute('aria-selected', String(i === idx));
  });

  if (video) {
    const newSrc = `media/${state.lang}/clips/${clip.id}.mp4`;
    const source = video.querySelector('source');
    if (source && source.getAttribute('src') !== newSrc) {
      source.setAttribute('src', newSrc);
      video.load();
    }
    video.muted = !state.reelUnmuted;
    if (shouldPlay) {
      video.play().catch(() => {});
    }
  }
}

/* ==================== SKILL BEFORE / AFTER ==================== */
function renderSkillPreview() {
  const el = document.getElementById('skill-preview');
  if (!el) return;

  if (state.skillView === 'aloud') {
    if (state.lang === 'tr') {
      el.innerHTML = `
        <div class="sp-line"><span class="sp-tag spoken">Başlangıç</span><span>Tamam, 80 görevi tarayıp durumlarına göre grupluyorum.</span></div>
        <div class="sp-line"><span class="sp-tag spoken">Adım</span><span>Şimdi takılan görevlerin günlüklerini inceliyorum.</span></div>
        <div class="sp-line"><span class="sp-tag spoken">Sesli Özet</span><span>80 görevin 20'si takıldı, 30'u hata verdi, kalanı uyarıyla bitti. Hatalardan mı başlayalım?</span></div>
        <div class="sp-line"><span class="sp-tag silent">Ekranda</span><span>~/dev/project/batch/errors.md · T3’te çift tıkla gör (okunmaz)</span></div>
      `;
    } else {
      el.innerHTML = `
        <div class="sp-line"><span class="sp-tag spoken">Start</span><span>Okay, I'm scanning the 80 tasks and grouping them by status.</span></div>
        <div class="sp-line"><span class="sp-tag spoken">Step</span><span>Now I'm reading the logs of the stuck tasks.</span></div>
        <div class="sp-line"><span class="sp-tag spoken">Spoken</span><span>Of 80 tasks, 20 got stuck, 30 failed and 30 finished with warnings. Start with the errors?</span></div>
        <div class="sp-line"><span class="sp-tag silent">Eyes only</span><span>~/dev/project/report.html · Stays silent; double-click to inspect in T3</span></div>
      `;
    }
  } else {
    el.innerHTML = `
      <div class="sp-line"><span class="sp-tag raw">Wall of text</span><span>Ran \`./scripts/batch-refactor.sh --workers=8\`. Processed 80 files across \`packages/core/src/handlers/*.ts\`.</span></div>
      <div class="sp-line"><span class="sp-tag raw">Unreadable</span><span>Exit code 1 on commit \`8f2a9c4d\`: TS2345 Argument of type 'Record&lt;string, unknown&gt;' is not assignable... (+42 lines)</span></div>
    `;
  }
}

/* ==================== PHONE HANDOFF TOGGLE ==================== */
function updatePhoneUI() {
  const btn = document.getElementById('phone-handoff-btn');
  const label = document.getElementById('phone-handoff-label');
  const macState = document.getElementById('phone-mac-state');
  const panelPhonePill = document.getElementById('tp-phone-pill');

  if (btn) btn.classList.toggle('is-on', state.phoneHandoff);
  if (label) label.textContent = state.phoneHandoff ? t('bento.phoneOn') : t('bento.phoneOff');
  if (macState) macState.textContent = state.phoneHandoff ? t('bento.macSilent') : t('bento.macActive');
  if (panelPhonePill) panelPhonePill.hidden = true; // Only shown on explicit toggle click below
}

/* ==================== WIRE ALL EVENTS ==================== */
function bindEvents() {
  // Theme switcher (both nav dots and bento theme cards)
  document.addEventListener('click', (e) => {
    const themeBtn = e.target.closest('[data-set-theme]');
    if (themeBtn) {
      applyTheme(themeBtn.dataset.setTheme);
      return;
    }

    const langBtn = e.target.closest('[data-set-lang]');
    if (langBtn) {
      const next = langBtn.dataset.setLang;
      if (next !== state.lang) {
        const wasPlaying = state.isPlaying;
        const ch = state.activeChannelId;
        state.lang = next;
        applyTranslations();
        if (wasPlaying && !state.auditionLang) playChannelAudio(ch);
      }
      return;
    }

    // Panel list clicks (listen / reply / row click)
    const listenBtn = e.target.closest('[data-action="listen"]');
    if (listenBtn) {
      e.stopPropagation();
      const id = listenBtn.dataset.id;
      if (state.activeChannelId === id && state.isPlaying && !state.auditionLang) {
        stopAudio();
      } else {
        playChannelAudio(id);
      }
      return;
    }

    const replyBtn = e.target.closest('[data-action="reply"]');
    if (replyBtn) {
      e.stopPropagation();
      openDock('reply', replyBtn.dataset.id);
      return;
    }

    const rowEl = e.target.closest('.tp-row');
    if (rowEl) {
      playChannelAudio(rowEl.dataset.threadId);
      return;
    }

    // Reel chapters
    const chBtn = e.target.closest('.reel-ch');
    if (chBtn) {
      selectReel(Number(chBtn.dataset.reelIdx), true);
      return;
    }

    // Audition buttons
    const audBtn = e.target.closest('[data-audition]');
    if (audBtn) {
      toggleVoiceAudition(audBtn.dataset.audition);
      return;
    }

    // Skill view toggle
    const skillBtn = e.target.closest('[data-skill-view]');
    if (skillBtn) {
      state.skillView = skillBtn.dataset.skillView;
      document.querySelectorAll('[data-skill-view]').forEach((b) => {
        b.classList.toggle('is-active', b === skillBtn);
        b.setAttribute('aria-selected', String(b === skillBtn));
      });
      renderSkillPreview();
      return;
    }

    // Install tabs
    const tabBtn = e.target.closest('[data-install-tab]');
    if (tabBtn) {
      const target = tabBtn.dataset.installTab;
      document.querySelectorAll('[data-install-tab]').forEach((b) => {
        b.classList.toggle('is-active', b === tabBtn);
        b.setAttribute('aria-selected', String(b === tabBtn));
      });
      document.querySelectorAll('[data-install-panel]').forEach((p) => {
        p.hidden = p.dataset.installPanel !== target;
      });
      return;
    }

    // Copy buttons
    const copyBtn = e.target.closest('.copy-btn');
    if (copyBtn) {
      navigator.clipboard?.writeText(copyBtn.dataset.copy || '').catch(() => {});
      const span = copyBtn.querySelector('span');
      if (span) {
        span.textContent = t('install.copied');
        setTimeout(() => {
          span.textContent = t('install.copy');
        }, 1800);
      }
      return;
    }

    // Scenario chips under simulator
    const simChip = e.target.closest('[data-sim]');
    if (simChip) {
      const mode = simChip.dataset.sim;
      if (mode === 'listen') {
        playChannelAudio('t-batch');
      } else if (mode === 'reply') {
        openDock('reply', 't-backfill');
      } else if (mode === 'computer') {
        setComputerAlert(!state.computerAlert);
      } else if (mode === 'drawer') {
        toggleDrawer();
      } else if (mode === 'compact') {
        toggleCompact();
      }
      return;
    }
  });

  // Hero "Hear real voice" CTA
  document.getElementById('hero-listen-btn')?.addEventListener('click', () => {
    playChannelAudio('t-checkout');
  });

  // Panel controls
  document.getElementById('tp-play-pause')?.addEventListener('click', () => {
    if (state.isPlaying && !state.auditionLang) {
      audioEl.pause();
      state.isPlaying = false;
      renderSimList();
      updatePlayerDisplay();
    } else if (audioEl.src && !state.auditionLang && audioEl.currentTime > 0 && !audioEl.ended) {
      audioEl.play().then(() => {
        state.isPlaying = true;
        renderSimList();
        updatePlayerDisplay();
      });
    } else {
      playChannelAudio(state.activeChannelId || 't-checkout');
    }
  });

  document.getElementById('tp-skip')?.addEventListener('click', () => {
    const idx = SIM_THREADS.findIndex((x) => x.id === state.activeChannelId);
    const next = SIM_THREADS[(idx + 1) % SIM_THREADS.length];
    playChannelAudio(next.id);
  });

  document.getElementById('tp-mute-all')?.addEventListener('click', () => {
    stopAudio();
    state.activeChannelId = null;
    renderSimList();
    updatePlayerDisplay();
    showToast(t('panel.mutedToast'));
  });

  document.getElementById('tp-progress')?.addEventListener('click', (e) => {
    if (!audioEl.duration) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    audioEl.currentTime = ratio * audioEl.duration;
  });

  document.getElementById('tp-new-task-btn')?.addEventListener('click', () => openDock('new_task'));
  document.getElementById('tp-metrics-btn')?.addEventListener('click', () => toggleDrawer());
  document.getElementById('tp-drawer-toggle')?.addEventListener('click', () => toggleDrawer());
  document.getElementById('tp-compact-btn')?.addEventListener('click', () => toggleCompact());
  document.getElementById('tp-dock-cancel')?.addEventListener('click', closeDock);
  document.getElementById('tp-dock-send')?.addEventListener('click', sendDock);
  document.getElementById('tp-dock-input')?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') sendDock();
    if (e.key === 'Escape') closeDock();
  });

  document.getElementById('tp-cu-stop-btn')?.addEventListener('click', () => {
    setComputerAlert(false);
    showToast(t('panel.stoppedToast'));
  });

  document.getElementById('tp-attention-card')?.addEventListener('click', () => {
    const row = document.querySelector('[data-thread-id="t-backfill"]');
    if (row) {
      row.classList.remove('is-highlight');
      void row.offsetWidth;
      row.classList.add('is-highlight');
    }
    playChannelAudio('t-backfill');
  });

  // Reel unmute toggle
  const reelSoundBtn = document.getElementById('reel-sound-toggle');
  reelSoundBtn?.addEventListener('click', () => {
    state.reelUnmuted = !state.reelUnmuted;
    const video = document.getElementById('reel-video');
    if (video) {
      video.muted = !state.reelUnmuted;
      if (state.reelUnmuted) {
        stopAudio(); // avoid overlapping audio
        video.play().catch(() => {});
      }
    }
    reelSoundBtn.setAttribute('aria-pressed', String(state.reelUnmuted));
    document.getElementById('reel-vol-off').hidden = state.reelUnmuted;
    document.getElementById('reel-vol-on').hidden = !state.reelUnmuted;
    document.getElementById('reel-sound-label').textContent = state.reelUnmuted ? t('reel.muted') : t('reel.unmute');
  });

  // Full 70s Walkthrough Modal
  const modal = document.getElementById('video-modal');
  const modalVideo = document.getElementById('modal-video');
  const openModal = () => {
    stopAudio();
    const reelVid = document.getElementById('reel-video');
    if (reelVid) reelVid.muted = true;
    modal.hidden = false;
    modalVideo.currentTime = 0;
    modalVideo.play().catch(() => {});
  };
  const closeModal = () => {
    modalVideo.pause();
    modal.hidden = true;
  };
  document.getElementById('open-walkthrough-btn')?.addEventListener('click', openModal);
  document.getElementById('reel-full-demo-btn')?.addEventListener('click', openModal);
  document.getElementById('modal-close')?.addEventListener('click', closeModal);
  modal?.addEventListener('click', (e) => {
    if (e.target === modal) closeModal();
  });

  // Kill-switch interactive keycap in Bento
  document.getElementById('ks-trigger')?.addEventListener('click', () => {
    state.ksRevoked = !state.ksRevoked;
    const badge = document.getElementById('ks-badge');
    if (badge) {
      badge.className = `ks-badge ${state.ksRevoked ? 'is-safe' : 'is-live'}`;
      badge.textContent = state.ksRevoked
        ? state.lang === 'tr'
          ? '✓ 0 aktif oturum · tümü iptal edildi'
          : '✓ 0 active sessions · all revoked'
        : state.lang === 'tr'
        ? '3 canlı oturum · omp'
        : '3 live sessions · omp';
    }
  });

  // Phone handoff interactive toggle
  document.getElementById('phone-handoff-btn')?.addEventListener('click', () => {
    state.phoneHandoff = !state.phoneHandoff;
    updatePhoneUI();
    const pill = document.getElementById('tp-phone-pill');
    if (pill) pill.hidden = !state.phoneHandoff;
  });

  // Keyboard Explorer Deck
  const kbReadout = document.getElementById('kb-readout');
  document.querySelectorAll('.kb-item').forEach((item) => {
    const showDesc = () => {
      document.querySelectorAll('.kb-item').forEach((x) => x.classList.remove('is-active'));
      item.classList.add('is-active');
      if (kbReadout) kbReadout.textContent = KB_DESCRIPTIONS[state.lang][item.dataset.kb] || '';
    };
    item.addEventListener('mouseenter', showDesc);
    item.addEventListener('click', showDesc);
  });

  // Global keyboard Escape closes modal/dock
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      if (modal && !modal.hidden) closeModal();
      else if (state.dockMode) closeDock();
    }
  });

  // Audio progress + end handlers
  audioEl.addEventListener('timeupdate', () => {
    if (!audioEl.duration || state.auditionLang) return;
    const pct = (audioEl.currentTime / audioEl.duration) * 100;
    const fill = document.getElementById('tp-progress-fill');
    const bar = document.getElementById('tp-progress');
    if (fill) fill.style.width = `${pct.toFixed(1)}%`;
    if (bar) bar.setAttribute('aria-valuenow', String(Math.round(pct)));
  });

  audioEl.addEventListener('ended', () => {
    state.isPlaying = false;
    state.auditionLang = null;
    const fill = document.getElementById('tp-progress-fill');
    if (fill) fill.style.width = '100%';
    renderSimList();
    updatePlayerDisplay();
    updateAuditionButtons();
  });
}

/* ==================== SCROLL REVEAL ==================== */
function initScrollReveal() {
  const els = document.querySelectorAll('.reveal');
  if (!('IntersectionObserver' in window)) {
    els.forEach((el) => el.classList.add('is-visible'));
    return;
  }
  const io = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add('is-visible');
          io.unobserve(entry.target);
        }
      }
    },
    { threshold: 0.12, rootMargin: '0px 0px -40px 0px' }
  );
  els.forEach((el) => io.observe(el));
}

/* ==================== BOOT ==================== */
function boot() {
  const params = new URLSearchParams(window.location.search);
  if (params.get('lang') === 'tr') state.lang = 'tr';

  applyTheme(state.theme);
  initSparklines();
  initVoiceWaves();
  applyTranslations();
  bindEvents();
  initScrollReveal();
  initSoundfieldCanvas();

  // Subtle interactive spotlight shimmer on Bento & Voice cards
  document.querySelectorAll('.bento-card, .voice-card').forEach((card) => {
    card.addEventListener('pointermove', (e) => {
      const r = card.getBoundingClientRect();
      card.style.setProperty('--mx', `${e.clientX - r.left}px`);
      card.style.setProperty('--my', `${e.clientY - r.top}px`);
    });
  });
}

boot();
