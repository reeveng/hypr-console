//! A calculator, the size of the one on a phone.
//!
//! Omarchy answers a sum typed into its launcher, and on a machine with a
//! keyboard that is the quicker road. This one has no keyboard in front of it:
//! typing is the on-screen keyboard, one letter at a time, so a sum is quicker
//! on keys that are already on the screen. [`sum`] is what the keys mean and
//! [`card`] draws them; the launcher can ask `sum` the day it wants to answer
//! a sum typed into it.

pub mod card;
pub mod sum;

pub use card::{WHO, card, door};
