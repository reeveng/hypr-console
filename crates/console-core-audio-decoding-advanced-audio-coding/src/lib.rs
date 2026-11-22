//! An AAC low-complexity decoder and an ADTS reader, carried from Symphonia.
//!
//! Symphonia's decoder was itself ported, with its author's leave, from NihAV.
//! It does the low-complexity profile with one or two channels, which is what
//! nearly every AAC file a person owns is, and refuses the rest by name instead
//! of guessing. What was carried over decoded a file to the sample, and what
//! `tests/the_decoder.rs` holds is that it still does.
//!
//! It is written for streams nobody vetted: every position a bitstream can name
//! is asked of its list rather than indexed, so a corrupt packet is an error
//! for that packet and not a panic in the player.

#![forbid(unsafe_code)]

mod adts;
mod codebooks;
mod config;
mod decoder;
mod ics;
mod lane;
mod pair;
mod pulse;
mod spectrum;
mod subbands;
mod synthesis;
mod tables;
mod tns;

pub use adts::{AdtsFrame, AdtsReader, adts_frame};
pub use decoder::AacDecoder;
