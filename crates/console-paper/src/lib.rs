//! The wallpaper renderer's request/response protocol, scoped to one Wayland display.

use std::cmp::Ordering;
use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
use std::os::unix::net::{SocketAddr, UnixDatagram};
use std::path::{Path, PathBuf};
use std::time::Duration;

use console_core_never::Never;

pub const NAMESPACE: &str = "console-paper";
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "this fixed byte capacity is the length of a Rust array"
  )
)]
const PACKET_BYTES: usize = 48 * 1024;
const REPLY_TIMEOUT: Duration = Duration::from_secs(4);
const LOAD_REPLY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
  Play,
  Freeze,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
  Show(PathBuf, Motion),
  Clear([u8; 3]),
  Query,
  Quit,
}

impl Request {
  fn packet(&self) -> Result<Vec<u8>, Never> {
    Ok(match self {
      Self::Show(path, motion) => {
        let command = match motion {
          Motion::Play => b'I',
          Motion::Freeze => b'F',
        };
        let mut bytes = vec![command];
        bytes.extend_from_slice(path.as_os_str().as_bytes());
        bytes
      }
      Self::Clear([red, green, blue]) => vec![b'C', *red, *green, *blue],
      Self::Query => vec![b'Q'],
      Self::Quit => vec![b'X'],
    })
  }

  fn read(bytes: &[u8]) -> io::Result<Self> {
    match bytes {
      [b'Q'] => Ok(Self::Query),
      [b'X'] => Ok(Self::Quit),
      [b'C', red, green, blue] => Ok(Self::Clear([*red, *green, *blue])),
      [b'I' | b'F'] => invalid("unknown wallpaper request"),
      [command @ (b'I' | b'F'), path @ ..] => {
        let place = PathBuf::from(std::ffi::OsString::from_vec(path.to_vec()));

        match (place.is_absolute(), path.contains(&0)) {
          (true, false) => {
            let motion = match command {
              b'I' => Motion::Play,
              _ => Motion::Freeze,
            };
            Ok(Self::Show(place, motion))
          }
          _ => invalid("the wallpaper path must be absolute"),
        }
      }
      _ => invalid("unknown wallpaper request"),
    }
  }
}

pub fn color(value: &str) -> io::Result<[u8; 3]> {
  match (
    value.as_bytes(),
    u32::from_str_radix(value, 16),
    value.bytes().all(|byte| byte.is_ascii_hexdigit()),
  ) {
    ([_, _, _, _, _, _], Ok(color), true) => {
      let [_, red, green, blue] = color.to_be_bytes();
      Ok([red, green, blue])
    }
    _ => invalid("expected a six-digit RGB color"),
  }
}

fn invalid<T>(message: &'static str) -> io::Result<T> {
  Err(io::Error::new(io::ErrorKind::InvalidInput, message))
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  expect(
    explicit026_env_read_once,
    reason = "the IPC boundary resolves the Wayland display once when opening its socket"
  )
)]
fn address() -> io::Result<PathBuf> {
  let Ok(runtime) = console_core_places::runtime();
  let root = runtime
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "XDG_RUNTIME_DIR is missing"))?;
  let display = std::env::var_os(console_compositor::session::DISPLAY);
  let name = match display.as_deref() {
    Some(present) => present,
    None => OsStr::new("wayland-0"),
  };
  let path = Path::new(name);
  let component = match path.file_name() {
    Some(base) => base,
    None => return invalid("the Wayland display is not a socket name"),
  };
  let mut file = component.to_os_string();
  file.push("-console-paper.sock");

  match (path.parent(), bool::then_some(path.is_absolute(), ())) {
    (Some(_), Some(())) => Ok(path.with_file_name(file)),
    (Some(parent), None) => match parent.as_os_str().as_bytes() {
      [] => Ok(root.join("wayland").with_file_name(file)),
      [_, ..] => invalid("the Wayland display is not a socket name"),
    },
    (None, _) => invalid("the Wayland display has no parent"),
  }
}

pub fn ask(request: &Request) -> io::Result<String> {
  let file = address()?;
  use rustix::net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType, bind, socket_with};

  let fd = socket_with(
    AddressFamily::UNIX,
    SocketType::DGRAM,
    SocketFlags::CLOEXEC,
    None,
  )?;
  bind(&fd, &SocketAddrUnix::new_unnamed())?;
  let socket = UnixDatagram::from(fd);
  let timeout = match request {
    Request::Show(..) => LOAD_REPLY_TIMEOUT,
    Request::Clear(_) | Request::Query | Request::Quit => REPLY_TIMEOUT,
  };
  socket.set_read_timeout(Some(timeout))?;
  socket.set_write_timeout(Some(REPLY_TIMEOUT))?;
  socket.connect(file)?;
  let Ok(bytes) = request.packet();

  match bytes.len().cmp(&PACKET_BYTES) {
    Ordering::Less => {
      socket.send(&bytes)?;
    }
    Ordering::Equal | Ordering::Greater => return invalid("the wallpaper command is too long"),
  }

  let mut buffer = [0; PACKET_BYTES];
  let received = socket
    .recv(&mut buffer)
    .and_then(|length| complete(&buffer, length))?;

  match received {
    [b'+', text @ ..] => String::from_utf8(text.to_vec()).map_err(io::Error::other),
    [b'-', text @ ..] => {
      let error = String::from_utf8(text.to_vec()).map_err(io::Error::other)?;
      Err(io::Error::other(error))
    }
    _ => invalid("invalid reply from the wallpaper daemon"),
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the Unix datagram interface supplies the number of bytes as a Rust slice length"
  )
)]
fn complete(buffer: &[u8], length: usize) -> io::Result<&[u8]> {
  match length.cmp(&buffer.len()) {
    Ordering::Less => buffer
      .get(..length)
      .ok_or(io::Error::from(io::ErrorKind::InvalidData)),
    Ordering::Equal | Ordering::Greater => invalid("the wallpaper packet is too long"),
  }
}

pub struct Socket {
  pub channel: UnixDatagram,
  file: PathBuf,
  _lock: std::fs::File,
}

#[cfg_attr(
  dylint_lib = "explicit040_no_torn_write",
  expect(
    explicit040_no_torn_write,
    reason = "this empty advisory lock file must keep one inode across starts; it carries no data"
  )
)]
fn open_lock(path: &Path) -> io::Result<std::fs::File> {
  std::fs::OpenOptions::new()
    .create(true)
    .mode(0o600)
    .truncate(false)
    .write(true)
    .open(path)
}

fn bind_server(path: &Path) -> io::Result<UnixDatagram> {
  match UnixDatagram::bind(path) {
    Ok(channel) => Ok(channel),
    Err(fault) => match bool::then_some(fault.kind() == io::ErrorKind::AddrInUse, ()) {
      Some(()) => replace_stale(path, fault),
      None => Err(fault),
    },
  }
}

fn replace_stale(path: &Path, occupied: io::Error) -> io::Result<UnixDatagram> {
  let entry = std::fs::symlink_metadata(path)?;

  match bool::then_some(entry.file_type().is_socket(), ()) {
    Some(()) => {}
    None => return Err(occupied),
  }

  let probe = UnixDatagram::unbound()?;

  match probe.connect(path) {
    Ok(()) => Err(occupied),
    Err(fault) => match bool::then_some(fault.kind() == io::ErrorKind::ConnectionRefused, ()) {
      Some(()) => {
        std::fs::remove_file(path)?;
        UnixDatagram::bind(path)
      }
      None => Err(fault),
    },
  }
}

impl Socket {
  pub fn open() -> io::Result<Self> {
    let file = address()?;

    let lock = open_lock(&file.with_extension("lock"))?;
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
    let channel = bind_server(&file)?;
    channel.set_nonblocking(true)?;
    Ok(Self {
      channel,
      file,
      _lock: lock,
    })
  }

  pub fn receive(&self) -> io::Result<Option<(SocketAddr, Request)>> {
    let mut bytes = [0; PACKET_BYTES];

    match self.channel.recv_from(&mut bytes) {
      Ok((length, peer)) => match complete(&bytes, length).and_then(Request::read) {
        Ok(command) => Ok(Some((peer, command))),
        Err(fault) => {
          let Ok(()) = self.reply(&peer, Err(&fault.to_string()));
          Ok(None)
        }
      },
      Err(fault) => match bool::then_some(fault.kind() == io::ErrorKind::WouldBlock, ()) {
        Some(()) => Ok(None),
        None => Err(fault),
      },
    }
  }

  pub fn reply(&self, peer: &SocketAddr, result: Result<&str, &str>) -> Result<(), Never> {
    let (status, text) = match result {
      Ok(text) => ('+', text),
      Err(fault) => ('-', fault),
    };
    let packet = format!("{status}{text}");
    let sent = match packet.len().cmp(&PACKET_BYTES) {
      Ordering::Less => packet.as_bytes(),
      Ordering::Equal | Ordering::Greater => b"-wallpaper reply exceeds the packet limit",
    };

    match self.channel.send_to_addr(sent, peer) {
      Ok(_) => {}
      Err(fault) => console_core_logging::warn!("could not reply to wallpaper request: {fault}"),
    }

    Ok(())
  }
}

impl Drop for Socket {
  fn drop(&mut self) {
    match std::fs::remove_file(&self.file) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::debug!("could not remove the wallpaper socket: {fault}")
      }
    }
  }
}
