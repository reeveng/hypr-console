# Text direction

The Unicode Bidirectional Algorithm, Unicode Standard Annex #9, over Unicode
16.0: which way each character of a text runs, and the order a line of it is
shown in.

`BidiInfo::new` splits a text into paragraphs and gives every byte an embedding
level, taking each paragraph's direction from its first strong character unless
the caller names a level. `visual_runs` takes one line of a paragraph and
answers its levels made ready for display and the runs of one level in the order
they are shown. `reorder_visual` answers that order for levels the caller worked
out itself. `bidi_class` is the character property the algorithm reads.
Positions are byte offsets held in a `u32`.

It passes every case of BidiTest.txt and BidiCharacterTest.txt for Unicode 16.0;
`tests/the_conformance.rs` carries a sample of both.

The crate began as unicode-bidi 0.3.18, by the Servo Project Developers, and
keeps only what the text layout and the vector graphics renderer ask of it. Its
MIT and Apache-2.0 licenses and copyright notice are retained; this workspace
offers its copy under AGPL-3.0-or-later.
