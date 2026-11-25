//! Client bindings for Hyprland's session lock notifications.

#![allow(
  clippy::all,
  unused_imports,
  reason = "the Wayland scanner generates the protocol bindings"
)]

use console_wayland::client as wayland_client;

pub mod __interfaces {
  use super::wayland_client;
  use wayland_client::backend as wayland_backend;

  wayland_scanner::generate_interfaces!("protocols/hyprland-lock-notify-v1.xml");
}

use self::__interfaces::*;

wayland_scanner::generate_client_code!("protocols/hyprland-lock-notify-v1.xml");
