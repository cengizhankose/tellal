# crates/app/assets

Every file here is a single-colour SVG (`currentColor`), so the app tints it with a theme token
(gpui `svg()` paints the rendered alpha in the element's text colour). Rendered proof:
[`research/ui/assets-preview.png`](../../../research/ui/assets-preview.png). Study behind the choices:
[`research/ui/t3-sidebar.md`](../../../research/ui/t3-sidebar.md).

T3 source references below point at github.com/pingdotgg/t3code, commit
[`17a6a01`](https://github.com/pingdotgg/t3code/tree/17a6a01d84e3adf9f29df2639d6b6079a434e1b2) (main, 2026-11-10).

## `logo.svg`

The Tellal app mark: the town crier's megaphone. The geometry is Lucide's `megaphone`
(`lucide-static@0.564.0`, ISC, same file as `icons/megaphone.svg`), drawn as one `currentColor`
stroke on a 24 × 24 grid so it stays crisp at 16 px (header, tinted with `accent`), 24 px and
64 px. It does not reuse T3's wordmark (`apps/web/src/components/T3Wordmark.tsx`).

## `icons/` — Lucide

- **Source:** every file of `lucide-static@0.564.0`
  (<https://registry.npmjs.org/lucide-static/-/lucide-static-0.564.0.tgz>, also
  <https://unpkg.com/lucide-static@0.564.0/icons/>). Files are copied verbatim (each keeps its
  `@license lucide-static v0.564.0 - ISC` comment).
  - 0.564.0 is the version T3 pins (`apps/web/package.json:72`, `"lucide-react": "^0.564.0"`).
  - Names match T3's stored names and Lucide's docs.
- **Licence:** ISC, with portions under MIT from Feather.
  - Copyright (c) for portions of Lucide are held by Cole Bemis 2013-2026 as part of Feather (MIT).
  - All other copyright (c) for Lucide is held by Lucide Contributors 2026.
  - Full text: <https://github.com/lucide-icons/lucide/blob/main/LICENSE>.
- **Why the whole set ships:** T3's project-icon picker searches every Lucide name.
  - `filterProjectIconNames` in `apps/web/src/projectIconOptions.ts:64-68` uses `lucideIconNames` from `lib/lucideIconByName.ts:22`.
  - A stored project icon can therefore be any of the 1917 names (canonical names plus Lucide's alias files).
- **`icons/project-icons.json`** is generated from T3's source and bundle. It has four parts:
  - **`popular`:** the 24 icons the picker shows before you type (`projectIconOptions.ts:4-29`).
  - **`colors`:** for each of T3's 18 project-icon colour names, a `dark` hex (Tailwind `<c>-400`), a `light` hex (`<c>-600`) and a `swatch` hex (`<c>-500`).
    - The classes come from `apps/web/src/projectIconColors.ts`.
    - The OKLCH values come from the installed T3 Nightly bundle CSS (`apps/server/dist/client/assets/main-vDPMgj0W.css`), clipped per channel to sRGB the way Chromium shows them.
  - **`aliases`:** the six names T3 can store that differ from the file name.
    - T3 kebab-cases PascalCase names (`lib/lucideIconByName.ts:18-20`). For example, `grid-2x-2` is stored for `grid-2x2.svg`.
  - **`lookup`:** the resolution rule.
    - Unknown names fall back to `folder-code`, as T3 does (`ProjectFavicon.tsx:20`).
    - T3's scratch project ("No project") uses `message-square-dashed` in `gray` (`apps/server/src/project/ManagedProjectFolders.ts:303`).

The DESIGN.md glyphs used by the panel UI, by role. All are plain Lucide names under `icons/`.

| Role | Icons |
|---|---|
| Status, as T3 uses them (`SidebarThreadRow.tsx:520-529`) | `circle-dashed` (working), `circle-dot-dashed` (waiting), `circle-check` (done), `circle-alert` (failed/limited), `shield-question` (approval), `message-circle-question` (input), `alarm-clock` (woke), `alarm-clock-off`, `circle-pause` |
| Listen toggle | `volume`, `volume-1`, `volume-2`, `volume-x` |
| Row actions | `message-square-reply`, `square-arrow-out-up-right`, `pin`, `pin-off`, `clock`, `check`, `undo-2` |
| Chrome | `chevron-right`, `chevron-down`, `search`, `x`, `minimize-2`, `maximize-2`, `settings`, `refresh-cw`, `bell` |
| Player | `pause`, `play`, `skip-forward`, `fast-forward`, `chevrons-right`, `radio`, `activity` |
| Metrics | `gauge`, `chart-no-axes-column` (T3's Usage glyph) |
| Reply dock | `mic`, `send` |
| States | `triangle-alert`, `folder`, `folder-code`, `message-square-dashed` |
| Computer use and new task | `mouse-pointer-2` (banner, `ekran` pill), `octagon-x` (`Durdur`), `shield-alert` (permission row), `plus` (new task) |

## `providers/` — provider marks as T3 draws them

Each mark is T3's own path data. It is wrapped in a `<g transform>` that fits T3's viewBox into
`0 0 24 24`, centred, with `fill="currentColor"`. The first line of each file is a comment with
its exact source URL.

**Colours.** Detail fills and gradients are dropped, because the marks are drawn monochrome:
- Kiro keeps its tile, with the ghost as a hole. This follows T3's comment at `provider-kiro/src/client.ts:36`.
- OpenCode's inner block is kept at 40 % opacity.
- Muse's gradients become the single tint.

**Licences.** T3 Code is MIT (© 2026 T3 Tools Inc., <https://github.com/pingdotgg/t3code/blob/main/LICENSE>). The marks themselves are trademarks of their owners. They are used here only to identify the provider, the same way T3 uses them.

| File | Source (commit 17a6a01) | Origin / licence of the drawing | T3 colour |
|---|---|---|---|
| `openai.svg` | `apps/web/src/components/Icons.tsx#L405` | OpenAI logo; T3 (MIT) | foreground |
| `claude.svg` | `Icons.tsx#L420` | Claude spark; T3 (MIT) | `#d97757` |
| `anthropic.svg` | `Icons.tsx#L444` | Simple Icons (CC0), per `Icons.tsx:431` | foreground |
| `openrouter.svg` | `Icons.tsx#L432` | Simple Icons (CC0), per `Icons.tsx:431` | foreground |
| `ollama.svg` | `Icons.tsx#L438` | Simple Icons (CC0), per `Icons.tsx:431` | foreground |
| `acp-registry.svg` | `Icons.tsx#L459` | ACP Registry mark; T3 (MIT) | foreground |
| `cursor.svg` | `packages/provider-cursor/src/client.ts` | T3 (MIT) | `#26251E` / `#EDECEC` |
| `grok.svg` | `packages/provider-grok/src/client.ts` | T3 (MIT) | foreground |
| `fx.svg` | `packages/provider-fx/src/client.ts` | T3 (MIT) | foreground |
| `opencode.svg` | `packages/provider-opencode/src/client.ts` | T3 (MIT) | foreground |
| `kiro.svg` | `packages/provider-kiro/src/client.ts` | T3 (MIT) | `#9046FF` |
| `muse.svg` | `packages/provider-muse/src/client.ts` | T3 (MIT) | `#0081fb` |
| `pi.svg` | `packages/provider-pi/src/client.ts` | T3 (MIT) | foreground |

`providers/providers.json` maps T3 provider ids to these files:
- **`drivers`:** maps a T3 driver kind (`codex`, `claudeAgent`, `acpRegistry`, `cursor`, …) to `{file, label, color}`. The driver-to-icon table is `ProviderInstanceIcon.tsx:29-53` plus `packages/client-runtime/src/providerClients.ts:30-60`. A `color` of `null` means the theme foreground.
- **`instances`:** maps known instance ids to their driver.
  - `omp` and `omp-firstmate` are local `acpRegistry` instances in this Mac's T3 settings, so T3 shows the ACP Registry mark with a 2-letter instance badge (OM / OF).
  - `claude-proxy` is a `claudeAgent` instance.
  - `codex` and `claudeAgent` are the default instance ids.
- **`monogramOnly`:** drivers with no vector mark. Antigravity ships as a raster PNG, and `openaiCompatible` endpoints use initials.
