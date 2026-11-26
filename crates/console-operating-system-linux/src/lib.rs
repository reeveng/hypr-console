//! Linux adapters for the console's operating-system contracts.

#![cfg(target_os = "linux")]
#![feature(process_setsid)]

pub mod events;
pub mod notification;
pub mod program_lifetime;
pub mod program_runtime;
pub mod systemd;
pub mod unix_stream;
