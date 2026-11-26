//! Which Hyprland window a listed toplevel is.
//!
//! `ext-foreign-toplevel-list` names a window with an identifier the
//! compositor made up for it, and every move and close this overview asks for
//! names the window by its Hyprland address. Hyprland answers the one from the
//! other with its own protocol. The scanner generates the client types from
//! the XML shipped with this adapter.

#![allow(
  clippy::all,
  unused_imports,
  reason = "generated from the protocol's XML by wayland-scanner, which is not written to this tree's rules"
)]

#[cfg_attr(
  dylint_lib = "explicit106_no_import_alias",
  expect(
    explicit106_no_import_alias,
    reason = "wayland-scanner generates client code that resolves the name wayland_client"
  )
)]
use console_wayland::client as wayland_client;
use console_wayland::{protocols, protocols_wlr};
use protocols::ext::foreign_toplevel_list::v1::client::*;
use protocols_wlr::foreign_toplevel::v1::client::*;
use wayland_client::protocol::*;

pub mod __interfaces {
  use super::{protocols, protocols_wlr, wayland_client};
  use protocols::ext::foreign_toplevel_list::v1::client::__interfaces::*;
  use protocols_wlr::foreign_toplevel::v1::client::__interfaces::*;
  #[cfg_attr(
    dylint_lib = "explicit106_no_import_alias",
    expect(
      explicit106_no_import_alias,
      reason = "wayland-scanner generates interface constants that resolve the name wayland_backend"
    )
  )]
  use wayland_client::backend as wayland_backend;
  use wayland_client::protocol::__interfaces::*;

  wayland_scanner::generate_interfaces!("protocols/hyprland-toplevel-mapping-v1.xml");
}

use self::__interfaces::*;

wayland_scanner::generate_client_code!("protocols/hyprland-toplevel-mapping-v1.xml");
