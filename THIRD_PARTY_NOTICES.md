# Third-party notices

Tellal itself is MIT-licensed ([`LICENSE`](LICENSE)). It ships or uses the third-party work
below. Details on every bundled asset file: [`crates/app/assets/README.md`](crates/app/assets/README.md).

## Bundled in the binary

### Lucide icons — ISC (portions MIT)

`crates/app/assets/icons/` is the full `lucide-static@0.564.0` icon set, copied verbatim (each file
keeps its `@license lucide-static v0.564.0 - ISC` comment) and embedded at build time.

- ISC License. Copyright (c) for portions of Lucide are held by Cole Bemis 2013-2026 as part of
  Feather (MIT License). All other copyright (c) for Lucide is held by Lucide Contributors 2026.
- Full text: <https://github.com/lucide-icons/lucide/blob/main/LICENSE>

`crates/app/assets/icons/project-icons.json` (popular icons, colour map, aliases) is derived from
T3 Code's source and bundle (MIT, see below).

### Provider marks — from T3 Code (MIT) and Simple Icons (CC0-1.0)

`crates/app/assets/providers/*.svg` reuse the path data T3 Code draws for each provider, at
commit [`17a6a01`](https://github.com/pingdotgg/t3code/tree/17a6a01d84e3adf9f29df2639d6b6079a434e1b2).
Each file's first line names its exact source.

- T3 Code: MIT License, Copyright (c) 2026 T3 Tools Inc.,
  <https://github.com/pingdotgg/t3code/blob/main/LICENSE>.
- `anthropic.svg`, `openrouter.svg`, `ollama.svg`: Simple Icons, CC0-1.0
  (<https://github.com/simple-icons/simple-icons/blob/develop/LICENSE.md>), as noted in T3's
  `Icons.tsx`.
- **Trademarks.** OpenAI, Claude, Anthropic, OpenRouter, Ollama, Cursor, Grok, Kiro, OpenCode,
  Muse, the ACP Registry and the other provider names and logos are trademarks of their
  respective owners. They are used only to identify which provider runs a thread, the same way
  T3 Code shows them. No endorsement is implied.

### GPUI — Apache-2.0

The UI framework from [Zed](https://github.com/zed-industries/zed) (`gpui` 0.2.2 and its
`gpui_*` crates). Apache License 2.0, Copyright Zed Industries, Inc.

### normalizer-tr — Apache-2.0

Turkish text normalizer (numbers, times, percentages, abbreviations) before synthesis.
[`normalizer-tr`](https://github.com/erdemtuna/normalizer-tr) 0.8.1, Apache License 2.0.

### ONNX Runtime — MIT

Runs the voice models. The [`ort`](https://github.com/pykeio/ort) crate (MIT OR Apache-2.0)
downloads Microsoft's prebuilt ONNX Runtime at build time and copies the library next to the
binary. ONNX Runtime: MIT License, Copyright (c) Microsoft Corporation,
<https://github.com/microsoft/onnxruntime/blob/main/LICENSE>.

### Voice downloads — `ureq` (MIT OR Apache-2.0), `hmac-sha256` (ISC)

The app downloads voices itself: [`ureq`](https://github.com/algesten/ureq) 3.5.1 (MIT OR
Apache-2.0) fetches the files over HTTPS with macOS's TLS, and
[`hmac-sha256`](https://github.com/jedisct1/rust-hmac-sha256) 1.1.15 (ISC, Copyright (c)
Frank Denis) checks each file's pinned SHA-256 sum.

### Supertonic reference code — MIT

`crates/core/src/supertonic.rs` ports the inference helper of Supertone's reference
implementation ([`supertone-oss-archive/supertonic`](https://github.com/supertone-oss-archive/supertonic),
`rust/src/helper.rs`), MIT License,
<https://github.com/supertone-oss-archive/supertonic/blob/main/LICENSE>.

## Downloaded at run time (not redistributed)

### EMA Lightning voice model — Apache-2.0

The Turkish voice (`tr-ema-lightning`). The app (or [`scripts/fetch-models.sh`](scripts/fetch-models.sh))
downloads the ONNX export [`ozcancelik/ema-lightning-onnx`](https://huggingface.co/ozcancelik/ema-lightning-onnx)
of [EMA Lightning](https://huggingface.co/canberkkkkkk/ema-lightning) at a pinned revision into
`~/Library/Application Support/tellal/models/ema-lightning-onnx/`, together with its `LICENSE`
and `NOTICE`. The weights are not part of this repository, of any build or of the DMG. Apache
License 2.0; see the model cards for attribution.

### Supertonic 2 voice model — BigScience OpenRAIL-M

The English voice (`en-supertonic-2`, voice style `M1`). The app (or `scripts/fetch-models.sh en`)
downloads, at pinned revisions, into `~/Library/Application Support/tellal/models/supertonic-2-en/`:

- from [Supertone's `Supertone/supertonic-2`](https://huggingface.co/Supertone/supertonic-2):
  `tts.json`, `unicode_indexer.json`, `voice_styles/M1.json`, `duration_predictor.onnx`,
  `text_encoder.onnx` and the `LICENSE`, which is installed next to the weights;
- from the sherpa-onnx maintainer's Hugging Face repository
  [`csukuangfj2/sherpa-onnx-supertonic-tts-int8-2026-03-06`](https://huggingface.co/csukuangfj2/sherpa-onnx-supertonic-tts-int8-2026-03-06):
  the int8-quantized `vector_estimator.int8.onnx` and `vocoder.int8.onnx` (derivatives of
  Supertone's weights, under the same licence).

The weights are not part of this repository, of any build or of the DMG. Published by Supertone;
licensed under the BigScience OpenRAIL-M License,
<https://huggingface.co/Supertone/supertonic-2/blob/main/LICENSE>. The licence is royalty-free
but not an OSI licence: its use-based restrictions (paragraph 5 and Attachment A) apply to you
as a user of Tellal's English voice, and Tellal passes them on here. In short, you agree not to
use the model or its output:

- in any way that violates applicable national, federal, state, local or international law;
- to exploit or harm minors;
- to generate or spread verifiably false information meant to harm others;
- to generate or spread personal identifiable information that can be used to harm someone;
- to generate or spread content without expressly and intelligibly disclaiming that it is
  machine-generated;
- to defame, disparage or harass others;
- to impersonate others (e.g. deepfakes) without their consent;
- for fully automated decision making that adversely affects an individual's legal rights or
  creates or modifies a binding obligation;
- to discriminate against or harm people based on social behaviour or personal or personality
  characteristics, or on legally protected characteristics;
- to exploit vulnerabilities of a group (age, social, physical or mental characteristics) so as
  to distort someone's behaviour in a way that causes or is likely to cause physical or
  psychological harm;
- to provide medical advice or interpret medical results;
- for administration of justice, law enforcement, immigration or asylum processes (e.g.
  predicting that someone will commit fraud or a crime).

This summary is not the licence; the full text (installed as `supertonic-2-en/LICENSE`) governs.

## Rust dependencies

The Cargo dependency graph for `aarch64-apple-darwin` (build and dev dependencies included)
has 506 third-party packages. All licences are permissive, except two MPL-2.0 crates (weak
copyleft, file-level). Summary from `cargo metadata`:

| Packages | Licence (SPDX) |
|---:|---|
| 311 | Apache-2.0 OR MIT |
| 86 | MIT |
| 21 | Apache-2.0 (gpui and its crates, cpal, normalizer-tr, …) |
| 18 | Unicode-3.0 (ICU4X data crates) |
| 16 | Apache-2.0 OR MIT OR Zlib |
| 9 | BSD-3-Clause (avif-serialize, bindgen, exr, lebe, ravif, sha1_smol, subtle, tiny-skia, tiny-skia-path) |
| 8 | MIT OR Unlicense (aho-corasick, byteorder, byteorder-lite, globset, memchr, same-file, termcolor, walkdir) |
| 4 | BSD-2-Clause (arrayref, av1-grain, rav1e, v_frame) |
| 4 | ISC (hmac-sha256, libloading, rustls-webpki, untrusted) |
| 4 | Apache-2.0 OR ISC OR MIT (rustls and related crates) |
| 4 | Zlib (foldhash, slotmap, zlib-rs) |
| 3 | Apache-2.0 OR BSD-2-Clause OR MIT (mach2, zerocopy, zerocopy-derive) |
| 3 | (Apache-2.0 OR MIT) AND Unicode-3.0 (unicode-ident, unicode-normalization, unicode-properties) |
| 2 | MPL-2.0 (cbindgen, option-ext) |
| 2 | Apache-2.0 OR BSD-3-Clause (moxcms, pxfm) |
| 2 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT (rustix) |
| 1 each | 0BSD OR Apache-2.0 OR MIT (adler2); Apache-2.0 WITH LLVM-exception (ar_archive_writer); Apache-2.0 OR CC0-1.0 OR MIT-0 (dunce); (Apache-2.0 OR MIT) AND BSD-3-Clause (encoding_rs); CC0-1.0 (hexf-parse); Apache-2.0 OR CC0-1.0 (imgref); Apache-2.0 AND ISC (ring); Apache-2.0 OR BSL-1.0 (ryu); CDLA-Permissive-2.0 (webpki-root-certs) |

SPDX spellings were normalized (`MIT/Apache-2.0` = `Apache-2.0 OR MIT`). Regenerate the raw
summary:

```bash
cargo metadata --format-version 1 --filter-platform aarch64-apple-darwin \
  | jq -r '(.resolve.nodes | map(.id)) as $ids
      | [.packages[] | select(.source != null and (.id as $i | $ids | index($i)))]
      | group_by(.license)[] | "\(length)\t\(.[0].license)"' | sort -rn
```

For the full licence text of every crate, generate a report with
[`cargo about`](https://github.com/EmbarkStudios/cargo-about) (`cargo about generate`).

## Not affiliated

Tellal is an independent project. It is not affiliated with, endorsed by or sponsored by
T3 Tools Inc. / Ping Labs, Zed Industries, Wispr or any provider shown in the panel.
