//! A WebExtension reports a hostname over the browser's native messaging API.
//! The recorder accepts it only for the compositor window owned by that browser.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};

use console_core_atomic_writes::{whole_with_folders, whole_with_folders_and_permissions};
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::activity::{Window, focused};
use crate::{Paths, Result, now, private_directory, setup::shell_word, stored, xdg};

const HOST: &str = "org.console.habits";
const GECKO_ID: &str = "habits@console.local";
const CHROMIUM_KEY: &str = "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAzqv6PZMz39KHvYj3+ZwDtR5NU1VpE4IXudo2LKGO6zA7poYMsbuK1e73wash8ah+STPm+NyJGsM2hejlxyFW+bA7b1xdlT1dWRyrTeulEGSaqthMHSzvuhpOgMqNQxuFL0mCWkWbZW7TtF9RDYKDwAQouEWW0apS6pzID/WhNR/IVmaWIhIfK4hq4YEi/fRb8Sf36Zd4FBufzOk7mUMwa62Z0xgOO/a+LrHLmZBB/fTXHUMf+s3cJReSaw4Fm981FCOG0zcO++okro43v5vU6kcOv9WGGR1l9YZVcAX1tzZVYDOYO1EvJIvGD5paIDvkagS+3rbSlvhne3pL/mVcOQIDAQAB";
const CHROMIUM_ID: &str = "dccdhemodoiaklmgogdccnlkneclehfb";
const FRESH_SECONDS: u64 = 40;
const CHECK_INPUT: Timespec = Timespec {
  tv_sec: 1,
  tv_nsec: 0,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
  domain: Option<String>,
}

#[derive(Serialize)]
struct Site {
  pid: u32,
  address: String,
  at: u64,
  domain: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteRecord {
  pid: u32,
  address: stored::Text<1024>,
  at: u64,
  domain: Option<stored::Text<1024>>,
}

impl From<SiteRecord> for Site {
  fn from(record: SiteRecord) -> Self {
    Self {
      pid: record.pid,
      address: record.address.0,
      at: record.at,
      domain: record.domain.map(|domain| domain.0),
    }
  }
}

pub struct Sites {
  socket: UnixDatagram,
  signals: BTreeMap<u32, Site>,
}

#[must_use]
fn socket_path(paths: &Paths) -> core::result::Result<PathBuf, Never> {
  Ok(paths.runtime.join("sites.sock"))
}

impl Sites {
  pub fn bind(paths: &Paths) -> Result<Self> {
    let Ok(path) = socket_path(paths);

    match fs::remove_file(&path) {
      Ok(()) => {}
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => return Err(Box::new(fault)),
        }
      }
    }

    let socket = UnixDatagram::bind(path)?;
    socket.set_nonblocking(true)?;
    Ok(Self {
      socket,
      signals: BTreeMap::new(),
    })
  }

  pub fn wait_for_signal(&mut self) -> Result<()> {
    let mut ready = [PollFd::new(&self.socket, PollFlags::IN)];

    match poll(&mut ready, Some(&CHECK_INPUT)) {
      Ok(_events) => self.receive(),
      Err(rustix::io::Errno::INTR) => Ok(()),
      Err(fault) => Err(Box::new(fault)),
    }
  }

  pub fn receive(&mut self) -> Result<()> {
    let Ok(at) = now();
    self
      .signals
      .retain(|_, signal| signal.at.abs_diff(at) <= FRESH_SECONDS);
    let mut bytes = [0; 1024];

    for _ in 0..64 {
      let ::core::result::Result::Ok(received) =
        ::console_core_number_conversion::fixed_index(match self.socket.recv(&mut bytes) {
          Ok(length) => length,
          Err(fault) => {
            match ::core::primitive::bool::then_some(
              fault.kind() == io::ErrorKind::WouldBlock,
              (),
            ) {
              ::core::option::Option::Some(()) => break,
              ::core::option::Option::None => return Err(Box::new(fault)),
            }
          }
        });

      let datagram = match bytes.get(
        ..match ::console_core_number_conversion::index(received) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      ) {
        Some(bytes_2) => bytes_2,
        None => continue,
      };
      let signal = match serde_json::from_slice::<SiteRecord>(datagram) {
        Ok(record) => Site::from(record),
        Err(_invalid_signal) => continue,
      };

      match bool::then_some(signal.at.abs_diff(at) <= FRESH_SECONDS, ()) {
        Some(()) => {
          self.signals.insert(signal.pid, signal);
        }
        None => {}
      }
    }

    Ok(())
  }

  #[must_use]
  pub fn domain(&self, window: &Window) -> core::result::Result<Option<String>, Never> {
    let pid = match window.pid {
      Some(pid) => pid,
      None => return Ok(None),
    };
    let signal = match self.signals.get(&pid) {
      Some(signal) => signal,
      None => return Ok(None),
    };

    match bool::then_some(
      signal.address != window.address
        || signal.at.abs_diff({
          let Ok(explicit_value) = now();
          explicit_value
        }) > FRESH_SECONDS,
      (),
    ) {
      Some(()) => {
        return Ok(None);
      }
      None => {}
    }

    Ok(signal.domain.clone())
  }
}

#[must_use]
fn normalized_domain(text: &str) -> core::result::Result<Option<String>, Never> {
  match bool::then_some(
    text.len() > 253 || text.bytes().any(|byte| byte.is_ascii_whitespace()),
    (),
  ) {
    Some(()) => {
      return Ok(None);
    }
    None => {}
  }

  let host = match url::Host::parse(text) {
    Ok(host) => host,
    Err(_invalid_host) => return Ok(None),
  };
  let normalized = host.to_string();

  match bool::then_some(normalized != text.to_ascii_lowercase(), ()) {
    Some(()) => {
      return Ok(None);
    }
    None => {}
  }

  let normalized_2 = normalized.trim_end_matches('.');
  Ok(Some(match normalized_2.strip_prefix("www.") {
    Some(short) => short.to_owned(),
    None => normalized_2.to_owned(),
  }))
}

struct ProcessLineage {
  ancestor: u32,
  window: u32,
}

enum Ownership {
  Matches,
  OutsideLineage,
}

#[must_use]
fn owns_window(check: ProcessLineage) -> core::result::Result<Ownership, Never> {
  let mut parent = check.ancestor;

  for _ in 0..8 {
    match parent.cmp(&1) {
      Ordering::Less | Ordering::Equal => {
        break;
      }
      Ordering::Greater => {}
    }

    match parent.cmp(&check.window) {
      Ordering::Equal => {
        return Ok(Ownership::Matches);
      }
      Ordering::Less | Ordering::Greater => {}
    }

    let stat = match fs::read_to_string(format!("/proc/{parent}/stat")) {
      Ok(stat) => stat,
      Err(_parent_unavailable) => break,
    };

    let fields = {
      let optional_fields = stat.rsplit_once(") ");

      match optional_fields {
        Some((_, matched_fields)) => matched_fields,
        None => break,
      }
    };

    let next = match fields.split_whitespace().nth(1) {
      Some(next) => next,
      None => break,
    };
    parent = match next.parse() {
      Ok(pid_2) => pid_2,
      Err(_invalid_pid) => break,
    };
  }

  Ok(Ownership::OutsideLineage)
}

fn relay(paths: &Paths, parent: u32, message: Message) -> Result<()> {
  let socket = UnixDatagram::unbound()?;
  socket.set_nonblocking(true)?;

  let Ok(path) = socket_path(paths);

  match socket.connect(path) {
    Ok(()) => {}
    Err(fault) => match matches!(
      fault.kind(),
      io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    ).then_some(()) {
      Some(()) => return Ok(()),
      None => return Err(Box::new(fault)),
    },
  }

  let window = {
    let optional_window = focused()?;

    match optional_window {
      Some(matched_window) => matched_window,
      None => return Ok(()),
    }
  };

  let pid = match window.pid {
    Some(pid) => pid,
    None => return Ok(()),
  };
  let Ok(ownership) = owns_window(ProcessLineage {
    ancestor: parent,
    window: pid,
  });

  match ownership {
    Ownership::Matches => {}
    Ownership::OutsideLineage => return Ok(()),
  }

  let domain = match message.domain.as_deref() {
    Some(text) => {
      let Ok(domain) = normalized_domain(text);
      domain
    }
    None => None,
  };
  let Ok(at) = now();
  let signal = Site {
    pid,
    address: window.address,
    at,
    domain,
  };

  let vec = serde_json::to_vec(&signal)?;

  match socket.send(&vec) {
    Ok(_) => Ok(()),
    Err(fault) => match matches!(
      fault.kind(),
      io::ErrorKind::WouldBlock | io::ErrorKind::ConnectionRefused
    ).then_some(()) {
      Some(()) => Ok(()),
      None => Err(Box::new(fault)),
    },
  }
}

pub fn host(paths: &Paths) -> Result<()> {
  let parent =
    match rustix::process::getppid().map(|process| i32::cast_unsigned(process.as_raw_pid())) {
      Some(present) => present,
      None => 0,
    };
  let input = io::stdin();
  let fd = input.as_fd();
  let flags = fcntl_getfl(fd)?;
  fcntl_setfl(fd, flags | OFlags::NONBLOCK)?;
  let outcome = iterate((), |()| {
    Ok(match next_message(fd) {
      Ok(Some(message)) => {
        match relay(paths, parent, message) {
          Ok(()) => {}
          Err(fault) => console_core_logging::warn!("Browser companion: {fault}"),
        }

        Step::Again(())
      }
      Ok(None) => Step::Halt(Ok(())),
      Err(fault) => Step::Halt(Err(fault)),
    })
  })?;
  outcome
}

fn next_message(fd: BorrowedFd<'_>) -> Result<Option<Message>> {
  let mut header = [0; 4];
  let length = read_native(fd, &mut header)?;

  match length {
    NativeRead::Complete => {}
    NativeRead::Closed => return Ok(None),
  }

  #[cfg(target_endian = "little")]
  let message_size = u32::from_le_bytes(header);
  #[cfg(target_endian = "big")]
  let message_size = u32::from_be_bytes(header);

  match message_size.cmp(&1024) {
    Ordering::Greater => return Err(Box::from("browser companion sent an oversized message")),
    Ordering::Less | Ordering::Equal => {}
  }

  let Ok(byte_count) = index(message_size);
  let mut storage = [0; 1024];
  let bytes = storage
    .get_mut(..byte_count)
    .ok_or("invalid companion message length")?;
  let body = read_native(fd, bytes)?;

  match body {
    NativeRead::Complete => serde_json::from_slice(bytes).map(Some).map_err(Box::from),
    NativeRead::Closed => Err(Box::from(
      "browser companion message ended before its declared length",
    )),
  }
}

enum NativeRead {
  Complete,
  Closed,
}

fn read_native(fd: BorrowedFd<'_>, bytes: &mut [u8]) -> Result<NativeRead> {
  let outcome = iterate(bytes, |remaining| {
    match remaining.is_empty().then_some(()) {
      Some(()) => return Ok(Step::Halt(Ok(NativeRead::Complete))),
      None => {}
    }

    let received = rustix::io::read(fd, &mut *remaining).map(|length| {
      let Ok(count) = fixed_index(length);
      count
    });

    Ok(match received {
      Ok(0) => Step::Halt(Ok(NativeRead::Closed)),
      Ok(count) => {
        let Ok(offset) = index(count);

        match remaining.get_mut(offset..) {
          Some(rest) => Step::Again(rest),
          None => Step::Halt(Err(Box::from("invalid companion read length"))),
        }
      }
      Err(rustix::io::Errno::INTR) => Step::Again(remaining),
      Err(rustix::io::Errno::AGAIN) => {
        let mut watched = [PollFd::new(&fd, PollFlags::IN)];

        match poll(&mut watched, Some(&CHECK_INPUT)) {
          Ok(_) | Err(rustix::io::Errno::INTR) => Step::Again(remaining),
          Err(fault) => Step::Halt(Err(Box::from(fault))),
        }
      }
      Err(fault) => Step::Halt(Err(Box::from(fault))),
    })
  })?;
  outcome
}

#[derive(Clone, Copy)]
enum BrowserFamily {
  Chromium,
  Gecko,
}

fn manifests(paths: &Paths, name: &str) -> Result<(BrowserFamily, Vec<PathBuf>)> {
  let Ok(config) = xdg(console_core_places::Base::Configuration, &paths.home);
  Ok(match name {
    "brave" => (
      BrowserFamily::Chromium,
      vec![config.join("BraveSoftware/Brave-Browser/NativeMessagingHosts")],
    ),
    "chromium" => (
      BrowserFamily::Chromium,
      vec![config.join("chromium/NativeMessagingHosts")],
    ),
    "chrome" => (
      BrowserFamily::Chromium,
      vec![config.join("google-chrome/NativeMessagingHosts")],
    ),
    "firefox" | "zen" => (
      BrowserFamily::Gecko,
      vec![paths.home.join(".mozilla/native-messaging-hosts")],
    ),
    "librewolf" => (
      BrowserFamily::Gecko,
      vec![
        paths.home.join(".librewolf/native-messaging-hosts"),
        paths.home.join(".mozilla/native-messaging-hosts"),
      ],
    ),
    _unknown => {
      return Err(Box::from(
        "unknown browser. Choose brave, chromium, chrome, firefox, librewolf, or zen",
      ));
    }
  })
}

fn launcher_script(executable: &Path) -> Result<String> {
  let program = shell_word(executable)?;
  Ok(format!("#!/bin/sh\nexec {program} native-messaging\n"))
}

fn write_launcher(launcher: &Path, executable: &Path) -> Result<()> {
  let wrapper = launcher_script(executable)?;
  whole_with_folders_and_permissions(
    launcher,
    wrapper.as_bytes(),
    fs::Permissions::from_mode(0o700),
  )?;
  Ok(())
}

pub(crate) fn update_installed_launcher(
  paths: &Paths,
  installed: &Path,
  source: &Path,
) -> Result<()> {
  let launcher = paths.state.join("native-messaging");
  let contents = match fs::read(&launcher) {
    Ok(contents) => contents,
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => return Ok(()),
        ::core::option::Option::None => return Err(Box::new(fault)),
      }
    }
  };

  for previous in [
    installed.with_file_name("habits"),
    source.with_file_name("habits"),
  ] {
    let script = launcher_script(&previous)?;

    match bool::then_some(contents == script.as_bytes(), ()) {
      Some(()) => {
        return write_launcher(&launcher, installed);
      }
      None => {}
    }
  }

  Ok(())
}

pub fn setup(paths: &Paths, name: &str) -> Result<()> {
  let (family, manifest_directories) = manifests(paths, name)?;
  let extension = match family {
    BrowserFamily::Gecko => paths.state.join("firefox-extension"),
    BrowserFamily::Chromium => paths.state.join("chromium-extension"),
  };
  private_directory(&extension)?;
  let launcher = paths.state.join("native-messaging");
  let current_exe = env::current_exe()?;
  write_launcher(&launcher, &current_exe)?;
  let mut extension_manifest = json!({
    "manifest_version": 3,
    "name": "Habits Companion",
    "version": "0.1.0",
    "description": "Report the active tab’s domain locally to Habits.",
    "incognito": "not_allowed",
    "permissions": ["tabs", "nativeMessaging", "alarms"]
  });
  let mut host_manifest =
    json!({"name": HOST, "description": "Habits Companion", "type": "stdio", "path": launcher});

  let extension_settings = extension_manifest
    .as_object_mut()
    .ok_or("invalid extension manifest")?;
  let host_settings = host_manifest
    .as_object_mut()
    .ok_or("invalid host manifest")?;

  match family {
    BrowserFamily::Gecko => {
      extension_settings.insert(
        String::from("background"),
        json!({"scripts": ["companion.js"]}),
      );
      extension_settings.insert(
        String::from("browser_specific_settings"),
        json!({"gecko": {"id": GECKO_ID, "strict_min_version": "128.0"}}),
      );
      host_settings.insert(String::from("allowed_extensions"), json!([GECKO_ID]));
    }
    BrowserFamily::Chromium => {
      extension_settings.insert(
        String::from("background"),
        json!({"service_worker": "companion.js"}),
      );
      extension_settings.insert(
        String::from("key"),
        serde_json::Value::String(String::from(CHROMIUM_KEY)),
      );
      host_settings.insert(
        String::from("allowed_origins"),
        json!([format!("chrome-extension://{CHROMIUM_ID}/")]),
      );
    }
  }

  let extension_manifest_2 = serde_json::to_vec_pretty(&extension_manifest)?;
  whole_with_folders(&extension.join("manifest.json"), &extension_manifest_2)?;
  whole_with_folders(
    &extension.join("companion.js"),
    include_bytes!("../web/companion.js"),
  )?;

  for directory in manifest_directories {
    let host_manifest_2 = serde_json::to_vec_pretty(&host_manifest)?;
    whole_with_folders(&directory.join("org.console.habits.json"), &host_manifest_2)?;
  }

  match family {
    BrowserFamily::Gecko => {
      println!(
        "Load {} using ‘Load Temporary Add-on’ on about:debugging#/runtime/this-firefox.",
        extension.join("manifest.json").display()
      );
    }
    BrowserFamily::Chromium => {
      println!(
        "Enable Developer mode on the browser’s Extensions page, then ‘Load unpacked’ from {}.",
        extension.display()
      );
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn accepts_hosts_and_rejects_urls_and_paths() {
    assert_eq!(
      normalized_domain("WWW.Example.org."),
      Ok(Some(String::from("example.org")))
    );
    assert_eq!(normalized_domain("[::1]"), Ok(Some(String::from("[::1]"))));

    for invalid in [
      "https://example.org",
      "example.org/path",
      "example.org:1234",
      "a@example.org",
      "a b",
    ] {
      assert_eq!(normalized_domain(invalid), Ok(None));
    }
  }
}
