//! Telling systemd that a unit started as `Type=notify` has done what starting
//! meant.
//!
//! A unit whose start is a program coming up and staying up cannot be judged
//! ready by its process existing. The recovery menu is ready when it is drawn
//! on the console, and a lock screen is ready when the compositor says the
//! session is locked -- which is the moment a suspend waiting on it may go
//! ahead, and not the moment before. `systemctl start` returns when this is
//! said, so a caller that started the unit can believe what it sees next.
//!
//! It is one datagram to the socket systemd names, and libsystemd is not
//! linked to send it.

use std::env;
use std::io;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixDatagram};

const READY: &[u8] = b"READY=1";

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "NOTIFY_SOCKET is systemd's handed to this unit alone, read once at the one moment it is answered, and this is the one place the tree speaks the notify protocol"
    )
)]
pub fn ready() -> io::Result<()> {
    let socket = match env::var_os("NOTIFY_SOCKET") {
        Some(socket) => socket,
        None => return Ok(()),
    };
    let named = match socket.as_encoded_bytes().split_first() {
        Some((b'@', name)) => SocketAddr::from_abstract_name(name),
        Some(_) => SocketAddr::from_pathname(&socket),
        None => return Ok(()),
    };
    let address = named?;
    let made = UnixDatagram::unbound();
    let sender = made?;

    sender.send_to_addr(READY, &address)?;

    Ok(())
}
