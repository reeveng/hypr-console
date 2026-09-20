//! The month, over the desktop, from the clock on the bar.
//!
//! `month` is which month is being looked at and what `cal` said about it,
//! which is arithmetic and a reading and touches no screen. `card` is that,
//! drawn. The clock the bar draws is `console-status-bar`'s and stays there:
//! what it says is a reading of the machine, and this is the one thing a
//! reading cannot be asked -- what is around today.

pub mod card;
pub mod month;

pub use card::{WHO, card, door};
