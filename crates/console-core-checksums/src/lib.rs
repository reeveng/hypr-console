//! The checksums this desktop has to be able to say out loud.
//!
//! None of these is here because anything wanted a checksum. They are here
//! because somebody else's format names one: the thumbnail spec files a picture
//! under the MD5 of its address, so a picture Dolphin made is one this finds
//! and the other way about, and both a zip entry and a PNG chunk carry a CRC32
//! that the reader checks. A PNG's pixels are a zlib stream, which ends with an
//! Adler-32 that a reader may pass over and a writer has to put there.
//!
//! They were written twice, a crate apart, which is how a checksum goes wrong:
//! the second copy is the one nobody tests against a published vector. Here
//! they are one implementation each with the published vectors beside them.
//!
//! Nothing here is a hash for security and nothing should reach for one that
//! way. MD5 is broken and is named by a specification written before it was;
//! CRC32 and Adler-32 are error checks and were never anything else.

pub mod adler32;
pub mod crc32;
pub mod md5;
