# skrifa

[![Crates.io](https://img.shields.io/crates/v/skrifa.svg)](https://crates.io/crates/skrifa)
[![Docs](https://docs.rs/skrifa/badge.svg)](https://docs.rs/skrifa)
[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](#license)

This crate aims to be a robust, ergonomic, high performance library for reading
OpenType fonts. It is built on top of the
[read-fonts](https://github.com/googlefonts/fontations/tree/main/read-fonts)
low level parsing library and is also part of the
[oxidize](https://github.com/googlefonts/oxidize) project.

## Features

### Metadata

The following information is currently exposed:

* Global font metrics with variation support (units per em, ascender,
descender, etc)
* Glyph metrics with variation support (advance width, left side-bearing, etc)
* Codepoint to nominal glyph identifier mapping
    * Unicode variation sequences
* Localized strings
* Attributes (stretch, style and weight)
* Variation axes and named instances
    * Conversion from user coordinates to normalized design coordinates

Future goals include:

* Color palettes
* Embedded bitmap strikes

### Glyph formats

| Source | Decoding | Variations | Hinting |
|--------|----------|------------|---------|
| glyf   | ✔️      | ✔️         | ✔️     |
| CFF    | ✔️      | -          | ✔️      |
| CFF2   | ✔️      | ✔️         | ✔️     |
| COLRv0 | ✔️      | -          | -       |
| COLRv1 | ✔️      | ✔️         | -       |
| EBDT   | ✔️*     | -          | -       |
| CBDT   | ✔️*     | -          | -       |
| sbix   | ✔️*     | -          | -       |

\* Raw support available through the `read-fonts` crate.

## Autohint tables

Regenerate the autohint script and coverage tables from the workspace root.

```sh
mkdir -p target
cargo run -p console-core-font-metrics --example generate_autohint_styles > target/generated_autohint_styles.rs &&
  mv target/generated_autohint_styles.rs crates/console-core-font-metrics/generated/generated_autohint_styles.rs
```

## Panicking

This library should not panic regardless of API misuse or use of
corrupted/malicious font files. Please file an issue if this occurs.

## The name?

Following along with our theme, *skrifa* is Old Norse for "write" or "it is
written." And so it is named.

## Safety

Unsafe code is forbidden by a `#![forbid(unsafe_code)]` attribute in the root
of the library.

## Embedded TrueType hinting

The bytecode interpreter lives in `console-core-font-hinting`. This crate supplies
scaled glyph geometry and reusable scratch buffers to that interpreter; font
metadata, outline loading, variation deltas, automatic hinting and drawing remain
here. Existing public target settings and hinting error types are re-exported.
