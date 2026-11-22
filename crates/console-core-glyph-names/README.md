# Glyph names

Maps Adobe glyph names to Unicode text, retaining multi-character mappings and
the table's canonical reverse mapping for individual characters. Extracted from
Hayro's PDF interpreter. `get` and `get_reverse` return `None` for unknown input.

This crate is `no_std` and does not allocate. Compose it with
`console-core-font-encodings` to turn an encoded character into Unicode text;
PDF-specific fallback policy stays in the interpreter.

The upstream MIT and Apache notices are retained in `LICENSE-MIT` and
`LICENSE-APACHE`; this workspace offers its copy under AGPL-3.0-or-later.
