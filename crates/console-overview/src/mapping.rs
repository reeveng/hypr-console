//! Which Hyprland window a listed toplevel is.
//!
//! `ext-foreign-toplevel-list` names a window with an identifier the
//! compositor made up for it, and every move and close this overview asks for
//! names the window by its Hyprland address. Hyprland answers the one from the
//! other with its own small protocol, which no crate carries, so the XML is
//! kept beside this crate as Hyprland wrote it and the client code is generated
//! from it the way `wayland-protocols` generates its own.

#![allow(
    clippy::all,
    missing_docs,
    unused_imports,
    reason = "generated from the protocol's XML by wayland-scanner, which is not written to this tree's rules"
)]

use wayland_client;
use wayland_client::protocol::*;
use wayland_protocols::ext::foreign_toplevel_list::v1::client::*;
use wayland_protocols_wlr::foreign_toplevel::v1::client::*;

pub mod __interfaces {
    use wayland_client::backend as wayland_backend;
    use wayland_client::protocol::__interfaces::*;
    use wayland_protocols::ext::foreign_toplevel_list::v1::client::__interfaces::*;
    use wayland_protocols_wlr::foreign_toplevel::v1::client::__interfaces::*;

    wayland_scanner::generate_interfaces!("protocols/hyprland-toplevel-mapping-v1.xml");
}

use self::__interfaces::*;

wayland_scanner::generate_client_code!("protocols/hyprland-toplevel-mapping-v1.xml");
