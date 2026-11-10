# t3-panel v0.3 — design spec

The panel is a remote control for T3 Code that you *listen* to. It sits above every app, so it must
be calm, small and instantly readable. This spec is the contract for the v0.3 redesign; every rule
cites where it comes from.

Sources:
- **E** = evidence rules, `~/.agents/skills/ui-evidence/` (`kb/GOLDEN-RULES.md`, G-IDs, grade A/B).
- **A** = Atlas design notes saved in [`research/ui/atlas/`](../research/ui/atlas/): "Tasarımın 14 kuralı", "15 grafik tasarım trendi", Kıble and Kazan design systems (token architecture, motion, empty states).
- **T3** = T3 Code's own sidebar (study: [`research/ui/t3-sidebar.md`](../research/ui/t3-sidebar.md), source: github.com/pingdotgg/t3code `apps/web`). The panel should feel like a detached piece of that sidebar.

## 1. Principles

1. **Calm by default** (A: Kıble §3.1; E: G1-8). Low visual complexity: one surface colour, one accent, hairline borders, no decorative gradients. Detail appears only when it carries state.
2. **One focal point** (A: rule 01 Emphasis, 10 Hierarchy). The thing being spoken is the hero: the *Now playing* bar. Everything else is quieter.
3. **Position carries priority, not colour** (E: G1-2). Attention items at the top, the player at the bottom, the list in between; these places never move.
4. **Stable order** (E: G2-4; T3: "thread activity does not change the order"). Rows never jump because a thread got a new message. New threads appear at the top of their group with a short enter transition (E: G1-11).
5. **Every clickable thing looks clickable; nothing lives only behind hover** (E: G1-1, G2-11). The listen toggle is always visible. Row actions that appear on hover are also on the keyboard and on the selected row.
6. **Meaning never by colour alone** (E: G1-5). Status = icon shape + colour + tooltip text.
7. **Motion explains state, never decorates** (A: Kıble §56; E: G1-10, G1-11). Nothing moves continuously unless the user started it (audio they unmuted). Running threads get a *static* activity glyph, not a spinner. Honour macOS Reduce Motion.
8. **Legible** (E: G1-3, G1-4). Body 13 px, secondary 12 px, nothing under 11 px (11 only for badges and section labels). Text ≥ 4.5:1 contrast on its background in every theme.
9. **Speak the user's words** (E: G2-2, G3-11, G3-12). Turkish UI copy, short, first word carries meaning; errors say what happened and what to do.

## 2. Identity (T3 parity)

- Header mirrors T3's sidebar header: app mark (16 px) + wordmark **T3 Panel** (13 px semibold) + a small pill like T3's channel badge, showing the panel's own state: `canlı` (connected), `bağlanıyor`, `ses yükleniyor`, `hata`.
- The app mark is our own (do not reuse T3's logo as ours): a rounded square in the accent colour with a "T3" monogram and a small sound-wave notch. SVG in `crates/app/assets/logo.svg`; the same SVG is the Dock-less app's about icon.
- Provider marks on rows like T3 shows them (Claude, OpenAI/Codex, omp, …): small 12 px monochrome glyphs from T3's own assets where available, otherwise a 2-letter monogram. Tooltip = provider + model.
- Project groups use the project icon from T3 (`ThreadView.project_icon`, lucide name + T3 colour name) or the project's first letter in a tinted rounded square, exactly as T3 falls back.

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

Type: T3's interface font (`ThemeView.font_family`, empty = system) at `font_size - 1` for body (13 px when T3 is at 14), secondary −2, badges 11. Weights: 600 titles, 500 labels, 400 body. Tabular numerals for times and metrics.

Spacing scale 4 / 8 / 12 / 16; radius 6 (controls), 10 (cards), 14 (window). Elevation: one shadow for the window, none inside (A: Kazan/Kıble elevation: shadows only for floating layers).

## 4. Motion tokens (A: Kıble §57–59)

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
│ ▣ T3 Panel  (canlı)        ⌕  ▤  ⤡  ✕  │ header 40px, drag area
├─────────────────────────────────────────┤
│ ⚠ 2 thread seni bekliyor  ·  Kazan, …  →│ attention card (only when ≥1 waiting)
├─────────────────────────────────────────┤
│ 📌 Sabitlenmiş                           │ section labels (11px caps, muted), T3 order
│ ▾ 🍲 Kazan                         3    │ project group header, collapsible
│   ◉  Kalan işleri planla      🔊  2 dk │ row: status · title · listen · time
│      “Şimdi testleri çalıştırıyorum.” ▮▮▮│ second line only when it carries state
│   ✓  İsim kontrolü              🔈 1 sa │
│ ▸ 💬 No project                    12   │
├─────────────────────────────────────────┤
│ ♪ Kazan · Kalan işleri planla      2 sırada│ Now playing (only when speaking/paused/queued)
│ “Şimdi testleri çalıştırıyorum.”        │
│ ━━━━━━━━━━━━━━━━━━──────   ⏸  ⏭  ⇥  🔇 │ progress + controls
├─────────────────────────────────────────┤
│ → Kalan işleri planla                   │ Reply dock (when open)
│ [ metin kutusu                       ]  │
│ 🎙 Wispr dinliyor…        [İptal][Gönder]│
├─────────────────────────────────────────┤
│ ● 14 thread · 1 dinleniyor     ölçümler ▴│ status line; click opens the metrics drawer
└─────────────────────────────────────────┘
```

### Rows
- Height 32 px (one line) / 52 px (with state line). Hit area ≥ 28 px for every control (desktop pointer; E: G2-3 is for touch).
- Status glyph (14 px): running = static activity ring (accent), waiting for you = `!` in warning, completed/idle = check (success, muted when settled), interrupted/cancelled = pause bars (warning), unknown = dot (muted). Tooltip with the Turkish label (`çalışıyor`, `seni bekliyor`, `bitti`, `durduruldu`).
- Title (13 px, 1 line, ellipsis) + provider glyph before the time. Right: listen toggle (🔈 muted / 🔊 listening, accent fill when on, equalizer while this row speaks) then relative time (`şimdi`, `3 dk`, `2 sa`, `dün`).
- Hover (and selected): time is replaced by `💬 Cevapla` and `↗` (open in T3) buttons with a `fast` fade. Double-click on the row also opens in T3 (keep v0.2 behaviour).
- Unheard messages on a muted row: accent pill `3 yeni` (spring in when it increments).
- State line: current utterance text (italic, muted) while listening; `seni bekliyor: <onay/soru>` when waiting.

### Grouping and order (T3 parity)
Sections in T3's order: **Sabitlenmiş** (pinned threads), then project groups (pinned projects first, then the order in `Core::threads()`), then a collapsed **Bitti / Ertelendi** shelf for settled and snoozed threads. Project groups collapse/expand with a chevron (`normal`), state remembered for the session. A thread waiting for you also stays in its group; the attention card links to it (scroll + selection highlight pulse once).

### Now playing bar
- Appears with a `normal` slide-up when any channel is speaking, paused or queued; disappears the same way.
- Shows thread label (project · title), the current sentence (2 lines max), progress bar (`ChannelView.now_elapsed_ms / now_total_ms`), queue count chip `N sırada`.
- Controls (all with tooltips + shortcuts): ⏸/▶ (Space), ⏭ `okey, geç` (→), ⇥ `canlıya geç` (L, only when queued), 🔇 `hepsini sustur` (M).

### Reply dock
Keep v0.2 behaviour (Wispr hands-free, paste/type, auto-send). Restyle to tokens; slide-up `normal`; state chip with icon; success toast `✓ Gönderildi` (spring) instead of text in place; errors stay in the dock (G2-5).

### Status line + metrics drawer
- Status line: connection dot + `N thread · M dinleniyor` + `ölçümler`.
- Drawer (expands `normal`): grid of the v0.2 HUD numbers (RSS, CPU, T3 poll, live poll, TTS, RTF, click→audio, msg→audio, dictation, queue, run dir) + two 60-second sparklines (RSS, CPU) from the 1 s `live_stats()` samples. Clicking the run dir opens it in Finder (v0.2).

### States
- Loading: header pill `bağlanıyor` / `ses yükleniyor`; list shows 3 placeholder rows *without* shimmer (E: skeleton myth).
- Empty: icon + `Şu an açık thread yok` + `T3'te yeni bir thread başlatınca burada görünür.`
- Error (`CoreEvent::Error`): inline banner under the header, `danger` icon, message + action (`Tekrar dene` / `Kayıtları aç`), dismissible.

### Compact mode
`⤡` (and ⌘⇧C) collapses the panel to header + Now playing (`slow`), for when you only want to listen. Remembered for the session.

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
| ⌘F | search box (filters by title/project as you type; Esc clears) |
| ⌘⇧C | compact mode |
| Esc | close dock / search / drawer, in that order |
| Global ⌃⌥Space (env `T3P_HOTKEY`) | show/hide the panel from any app |

## 7. Telemetry

Every new interaction logs `ui.*` through `Core::log_ui` (key, target, before/after state). Theme resolution logs `theme.resolve` (source, name, colours, ms) in core. The metrics drawer reads only `live_stats()` — no new polling.

## 8. Acceptance

- Screenshots (dark + light T3 theme): full, hover, compact, reply dock, empty, error, metrics drawer.
- Contrast check script over the resolved theme (text, text_muted, accent_text) ≥ 4.5:1, printed in the run log.
- Frame budget: no visible jank; idle CPU of the panel process ≤ v0.2 (measure 60 s idle, report).
- Review against `ui-evidence` `kb/GOLDEN-RULES.md` §3 checklist; list every deliberate deviation.
