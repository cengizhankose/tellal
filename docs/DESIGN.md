# Tellal v0.3 — design spec

The panel is a remote control for T3 Code that you *listen* to. It sits above every app, so it must
be calm, small and instantly readable. This spec is the contract for the v0.3 redesign; every rule
cites where it comes from.

Sources:
- **E** = evidence rules from the author's `ui-evidence` knowledge base (`kb/GOLDEN-RULES.md`, rule ids G…, grade A/B). It is private and not published; every rule this spec relies on is stated where it is cited.
- **A** = Atlas: private design-system notes ("Tasarımın 14 kuralı", "15 grafik tasarım trendi" and two design-system write-ups covering token architecture, motion and empty states). They are not published; the principles taken from them are summarized in §1 below. `A: DS §n` cites a section of the design-system write-ups.
- **T3** = T3 Code's own sidebar (study: [`research/ui/t3-sidebar.md`](../research/ui/t3-sidebar.md), source: github.com/pingdotgg/t3code `apps/web`). The panel should feel like a detached piece of that sidebar.

## 1. Principles

1. **Calm by default** (A: DS §3.1; E: G1-8). Low visual complexity: one surface colour, one accent, hairline borders, no decorative gradients. Detail appears only when it carries state.
2. **One focal point** (A: rule 01 Emphasis, 10 Hierarchy). The thing being spoken is the hero: the *Now playing* bar. Everything else is quieter.
3. **Position carries priority, not colour** (E: G1-2). Attention items at the top, the player at the bottom, the list in between; these places never move.
4. **Stable order** (E: G2-4; T3: "thread activity does not change the order"). Rows never jump because a thread got a new message. New threads appear at the top of their group with a short enter transition (E: G1-11).
5. **Every clickable thing looks clickable; nothing lives only behind hover** (E: G1-1, G2-11). The listen toggle is always visible. Row actions that appear on hover are also on the keyboard and on the selected row.
6. **Meaning never by colour alone** (E: G1-5). Status = icon shape + colour + tooltip text.
7. **Motion explains state, never decorates** (A: DS §56; E: G1-10, G1-11). Nothing moves continuously unless the user started it (audio they unmuted). Running threads get a *static* activity glyph, not a spinner. Honour macOS Reduce Motion.
8. **Legible** (E: G1-3, G1-4). Body 13 px, secondary 12 px, nothing under 11 px (11 only for badges and section labels). Text ≥ 4.5:1 contrast on its background in every theme.
9. **Speak the user's words** (E: G2-2, G3-11, G3-12). UI copy in the user's language (Turkish or English, `TELLAL_LANG`, default = macOS language), short, first word carries meaning; errors say what happened and what to do. English copy follows T3's own English wording where T3 has one: shelf names (Pinned, Working, Active, Snoozed, Settled) and compact times (`now`, `5m`, `3h`, `2d`; working timer `<1m`, `16m`, `1h 9m`). Every string lives in `crates/app/src/strings.rs`; Turkish examples below have English counterparts there.

## 2. Identity (T3 parity)

- Header mirrors T3's sidebar header: the Tellal mark (`logo.svg`, Lucide's megaphone, 16 px in `accent`) followed by the wordmark **Tellal** in T3's wordmark treatment (bold, 14 px). A T3-style pill (16 px tall, 11 px text) appears only when the panel is *not* live: `bağlanıyor` / connecting, `ses yükleniyor` / loading voice, `ses yok` / no voice (no voice installed that can speak), `ses iniyor` / downloading voice (in `info`), `hata` / error; when live a 6 px success dot (tooltip `canlı`). While an agent may act on the screen a second pill, pointer icon + `ekran` in `danger`, sits next to it (also in compact mode).
- The app mark is our own (do not reuse T3's logo as ours): a rounded square with a "T3" monogram and a sound-wave notch, `crates/app/assets/logo.svg`. It ships in the assets (about/tooltip use is optional) but is not shown in the header.
- Provider marks on rows like T3 shows them (Claude, OpenAI/Codex, ACP registry for omp, …): 14 px at 60 % opacity, T3's brand colours where it has them; generic ACP-registry marks get T3's 2-letter instance tile (`OM`, `OF`); unknown providers a 2-letter monogram. Tooltip = provider + model.
- Every row shows its project: the T3 project icon (`ThreadView.project_icon`, lucide name + T3 colour map), the "No project" dashed square, or T3's generated two-letter monogram tile in the name-hashed identity colour (`projectIdentity.ts`), exactly as T3 falls back.

## 3. Colour and type tokens

All colours come from `Core::theme()` (`ThemeView`), i.e. the theme selected in the T3 app, re-applied live on `CoreEvent::ThemeChanged` with a 200 ms cross-fade. The app derives the rest; never hard-code a colour outside the token module.

| token | from | use |
|---|---|---|
| `canvas` | theme | window background (window uses `Blurred` background with canvas at ~92 % alpha) |
| `surface` | theme | player bar, reply dock, popovers, attention card |
| `surface_hover` | theme | row hover, pressed (pressed state ≤ 85 ms, E: G2-6) |
| `selected` | accent @ 14 % over canvas | selected row |
| `border` | theme | 1 px hairlines, window edge |
| `text` / `text_muted` | theme | primary / secondary text |
| `accent` / `accent_text` | theme | listen-on state, primary button, focus ring, progress |
| `success` `warning` `danger` `info` | theme | status glyphs, waiting card (warning), errors |

Type: T3's interface font (`ThemeView.font_family`, empty = system). Thread titles 14 px / 500 (T3), body `font_size - 1`, secondary/meta 12 px, badges 11 px; section labels 12 px / 500 in sentence case (T3). Weights: 600 headings, 500 titles and labels, 400 body. Tabular numerals for times and metrics.

Spacing scale 4 / 8 / 12 / 16; radius 6 (controls), 10 (cards), 14 (window). Elevation: one shadow for the window, none inside (A: DS elevation: shadows only for floating layers).

## 4. Motion tokens (A: DS §57–59)

| token | ms | easing | used for |
|---|---|---|---|
| `fast` | 120 | ease-out | hover/pressed fades, icon swaps |
| `normal` | 200 | ease-in-out | expand/collapse, dock/player slide, theme cross-fade, row enter |
| `slow` | 320 | ease-out | panel compact ↔ full height |
| `spring` | 240 | overshoot ≤ 4 % | listen toggle on, sent ✓ |

Reduced Motion (macOS `accessibilityDisplayShouldReduceMotion`): all durations → 0 except opacity fades capped at 120 ms; no equalizer bars (show a static speaker glyph instead).

Allowed continuous motion, only while audio the user enabled is playing: the player's equalizer bars and its progress bar.

## 5. Layout (380 px wide; height = content, 160–640 px)

```
┌─────────────────────────────────────────┐
│ Tellal • ⌖ekran        +  ⌕  ▤  ⤡  ✕  │ header 40px, drag area
├─────────────────────────────────────────┤
│ ⌖ Ajan ekranını kullanıyor  [2]   3dk │ computer-use banner (only while ≥1 cua-driver
│   TextEdit · click "Kaydet"   [Durdur] │ session; also in compact mode)
├─────────────────────────────────────────┤
│ ⚠ 2 thread seni bekliyor · acme-web, … →│ attention card (only when ≥1 waiting)
├─────────────────────────────────────────┤
│ Çalışıyor 4                          ▾  │ section header (12px/500, count, chevron)
│ 🌐 Fix flaky auth tests     ◌  ✳  🔊  16dk│ row 36px: project · title · status · provider · listen · time/timer
│    “Şimdi testleri çalıştırıyorum.”     │ second line only when it carries state
│ Aktif 8                              ▾  │
│ AS Add rate limiting       ✓  ✳  🔈  1sa │
│ Ertelendi 1                          ▸  │ shelves start collapsed
├─────────────────────────────────────────┤
│ ♪ acme-web · Fix flaky auth…    2 sırada│ Now playing (only when speaking/paused/queued)
│ “Şimdi testleri çalıştırıyorum.”        │
│ ━━━━━━━━━━━━━━━━━━──────   ⏸  ⏭  ⇥  🔇 │ progress + controls
├─────────────────────────────────────────┤
│ → Fix flaky auth tests                  │ Reply dock (when open)
│ [ metin kutusu                       ]  │
│ 🎙 Wispr dinliyor…        [İptal][Gönder]│
├─────────────────────────────────────────┤
│ ⛉ Bilgisayar kullanımı: izin [İzin ver]│ permission row (installed, permissions missing/unknown)
├─────────────────────────────────────────┤
│ English voice not installed  [Download] │ voice row (needed voice missing/downloading/failed)
├─────────────────────────────────────────┤
│ ● 14 thread · 1 dinleniyor     ölçümler ▴│ status line; click opens the metrics drawer
└─────────────────────────────────────────┘
```

### Rows
- Height 36 px (one line, T3's slim row) / 56 px (with state line). Hit area ≥ 28 px for every control (desktop pointer; E: G2-3 is for touch).
- Left: project mark (14–16 px, §2), then the title (14 px / 500, 1 line, ellipsis).
- Status glyph (14 px, static, T3's set): working = dashed circle in `info` with T3's working timer (`<1dk`, `16dk`, `1sa 9dk`, from `ThreadView.running_since`) in the time slot; waiting for you = `!` in warning; unseen completion = check in success (seen once the user listened, opened or selected the row this session; seen completions show no glyph, as in T3); interrupted/cancelled = pause in muted (our extra state). Tooltip with the Turkish label (`çalışıyor`, `seni bekliyor`, `bitti · henüz bakmadın`, `durduruldu`).
- Then provider mark (§2), listen toggle (🔈 muted / 🔊 listening, accent fill when on, equalizer while this row speaks) and relative time in T3's compact form (`şimdi`, `5dk`, `3sa`, `2g`; no `dün`).
- Hover (and selected): time is replaced by `💬 Cevapla` and `↗` (open in T3) buttons with a `fast` fade. Double-click on the row also opens in T3 (keep v0.2 behaviour).
- Unheard messages on a muted row: accent pill `3 yeni` (spring in when it increments).
- State line: current utterance text (italic, muted) while listening; `seni bekliyor: <onay/soru>` when waiting.

### Grouping and order (T3 parity)
T3's default flat sidebar, not project folders (those are T3's Labs "Boring sidebar"). A thread's section resolves like T3: snoozed → settled → pinned → working → active. Order: **Sabitlenmiş**, **Çalışıyor**, **Aktif**, then the **Ertelendi** and **Bitti** shelves, which start collapsed (T3). Inside a section the order is `Core::threads()` (stable). Every section header collapses/expands (state remembered for the session); a collapsed section with a waiting or listened thread shows a 6 px warning dot. The attention card links to waiting threads (expands the section, scrolls, highlight pulse once).

### Now playing bar
- Appears with a `normal` slide-up when any channel is speaking, paused or queued; disappears the same way.
- Shows thread label (project · title), the current sentence (2 lines max), progress bar (`ChannelView.now_elapsed_ms / now_total_ms`), queue count chip `N sırada`.
- Controls (all with tooltips + shortcuts): ⏸/▶ (Space), ⏭ `okey, geç` (→), ⇥ `canlıya geç` (L, only when queued), 🔇 `hepsini sustur` (M).

### Reply dock
Keep v0.2 behaviour (Wispr hands-free, paste/type, auto-send). Restyle to tokens; slide-up `normal`; state chip with icon; success toast `✓ Gönderildi` (spring) instead of text in place; errors stay in the dock (G2-5).

### Status line + metrics drawer
- Status line: connection dot + `N thread · M dinleniyor` + `ölçümler`.
- Drawer (expands `normal`): grid of the v0.2 HUD numbers (RSS, CPU, T3 poll, live poll, TTS, RTF, click→audio, msg→audio, dictation, queue, run dir) + two 60-second sparklines (RSS, CPU) from the 1 s `live_stats()` samples. Clicking the run dir opens it in Finder (v0.2).
- Drawer **Voices** section (below the grid, when `Core::voices()` is non-empty): a 26 px block for the title (`Sesler` / Voices) and one 26 px line per voice: 6 px state dot (success = ready, info = downloading, danger = failed, muted = not installed), the voice name (500), `language · size` or `language · state` in `text_muted`, and on the right `hazır` / ready, a ghost download button, a 60 px progress track + percent, or a ghost Retry. Below the voices, a 32 px **speed** row: `Hız` / Speed (500) on the left and on the right a segmented control (28 px tall, 1 px `border`, radius 7, 2 px inset) with the steps `1× 1,25× 1,5× 1,75× 2×` (Turkish decimal comma; `1.25×` in English) in tabular numerals; the active step is `accent` text on a 16 % `accent` tint (as active icon buttons), the others `text_muted` with a `surface_hover` hover. A click calls `Core::set_speed` (logged `ui.click {action: "speed", before, speed}`), which applies from the next utterance and saves the value; tooltip `konuşma hızı · bir sonraki mesajdan itibaren geçerli`. The drawer grows by 26 + 26 × voices + 32 px.

### Voice row
- 32 px row directly above the status line (below the permission row), only while the voice the panel needs (the UI language's, or `TELLAL_VOICE`'s) is not ready; fades in at `normal`. Missing: download icon in `info`, `İngilizce ses yüklü değil` / `English voice not installed`, button `Sesi indir (96 MB)` / `Download voice (96 MB)`. Downloading: dashed-circle icon in `info`, `İngilizce ses iniyor · 12 / 96 MB` / `Downloading the English voice · 12 / 96 MB`, tabular percent on the right and a 3 px `accent` track under the text. Failed: warning icon in `danger`, `… ses inmedi: <error>` / `… voice download failed: <error>`, ghost `Tekrar dene` / `Retry`. Key `D` triggers the row's action. The UI language's voice starts downloading by itself at first run, so most users only see the progress state; when it is ready the row leaves and a toast says `Ses hazır: <name>` / `Voice ready: <name>`.

### States
- Loading: header pill `bağlanıyor` / `ses yükleniyor`; list shows 3 placeholder rows *without* shimmer (E: skeleton myth). No usable voice: pill `ses yok` or `ses iniyor` plus the voice row.
- Empty: icon + `Şu an açık thread yok` + `T3'te yeni bir thread başlatınca burada görünür.`
- Error (`CoreEvent::Error`): inline banner under the header, `danger` icon, message + action (`Tekrar dene` / `Kayıtları aç`), dismissible.

### Compact mode
`⤡` (and ⌘⇧C) collapses the panel to header + Now playing (`slow`), for when you only want to listen. Remembered for the session. The computer-use banner stays visible in compact mode (safety first).

### Computer use (cua-driver)
Agents (omp in T3, or any MCP client) drive the screen through the local cua-driver daemon; the panel only watches (`Core::computer()`, `CoreEvent::ComputerUpdated`) and can stop them.
- **Banner** (top of the content, above the error banner and the attention card; safety first): only while `ComputerView.sessions` is non-empty. Pointer icon (`mouse-pointer-2`) in `danger`, `Ajan ekranını kullanıyor`, `N oturum` pill when > 1, elapsed time since the first session started at minute resolution (`<1dk`, `3dk`, like T3's working timer, so it re-renders once a minute), and one truncated line with the app and last action (`TextEdit · click "Kaydet"`; cua-driver itself reports no content, so real sessions show `client · son işlem 2dk`). Card: 10 % `danger` tint, 50 % `danger` border, 52 px; one `normal` slide-up on appear (Reduce Motion: 120 ms fade), then static: no pulsing, no ticking seconds (E: G1-10).
- **Durdur**: the one filled `danger` button (canvas-coloured label; core floors `danger` ≥ 4.5:1 against `canvas` in every theme, so the label keeps 4.5:1) → `Core::computer_stop_all()` (deny-only `revoke --all`) + toast `Ekran kontrolü durduruldu`. Keyboard: ⌘. in the panel (also while typing) and the global ⌃⌥⎋ from any app (env `TELLAL_STOP_HOTKEY`, `off` disables). The stop is always sent, even when the panel sees no session.
- **Permission row** (above the status line, 32 px): only when cua-driver is installed and its permissions are missing (`izin gerekli`) or unknown (`izin durumu bilinmiyor`, no daemon). `İzin ver` (I) → `Core::computer_grant_permissions()`; the row then says `macOS pencerelerini onayla: Erişilebilirlik ve Ekran Kaydı` until the grant is reported, then disappears with the toast `Bilgisayar kullanımı hazır`. Not installed → nothing in the main UI.
- **Drawer**: a `Bilgisayar kullanımı` section (cua-driver version or `kurulu değil`, servis, mod, izin, canlı oturum, durdur shortcut).

### New task (by voice)
`+` in the header (tooltip `Yeni görev (N)`, key N) opens the reply dock in new-task mode: label `→ Yeni görev · <project>` (`TELLAL_TASK_PROJECT`, else T3's `No project`), dictation target `NEW_TASK`, and every v0.2 dock behaviour (Wispr hands-free, focus/restore, paste/typing, History fill-or-compare, auto-send after Gönder, Esc cancels). Gönder → `Core::start_task(text)` (chip `Görev başlatılıyor…`); `TaskStarted` ok → toast `Görev başladı`, the dock closes, and the new thread is selected, its section expanded, scrolled to and pulsed once as soon as it is listed (it is already listening). Errors stay in the dock with the text (G2-5).

## 6. Keyboard (E: G2-11, every action has a non-pointer path)

| key | action |
|---|---|
| ↑ / ↓ | move selection (wraps inside the visible list) |
| Space | toggle listening on the selected row (pause/resume when the player has focus) |
| Enter or R | reply to the selected thread |
| ⌘O | open selected thread in T3 |
| P | pause/resume the playing channel |
| → | skip (`okey, geç`) |
| L | jump to live |
| M | mute all |
| ⌘K | search box (filters by title/project as you type; Esc clears) — T3's search key |
| ⌘⇧C | compact mode |
| Esc | close dock / search / drawer, in that order |
| N | new task by voice (dock in new-task mode) |
| I | `İzin ver` on the computer-use permission row (only while it shows) |
| D | the voice row's action: download or retry the needed voice (only while it shows) |
| ⌘. | stop every computer-use session (works while typing too) |
| Global ⌃⌥Space (env `TELLAL_HOTKEY`) | show/hide the panel from any app |
| Global ⌃⌥⎋ (env `TELLAL_STOP_HOTKEY`) | stop every computer-use session from any app |

## 7. Telemetry

Every new interaction logs `ui.*` through `Core::log_ui` (key, target, before/after state). Theme resolution logs `theme.resolve` (source, name, colours, ms) in core. The metrics drawer reads only `live_stats()` — no new polling. Computer use and new tasks: `ui.stop_hotkey` (registration `{spec, ok, error}` and every press `{action: "pressed", sessions, frontmost}`), `ui.click`/`ui.key` `computer_stop` / `computer_grant` / `new_task` / `task_select`, `ui.computer_banner` (shown/hidden, session count), `ui.computer_permissions` (before/after), `ui.task_send`, `ui.task_started` (ok, thread, detail, open ms).

## 8. Acceptance

- Screenshots (dark + light T3 theme): full, hover, compact, reply dock, empty, error, metrics drawer.
- Contrast check script over the resolved theme (text, text_muted, accent_text) ≥ 4.5:1, printed in the run log.
- Frame budget: no visible jank; idle CPU of the panel process ≤ v0.2 (measure 60 s idle, report).
- Review against `ui-evidence` `kb/GOLDEN-RULES.md` §3 checklist; list every deliberate deviation.

## 9. v0.3 implementation: deviations and review

Implemented in `crates/app/src` (modules: `theme`, `motion`, `icons` + `assets` (embedded via `build.rs`), `model` (sections), `panel/{header,list,player,dock,computer,metrics,keys,selftest}`, `hotkey`, `mac`). Screenshots, made from the demo fixture ([`demo/fixture.json`](../demo/fixture.json)), dark and light: [`docs/media/`](media/) (`panel-dark.png`, `panel-light.png`, `now-playing.png`, `reply.png`, `drawer.png`, `compact.png`, `computer.png`, `computer-permissions.png`, `new-task.png`).

### Deviations from the original text above

Parent decision (2026-11-11): where this spec and T3 differ on look and feel, T3 wins; the evidence-based behaviour rules stay. The spec text in §2, §3, §5 and §6 is already updated; the list records what changed and the remaining gaps.

1. **Header** (§2): T3's wordmark treatment (bold name, 14 px) after the Tellal mark (megaphone, `accent`), where T3 shows no mark; the state pill only when not live, a 6 px success dot when live. Source: T3 F5 (no icon in T3's header); the mark is ours.
2. **Grouping** (§5): T3's default flat sections (Sabitlenmiş, Çalışıyor, Aktif, Ertelendi, Bitti) instead of project folders and one "Bitti / Ertelendi" shelf. The project moves onto every row. T3 shows the Pinned/Active labels only while dragging; the panel shows every section label, because it has no drag and the labels are its only orientation.
3. **Status glyphs** (§5 Rows): T3's set. Working = static dashed circle in `info` + T3's working timer from `ThreadView.running_since` (minute resolution `<1dk`/`16dk`/`1sa 9dk`, so the list re-renders once a minute instead of every second). Check only for an *unseen* completion: "seen" is per session (listened, opened, replied or selected), and completions older than the panel's start count as seen (core has no persistent unseen flag). Interrupted/cancelled = muted pause (our state; T3 has none).
4. **Sizes**: rows 36 px / 56 px with the state line, titles 14 px / 500, section labels 12 px / 500 sentence case, provider marks 14 px at 60 %. Nothing below 11 px.
5. **Project monogram tile**: T3's two letters and name-hashed colour, but at 11 px bold instead of T3's 8.25 px mono (§1.8 legibility floor); the tile widens to fit.
6. **ACP-registry provider badge**: T3 overlays a 7 px `OM`/`OF` pill on the mark; the panel puts an 11 px bordered tile next to it (legibility floor).
7. **Relative time**: T3's compact form `şimdi / 5dk / 3sa / 2g`; no `dün`.
8. **Search**: ⌘K (T3's key); ⌘F unbound.
9. **Hover** (§5 Rows): the row background changes immediately (gpui's hover style cannot be animated); the actions that replace the time fade in at `fast`. Pressed state is immediate (≤ 85 ms, G2-6).
10. **Waiting state line**: `seni bekliyor: onay ya da cevap gerekiyor`. Core does not say whether the thread wants an approval or an answer, so the glyph is one amber `!` for both (T3 uses `shield-question` / `message-circle-question`).
11. **Progress bar**: drawn from the latest `channels()` snapshot, which the pump polls every 150 ms while a channel is audible (core has no per-tick event and `now_total_ms` grows while synthesis is still running, so interpolating toward the total would overshoot). Static under Reduce Motion and while paused.
12. **Reply dock success**: the dock closes at once and `✓ Gönderildi` (spring) appears as a toast; v0.2 kept the dock 1.2 s with an in-place chip. All other v0.2 behaviour is unchanged (Wispr hands-free, focus/activate/restore, paste/typing, History fill-or-compare, auto-send after Gönder, retarget, logs).
13. **Error banner**: `Tekrar dene` appears only when the failed action is known and repeatable (opening a thread); other errors offer `Kayıtları aç` and dismiss.
14. **Window height**: computed from fixed region heights (header 40, banner 76, attention 48, search 44, sections 32, rows 36/56, player 112, dock 116, permission row 32, voice row 32, status 32, drawer 288 + 26 + 26 per voice + 32 (speed row) when voices are listed), clamped to 160–640; past 640 the list scrolls. Changes animate at `normal`, compact ↔ full at `slow`; Reduce Motion jumps.
15. **Global shortcut**: hide = `orderOut`; show = `makeKeyAndOrderFront` on the non-activating panel, so it takes the keyboard without bringing the app forward. Re-hiding re-activates the app that was frontmost. `TELLAL_HOTKEY=off` disables it (used by test instances).
16. **Additions not in T3**: the attention card, the Now playing bar, the listen toggle, the reply dock and the metrics drawer are panel features with no T3 counterpart; they follow the tokens and T3's control styles.
17. **Computer use and new task** (v0.4): the computer-use banner, the `ekran` pill, the permission row, the drawer section and the `+` new-task action have no T3 counterpart. The banner sits *above* the attention card and the error banner (stopping an agent that acts on the screen outranks answering one). The emergency stop's global default is ⌃⌥⎋: not taken by macOS (⌘⌥⎋ is Force Quit), registrable through Carbon without Accessibility permission, and next to Force Quit in meaning; ⌘. is the in-panel stop (macOS's "cancel/stop" convention). The banner's elapsed time is at minute resolution on purpose (no ticking seconds). `Durdur` is the only `danger`-filled control in the panel.

### Measurements (2026-11-11)

- **Idle CPU** of a test instance (`TELLAL_HOTKEY=off`, nothing listening), 60 one-second samples from its `metrics.jsonl` (samples 10–69 after start): average **1.69 %**, median 1.04 %, max 14.4 % (one T3 poll burst). v0.2 baseline: 3–6 %. Idle renders happen once a minute (relative times) or on events; the open drawer re-renders once a second.
- **Idle CPU with computer use** (v0.4, 2026-11-11): 60 s demo run (`TELLAL_DEMO=demo/fixture.json TELLAL_OUTPUT_VOLUME=0 TELLAL_HOTKEY=off`, nothing listening, the fixture's computer-use sessions showing the banner 8–35 s and from 50 s, the stop hotkey registered), samples 10–69: average **2.02 %**, median 0.68 %, max 8.98 % (scripted demo events), RSS median 161 MB.
- **Contrast** (logged as `ui.theme` / `ui.ready.theme.contrast` on every theme application). T3 Chat dark: text/canvas 18.69, text_muted/canvas 12.87, text_muted/surface 10.54, text_muted/selected 11.80, accent_text/accent 6.19. T3 Chat light: text/canvas 16.85, text_muted/canvas 5.49, text_muted/surface 6.28, text_muted/selected 4.54, accent_text/accent 4.57. All ≥ 4.5:1. `danger` on `surface` is 3.70:1 in the dark theme; danger is used only as icon/pill colour on canvas tints (core guarantees ≥ 4.5:1 on canvas), never for body text. Computer use (dark): canvas/danger (the `Durdur` label) 4.51, text/danger_tint (banner title) 16.99, text_muted/danger_tint (banner detail) 11.71.
- **Theme switch**: a live appearance change in a temp copy of T3's settings produced `ThemeChanged` → `ui.theme` within the 2 s watcher tick; tokens cross-fade over 200 ms (120 ms under Reduce Motion).

### ui-evidence §3 checklist

1. *Clickable vs not (G1-1)*: buttons have a border or fill (listen toggle bordered when off, accent-filled when on; Cevapla bordered; primary Gönder filled); section headers and rows show hover/pressed backgrounds; static text has none. ✓
2. *Primary info top-left (G1-2, G2-8)*: attention card directly under the header; list starts in the first 80 px; position, not colour, carries priority. ✓
3. *Icon-only controls (G1-6, G4-1)*: every icon button has a Turkish tooltip with its shortcut; abstract functions (Cevapla, Listeyi aç, Tekrar dene, Kayıtları aç, Gönder/İptal) carry visible labels. gpui exposes no accessibility tree, so there is no screen-reader name. ✗ (platform gap, see 13)
4. *Target size (G2-3 …)*: desktop pointer; every control ≥ 28 × 28 px with ≥ 4 px gaps. The touch thresholds do not apply. ✓ (desktop)
5. *Hidden-gesture-only functions (G2-11)*: double-click open also has `↗` and ⌘O; hover actions also appear on the selected row and on Enter/R; drag-to-move is the only gesture and has no alternative (the panel stays where it opens). ✓
6. *User's words (G2-2)*: Turkish labels from the user's own vocabulary (Cevapla, okey geç, canlıya geç, hepsini sustur, seni bekliyor). No first-click test run yet. ◐
7. *Text size/contrast/colour-only (G1-3, G1-4, G1-5)*: ≥ 11 px everywhere, body 13–14 px; contrast above; status = shape + colour + tooltip. ✓
8. *Unrequested motion (G1-10, G1-11)*: nothing moves unless audio the user enabled plays (equalizer); position changes (dock, player, attention card, rows, window height) use short transitions; Reduce Motion honoured. ✓
9. *Feedback for slow requests (G2-7)*: header pill while connecting/loading voice; dock chip `Gönderiliyor…` / `Wispr metni bekleniyor…`; open-in-T3 logs and errors surface in the banner. ✓
10. *Errors next to the field, say what to do (G2-5, G3-11)*: reply errors stay in the dock with the text kept and "Gönder ile tekrar dene"; global errors in the banner with an action. ✓
11. *Reject as visible as accept (G3-1)*: İptal and Gönder sit side by side at equal size; İptal is bordered, Gönder filled. ◐ (unequal weight is deliberate: Gönder is the expected path)
12. *Interruptions (G3-8)*: no modals; the attention card and toast never take focus; the panel takes the keyboard only for the reply dock or the global shortcut. ✓
13. *Screen reader and keyboard tried (G4-5, G4-6)*: full keyboard map implemented (§6) but not exercised by hand in this pass (no synthetic keystrokes on the user's machine); screen readers cannot see gpui content. ✗
14. *AI output uncertainty (G4-18, G4-21, G4-22)*: the panel reads agent text verbatim and links to the source thread (↗ / ⌘O); it adds no AI judgement. n/a
15. *Behavioural success metric (G1-9, G4-13)*: every interaction logs `ui.*` with before/after; candidate metrics: time from `ui.attention_jump` to `ui.reply_send`, share of replies via keyboard, click→audio and msg→audio latency in the drawer. ◐ (instrumented, not yet analysed)
