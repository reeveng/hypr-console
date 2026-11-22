//! Linux adapters for the console's operating-system contracts.

#![cfg(target_os = "linux")]

pub mod events;
pub mod program_lifetime;
pub mod program_runtime;
pub mod systemd;
