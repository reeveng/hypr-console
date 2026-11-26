//! Hyprland's compositor-side color transform protocol.

#![allow(
  clippy::all,
  unused_imports,
  reason = "the Wayland scanner generates the protocol bindings"
)]

#[cfg_attr(
  dylint_lib = "explicit106_no_import_alias",
  expect(
    explicit106_no_import_alias,
    reason = "wayland-scanner generates client code that resolves the name wayland_client"
  )
)]
use console_wayland::client as wayland_client;
use wayland_client::protocol::*;

pub mod __interfaces {
  use super::wayland_client;
  #[cfg_attr(
    dylint_lib = "explicit106_no_import_alias",
    expect(
      explicit106_no_import_alias,
      reason = "wayland-scanner generates interface constants that resolve the name wayland_backend"
    )
  )]
  use wayland_client::backend as wayland_backend;
  use wayland_client::protocol::__interfaces::*;

  wayland_scanner::generate_interfaces!("protocols/hyprland-ctm-control-v1.xml");
}

use self::__interfaces::*;

wayland_scanner::generate_client_code!("protocols/hyprland-ctm-control-v1.xml");
