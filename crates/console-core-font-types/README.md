# console-core-font-types

The scalar types an OpenType font is written in, and the big-endian bytes each
one is stored as: the six fixed-point widths, 24-bit integers, glyph and name
identifiers, tags, offsets, versions, dates, and the point, matrix and bounding
box a reader builds out of them. The font reader, metrics and hinting crates
read a font through these.

It began as fontations' `font-types` 0.12.6 and keeps its names and its
arithmetic: the same wrapping fixed-point operations FreeType uses, the same
rounding, and the same shortest decimal when a value is printed. What it keeps
of the workspace is the rest -- every call answers a `Result`, a length is a
`u64`, nothing needs more than `core`, and the tests live in `tests/`.

The upstream code is MIT or Apache-2.0 at your choice; both licences are in
this directory.
