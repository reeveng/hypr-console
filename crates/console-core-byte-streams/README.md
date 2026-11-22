# Byte streams

Reads buffered byte streams without knowing the format carried in them. Memory,
files, and forward-only readers share the same byte-reading contract. Bounded
views, bit readers in either bit order, and codebooks compose over that contract.

`MediaSourceStream` owns buffering and cached seeking; `ScopedStream` bounds a
reader to a container region. `BitStreamLtr` reads fields from a byte reader,
while `BitReaderLtr` and `BitReaderRtl` borrow byte slices.
`MonitorStream` feeds successful reads to an incremental checksum. Its `Monitor`
trait is an alias of `console_core_checksums::Checksum`, so arithmetic stays in
the checksum crate. Skipped bytes retain the original reader's behavior.

Counts, lengths and positions are `u64`, the width a stream measures itself in;
the standard library's `usize` is met only where a slice is cut and in the two
`std::io::Read` impls, whose trait answers in it.

The crate has no audio sample, packet, codec, or container types. Its file-backed
source implementation reports a supplied file's length and seekability; callers
open the source. The audio interfaces re-export this crate as `io`, preserving
the original types and import paths.

Extracted from the Symphonia code carried in this workspace. The MPL-2.0 notice
is carried in the crate's module head beside the retained `LICENSE`; this
workspace offers its copy under AGPL-3.0-or-later.
