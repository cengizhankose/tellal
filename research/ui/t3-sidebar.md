# T3 Code sidebar — design study for t3-panel v0.3

This study measures T3 Code's thread sidebar so the 380 px panel can feel like a detached piece of it.

**Sources and conventions**
- **Code** is T3 Code `main` at commit
  [`17a6a01d84e3adf9f29df2639d6b6079a434e1b2`](https://github.com/pingdotgg/t3code/tree/17a6a01d84e3adf9f29df2639d6b6079a434e1b2)
  (2026-11-10).
  - A ref like `SidebarThreadRow.tsx:1437` means `apps/web/src/components/sidebar/SidebarThreadRow.tsx`, line 1437.
  - Bare component names live under `apps/web/src/components/`.
  - Full URL form: `https://github.com/pingdotgg/t3code/blob/17a6a01d84e3adf9f29df2639d6b6079a434e1b2/<path>#L<n>`.
- **Bundle** is the installed `T3 Code (Nightly)` 0.1.8-nightly.20261109.4399. Its extracted copy is `/tmp/t3asar`. The CSS is `apps/server/dist/client/assets/main-vDPMgj0W.css`. Every value I checked matches `index.css`.
- **Shot** is a real screenshot of the user's T3 window: [`shots/t3-window-theme.png`](shots/t3-window-theme.png).
  - The sidebar crop is [`shots/t3-sidebar-crop.png`](shots/t3-sidebar-crop.png), at 1×, 396 pt wide.
  - The user's sidebar is resized to 396 pt. The default width is 256.
- **Units:** pixel values assume T3's default 16 px root.
  - Tailwind v4: 1 unit = 4 px (bundle `--spacing:.25rem`); `text-xs` = 12/16; `text-sm` = 14/20.
  - The interface-size setting (12–20 px) scales every rem value (`packages/contracts/src/settings.ts:166-172`, `appearanceFonts.ts:72-105`).
- **Rendering:** where T3 uses `color-mix`/OKLCH, the hex given is the sRGB value Chromium shows on an sRGB display.
- `[INFERENCE]` marks anything not read directly.

---

## A. Container and tokens

**F1 — Width and surface.**
- Width: default 256 px, minimum 208 px (`threadSidebarWidth.ts:2-3`). The maximum is viewport − 640 (`threadSidebarWidth.ts:4,10-15`).
- Background: `bg-sidebar surface-grain` (`ui/sidebar.tsx:389`).
- Right edge: a 1 px border, `color-mix(contrast-sidebar-foreground 10%, transparent)` (`ui/sidebar.tsx:382`, `index.css:1584`) [INFERENCE: resolved through `[data-app-sidebar]`].
- Content inset: the list group has 8 px padding all round (`--sidebar-content-inset: .5rem`, `index.css:104`; `ui/sidebar.tsx:686-692`).
- Rows sit 1 px apart (`gap-px`, `Sidebar.tsx:3331-3332`).
- The scroll area hides scrollbars and fades its edges over 12 px (`ui/sidebar.tsx:662-669`).

**F2 — Sidebar colour tokens.** Default theme; light values from `index.css:1245-1252`, dark from `index.css:1264-1276`.

| token | light | dark (in `[data-app-sidebar]`) | used for |
|---|---|---|---|
| `--sidebar` | zinc-50 `#fafafa` | `#000` | surface |
| `--sidebar-foreground` | zinc-900 `#18181b` | `#f1f3f7` | titles |
| `--sidebar-muted-foreground` | zinc-500 `#71717b` | `#a3a3a3` | section headers, counts |
| `--secondary-label` | = muted-foreground | `#8f8f8f` | project name, time, meta line |
| `--sidebar-row-hover` | zinc-25 `oklch(99.2% 0 0)` | foreground @ 8 % | row hover |
| `--sidebar-row-selected` | white | foreground @ 7 % | selected row |
| `--sidebar-row-active` | white (+ 1 px inset ring `sidebar-border/70`) | foreground @ 11 % | open thread |
| `--sidebar-border` | zinc-200 `#e4e4e7` | `rgb(255 255 255 / 8%)` | hairlines |

Custom and built-in themes override these through `--app-theme-*` (`index.css:1374-1382`). This is why `Core::theme()` must resolve per theme rather than hard-code them.

**F3 — Status colour tokens.** Sources: `index.css:1147-1165,1208-1217`; hex from the bundle palette.

| role | light (text) | dark (text) | fill (dots) |
|---|---|---|---|
| info (working) | blue-700 `#1447e6` | blue-400 `#51a2ff` | blue-500 |
| success (done) | emerald-700 `#007a55` | emerald-400 `#00d492` | emerald-500 |
| warning (approval, woke, limited) | amber-700 `#bb4d00` | amber-400 `#ffb900` | amber-500 |
| error (failed) | red-700 `#c10007` | red-400 `#ff6467` | red-500 |
| input (question) | indigo-600 `#4f39f6` | indigo-300 `#a3b3ff` | — (`SidebarThreadRow.tsx:770-776` via `Sidebar.logic.ts:861-891`) |
| guest request | fuchsia-600 `#c800de` | fuchsia-300 `#f4a8ff` | — |

`--primary` (focus ring, drop targets) is `oklch(.488 .217 264)` in light and `oklch(.571 .21 264)` in dark (`index.css:1130,1199`).

**F4 — Radii and type scale.**
- Radii: `--radius` 8 px (`index.css:1115`). From it, `rounded-sm` 4, `rounded-md` 6, `rounded-lg` 8, `rounded-xl` 12 (`index.css:289-292`). `--control-radius` 8 (`index.css:103`).
- Thread rows use `rounded-md`, 6 px (`SidebarThreadRow.tsx:958`). Folder headers and header buttons use the 8 px control radius.
- Fonts:
  - `--font-sans` is `-apple-system, BlinkMacSystemFont, "Inter Variable", …` (`index.css:171-172`).
  - T3 adds `text-2xs` 11 px, `text-3xs` 10, `text-4xs` 8 and `text-5xs` 7 (`index.css:183-190`).
- `font-sans`, `font-mono` and the root size come from Appearance settings (`appearanceFonts.ts:72-105`, keys `fontSizeInterface`/`fontSizePrompt`/`fontSizeCode` in `settings.ts:467-475`).

## B. Header anatomy

**F5 — Header bar.** Source: `sidebar/SidebarChrome.tsx:43-93`.
- Container: `flex h-[var(--workspace-topbar-height)] items-center gap-2 pr-3`.
  - Height is **52 px** (`index.css:123`), or 40 px under window-controls-overlay (`index.css:156`).
  - It is an Electron drag region.
- Inner row: 32 px tall (`h-8 … py-0.5`, `:74`).
- Left to right:
  1. **Sidebar toggle.** On desktop it is a separate fixed 28 px ghost button with a 16 px `PanelLeftClose` ↔ `PanelLeft` morph. Tooltip "Toggle main sidebar (⌘B)" (`AppSidebarLayout.tsx:376-405`, `ui/sidebar.tsx:401-425`). It sits right of the macOS traffic lights; the brand's left margin leaves room for it (`ui/sidebar.tsx:235-236`).
  2. **Brand:** a 28 px tall link, "T3 Code home" (`SidebarChrome.tsx:167-178`). It contains:
     - the **T3 wordmark SVG** (`T3Wordmark.tsx:5-9`, one `currentColor` path, viewBox `15.5309 37 94.3941 56.96`), sized `h-[1cap]`, about 10 px tall and 16–17 px wide [INFERENCE from the aspect ratio];
     - 4 px gap, then the word **"Code"**;
     - wrapper `inline-flex items-baseline gap-1 text-sm font-medium tracking-tight`: 14 px, weight 500, −0.025 em.
     - "T3" is `text-foreground` and "Code" is `text-muted-foreground` (`:182-193`). The shot shows a bold-white "T3" next to grey "Code".
  3. **Back / forward**, Electron only: `ArrowLeft`/`ArrowRight` at 14 px in 28 px ghost buttons (`:77-82,123-158`).
  4. **Channel badge**, only in `pill` identification mode (`:54-57,84-90`):
     - `<Badge size="sm" variant="secondary">` reading **"Nightly"** or **"Dev"** (`SidebarStageBackdrop.tsx:27-34`).
     - Size: 16 px tall, **10 px text**, weight 500, 3 px side padding, 4 px radius (`ui/badge.tsx:8,22,31`).
     - Colours: light `#fafafa` background with zinc-800 text; dark white @ 3 % background with neutral-100 text (`index.css:1132-1133,1200-1201`).
- **There is no app icon, no product logo image and no "by …" credit line in the header.**
  - `T3AppIcon` (`T3AppIcon.tsx`) is a 180 px PNG used only in source-control sign-in UI.
  - The window title carries the channel instead: "T3 Code (Nightly)", via `formatAppDisplayName` → `"${base} (${stage})"` (`branding.logic.ts:3-12`, stage text from `branding.ts:13-26`).
- The stage label is resolved as:
  - "Nightly" when the server version matches `-nightly.YYYYMMDD.N` (`branding.logic.ts:1,14-22`);
  - otherwise "Dev" in development and "Beta" elsewhere.

**F6 — Channel artwork (the default and the user's setting).**
- `environmentIdentificationMode` defaults to `"artwork"` (`SidebarStageBackdrop.tsx:16`). This Mac has `"artwork"` (`~/.t3/userdata/client-settings.json`).
- For Nightly, the header has no pill. It draws an **80 px starry purple backdrop** behind it (`SidebarStageBackdrop.tsx:52-60`).
  - The backdrop is procedural SVG `0 0 8192 96`, faded with `linear-gradient(black 0–55 %, transparent 92 %)` (`index.css:528-532`).
  - Over it, brand and buttons turn white, with "Code" at 70 %.
- This is the purple glow at the top of the shot.

**F7 — Search row**, directly under the header (`sidebar/SidebarThreadHeader.tsx:60-164`).
- Search field: `h-8 flex-1 gap-2 rounded-md px-2 text-sm font-medium text-sidebar-muted-foreground`, with a 16 px `search` icon and placeholder "Search".
  - The field shows a hover background; when focused it adds a 1 px inset ring at `ring/70`.
  - While a query is present, a clear button appears: 20 px with a 12 px `x`.
- Right: 28 px icon buttons, in the shot as filter (`list-filter`), `folder-plus` ("Add project") and `square-pen` ("New thread", ⌘N).
- **Footer** (`SidebarChrome.tsx:259-290`): 32 px icon buttons `settings`, pull-requests glyph and `chart-no-axes-column` ("Usage"). In the shot an update button sits on the right.

## C. Thread rows

**F8 — Four row variants** (`SidebarThreadRow.tsx:540`; chosen at `Sidebar.tsx:3351-3359`):
- **card:** for pinned, active and working threads.
- **slim:** for snoozed and settled threads.
- **folder / folder-multiline:** used only with "Boring sidebar".

**F9 — Card row geometry** (`SidebarThreadRow.tsx:1402-1600`). This is what the default sidebar and the shot show.
- The `<li>` has 2 px vertical padding (`py-0.5`, `:1408`).
- The row is **78 px tall** with 10 px side padding and 8 px vertical padding: `h-[4.875rem] px-(--sidebar-row-content-inset) py-(--sidebar-content-inset)`, `:1437`.
  - 78 = 8 + 20 + 4 + 20 + 2 + 16 + 8.
  - With no meta line the row is auto height, about 60 px [INFERENCE].
- Surface: 6 px radius.
  - Hover is `bg-sidebar-row-hover`; selected is `-selected`; open is `-active`.
  - Focus is a 2 px inset ring (`:957-978`).
  - An unsent draft tints the row `bg-warning/4`, `hover:/8` (`:438`).
- **Line 1** (20 px, gap 6; `:1441-1524`), left to right:
  1. draft pen, 12 px;
  2. **project icon, 16 px**;
  3. pin indicator, 12 px;
  4. **project name**, 12 px / 400 in `--secondary-label`, truncating;
  5. status slot on the right (`ml-auto min-w-8`, 32 px minimum).
- **Line 2** (20 px, 4 px above; `:1586`): the **thread title**.
  - It is **14 px / 500**, `text-foreground`, single line with ellipsis (`:1026-1054`).
  - When the meta line is empty, the provider icon follows the title on this line (`:1193-1201`).
- **Meta line** (16 px, 2 px above; `:1153`): 12 px `--secondary-label`, gap 6.
  - Left: the branch (worktree icon plus middle-truncated name) or the machine label (laptop icon plus "Sino's MacBook Pro (2)").
  - Then: terminal and PR badge (`git-pull-request` icon plus number, green when open, e.g. "↱ 899").
  - Right, `ml-auto gap-1`: the machine icon, then the **provider icon**.
- **Receded rows** (`Sidebar.logic.ts:827-849`) are working, waiting, or done-and-seen threads.
  - The title drops to `text-foreground/75` and weight 400.
  - A working row is also at `opacity-70` until hovered (`:957-978,1026-1054`).
  - Rows needing you (input, request, unread) never recede.
  - In the shot: "First mate" (Done, unseen) is bright 500, and the working rows are grey.

**F10 — Slim row** (`SidebarThreadRow.tsx:1203-1400`).
- Size: **36 px** tall (`h-9`), 10 px gap and side padding (`:1230-1237`).
- Left to right:
  - project icon, 16 px; it is `opacity-40 grayscale` unless the row is hovered or active (`:1250-1258`);
  - title, 14 px;
  - pin;
  - time slot (24 px tall, ≥ 32 px wide, `:1276`).
- No provider icon and no second line.
- Folder rows add 36 px left padding (`ps-9`) so titles line up with the folder name (`:1236`).

**F11 — Relative time.**
- Formatter: `formatRelativeElapsed` (`apps/web/src/timestampFormat.ts:262-271`), then `compactSidebarTimeLabel` (`SidebarThreadRow.tsx:137-140`). Output:
  - `now` under 60 s;
  - `Nm` under 60 min;
  - `Nh` under 24 h;
  - `Nd` beyond that, with no upper bound: no "yesterday", no dates, no "ago".
- Units are floored (`timestampFormat.ts:230-247`). Future timestamps show `now`.
- Rendered as 12 px `tabular-nums` in `--secondary-label` (`:1283,1322`), re-rendered on a 60 s tick (`useNowMinute`, `:144-151`; `client-runtime/src/clock.ts:67-71`).
- Timestamp source: the latest user send or run request/start/complete, falling back to `updatedAt`. Settled rows use `settledAt` (`packages/client-runtime/src/state/threadSort.ts:26-52`).
- No absolute-time tooltip on sidebar rows. The row tooltip is a hover card with project, machine, branch and model (`:231-368`).
- The shot shows "57m", "1d", "10h", "2d" and "16h".

**F12 — Working duration** (a different clock).
- Shown after the "Working"/"Setting up"/"Waiting" label on card rows.
- `formatWorkingDurationLabel` (`packages/client-runtime/src/state/models.ts:395-401`) gives `Ns` → `Nm` → `Nh Mm` (e.g. `1h 9m`).
- It ticks every second (`SidebarThreadRow.tsx:164-177`, `:1510-1518`).

**F13 — Hover actions** replace the time in the same slot, never added beside it.
- The time gets `absolute opacity-0`. The action cluster goes from `opacity-0` to `opacity-100`, on `any-hover` or a focus-visible descendant (`SidebarThreadRow.tsx:1468-1578`; the variant is in `index.css:11-15`).
- Card rows:
  - discard draft: `x` at 14 px;
  - snooze: `clock` at 12 px, opening the snooze menu;
  - settle: `check` at 14 px plus the word "Settle".
  - Each is `px-1.5 text-xs` in a ghost-muted style.
- Slim rows:
  - settle: `check` 12;
  - un-settle: `undo-2` 14;
  - wake: `alarm-clock-off` 12 (`:1345-1378`).
- Pressing an action and dragging sweeps it across neighbouring rows (`:892-896`; `docs/user/thread-sidebar.md:183-186`).
- Holding ⌘ swaps the slot for a `Kbd` jump hint, 1–9 (`:153-162`).

**F14 — Pin indicator.** `pin` at 12 px in `text-muted-foreground/65`. It becomes a `pin-off` button ("Unpin thread") on hover (`SidebarThreadRow.tsx:1103-1136`).

## D. Status and indicator semantics

**F15 — Row status.**
- It is resolved by `resolveSidebarThreadStatus` (`Sidebar.logic.ts:861-891`); the first match wins.
- It renders in the right slot of line 1 as an **icon (16 px on cards, 12 px on folder rows) + label (12 px / 500) + optional duration**, all in the status colour (`SidebarThreadRow.tsx:520-534, 1499-1518`).
- **No status icon animates.** Working is a *static* dashed circle (`:1506-1508` comment; icons in `TOP_STATUS_ICONS`, `:520-529`).

| # | condition | label | lucide icon | colour |
|---|---|---|---|---|
| 1 | pending approval | Approval | `shield-question` | warning-foreground |
| 2 | pending user input | Input | `message-circle-question` | indigo-600 / indigo-300 |
| 3 | pending guest message | Request | `user-round` | guest (fuchsia) |
| 4 | running but blocked on a thread wait | Waiting + duration | `circle-dot-dashed` | muted-foreground |
| 5 | runtime preparing | Setting up + duration | `circle-dashed` | info-foreground |
| 6 | queued / starting / running | **Working** (or "Goal") + duration | `circle-dashed` | info-foreground |
| 7 | runtime idle with background work | Waiting + duration | `circle-dot-dashed` | muted-foreground |
| 8 | failed | Failed, or Limited (usage limit) | `circle-alert` | error-foreground / warning-foreground |
| 9 | ready, woken from snooze | Woke (a button that dismisses it) | `alarm-clock` | warning-foreground |
| 10 | ready with an unseen completion | **Done** | `circle-check` | success-foreground |
| 11 | ready and seen | — (time only) | — | — |

These sources back the table:
- Rows 9–10: `SidebarThreadRow.tsx:778-789`, with unseen completion from `Sidebar.logic.ts:698-707`.
- Row 8: "Limited" is documented at `docs/user/thread-sidebar.md:248`.
- The shot shows rows 4, 6, 9 and 10 exactly: Waiting with a dotted circle, Working in blue with "16m", Woke in amber, and Done in green.

**F16 — No unread dot on rows.**
- Unread equals the green "Done" label plus a non-receded, full-weight title.
- A draft is a pen icon plus a warning tint (F9).
- Dots appear only on **collapsed folder headers**: a 6 px dot coloured warning (needs you) > error > success (unseen done) > info (working) (`SidebarProjectFolderHeader.tsx:14-19,82-86`; priority at `Sidebar.logic.ts:897-917`).
- They also appear in the command palette's status pill: a 6 px dot with a 2 s six-step opacity pulse, used only for working/connecting (`ThreadStatusIndicators.tsx:606-752`, `index.css:226,332-346`).
- A running terminal gets a pulsing teal `terminal` glyph (`ThreadStatusIndicators.tsx:508,562-565`). This is the only pulsing thing on a sidebar row.

**F17 — Provider marks.**
- Rendered by `ProviderInstanceIcon` with `size="compact"` and `iconClassName="opacity-60"`: **14 px at 60 % opacity**, at the right end of the meta line or title line, `aria-hidden` (`SidebarThreadRow.tsx:183-229`; `chat/ProviderInstanceIcon.tsx:114`).
- After a handoff, earlier providers show first at 12 px, `opacity-35 grayscale`, gap 6 (`:207-228`).
- Resolution order (`ProviderInstanceIcon.tsx:124-174`):
  1. **`acpRegistry`** draws the agent's registry icon URL. Without one it falls back to the ACP Registry mark (`settings/AcpRegistryIcon.tsx:140-144`).
  2. **`openaiCompatible`** draws an initials tile with a mini OpenAI badge.
  3. Next, a package icon (`packages/provider-*/src/client.ts`).
  4. Next, the bundled map: codex → OpenAI, claudeAgent → Claude, plus anthropic, openrouter and ollama (`:29-38`).
  5. Otherwise initials text.
- Colours:
  - Monochrome foreground, except Claude `#d97757`, Kiro `#9046FF`, Muse `#0081fb` and Cursor `#26251E`/`#EDECEC` (`:40-53`).
  - Antigravity is a raster PNG (`Icons.tsx:451-456`).
- **Instance badge.** When several enabled instances share a driver (or an instance has an accent colour), a pill is drawn on the mark's bottom-right (`ProviderInstanceIcon.tsx:175-190`; `packages/client-runtime/src/state/providerInstanceDisplay.ts:64-…`).
  - Geometry: offset −3 px, 12 px tall, 7 px text, weight 600, card background.
  - Content: the 2-letter instance initials (`providerInstanceDisplay.ts:29-45`).
- **omp on this Mac.** The instances `omp` ("omp") and `omp-firstmate` ("omp (firstmate)") are both local `acpRegistry` instances with no registry agent id (`~/.t3/userdata/settings.json`).
  - T3 therefore draws the ACP Registry mark with badges **OM** and **OF**. This is visible in the shot.
  - "Home" threads show **HE** on the same mark.
  - `claude-proxy` is a `claudeAgent` instance, drawn as the Claude spark.
- No driver called "omp" exists in T3 (`packages/client-runtime/src/providerClients.ts:30-60`).

## E. Project identity

**F18 — Project icon kinds.** Contract: `packages/contracts/src/project.ts:48-110`. Each kind renders at 16 px (`size-4`) through `ProjectFavicon`:
- **lucide** `{name, color}`: a lucide icon tinted by colour (`ProjectFavicon.tsx:59-76`). An unknown name falls back to `folder-code` (`:20`). The shot shows "Kazan" with a red `cooking-pot`.
- **emoji**: the glyph fills 80 % of the box (`:50-58,136-147`).
- **monogram** `{text, color}`, or **no icon**: a project favicon image (radius 25 %). If that is missing, a **derived monogram** (`:129-134`).
  - With no name at all: `folder-code` in `text-icon-muted` (`:150`).
- **Scratch project ("No project")**: lucide `message-square-dashed` in `gray` (`apps/server/src/project/ManagedProjectFolders.ts:303`). The shot shows a dashed square before "No project".

**F19 — Monogram tile** (`ProjectMonogram.tsx:18-47`; identity at `apps/web/src/projectIdentity.ts`).
- Geometry: a 16 × 16 SVG with 25 % radius (4 px).
  - Background: the colour at **14 %** (`color-mix(currentColor 14%, transparent)`).
  - Text: the colour itself, `font-mono` 8.25 px, weight 700, squeezed to width 6 (one glyph) or 12 (two).
- Text is **two characters**: the first letter of the first word, then one of (`projectIdentity.ts:13-25`):
  - the first digit inside that word,
  - else the first letter of the last word,
  - else the first word's last glyph.
- Upper-cased; "PR" when the name is empty. Examples: "demo" → **DO**, "Mobile App" → **MA**, "firstmate" → **FE**.
- Colour: `index = (index·31 + codepoint) mod 18` over the lower-cased name, giving one of the 18 colours below (`projectIdentity.ts:27-34`).

**F20 — Project colour names → hex.**
- Tint classes: `text-<c>-600` in light, `text-<c>-400` in dark. The picker swatch is `bg-<c>-500` (`apps/web/src/projectIconColors.ts`, order as listed).
- Hex values are the bundle's Tailwind OKLCH, clipped to sRGB. They are identical to Tailwind's published hex.
- Machine-readable copy: [`crates/app/assets/icons/project-icons.json`](../../crates/app/assets/icons/project-icons.json).

| name | dark (‑400) | light (‑600) | swatch (‑500) |
|---|---|---|---|
| gray | `#99a1af` | `#4a5565` | `#6a7282` |
| red | `#ff6467` | `#e7000b` | `#fb2c36` |
| orange | `#ff8904` | `#f54900` | `#ff6900` |
| amber | `#ffb900` | `#e17100` | `#fe9a00` |
| yellow | `#fdc700` | `#d08700` | `#f0b100` |
| lime | `#9ae600` | `#5ea500` | `#7ccf00` |
| green | `#05df72` | `#00a63e` | `#00c950` |
| emerald | `#00d492` | `#009966` | `#00bc7d` |
| teal | `#00d5be` | `#009689` | `#00bba7` |
| cyan | `#00d3f2` | `#0092b8` | `#00b8db` |
| sky | `#00bcff` | `#0084d1` | `#00a6f4` |
| blue | `#51a2ff` | `#155dfc` | `#2b7fff` |
| indigo | `#7c86ff` | `#4f39f6` | `#615fff` |
| violet | `#a684ff` | `#7f22fe` | `#8e51ff` |
| purple | `#c27aff` | `#9810fa` | `#ad46ff` |
| fuchsia | `#ed6aff` | `#c800de` | `#e12afb` |
| pink | `#fb64b6` | `#e60076` | `#f6339a` |
| rose | `#ff637e` | `#ec003f` | `#ff2056` |

**F21 — Icon picker set.**
- With an empty query it shows 24 "popular" icons (`projectIconOptions.ts:4-29`):
  - `folder-code`, `code-2`, `terminal`, `globe-2`, `server`, `database`, `bot`, `sparkles`;
  - `smartphone`, `monitor`, `cloud-cog`, `package`, `book-open`, `flask-conical`, `shield-check`, `rocket`;
  - `gamepad-2`, `music`, `image`, `shopping-bag`, `git-branch`, `workflow`, `wrench`, `layers-3`.
- With a query it searches **every Lucide name** (top 60 shown; `:64-68`).
- 30 emoji presets are offered too (`:31-62`).
- So a stored project icon can be any Lucide name at T3's pinned version, `lucide-react ^0.564.0` (`apps/web/package.json:72`). The panel ships the full `lucide-static@0.564.0` set.

## F. Sections, grouping and order

**F22 — Sections in the default sidebar.** Section is resolved snoozed → settled → pinned → active (`Sidebar.logic.ts:174-185`). Render order (`Sidebar.tsx:1945-1964`):
1. **Pinned** rows.
2. **Active** rows.
3. **Working** shelf: on by default (`settings.ts:527`). It holds threads that are working or waiting on resumable work (`docs/user/thread-sidebar.md:133-141`).
4. **Snoozed** shelf.
5. **Settled** shelf.

How the sections look and behave:
- **The "Pinned" and "Active" labels are visible only while dragging** (`Sidebar.tsx:3506-3526`; `docs/user/thread-sidebar.md:87-88`). Normally pinned and active rows read as one list.
- Shelf headers (`sidebar/SidebarListMarkers.tsx:113-147`):
  - 32 px, `text-xs font-medium` (**12 px / 500**), **sentence case, no caps, no tracking**, `text-sidebar-muted-foreground`, padding 8.
  - Content: label, then the count (`tabular-nums`, weight 400), then a 12 px chevron rotated −90° when collapsed.
- **All shelves start collapsed** and remember their state in localStorage (`Sidebar.tsx:330-332,450-464`).
- With nothing above them they pin to the bottom of the scroll area (`mt-auto`, `:3551,3566,3582`).
- A collapsed shelf still shows the open thread (`:515-523`).
- Settled shows 10 rows, then "Show 25 more" (`:328-329,3617-3638`).

**F23 — Order is stable.**
- **Pinned:** by pin order key (`threadSort.ts:268-301`).
- **Active:** new (keyless) threads come first, newest created or un-settled first. Then manual order (`threadSort.ts:303-336`).
  - "Thread activity does not change the order" (`docs/user/thread-sidebar.md:111-114`).
- **Active with the Working shelf on (the default):** threads that came back to you come first.
  - "Ordered by when each thread last came back to you"; manual drag is disabled (`docs/user/thread-sidebar.md:143-148`).
- **Working:** latest send first.
- **Snoozed:** soonest wake first (`Sidebar.tsx:703-708`).
- **Settled:** newest `settledAt` first (`threadSort.ts:54-69`).

**F24 — Project grouping is a Labs option, not the default.**
- "Settings → Labs → Boring sidebar (group by project)" (`settings.ts:513-516`, default `false`; `docs/user/thread-sidebar.md:150-160`).
- Folders appear between Pinned and Snoozed/Settled. Pinned projects come first and the Working shelf is off.
- **Folder header** (`sidebar/SidebarProjectFolderHeader.tsx:63-118`):
  - 32 px tall, gap 10, padding 10, 8 px radius, hover background; the `<li>` has 8 px above it (except the first).
  - Left to right: 16 px project icon, then the name (**14 px / 500**, foreground @ 90 %).
  - When collapsed, it adds the 6 px status dot (F16) and the thread count (12 px tabular, muted, hidden on hover).
  - Last, a 12 px `chevron-down` rotated −90° when collapsed, with a transform transition.
  - On hover, a `square-pen` "New thread in {project}" button appears 24 px from the right.
- Child rows are one-line 36 px slim rows indented 36 px.
- Context menu: New thread, Copy path, Pin/Unpin project, Project settings (`Sidebar.tsx:1260-1271`).

**F25 — Snooze presets.**
- Presets (`packages/contracts/src/settings.ts:129-135`; labels from `threadSettled.ts:261-270`): "In 1 hour", "In 3 hours", "This evening" (18:00), "Tomorrow" (09:00), "Next week" (Mon 09:00), "Custom…".
- Each is shown with its wake time; options whose time has passed are hidden.
- A snoozed row's slot shows a 12 px `alarm-clock` plus the wake label in info-foreground (`SidebarThreadRow.tsx:1287-1292`).

## G. Empty states

**F26 — Empty-state copy** (`Sidebar.tsx:3645-3672,3301-3306,3127`). All are text only, with no illustration: 12 px, `muted-foreground/60`, centred, 24 px vertical padding.

| Situation | Copy |
|---|---|
| No projects | "No projects yet", plus a bordered `+ Add project` button (11 px / 500, 12 px `plus`) |
| Nothing in the sidebar | "No threads yet" |
| One project scoped | "No threads in {project} yet" |
| Several projects scoped | "No threads in these projects yet" |
| Search pending | "Searching thread messages…" |
| Search with no hits | "No threads found" |
| Project filter with no hits | "No matching projects." |

During a drag, an empty section shows a dashed 36 px drop box labelled "Active" or "Settled" (`SidebarListMarkers.tsx:67`).

## H. Motion and keyboard

**F27 — Motion.**
- **List changes are FLIP-animated at 150 ms ease-out** (`Sidebar.motion.ts:1`):
  - moved rows slide `translateY(Δ → 0)` (`:101-118`);
  - entering rows fade 0 → 1 while rising `min(height, 40 px)` (`:192-211`);
  - leaving rows are cloned and fade out (`:40-89`).
- The list animation is skipped when more than 40 rows change, and under `prefers-reduced-motion` (`:18-20,145-149,221`).
- Hover and fade transitions use Tailwind's default **150 ms `cubic-bezier(.4,0,.2,1)`** (bundle `--default-transition-duration`/`-timing-function`). Each has `motion-reduce:transition-none` (`SidebarThreadRow.tsx:973,1029`).
- Sidebar open/close (Panel animations) is **0 ms by default**, adjustable to 400 ms, and off under Reduced Motion (`settings.ts:156,164`; `docs/user/appearance.md:44-49`; `panelAnimations.ts:11-39`).
- Pills: 180 ms with `--ease-drawer: cubic-bezier(.32,.72,0,1)` (`SidebarProviderUpdatePill.tsx:143`, `index.css:195`).
- The undo notice fades in over 200 ms (`SidebarThreadUndoNotice.tsx:46`).
- There are **no spinners on rows**. The only continuous motion is the 2 s status-pulse on terminal glyphs and palette dots (F16).

**F28 — Keyboard.** Defaults are in `packages/shared/src/keybindings.ts`; ⌘ is shown as `mod`.

| Action | Shortcut | Source |
|---|---|---|
| Toggle sidebar | ⌘B | `:58` |
| Previous / next thread | ⌘⇧[ / ⌘⇧] | `:127-128` |
| Jump to thread 1–9 | ⌘1–9 (desktop) | `:133-137` |
| New thread | ⌘N | `:111` |
| Command palette / search threads | ⌘K | `:85` |
| Find in thread | ⌘F | `:115` |
| Pin | ⌘⇧P | `:131` |
| Settle | ⌘⇧S | `:130` |
| Undo sidebar action | ⌘Z, with a 5 s notice | `:132`; `docs/user/keybindings.md:170-176` |
| Copy thread reference | ⌘⇧C | `:129` |
| Rename | F2 or double-click | `SidebarThreadRow.tsx:847-876` |

- Snooze and archive have no shortcut.
- The search field is a combobox navigated with the arrow keys (`SidebarThreadHeader.tsx:72-80`).

---

## I. What applies to a 380 px floating panel, by DESIGN.md section

The panel is 380 px wide, which is wider than T3's default 256 and close to the user's 396.

| DESIGN.md | T3 finding to adopt | Notes for PanelUI |
|---|---|---|
| §1.6 meaning never by colour alone | F15 | Icon shape + label + colour, exactly T3's triple. |
| §1.7 motion | F15, F27 | Static dashed circle for working, no spinners; 150 ms fades. |
| §1.4 stable order | F23 | Matches T3; new threads on top. |
| §2 header | F5–F6 | Mark + two-tone wordmark (14/500, −0.025 em; "T3" foreground, "Panel" muted). Badge like T3's `Badge sm secondary`: 16 px tall, 4 px radius, 3 px side padding, `secondary` fill. Header height 40 is fine (T3 uses 40 under WCO). |
| §2 provider marks | F17 | `crates/app/assets/providers/*.svg` + `providers.json`; 14 px @ 60 %; OM/OF badge for omp instances. |
| §2 project icon | F18–F21 | `icons/<lucide>.svg` + `project-icons.json` colours; 2-letter monogram tile at 14 % tint. |
| §3 tokens | F2–F4 | Add `input` (indigo) and `guest` if the panel shows them. Map `surface_hover` / `selected` to T3's 8 % / 7 % / 11 % foreground mixes in dark. Rows use radius 6, controls 8. |
| §4 motion | F27 | `fast` 120 → consider 150 (T3). Row enter = fade + rise ≤ 40 px at 150 ms ease-out. |
| §5 rows | F9–F15 | Use T3's card logic, compressed: project · status on the right of line 1, title on line 2. Hover swaps the time slot for actions (F13). Receded styling (F9) is a calm-by-default win. |
| §5 grouping | F22, F24 | See C6/C7. |
| §5 states | F26 | Text-only copy pattern; ours adds an icon (fine). |
| §6 keyboard | F28 | See C10. |

## J. Contradictions with DESIGN.md (for PanelUI to align or record as deviations)

1. **C1 — Header anatomy (§2).**
   - DESIGN: "mirrors T3's sidebar header: app mark (16 px) + wordmark + channel badge pill".
   - T3's header has **no app mark**, only the "T3" wordmark at cap height plus "Code" in muted 14/500 (F5). The channel is shown as an 80 px starry backdrop by default; the pill (10 px text, 16 px tall) appears only in `pill` mode (F6). There is no "by …" line.
   - Our mark is fine as our identity, but record it as a deviation. Copy the two-tone wordmark and the pill geometry. The badge text is 11 px, not T3's 10 px, because of DESIGN §1.8.
2. **C2 — Provider glyph size (§2).** DESIGN says 12 px monochrome glyphs. T3 uses **14 px at 60 % opacity**, with brand colours for Claude, Kiro, Muse and Cursor, and a 7 px initials badge when instances share a driver (F17). Prefer 14 px; the badge needs ≥ 11 px text in our rules, so show the instance in the tooltip, or use a monogram chip instead of a 7 px badge.
3. **C3 — Project fallback (§2).** DESIGN says "the project's first letter in a tinted rounded square". T3 uses a **two-character** monogram (algorithm in F19), a hashed colour from 18 names, a 14 % tint background and 25 % radius. "No project" is `message-square-dashed` in gray (F18).
4. **C4 — Title weight (§3).** DESIGN says "600 titles". T3 sidebar titles are **500**, and 400 when receded. Nothing in the sidebar uses 600 (F9, F5); even the brand is 500.
5. **C5 — Running glyph colour (§5 Rows).**
   - DESIGN: running = "static activity ring (**accent**)"; waiting = `!` in warning; completed/idle = check; interrupted = pause bars.
   - T3: running = `circle-dashed` in **info** blue with label "Working" + duration.
   - T3's needs-you states are split into approval (`shield-question`, warning) and input (`message-circle-question`, indigo).
   - T3 shows a check **only for an unseen completion**; a seen ready thread shows just the time.
   - T3 has no "interrupted" state. Its failed is `circle-alert` in error; limited is the same icon in warning (F15).
   - Recommended: keep the accent for listen state only, and use info for working.
6. **C6 — Section labels (§5 grouping).** DESIGN: "11px caps, muted". T3 uses **12 px / 500 sentence case**, muted, with a count and chevron. The Pinned/Active labels are hidden except while dragging (F22).
7. **C7 — Grouping model (§5 grouping).**
   - DESIGN groups by project with collapsible headers, plus one "Bitti / Ertelendi" shelf.
   - T3's default is a **flat list with the project on each row** (F9, F22). Project folders are the Labs "Boring sidebar" (F24), whose folder header the DESIGN layout matches.
   - T3 keeps **Working, Snoozed and Settled as separate collapsed shelves**, in that order, each with its own sort (F22–F23).
   - If PanelUI keeps project groups, cite F24 as the parity source. Consider a separate `Ertelendi` shelf sorted by wake time, before `Bitti`.
8. **C8 — Row height (§5 Rows).** DESIGN: 32 / 52 px. T3: slim and folder rows **36 px**; cards 78 px. 32 is still ≥ 28 px hit area, so it is acceptable as a density deviation. 36 matches T3.
9. **C9 — Relative time (§5 Rows).**
   - DESIGN: `şimdi`, `3 dk`, `2 sa`, `dün`.
   - T3 never says "yesterday"; it goes `now`, `Nm`, `Nh`, `Nd` with floors and no upper bound (F11).
   - The parity mapping is `şimdi`, `N dk`, `N sa`, `N g`. Keeping `dün` is a deliberate deviation.
   - The running duration is a separate format, `45s` / `16m` / `1h 9m` (F12). Turkish: `45 sn`, `16 dk`, `1 sa 9 dk`.
10. **C10 — Search shortcut (§6).** DESIGN: ⌘F opens search. In T3, **⌘K** is thread search and ⌘F is find-in-thread (F28). Accept both in the panel, or switch to ⌘K.
    - Also, T3 binds ⌘⇧C to "copy thread reference". That is harmless, since the panel has its own focus, but worth knowing.
11. **C11 — Unread indicator (§5 Rows).** DESIGN uses an accent `3 yeni` pill. T3 has **no unread counts or dots on rows**: unread = "Done" label plus full-weight title (F16). The pill is panel-specific (unheard speech), which is fine. Record it as an addition, not parity.
12. **C12 — Motion durations (§4).** DESIGN `fast` is 120 ms. T3 uses 150 ms for hover fades and list FLIP. T3 sidebar collapse is instant by default (F27). This is not a hard conflict. Aligning `fast` to 150 gives exact parity; otherwise keep it.
13. **C13 — Empty state (§5 States).** T3 is text-only (F26). DESIGN adds an icon, which is a harmless addition.
