//! Wayland client setup and resources shared by Console's protocol handlers.

#![forbid(unsafe_code)]

pub mod buffer;
#[cfg(feature = "virtual-keyboard")]
pub mod keyboard;
pub mod memory;
pub mod pointer;
pub mod registry;
#[cfg(feature = "surface")]
pub mod surface;
pub mod waiting;

pub use wayland_client as client;
pub use wayland_protocols as protocols;
#[cfg(feature = "virtual-keyboard")]
pub use wayland_protocols_misc as protocols_misc;
pub use wayland_protocols_wlr as protocols_wlr;

pub fn connect() -> Result<client::Connection, client::ConnectError> {
  client::Connection::connect_to_env()
}
