# Duplicate responsibilities across the desktop and its forks

The desktop carries format knowledge in `crates/` and in `forks/`, and some of
that knowledge has more than one owner. The useful boundary is the smallest
responsibility both callers actually share. A codec keeps its syntax, a panel
keeps its policy, and the arithmetic or framing underneath is what moves.

Extracted responsibilities belong under `crates/`, with the workspace's names,
error handling and checks. A fork can re-export the extracted type while its
callers move. The implementation left in the fork is removed in the same
change; a new shared crate beside another private copy has not finished the
job. Keep the original license notices with carried code.

## Byte sources for video containers

Implemented. The contract and its implementations now live in
[`console-core-byte-sources`](../crates/console-core-byte-sources/src/lib.rs).
The Matroska and MPEG-4 forks re-export the same trait. Their private
`source.rs` files and the film crate's duplicate adapter module have gone.

Films uses `FileSource`, which measures its file once and uses positioned
reads. Length queries carry I/O failures rather than inventing an empty file.
Memory and seekable readers implement the same contract, and the original
FilmCraft notice travels with the port.

The source tests exercise boundaries, file positions, sequential reads and a
retry after a read that moved before failing. Container tests check that a
length failure remains an I/O failure; the film fixtures check pixels,
timestamps, seeking, orientation and subtitles through the shared source.

## MQ arithmetic decoding for document images

Implemented. The engine, one-byte contexts and probability table now live in
[`console-core-mq-decoding`](../crates/console-core-mq-decoding/src/lib.rs).
Both forks retain only adapters for their infallible bit-reader interface;
their private arithmetic and table copies have gone. JBIG2 selects contexts
and JPEG 2000 interprets bitplanes and chooses its initial probability states.

The shared crate is `no_std`, without allocation. It preserves byte stuffing,
marker handling and 0xFF padding beyond the input. Context resets validate the
state index and clear the most probable symbol. The original Hayro license
notices travel with the port.

Tests check the original bitstream, stuffing boundaries, padding, all 47
probability states and invalid resets. Pixel fixtures cover all four JBIG2
templates and lossless JPEG 2000 with ordinary, RESET and RESET+RESTART coding,
with default features and without them. Books renders both image filters in
PDF pages and checks every pixel.

## CABAC arithmetic decoding for video

Implemented. The range tables, packed context transitions, buffering and
arithmetic now live in
[`console-core-adaptive-arithmetic-decoding`](../crates/console-core-adaptive-arithmetic-decoding/src/lib.rs).
Both forks retain context initialization and storage, with adapters for their
existing calls. Their private engine and table copies have gone.

The shared engine uses named byte positions for starts and restarts and bit
positions for consumed input. AVC converts its bit positions at the adapter;
HEVC keeps byte positions, saved contexts and substream policy. Bypass runs
use batches of at most sixteen bins. Zero padding and the original overrun
allowance remain, and the FilmCraft license notices travel with the port.

The original codecs' CABAC round trips passed before extraction. Shared tests
round-trip decisions, bypass and termination from all 128 packed states, and
compare batched bypass with single bins across refills. Adapter tests check
position units, context preservation and errors. Pixel checksums captured
before extraction cover every frame of four AVC/HEVC films; the film tests
also check seeking, orientation and timestamps. Both forks are tested with
default features and without them.

## PDF and PostScript strings

Pending. The document syntax fork's `src/filter/ascii_85.rs` duplicates the
PostScript token fork's `src/string/ascii_85.rs`. Literal escape decoding is
also repeated in `src/object/string.rs` and `src/string/literal.rs`. Both
pairs carry instructions to keep them synchronized.

Move byte decoding into a small crate under `crates/`. Delimiters, borrowed
strings and output storage stay with their parsers. Preserve the existing
leniency around ASCII85 endings, octal escapes and newline handling. Verify
the existing string tests and document interpretation with no default
features as well as the ordinary build.

## Checksums

Pending. Reflected CRC32 lives in `console-core-checksums/src/crc32.rs`, the
Matroska video fork's `src/ebml.rs`, and the audio metadata fork's
`src/id3/v2/write/mod.rs`. MD5 lives in both `console-core-checksums` and the
audio decoding interfaces fork's `src/checksum/md5.rs`.

Extend `console-core-checksums` with incremental byte APIs. ID3 owns its
synchsafe encoding and the audio interfaces own their monitor adapter. Ogg's
CRC uses a different convention and needs its own named implementation; it
must not be replaced by the reflected CRC used for PNG and ZIP. Verify known
vectors, incremental splits, written tags and FLAC validation.

## PNG chunks in file thumbnails

Implemented. [`console-core-png-files`](../crates/console-core-png-files/src/lib.rs)
accepts generic text entries through `encoded_with_text`. Keywords and Latin-1
values are validated, then written through the existing chunk writer with its
length checks and CRC calculation.

[`console-files`](../crates/console-files/src/thumbnails.rs) supplies
`Thumb::URI` and `Thumb::MTime` while drawing the thumbnail. Its private chunk
writer, header walk and metadata insertion pass have gone. Thumbnail
vocabulary stays in Files.

PNG tests read the emitted text chunks, check their placement, reject invalid
text and decode the result back to the same pixels. Thumbnail tests verify
the address and modification time in a real generated picture alongside the
existing photograph and film fixtures.

## Picture dimensions

Pending. `console-applications/src/image.rs` reads PNG and JPEG headers beside
the native format crates and `forks/console-core-picture-sizes`. The latter
already serves embedded pictures in the SVG fork. Their recognition rules
differ.

Consolidate header inspection behind a lightweight API under `crates/`, then
remove Applications' private parser. Keep its bounded header reads, and keep
raw dimensions distinct from dimensions after orientation. Do not make
application discovery depend on film playback to measure an icon. Verify
truncated headers, supported JPEG frame markers and Steam artwork selection.

## Matroska structure

Pending. The audio and video Matroska container forks independently read EBML
integers, headers, lacing and tracks. Film playback uses the audio stack for
sound and the video stack for pictures.

Move EBML and lacing into owned crates before choosing a single container
model. Streaming playback and indexed reads can retain different traversal
implementations over the same framing. Preserve seeking, unknown sizes,
damaged-stream recovery and track timestamps. Only remove a container
implementation once its audio, video and subtitle cases have replacements.

## MPEG-4 structure

Pending. The audio decoding container's `src/atoms`, the video container's
`src/demux.rs` and the metadata fork's `src/mp4` independently understand box
headers, extended sizes and traversal.

Extract box framing into an owned MPEG-4 container crate. Audio playback,
video playback and metadata editing become callers of it. Fragmented files,
edit lists, sample tables, metadata rewriting and unrecognized boxes must
survive the move. Start with framing; moving an entire decoder into another
decoder's crate would retain the ownership problem.

## Ogg pages

Pending. `forks/console-core-ogg-container` reads pages for metadata editing;
`forks/console-core-audio-decoding-ogg-container/src/page.rs` reads the same
headers, segment tables and packet boundaries for playback.

Bring page mechanics inward into an owned Ogg container crate. Preserve
streaming resynchronization, continued packets and checksum verification,
with adapters for the existing readers. Codec mapping and seeking remain
above page framing. Verify metadata edits and playback of the same Opus and
Vorbis files.

## Deflate outside ZIP

Pending. `console-core-zip-files/src/inflate.rs` and the document syntax
fork's `src/filter/lzw_flate.rs` contain independent Deflate readers. The PDF
reader also tries flate2 before its recovery implementation.

Move compression out of ZIP ownership into an owned Deflate crate. Keep
output limits explicit and damaged-PDF recovery as a separate policy or
implementation. PDF predictors remain in the document reader. Verify ZIP and
PNG round trips alongside damaged PDF fixtures before retiring a reader.

## The viewer decoder catalogue

Pending. `console-media-viewer/src/decoding.rs` independently describes every
listed type as decoded by ffmpeg, while dispatch now lives in Pictures and
Films. Its consumers are tests rather than runtime dispatch.

Remove the stale catalogue or derive capabilities from the actual owners.
Keep checks for advertised MIME types and fallback package availability, but
make decoding real fixtures the evidence for support.
