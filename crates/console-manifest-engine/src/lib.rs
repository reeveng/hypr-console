//! What `desktop.conf` and `machines.conf` mean, and nothing that acts on them.
//!
//! The engine was a program and nothing else, so a test that wanted to know
//! what the manifest says had to read it again for itself, and several did,
//! each with its own walk over the headings. Two of them turned the machines
//! file into plain sections by hand, a crate apart, which is the reading
//! `machines` already does for an apply. A format is read in one place, so the
//! reading is a library here and the program is built on it like any caller.
//!
//! Only the reading is in it. Installing, building and asking a machine
//! anything stay in the program, where a test cannot start them by accident.

pub mod machines;
pub mod manifest;
pub mod modes;
pub mod unapplied;
