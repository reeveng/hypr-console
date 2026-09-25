//! What a program has asked to be told.
//!
//! This is the whole of the purity rule, and it is worth saying as a rule
//! because it is the one way this plan can leave the device worse than it
//! found it: **a program may hold only state it has a `Subscription` for.** Anything
//! with no subscription behind it is asked for at the moment it is drawn,
//! exactly as today. Held state that nothing refreshes is a reading that is
//! confidently wrong, and a confidently wrong reading is worse than a slow
//! one.

use std::time::Duration;

use console_core_never::Never;

use crate::event::Topic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subscription {
    Topic(Topic),
    Timer(Timer),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timer {
    pub name: &'static str,
    pub interval: Duration,
}

impl Timer {
    pub const fn new(name: &'static str, interval: Duration) -> Result<Self, Never> {
        Ok(Timer { name, interval })
    }
}
