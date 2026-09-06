//! The on-screen keyboard.
//!
//! # Where this came from
//!
//! This is a port of [wvkbd](https://git.sr.ht/~proycon/wvkbd), which is
//! licensed GPL-3.0-or-later. The layouts, the keymap names and the shape of
//! the drawing were taken from its C, so this is a derivative of it and not an
//! independent keyboard that happens to resemble one. The C sat beside this
//! crate until the port replaced it entirely; it is out of the tree now, and
//! `04ba2e3` is where it was last.
//!
//! That is why this crate is GPL-3.0-or-later. A port of copyleft work carries
//! the copyleft with it, and the licence is not a field in a manifest that can
//! be chosen freely once the code has a parent.
//!
//! It is also why this crate alone says something the workspace does not. The
//! rest of the tree is AGPL-3.0-or-later, and the AGPL is not a later version
//! of the GPL: what wvkbd's "or later" grants is the next GPL, not a different
//! licence with a network clause in it. So the two sit beside each other, which
//! the GPL's own section 13 is written to allow, and this crate keeps the
//! licence it was given.
//!
//! The keyboard has two halves that have to live together:
//!
//! - The **palette binding** in `palette`: every other surface on this machine
//!   is themed from `theme/palette.toml`, and the keyboard is started with its
//!   colours as a long command line. The binding is the contract that makes
//!   the keyboard read like the rest of the desktop, and it can be asked
//!   without a keyboard to ask it of.
//!
//! - The **runtime**, behind the `port` feature: the keyboard itself, rebuilt
//!   by `console apply` like every other program here rather than carried
//!   compiled.
//!
//! The halves used to be two programs. `keyboard-start` read the palette and
//! exec'd the keyboard, because the keyboard was C and could not be handed a
//! Rust crate to ask, and this file said of it that one day "it does not exist
//! at all, because the Rust binary reads the palette itself". That is what it
//! does: `dressed` in the binary puts the palette's colours in front of the
//! command line the unit gave it. What is left of the split is this one --
//! `palette` compiles without the runtime, so which colour every key is can be
//! asked without building a compositor client to ask it of.

pub mod asked;
pub mod palette;

#[cfg(feature = "port")]
pub mod config;
#[cfg(feature = "port")]
pub mod drawing;
#[cfg(feature = "port")]
pub mod gamepad;
#[cfg(feature = "port")]
pub mod keymap;
#[cfg(feature = "port")]
pub mod layout;
#[cfg(feature = "port")]
pub mod paint;
#[cfg(feature = "port")]
pub mod shared_memory;
#[cfg(feature = "port")]
pub mod surface;
#[cfg(feature = "port")]
pub mod typing;
