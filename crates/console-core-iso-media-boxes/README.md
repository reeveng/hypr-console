# Base media boxes

Defines the framing of boxes in base media and QuickTime files independently of
track metadata or codecs. Both audio and video MPEG-4 readers use these rules.

`BoxHeader::required_len` determines how many header bytes a streaming reader
needs, and `BoxHeader::BUFFER` is a zeroed array as long as the longest header
for that reader to gather them in. `BoxHeader::parse` decodes ordinary sizes,
extended sizes, zero sizes that extend to the parent boundary, and sixteen-byte
user types. `FullBoxHeader` decodes version and flags. `Children` borrows
payload slices, validates child bounds and stops at the first malformed box.
`FourCharacterCode` represents a box type, brand or other four-byte identifier.

The format crate consumes byte slices. Container readers supply buffering,
errors and recovery, then interpret box types as tracks, metadata or codec
configuration. The video reader retains its public `FourCc` import as an alias.

The crate supports `no_std` with allocation. Extracted from the clean-room base
media implementation carried in this workspace. Its MIT and Apache-2.0 licenses
are retained; this workspace offers its copy under AGPL-3.0-or-later.
