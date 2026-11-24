//! Every user-visible string of the panel, in Turkish and English (`Core::lang()` picks the
//! table). Fixed copy is a field; copy with numbers or names is a method that formats per
//! language. English follows T3's own wording where T3 has one (shelf titles, `now`/`5m`/`3h`/`2d`).

use tellal_core::{Lang, VoiceView};

use crate::model::Section;

pub struct Strings {
    pub lang: Lang,

    // Relative times and the working timer (`5dk` / `5m`).
    pub now: &'static str,
    pub min: &'static str,
    pub hour: &'static str,
    pub day: &'static str,

    // Header.
    pub pill_error: &'static str,
    pub pill_error_tip: &'static str,
    pub pill_connecting: &'static str,
    pub pill_connecting_tip: &'static str,
    pub pill_voice_loading: &'static str,
    pub pill_voice_loading_tip: &'static str,
    pub pill_no_voice: &'static str,
    pub pill_no_voice_tip: &'static str,
    pub pill_voice_downloading: &'static str,
    pub pill_voice_downloading_tip: &'static str,
    pub live_tip: &'static str,
    pub screen: &'static str,
    pub screen_tip: &'static str,
    pub new_task_tip: &'static str,
    pub search_tip: &'static str,
    pub metrics_close: &'static str,
    pub metrics_open: &'static str,
    pub metrics: &'static str,
    pub full_view_tip: &'static str,
    pub player_only_tip: &'static str,
    pub quit_tip: &'static str,

    // Error banner, search, attention card, compact strip.
    pub retry: &'static str,
    pub open_logs: &'static str,
    pub dismiss: &'static str,
    pub search_close_tip: &'static str,
    pub search_placeholder: &'static str,
    pub attention_tip: &'static str,
    pub show_list: &'static str,

    // List.
    pub status_waiting: &'static str,
    pub status_working: &'static str,
    pub status_done: &'static str,
    pub status_interrupted: &'static str,
    pub empty_title: &'static str,
    pub empty_body: &'static str,
    pub no_match_title: &'static str,
    pub no_match_body: &'static str,
    pub section_expand: &'static str,
    pub section_collapse: &'static str,
    pub section_signal_tip: &'static str,
    pub no_project: &'static str,
    pub row_body_tip: &'static str,
    pub listen_on_tip: &'static str,
    pub listen_off_tip: &'static str,
    pub reply: &'static str,
    pub reply_tip: &'static str,
    pub open_tip: &'static str,
    pub waiting_line: &'static str,

    // Computer use.
    pub agent: &'static str,
    pub cua_not_installed: &'static str,
    pub cua_installed: &'static str,
    pub perm_granted: &'static str,
    pub perm_missing: &'static str,
    pub perm_unknown: &'static str,
    pub cell_service: &'static str,
    pub service_running: &'static str,
    pub service_stopped: &'static str,
    pub cell_mode: &'static str,
    pub cell_permissions: &'static str,
    pub cell_sessions: &'static str,
    pub toast_computer_ready: &'static str,
    pub toast_computer_stopped: &'static str,
    pub toast_task_started: &'static str,
    pub stop: &'static str,
    pub banner_title: &'static str,
    pub perm_approve: &'static str,
    pub perm_status_unknown: &'static str,
    pub perm_needed: &'static str,
    pub perm_reopen: &'static str,
    pub perm_grant: &'static str,
    pub perm_grant_tip: &'static str,
    /// T3's own label for a thread without a project (the new-task dock's target).
    pub task_no_project: &'static str,

    // Dock.
    pub input_placeholder: &'static str,
    pub chip_dictating: &'static str,
    pub chip_editing: &'static str,
    pub chip_waiting_text: &'static str,
    pub chip_starting_task: &'static str,
    pub chip_sending: &'static str,
    pub chip_transcript_in: &'static str,
    pub toast_sent: &'static str,
    pub toast_sent_queued: &'static str,
    pub send: &'static str,
    pub send_task_tip: &'static str,
    pub send_tip: &'static str,
    pub cancel: &'static str,
    pub cancel_tip: &'static str,
    pub dock_new_task: &'static str,
    pub dock_reply: &'static str,

    // Status line and metrics drawer.
    pub conn_error_tip: &'static str,
    pub conn_ok_tip: &'static str,
    pub conn_connecting_tip: &'static str,
    pub mute: &'static str,
    pub mute_tip: &'static str,
    pub cell_t3_poll: &'static str,
    pub cell_live_poll: &'static str,
    pub cell_click_audio: &'static str,
    pub cell_msg_audio: &'static str,
    pub cell_dictation: &'static str,
    pub cell_queue: &'static str,
    pub cell_stop: &'static str,
    pub shortcut_off: &'static str,
    pub off: &'static str,
    pub computer_tip: &'static str,
    pub computer_title: &'static str,
    pub rss_60: &'static str,
    pub cpu_60: &'static str,
    pub voices_title: &'static str,

    // Player.
    pub queued_waiting: &'static str,
    pub resume_tip: &'static str,
    pub pause_tip: &'static str,
    pub skip_tip: &'static str,
    pub live_jump_tip: &'static str,
    pub mute_all_tip: &'static str,

    // Voices.
    pub voice_ready: &'static str,
    pub voice_missing: &'static str,
    pub voice_failed: &'static str,
    pub speed_title: &'static str,
    pub speed_tip: &'static str,

    // Self-test samples (shown in the `shots` walk).
    pub sample_task: &'static str,
    pub sample_reply: &'static str,
    pub sample_no_match: &'static str,
    pub sample_error: &'static str,
}

pub static TR: Strings = Strings {
    lang: Lang::Tr,
    now: "şimdi",
    min: "dk",
    hour: "sa",
    day: "g",

    pill_error: "hata",
    pill_error_tip: "bir işlem başarısız oldu · ayrıntı aşağıda",
    pill_connecting: "bağlanıyor",
    pill_connecting_tip: "T3'ten thread listesi bekleniyor",
    pill_voice_loading: "ses yükleniyor",
    pill_voice_loading_tip: "yerel ses modeli hazırlanıyor · dinleme birazdan başlar",
    pill_no_voice: "ses yok",
    pill_no_voice_tip: "konuşacak ses yüklü değil · aşağıdan indir",
    pill_voice_downloading: "ses iniyor",
    pill_voice_downloading_tip: "yerel ses modeli indiriliyor · bitince dinleme başlar",
    live_tip: "canlı · T3'e bağlı, ses hazır",
    screen: "ekran",
    screen_tip: "bir ajan ekranını kullanıyor · Durdur ya da ⌘.",
    new_task_tip: "Yeni görev (N)",
    search_tip: "ara (⌘K)",
    metrics_close: "ölçümleri kapat",
    metrics_open: "ölçümleri aç",
    metrics: "ölçümler",
    full_view_tip: "tam görünüm (⌘⇧C)",
    player_only_tip: "yalnız oynatıcı (⌘⇧C)",
    quit_tip: "Tellal'ı kapat (uygulamadan çıkar)",

    retry: "Tekrar dene",
    open_logs: "Kayıtları aç",
    dismiss: "kapat",
    search_close_tip: "aramayı kapat (Esc)",
    search_placeholder: "Ara: başlık ya da proje",
    attention_tip: "tıkla: sıradaki bekleyen thread'e git",
    show_list: "Listeyi aç",

    status_waiting: "seni bekliyor",
    status_working: "çalışıyor",
    status_done: "bitti · henüz bakmadın",
    status_interrupted: "durduruldu",
    empty_title: "Şu an açık thread yok",
    empty_body: "T3'te yeni bir thread başlatınca burada görünür.",
    no_match_title: "Eşleşen thread yok",
    no_match_body: "Başka bir kelime dene ya da Esc ile aramayı temizle.",
    section_expand: "bölümü aç",
    section_collapse: "bölümü daralt",
    section_signal_tip: "içinde seni bekleyen ya da dinlenen thread var",
    no_project: "Proje yok",
    row_body_tip: "tık: seç · çift tık: T3'te aç",
    listen_on_tip: "dinliyorsun · tıkla: sustur (Space)",
    listen_off_tip: "sessiz · tıkla: dinle (Space)",
    reply: "Cevapla",
    reply_tip: "cevap yaz ya da Wispr ile söyle (Enter / R)",
    open_tip: "T3'te aç (⌘O)",
    waiting_line: "seni bekliyor: onay ya da cevap gerekiyor",

    agent: "ajan",
    cua_not_installed: "kurulu değil",
    cua_installed: "kurulu",
    perm_granted: "var",
    perm_missing: "yok",
    perm_unknown: "bilinmiyor",
    cell_service: "servis",
    service_running: "çalışıyor",
    service_stopped: "kapalı",
    cell_mode: "mod",
    cell_permissions: "izin",
    cell_sessions: "canlı oturum",
    toast_computer_ready: "Bilgisayar kullanımı hazır",
    toast_computer_stopped: "Ekran kontrolü durduruldu",
    toast_task_started: "Görev başladı",
    stop: "Durdur",
    banner_title: "Ajan ekranını kullanıyor",
    perm_approve: "macOS pencerelerini onayla: Erişilebilirlik ve Ekran Kaydı",
    perm_status_unknown: "Bilgisayar kullanımı: izin durumu bilinmiyor",
    perm_needed: "Bilgisayar kullanımı: izin gerekli",
    perm_reopen: "Yeniden aç",
    perm_grant: "İzin ver",
    perm_grant_tip: "CuaDriver'ın macOS izin pencerelerini aç (I)",
    task_no_project: "No project",

    input_placeholder: "yaz, yapıştır ya da Wispr ile dikte et…",
    chip_dictating: "Wispr dinliyor · bitince Caps Lock ya da Gönder",
    chip_editing: "Yaz ya da düzenle · Enter gönderir",
    chip_waiting_text: "Wispr metni bekleniyor…",
    chip_starting_task: "Görev başlatılıyor…",
    chip_sending: "Gönderiliyor…",
    chip_transcript_in: "Wispr metni geldi · Enter gönderir",
    toast_sent: "Gönderildi",
    toast_sent_queued: "Gönderildi · sıraya alındı",
    send: "Gönder",
    send_task_tip: "görevi başlat (Enter) · dikte sürüyorsa önce bitirir",
    send_tip: "gönder (Enter) · dikte sürüyorsa önce bitirir",
    cancel: "İptal",
    cancel_tip: "vazgeç (Esc)",
    dock_new_task: "→ Yeni görev ·",
    dock_reply: "Cevap →",

    conn_error_tip: "hata var · banner'a bak",
    conn_ok_tip: "T3'e bağlı",
    conn_connecting_tip: "T3'e bağlanıyor",
    mute: "Sustur",
    mute_tip: "dinlenen tüm kanalları sustur (M)",
    cell_t3_poll: "T3 sorgu",
    cell_live_poll: "canlı sorgu",
    cell_click_audio: "tık → ses",
    cell_msg_audio: "mesaj → ses",
    cell_dictation: "dikte",
    cell_queue: "kuyruk",
    cell_stop: "durdur",
    shortcut_off: "kısayol kapalı",
    off: "kapalı",
    computer_tip: "cua-driver: ajanların ekranı kullandığı yerel servis · durdur = global kısayol (panelde ⌘.)",
    computer_title: "Bilgisayar kullanımı",
    rss_60: "RSS · 60 sn",
    cpu_60: "CPU · 60 sn",
    voices_title: "Sesler",

    queued_waiting: "sırada bekleyen mesaj var",
    resume_tip: "devam et (P)",
    pause_tip: "duraklat (P)",
    skip_tip: "okey, geç (→)",
    live_jump_tip: "canlıya geç (L)",
    mute_all_tip: "hepsini sustur (M)",

    voice_ready: "hazır",
    voice_missing: "yüklü değil",
    voice_failed: "inmedi",
    speed_title: "Hız",
    speed_tip: "konuşma hızı · bir sonraki mesajdan itibaren geçerli",

    sample_task: "Masaüstündeki ekran görüntülerini tarihe göre klasörle.",
    sample_reply: "Testleri çalıştır, sonra özetle.",
    sample_no_match: "zzzz-yok",
    sample_error: "T3 komutu yanıt vermedi (selftest örneği)",
};

pub static EN: Strings = Strings {
    lang: Lang::En,
    now: "now",
    min: "m",
    hour: "h",
    day: "d",

    pill_error: "error",
    pill_error_tip: "something failed · details below",
    pill_connecting: "connecting",
    pill_connecting_tip: "waiting for the thread list from T3",
    pill_voice_loading: "loading voice",
    pill_voice_loading_tip: "preparing the local voice model · listening starts shortly",
    pill_no_voice: "no voice",
    pill_no_voice_tip: "no voice installed to speak with · download one below",
    pill_voice_downloading: "downloading voice",
    pill_voice_downloading_tip: "downloading the local voice model · listening starts when it's done",
    live_tip: "live · connected to T3, voice ready",
    screen: "screen",
    screen_tip: "an agent is using your screen · Stop or ⌘.",
    new_task_tip: "New task (N)",
    search_tip: "search (⌘K)",
    metrics_close: "close metrics",
    metrics_open: "open metrics",
    metrics: "metrics",
    full_view_tip: "full view (⌘⇧C)",
    player_only_tip: "player only (⌘⇧C)",
    quit_tip: "Quit Tellal",

    retry: "Retry",
    open_logs: "Open logs",
    dismiss: "dismiss",
    search_close_tip: "close search (Esc)",
    search_placeholder: "Search: title or project",
    attention_tip: "click: go to the next waiting thread",
    show_list: "Show list",

    status_waiting: "waiting for you",
    status_working: "working",
    status_done: "done · not seen yet",
    status_interrupted: "interrupted",
    empty_title: "No open threads",
    empty_body: "Start a thread in T3 and it shows up here.",
    no_match_title: "No matching threads",
    no_match_body: "Try another word, or press Esc to clear the search.",
    section_expand: "expand section",
    section_collapse: "collapse section",
    section_signal_tip: "a thread in here waits for you or is being listened to",
    no_project: "No project",
    row_body_tip: "click: select · double-click: open in T3",
    listen_on_tip: "listening · click: mute (Space)",
    listen_off_tip: "muted · click: listen (Space)",
    reply: "Reply",
    reply_tip: "type a reply or say it with Wispr (Enter / R)",
    open_tip: "open in T3 (⌘O)",
    waiting_line: "waiting for you: needs an approval or an answer",

    agent: "agent",
    cua_not_installed: "not installed",
    cua_installed: "installed",
    perm_granted: "granted",
    perm_missing: "missing",
    perm_unknown: "unknown",
    cell_service: "service",
    service_running: "running",
    service_stopped: "stopped",
    cell_mode: "mode",
    cell_permissions: "permissions",
    cell_sessions: "live sessions",
    toast_computer_ready: "Computer use is ready",
    toast_computer_stopped: "Screen control stopped",
    toast_task_started: "Task started",
    stop: "Stop",
    banner_title: "An agent is using your screen",
    perm_approve: "Approve the macOS prompts: Accessibility and Screen Recording",
    perm_status_unknown: "Computer use: permission status unknown",
    perm_needed: "Computer use: permission needed",
    perm_reopen: "Reopen",
    perm_grant: "Grant",
    perm_grant_tip: "open CuaDriver's macOS permission prompts (I)",
    task_no_project: "No project",

    input_placeholder: "type, paste or dictate with Wispr…",
    chip_dictating: "Wispr is listening · Caps Lock or Send when done",
    chip_editing: "Type or edit · Enter sends",
    chip_waiting_text: "Waiting for Wispr's text…",
    chip_starting_task: "Starting the task…",
    chip_sending: "Sending…",
    chip_transcript_in: "Wispr's text is in · Enter sends",
    toast_sent: "Sent",
    toast_sent_queued: "Sent · queued",
    send: "Send",
    send_task_tip: "start the task (Enter) · ends a running dictation first",
    send_tip: "send (Enter) · ends a running dictation first",
    cancel: "Cancel",
    cancel_tip: "cancel (Esc)",
    dock_new_task: "→ New task ·",
    dock_reply: "Reply →",

    conn_error_tip: "something failed · see the banner",
    conn_ok_tip: "connected to T3",
    conn_connecting_tip: "connecting to T3",
    mute: "Mute",
    mute_tip: "mute every channel you listen to (M)",
    cell_t3_poll: "T3 poll",
    cell_live_poll: "live poll",
    cell_click_audio: "click → audio",
    cell_msg_audio: "message → audio",
    cell_dictation: "dictation",
    cell_queue: "queue",
    cell_stop: "stop",
    shortcut_off: "shortcut off",
    off: "off",
    computer_tip: "cua-driver: the local service agents use your screen through · stop = global shortcut (⌘. in the panel)",
    computer_title: "Computer use",
    rss_60: "RSS · 60 s",
    cpu_60: "CPU · 60 s",
    voices_title: "Voices",

    queued_waiting: "messages are queued",
    resume_tip: "resume (P)",
    pause_tip: "pause (P)",
    skip_tip: "skip (→)",
    live_jump_tip: "jump to live (L)",
    mute_all_tip: "mute all (M)",

    voice_ready: "ready",
    voice_missing: "not installed",
    voice_failed: "failed",
    speed_title: "Speed",
    speed_tip: "speech speed · applies from the next message",

    sample_task: "Sort the screenshots on the desktop into folders by date.",
    sample_reply: "Run the tests, then summarize.",
    sample_no_match: "zzzz-none",
    sample_error: "T3 command did not answer (selftest sample)",
};

pub fn strings(lang: Lang) -> &'static Strings {
    match lang {
        Lang::Tr => &TR,
        Lang::En => &EN,
    }
}

/// `1 thread` / `2 threads` (Turkish nouns stay singular after a number).
fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn mb(bytes: u64) -> u64 {
    bytes.div_ceil(1_000_000)
}

impl Strings {
    fn tr(&self) -> bool {
        self.lang == Lang::Tr
    }

    /// T3's shelf titles.
    pub fn section(&self, s: Section) -> &'static str {
        match (self.lang, s) {
            (Lang::Tr, Section::Pinned) => "Sabitlenmiş",
            (Lang::Tr, Section::Working) => "Çalışıyor",
            (Lang::Tr, Section::Active) => "Aktif",
            (Lang::Tr, Section::Snoozed) => "Ertelendi",
            (Lang::Tr, Section::Settled) => "Bitti",
            (Lang::En, Section::Pinned) => "Pinned",
            (Lang::En, Section::Working) => "Working",
            (Lang::En, Section::Active) => "Active",
            (Lang::En, Section::Snoozed) => "Snoozed",
            (Lang::En, Section::Settled) => "Settled",
        }
    }

    pub fn lang_name(&self, lang: Lang) -> &'static str {
        match (self.lang, lang) {
            (Lang::Tr, Lang::Tr) => "Türkçe",
            (Lang::Tr, Lang::En) => "İngilizce",
            (Lang::En, Lang::Tr) => "Turkish",
            (Lang::En, Lang::En) => "English",
        }
    }

    pub fn error_message(&self, message: &str) -> String {
        if self.tr() { format!("Bir şey ters gitti: {message}") } else { format!("Something went wrong: {message}") }
    }

    pub fn results(&self, n: usize) -> String {
        if self.tr() { format!("{n} sonuç") } else { plural(n, "result", "results") }
    }

    pub fn waiting_count(&self, n: usize) -> String {
        if self.tr() {
            format!("{n} thread seni bekliyor")
        } else {
            format!("{} waiting for you", plural(n, "thread", "threads"))
        }
    }

    pub fn compact_idle(&self, listening: usize) -> String {
        if self.tr() {
            format!("Şu an çalan yok · {listening} dinleniyor")
        } else {
            format!("Nothing playing · {listening} listening")
        }
    }

    pub fn unheard(&self, n: usize) -> String {
        if self.tr() { format!("{n} yeni") } else { format!("{n} new") }
    }

    pub fn task_failed_error(&self, detail: &str) -> String {
        if self.tr() { format!("Görev başlatılamadı: {detail}") } else { format!("Couldn't start the task: {detail}") }
    }

    pub fn last_action(&self, client: &str, when: &str) -> String {
        if self.tr() { format!("{client} · son işlem {when}") } else { format!("{client} · last action: {when}") }
    }

    pub fn connected(&self, client: &str) -> String {
        if self.tr() { format!("{client} · bağlı") } else { format!("{client} · connected") }
    }

    pub fn stop_tip(&self, global: Option<&str>) -> String {
        match (self.tr(), global) {
            (true, Some(g)) => format!("ajanların ekran kontrolünü hemen kes (⌘. · her yerden {g})"),
            (true, None) => "ajanların ekran kontrolünü hemen kes (⌘.)".to_owned(),
            (false, Some(g)) => format!("cut the agents' screen control now (⌘. · from any app {g})"),
            (false, None) => "cut the agents' screen control now (⌘.)".to_owned(),
        }
    }

    pub fn sessions(&self, n: usize) -> String {
        if self.tr() { format!("{n} oturum") } else { plural(n, "session", "sessions") }
    }

    pub fn banner_tip(&self, n: usize) -> String {
        if self.tr() {
            format!("{n} canlı oturum · ajanlar cua-driver ile ekranı kullanabilir")
        } else {
            format!("{} · agents can use the screen through cua-driver", plural(n, "live session", "live sessions"))
        }
    }

    pub fn task_failed_note(&self, detail: &str) -> String {
        if self.tr() {
            format!("Görev başlatılamadı ({detail}) · Gönder ile tekrar dene")
        } else {
            format!("Couldn't start the task ({detail}) · Send to retry")
        }
    }

    pub fn dictation_failed_error(&self, error: &str) -> String {
        if self.tr() { format!("Wispr dikte başlamadı: {error}") } else { format!("Wispr dictation didn't start: {error}") }
    }

    pub fn dictation_failed_note(&self, error: &str) -> String {
        if self.tr() {
            format!("Wispr çalışmadı ({error}) · yazarak cevaplayabilirsin")
        } else {
            format!("Wispr didn't work ({error}) · you can type your reply")
        }
    }

    pub fn reply_failed_error(&self, detail: &str) -> String {
        if self.tr() { format!("Cevap gönderilemedi: {detail}") } else { format!("Couldn't send the reply: {detail}") }
    }

    pub fn reply_failed_note(&self, detail: &str) -> String {
        if self.tr() {
            format!("Gönderilemedi ({detail}) · Gönder ile tekrar dene")
        } else {
            format!("Couldn't send ({detail}) · Send to retry")
        }
    }

    pub fn status_counts(&self, threads: usize, listening: usize) -> String {
        if self.tr() {
            format!("{threads} thread · {listening} dinleniyor")
        } else {
            format!("{} · {listening} listening", plural(threads, "thread", "threads"))
        }
    }

    pub fn queue_cell(&self, queued: usize, listening: usize) -> String {
        if self.tr() { format!("{queued} · {listening} dinleniyor") } else { format!("{queued} · {listening} listening") }
    }

    pub fn shortcut(&self, spec: &str) -> String {
        if self.tr() { format!("kısayol {spec}") } else { format!("shortcut {spec}") }
    }

    pub fn run_dir_tip(&self, run_dir: &str) -> String {
        if self.tr() { format!("{run_dir}\ntıkla: Finder'da aç") } else { format!("{run_dir}\nclick: open in Finder") }
    }

    pub fn queued(&self, n: usize) -> String {
        if self.tr() { format!("{n} sırada") } else { format!("{n} queued") }
    }

    /// Voice row: `İngilizce ses yüklü değil` / `English voice not installed`.
    pub fn voice_missing_line(&self, v: &VoiceView) -> String {
        if self.tr() {
            format!("{} ses yüklü değil", self.lang_name(v.lang))
        } else {
            format!("{} voice not installed", self.lang_name(v.lang))
        }
    }

    /// `Sesi indir (80 MB)` / `Download voice (80 MB)`.
    pub fn voice_download(&self, v: &VoiceView) -> String {
        if self.tr() { format!("Sesi indir ({} MB)", v.download_mb) } else { format!("Download voice ({} MB)", v.download_mb) }
    }

    /// Drawer button: `İndir (80 MB)` / `Download (80 MB)`.
    pub fn voice_download_short(&self, v: &VoiceView) -> String {
        if self.tr() { format!("İndir ({} MB)", v.download_mb) } else { format!("Download ({} MB)", v.download_mb) }
    }

    pub fn voice_downloading_line(&self, v: &VoiceView, done: u64, total: u64) -> String {
        if self.tr() {
            format!("{} ses iniyor · {} / {} MB", self.lang_name(v.lang), mb(done), mb(total))
        } else {
            format!("Downloading the {} voice · {} / {} MB", self.lang_name(v.lang), mb(done), mb(total))
        }
    }

    pub fn voice_failed_line(&self, v: &VoiceView, error: &str) -> String {
        if self.tr() {
            format!("{} ses inmedi: {error}", self.lang_name(v.lang))
        } else {
            format!("{} voice download failed: {error}", self.lang_name(v.lang))
        }
    }

    /// `%42` / `42%`.
    pub fn percent(&self, done: u64, total: u64) -> String {
        let pct = done.saturating_mul(100).checked_div(total).unwrap_or(0);
        if self.tr() { format!("%{pct}") } else { format!("{pct}%") }
    }

    pub fn voice_tip(&self, v: &VoiceView) -> String {
        if self.tr() {
            format!("{} · {} · {} MB · bu Mac'te, CPU'da çalışır (D)", v.name, self.lang_name(v.lang), v.download_mb)
        } else {
            format!("{} · {} · {} MB · runs on this Mac's CPU (D)", v.name, self.lang_name(v.lang), v.download_mb)
        }
    }

    pub fn toast_voice_ready(&self, v: &VoiceView) -> String {
        if self.tr() { format!("Ses hazır: {}", v.name) } else { format!("Voice ready: {}", v.name) }
    }

    /// Speed step: `1,25×` / `1.25×`, `2×`.
    pub fn speed_step(&self, speed: f32) -> String {
        let step = format!("{speed}×");
        if self.tr() { step.replace('.', ",") } else { step }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_plurals_and_turkish_counts() {
        assert_eq!(EN.waiting_count(1), "1 thread waiting for you");
        assert_eq!(EN.waiting_count(2), "2 threads waiting for you");
        assert_eq!(TR.waiting_count(2), "2 thread seni bekliyor");
        assert_eq!(EN.status_counts(1, 0), "1 thread · 0 listening");
        assert_eq!(TR.percent(42, 100), "%42");
        assert_eq!(EN.percent(42, 100), "42%");
        assert_eq!(EN.section(Section::Settled), "Settled");
        assert_eq!(TR.section(Section::Settled), "Bitti");
        assert_eq!(TR.speed_step(1.25), "1,25×");
        assert_eq!(EN.speed_step(2.0), "2×");
    }
}
