//! Text shaping, font fallback, paragraph layout, and glyph rasterization.
//!
//! Adapted from COSMIC Text; its licenses and origin travel with the source.

#![deny(clippy::cast_ptr_alignment)]
#![deny(clippy::unwrap_used)]
#![deny(missing_debug_implementations)]
#![deny(unreachable_patterns)]
#![deny(unused_must_use)]

extern crate alloc;

pub use self::attrs::*;
mod attrs;

pub use self::bidi_para::*;
mod bidi_para;

pub use self::buffer::*;
mod buffer;

pub use self::buffer_line::*;
mod buffer_line;

pub use self::cached::*;
mod cached;

pub use self::glyph_cache::*;
mod glyph_cache;

pub use self::cursor::*;
mod cursor;

pub use self::font::*;
mod font;

pub use self::layout::*;
mod layout;

pub use self::line_ending::*;
mod line_ending;

pub use self::render::*;
mod render;

pub use self::shape::*;
mod shape;

pub use self::shape_run_cache::*;
mod shape_run_cache;

pub use self::glyphs::*;
mod glyphs;

type BuildHasher = core::hash::BuildHasherDefault<std::hash::DefaultHasher>;

type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasher>;

mod choices;
pub use choices::*;

mod quantities;
pub use quantities::*;
