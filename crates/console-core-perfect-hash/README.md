# Perfect-hash tables

Finds candidate slots in immutable, precomputed perfect-hash tables. The key
symbol tables and XKB keyword tables share this arithmetic without depending
on one another.

`Map::new` takes the generator's seed, displacements, and value slots. These
must come from the same generator and use the supplied `Hash` implementation.
The map owns no allocation and does not generate a layout at runtime.

`Map::candidate` returns a candidate value, not proof of membership. The layout
stores values without their keys, so an unknown key can select an occupied
slot. Each caller compares the stored name or identifier before accepting it.
Empty displacement or value tables return no candidate.

Extracted from the kbvm code carried in this workspace, originally by mahkoh
under MIT or Apache-2.0. This workspace offers its copy under AGPL-3.0-or-later.
