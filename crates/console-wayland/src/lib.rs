//! Wayland client setup and resources shared by Console's protocol handlers.

#![forbid(unsafe_code)]

pub mod buffer;
pub mod key_repeat;
#[cfg(feature = "virtual-keyboard")]
pub mod keyboard;
pub mod memory;
pub mod pointer;
pub mod registry;
#[cfg(feature = "surface")]
pub mod surface;
pub mod waiting;

pub mod client {
  pub use wayland_client::*;
}

pub mod protocols {
  pub use wayland_protocols::*;
}

#[cfg(feature = "virtual-keyboard")]
pub mod protocols_misc {
  pub use wayland_protocols_misc::*;
}

pub mod protocols_wlr {
  pub use wayland_protocols_wlr::*;
}

pub fn connect() -> Result<client::Connection, client::ConnectError> {
  client::Connection::connect_to_env()
}
