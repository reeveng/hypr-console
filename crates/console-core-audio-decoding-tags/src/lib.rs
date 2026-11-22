//! Readers for the tags an audio file carries: ID3v2, the genre numbers ID3v1
//! left behind, FLAC and Vorbis comments, RIFF INFO chunks and the names
//! iTunes gives free-form atoms. Each takes bytes already cut out of its
//! container and hands back tags and pictures, so the containers decide where
//! a block is and none of them reads a tag format twice.
//! Text tags replace each malformed UTF-8 sequence with U+FFFD so a damaged
//! tag can still display its valid text.

#![forbid(unsafe_code)]

use console_core_never::Never;
use std::borrow::Cow;

pub mod flac;
pub mod id3v1;
pub mod id3v2;
pub mod itunes;
pub mod riff;
pub mod vorbis;

fn utf8_with_replacement(bytes: &[u8]) -> Result<Cow<'_, str>, Never> {
  let (text, _had_malformed_sequences) = encoding_rs::UTF_8.decode_without_bom_handling(bytes);

  Ok(text)
}
