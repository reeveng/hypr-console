# Extensible Binary Meta Language elements

Defines Extensible Binary Meta Language element encodings independently of a document schema.

Variable integers preserve their encoded width and all-ones marker. Only
`read_size` interprets that marker as an unknown element size; track numbers and
signed lace deltas retain ordinary integer semantics. IDs retain their marker
bits and validate reserved encodings. Typed values decode from slices, and `Children` borrows element payloads,
so callers can layer streaming, indexed access or writing over the same format
rules.

The audio Matroska reader supplies a small streaming adapter. The video reader
re-exports this API through its existing `ebml` module. Matroska element meaning,
recovery, track selection and packet timestamps stay with the container readers.
CRC arithmetic stays in `console-core-checksums`; laced frame boundaries belong
to `console-core-matroska-blocks`.

The crate supports `no_std` with allocation. Extracted from the clean-room
Matroska implementation carried in this workspace. The original MIT and
Apache-2.0 licenses are retained; this workspace offers its copy under
AGPL-3.0-or-later.
