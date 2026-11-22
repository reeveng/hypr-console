//! The checksums this desktop has to be able to say out loud.
//!
//! None of these is here because anything wanted a checksum. They are here
//! because somebody else's format names one: the thumbnail spec files a picture
//! under the MD5 of its address, so a picture Dolphin made is one this finds
//! and the other way about, and both a zip entry and a PNG chunk carry a CRC32
//! that the reader checks. A PNG's pixels are a zlib stream, which ends with an
//! Adler-32 that a reader may pass over and a writer has to put there.
//! Every Ogg page carries a CRC32 of its own, the same polynomial read the
//! other way round, and a page written without the right one is a page a
//! player steps over. FLAC headers and frames carry CRC8 and CRC16, MPEG
//! audio carries a reflected CRC16, and FLAC verifies decoded samples with MD5.
//!
//! They were written twice, a crate apart, which is how a checksum goes wrong:
//! the second copy is the one nobody tests against a published vector. Here
//! they are one implementation each with the published vectors beside them.
//!
//! PDF encryption also names SHA-256, SHA-384 and SHA-512. FNV-1a preserves
//! deterministic fingerprints, and OpenType tables carry a padded word sum.
//! MD5 is retained for legacy formats; CRCs, Adler-32 and FNV are not security
//! hashes. Callers own the protocol and security policy around these primitives.

#![no_std]

extern crate alloc;

#[cfg(test)]
#[macro_use]
extern crate std;

mod tables;

pub mod adler32;
pub mod crc32;
pub mod md5;
pub mod ogg;

mod ogg_stream;
mod streaming;
mod streaming_md5;

pub mod crc16;
pub mod crc8;

pub use streaming::Checksum;

mod padding;

pub mod fnv1a;
pub mod opentype;
pub mod sha256;
pub mod sha384;
pub mod sha512;
