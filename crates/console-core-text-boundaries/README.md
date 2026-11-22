# Grapheme boundaries

Unicode 17 extended and legacy grapheme clusters for contiguous UTF-8 strings.
The keyboard, text layout and diagram renderer use this crate to keep combining
marks, Indic conjuncts, emoji sequences and regional-indicator pairs together.

`UnicodeSegmentation::graphemes` returns a forward iterator over clusters.
`grapheme_indices` also returns each cluster's byte position as a `u64`.
Both take `ClusterKind::Extended` or `ClusterKind::Legacy` and return
`Result<_, Never>`. The iterators borrow the input and allocate nothing.

The properties and Unicode conformance vectors come from
unicode-segmentation 1.13.3. The former word and sentence APIs, chunked rope
cursor and reverse iteration had no callers in this repository and were removed.
The renderer consumes its last pending cluster with the forward iterator.
The crate began as unicode-segmentation 1.13.3, by kwantam and Manish
Goregaokar. Its MIT and Apache-2.0 licenses and copyright notice are retained;
this workspace offers its copy under AGPL-3.0-or-later.

`cargo test -p console-core-text-boundaries` checks both cluster modes against
all retained Unicode 17 grapheme vectors, mixed-width byte positions, Indic and
emoji context, and exhausted iterators. Outside the tree, the original
implementation and this one were run side by side over Unicode 17's
GraphemeBreakTest.txt, generated strings and every code point between a letter
and a mark, in both modes and at every byte position, and differed on none.
