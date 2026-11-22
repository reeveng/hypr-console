//! XKB key-symbol values interpreted without compiling a keyboard layout.
//!
//! The same `Keysym` type supplies names, character mappings, casing, keypad
//! classification, and X11 constants. The keymap compiler re-exports this type;
//! it owns layout selection and modifier/group lookup separately.
//!
//! Extracted from this workspace's kbvm copy, originally by mahkoh under
//! MIT or Apache-2.0.

#![forbid(unsafe_code)]

mod keysym;
pub mod syms;

pub use keysym::hidden::Keysym;
pub use keysym::{Lettercase, Region};
