# Macintosh Roman

Maps Macintosh Roman character codes to Unicode and back. The font database
uses `decode` for font names; the text shaper uses `encode` for Macintosh
character maps. Both directions use one table, including the modifier symbols
used in Macintosh fonts' control range.

An unrepresentable Unicode code point returns `None`. A font reader decides
which character map to use and what to do when a character is absent. Mapping
font codes to glyph names remains in `console-core-font-encodings`.

This crate is `no_std` and does not allocate. The table comes from fontdb;
its MIT notice is retained, and this copy is offered under AGPL-3.0-or-later.
