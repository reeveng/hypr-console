# Matroska blocks

Split Matroska blocks into frames without knowing their tracks.

`BlockHeader::parse` reads the track number, relative timestamp and flags.
`Lacing::from_flags` selects no lacing, Xiph, fixed-size or EBML lacing.
`parse_lacing` returns validated frame offsets and lengths without copying frame
bytes. It also handles the Xiph packet layout of Matroska Vorbis codec-private
data. The Extensible Binary Meta Language integer rules come from `console-core-extensible-binary-meta-language`.

It is the part of a block the audio and the video Matroska readers both read.
Codec selection, packet ownership, absolute timing and frame duration remain
with those readers.

The crate supports `no_std` with allocation. Extracted from the clean-room
Matroska implementation carried in this workspace. The original MIT and
Apache-2.0 licenses are retained; this workspace offers its copy under
AGPL-3.0-or-later.
