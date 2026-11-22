//! XKB, read here rather than through libxkbcommon.
//!
//! An XKB keymap is two questions: which file of symbols a layout's name
//! comes to, and which keysym a key says under the modifiers and group the
//! compositor last reported. The on-screen keyboard asks the first, to compose
//! a keymap per language for its virtual keyboard; every surface asks the
//! second, of the keymap the compositor hands over. Both were libxkbcommon, a C
//! library reached through bindings, which is a format read somewhere this
//! tree cannot see and a fault that arrives as a null pointer.
//!
//! This is kbvm, by mahkoh (MIT or Apache-2.0, as `README.md` beside this
//! says), brought into the tree and held to its rules. What it could do and
//! nothing here asked was cut rather than kept: the key-press state machine,
//! compose tables, the X11 keymap, the layout registry, the evdev names, and
//! the RMLVO model and options. A keymap is compiled for `pc105` with no
//! options, from the rules the system installs, and the home-directory and
//! environment lookups both libraries make are not made, so where a keymap
//! comes from is the include path the caller built and nothing the process
//! happened to inherit.
//!
//! What was kept answers as kbvm answered. The keymap text and the key lookups
//! for a spread of layouts were compared byte for byte with kbvm's own, before
//! and after every change that brought it under the rules, and kbvm's had been
//! compared with libxkbcommon's.

pub use builder::Builder;
pub use components::Components;
pub use config::DEFAULT_INCLUDE_DIR;
pub use console_input_key_symbols::Keysym;
pub(crate) use console_input_key_symbols::{Lettercase, Region};
pub use group::GroupIndex;
pub use keycode::Keycode;
pub use lookup::Lookup;
pub use lookup::LookupTable;
pub use modifier::hidden::ModifierMask;
pub use xkb::Context;
pub use xkb::ContextBuilder;
pub use xkb::Keymap;
pub use xkb::diagnostic::Diagnostic;
pub use xkb::diagnostic::DiagnosticHandler;
pub use xkb::diagnostic::DiagnosticKind;
pub use xkb::diagnostic::Severity;
pub use xkb::keymap::Formatter;
pub use xkb::rmlvo::Group;

pub(crate) use group_type::hidden::GroupType;
pub(crate) use modifier::hidden::ModifierIndex;

#[macro_use]
mod macros;
mod builder;
mod by_variant;
mod components;
mod config;
mod controls;
mod group;
mod group_type;
mod key_storage;
mod keycode;
mod lookup;
mod modifier;
pub(crate) use console_input_key_symbols::syms;
mod xkb;
