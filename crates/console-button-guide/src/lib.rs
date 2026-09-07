//! What every button does.
//!
//! The guide is read out of the one table that decides it, so it cannot drift
//! from what the device actually does. That table names each job in plain
//! words and says what plays it on this machine, and those are what you see:
//! move a job onto another button and the guide moves with it.

pub mod binds;
pub mod guide;
pub mod printed;
