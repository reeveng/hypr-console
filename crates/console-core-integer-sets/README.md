# Integer sets

Ordered, invertible sets of unsigned integers. `U32Set` is the stored set: a map
of 512-value pages keyed by the first value each holds, every page a run of
64-bit words. `IntSet<T>` is a set over a `Domain`, held either as its members
or as the exceptions to a whole domain, so inverting one is a flip rather than a
walk over four billion values. A domain says which `u32` values belong to it and
whether they run without gaps, which decides whether a range is inserted as one
span or value by value.

Every question a set is asked comes back as the answer's name rather than as a
`bool`: `contains` answers a `Presence`, `is_empty` an `Emptiness`,
`intersects_set` and `intersects_range` an `Overlap`, and `membership` whether
the set is `Inclusive` or `Exclusive`. `insert` and `remove` answer with the
presence the value had before the call.

`sparse_bit_set` reads and writes the IFT sparse bit set encoding: a header
naming the branch factor and the tree height, then the tree's nodes breadth
first. `to_sparse_bit_set` picks the branch factor that writes fewest bytes, and
`to_sparse_bit_set_with_bf` writes the one asked for. Decoding is bounded by the
bytes there are, and `from_sparse_bit_set_bounded` adds a bias to every value
and drops what falls past a maximum.

The default `std` feature can be disabled for `no_std` with `alloc`. `serde`
reads and writes sets, and still reads the paged shape the set was stored in
before its pages were keyed by value. `font-types` adds domains for this
workspace's OpenType identifiers without requiring a font parser; the font
reader re-exports these types through its `collections` paths.

`tests/the_sets.rs` walks seeded sequences of inserts, removals, ranges,
inversions and set operations over every kind of set and asks each the same
questions as a `BTreeSet` holding the same members. Its last test writes seeded
sets in every branch factor and holds a digest of the bytes, recorded from the
upstream reader before the rewrite. `tests/fixtures/legacy-pages.json` is a set
stored in the old paged shape.

Extracted from the Fontations font reader. The upstream MIT and Apache notices
are retained in `LICENSE-MIT` and `LICENSE-APACHE`; this workspace offers its
copy under AGPL-3.0-or-later.
