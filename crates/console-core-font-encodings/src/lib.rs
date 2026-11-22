//! Single-byte font encodings: character codes mapped to glyph names.
//!
//! Each module retains its own table and fallback rules. Mac Roman and Mac OS
//! Roman also provide reverse lookups. Mac OS Roman contains supplemental entries
//! and is combined with Mac Roman by callers that need the complete encoding. Compose a table with
//! `console_core_glyph_names::get` when Unicode text is needed.
//! The tables come from the Hayro PDF interpreter.

#![no_std]
#![forbid(unsafe_code)]

pub mod mac_expert;
pub mod mac_os_roman;
pub mod mac_roman;
pub mod standard;
pub mod symbol;
pub mod win_ansi;
pub mod zapf_dings;
