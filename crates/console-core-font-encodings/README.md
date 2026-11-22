# Font encodings

Maps single-byte character codes to glyph names using Standard, Mac Roman,
Mac OS Roman, Mac Expert, Windows ANSI, Symbol, and Zapf Dingbats tables.
Extracted from Hayro's PDF interpreter.

Each module exposes `get`; Mac Roman and Mac OS Roman also expose `get_inverse`.
Mac OS Roman holds supplemental entries and is composed with Mac Roman for the
complete encoding. Each table retains its original fallback behavior, including Windows ANSI's
bullet fallback. Combining tables, a font's built-in encoding, and PDF
`.notdef` handling remain decisions of the font reader.

This crate is `no_std` and does not allocate. Unicode text is a separate question
answered by `console-core-glyph-names`.

The upstream MIT and Apache notices are retained in `LICENSE-MIT` and
`LICENSE-APACHE`; this workspace offers its copy under AGPL-3.0-or-later.
