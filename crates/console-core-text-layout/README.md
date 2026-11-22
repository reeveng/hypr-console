# Console text layout

Local adaptation of COSMIC Text for text shaping, layout and glyph
rasterization. The upstream editing APIs, cursor movement, syntax highlighting, and Peniko adapter
are omitted because the console does not use them. Font bytes are borrowed
directly from the shaping cache. Rendering tests use the local
`console-core-path-rasterizer` and compare decoded pixels with the reference
images so PNG encoder differences do not affect the results.

This local crate uses the standard library throughout; the upstream `no_std`
build and its math/hash-map dependencies are omitted. Font-family names use
owned `String` values. Per-line attribute spans use a sorted vector
with binary-search lookup, merging adjacent equal spans after edits.

The text algorithms and font pipeline are maintained locally in `crates/`.
The SVG renderer shares the shaping and metrics crates. Each imported crate
retains its licenses and records its original release, archive checksum, and
source revision in `UPSTREAM.toml`.

| Local crate | Upstream | Purpose |
| --- | --- | --- |
| `console-core-text-shaping` | HarfRust | Turn script text into positioned glyphs |
| `console-core-font-metrics` | Skrifa | Font axes, character maps, and glyph metrics |
| `console-core-font-reading` | read-fonts | Parse OpenType tables |
| `console-core-font-types` | font-types | Shared OpenType binary types |
| `console-core-path-rasterizer` | tiny-skia | Fill glyph outlines into coverage masks |
| `console-core-png-files` | Console codec | Read the pictures a colour glyph is stored as |
| `console-core-picture-scaling` | Console codec | Bring a glyph picture to the size asked for |
| `console-core-deflate` | Console codec | Shared DEFLATE/zlib encoding and bounded decoding |
| `console-core-text-direction` | unicode-bidi | Bidirectional text ordering |
| `console-core-writing-scripts` | unicode-script | Unicode script properties |
| `console-core-text-boundaries` | unicode-segmentation | Grapheme and word boundaries |

Line breaking uses the shared `console-core-line-breaking` package, also used
by Mermaid. Locale discovery and POSIX
locale normalization use `console-core-localization`, including `LANGUAGE`
preferences and the `en-US` fallback.

The direct registry dependencies are `log` for the shared logging facade,
`rustc-hash` for the font and glyph caches, and `self_cell` for the cached
shaper's borrowed font data. The upstream overview follows.

# COSMIC Text

[![crates.io](https://img.shields.io/crates/v/cosmic-text.svg)](https://crates.io/crates/cosmic-text)
[![docs.rs](https://docs.rs/cosmic-text/badge.svg)](https://docs.rs/cosmic-text)
![license](https://img.shields.io/crates/l/cosmic-text.svg)
[![Rust workflow](https://github.com/pop-os/cosmic-text/workflows/Rust/badge.svg?event=push)](https://github.com/pop-os/cosmic-text/actions)

Pure Rust multi-line text handling.

COSMIC Text provides advanced text shaping, layout, and rendering wrapped up
into a simple abstraction. Shaping is provided by HarfRust, and supports a
wide variety of advanced shaping operations. Glyph outlines are read and hinted by
Skrifa and filled by the path rasterizer, which fills everything else this
desktop draws; color emoji are the pictures the font holds. Layout is implemented custom, in safe
Rust, and supports bidirectional text. Font fallback is also a custom
implementation, reusing some of the static fallback lists in browsers such as
Chromium and Firefox. Linux, macOS, and Windows are supported with the full
feature set. Other platforms may need to implement font fallback capabilities.

## Screenshots

Arabic translation of Universal Declaration of Human Rights
[![Arabic screenshot](screenshots/arabic.png)](screenshots/arabic.png)

Hindi translation of Universal Declaration of Human Rights
[![Hindi screenshot](screenshots/hindi.png)](screenshots/hindi.png)

Simplified Chinese translation of Universal Declaration of Human Rights
[![Simplified Chinses screenshot](screenshots/chinese-simplified.png)](screenshots/chinese-simplified.png)

[View Universal Declaration of Human Rights on OHCHR](https://www.ohchr.org/en/universal-declaration-of-human-rights)

## Roadmap

The following features must be supported before this is "ready":

- [x] Font loading (using fontdb)
  - [x] Preset fonts
  - [x] System fonts
- [x] Text styles (bold, italic, etc.)
  - [x] Per-buffer
  - [x] Per-span
- [x] Font shaping (using HarfRust)
  - [x] Cache results
  - [x] RTL
  - [x] Bidirectional rendering
- [x] Font fallback
  - [x] Choose font based on locale to work around "unification"
  - [x] Per-line granularity
  - [x] Per-character granularity
- [x] Font layout
  - [x] Click detection
  - [x] Simple wrapping
  - [ ] Wrapping with indentation
  - [ ] No wrapping
  - [ ] Ellipsize
- [x] Font rendering (Skrifa outlines, filled by the path rasterizer)
  - [x] Cache results
  - [x] Font hinting
  - [x] Ligatures
  - [x] Color emoji
- [x] Text editing
    - [x] Performance improvements
    - [x] Text selection
    - [x] Can automatically recreate https://unicode.org/udhr/ without errors (see below)
    - [x] Bidirectional selection
    - [ ] Copy/paste
- [x] no_std support (with `default-features = false`)
    - [ ] no_std font loading
    - [x] no_std shaping
    - [x] no_std layout
    - [ ] no_std rendering

The UDHR (Universal Declaration of Human Rights) test involves taking the entire
set of UDHR translations (almost 500 languages), concatenating them as one file
(which ends up being 8 megabytes!), then via the `editor-test` example,
automatically simulating the entry of that file into cosmic-text per-character,
with the use of backspace and delete tested per character and per line. Then,
the final contents of the buffer is compared to the original file. All of the
106746 lines are correct.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
