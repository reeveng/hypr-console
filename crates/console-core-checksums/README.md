# Checksums

Computes the error checks and legacy hashes named by the file formats this
workspace reads. PNG, ZIP and EBML use reflected CRC32; Ogg uses a non-reflected CRC32
with different initialization. FLAC headers use CRC8, FLAC frames use CRC16, and
MPEG audio uses reflected CRC16. MD5 names thumbnails and verifies decoded FLAC
samples and supports legacy PDF encryption. PDF also uses SHA-256, SHA-384 and
SHA-512. OpenType tables use a sum of zero-padded big-endian words. FNV-1a
fingerprints support format probing, rendering identities and regression tests.
CRCs, Adler-32, MD5 and FNV must not be used for new security designs.

`Checksum` accepts incremental byte updates. The concrete checksum types retain
their own initialization and result methods. Stream readers can feed this
contract without the arithmetic depending on a stream, format reader, or codec.
The original audio `io::Monitor` path aliases this same trait.

The `new` constructors, update methods, and checksum queries return
`Result<_, console_core_never::Never>`. Callers handle successful updates and
values explicitly with `let Ok(...) = ...`.

`crc32::update` accepts a raw register for incremental IEEE CRC32 checks;
`crc32::of` supplies its initial and final inversion using the same kernel.

The whole-message `md5::of` and `ogg::of` functions use the same kernels as their
incremental types. Ogg's immutable eight-byte slicing tables derive from the
existing polynomial table. CRC initialization, bit reflection, and finalization
remain specific to each variant. The audio interfaces re-export the original
checksum type names.

The incremental implementations and update contract were extracted from the
Symphonia code carried in this workspace. Its source notices and
`LICENSE-MPL-2.0` are retained; this workspace offers its copy under
AGPL-3.0-or-later.

The crate supports `no_std` with `alloc`. Its number-conversion dependency disables
`std`; that dependency retains its default `std` feature for rounding APIs used
elsewhere in the workspace.

`md5::digest` accepts arbitrary bytes and returns the raw 16-byte digest;
`md5::of` retains the thumbnail API returning lowercase hexadecimal for a string.
`sha256::of`, `sha384::of` and `sha512::of` return raw fixed-size digests.
They came from the PDF reader as ports of PDF.js and were written again from
FIPS 180-4, one padding shared by the three and the published vectors beside
each; the PDF reader's Apache and MIT notices stay beside this crate.

`fnv1a` exposes byte updates and whole-message 32/64-bit fingerprints. Its word
and UTF-16 entry points preserve a single XOR/multiply per input unit, rather
than serializing those units to bytes. `legacy_av1` preserves the two historical
64-bit accumulators used by the AV1 regression suite; it is not standard FNV-128.
Format adapters keep serialization, work accounting and cancellation checkpoints
at their call sites. Hash-table algorithms and random generators remain with
their owning crates; third-party digest implementations are not custom copies.
