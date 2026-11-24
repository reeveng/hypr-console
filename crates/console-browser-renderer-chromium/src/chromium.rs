//! Chromium owns the document and its sandboxed renderer processes. The
//! adapter reads compositor updates from its private DevTools pipe.
//! Chromium reads descriptor 3 and writes to descriptor 4. The launch wrapper
//! copies the child's standard streams to those descriptors before redirecting them.

#![forbid(unsafe_code)]

#[path = "chromium/context.rs"]
mod context;
#[path = "chromium/host.rs"]
mod host;
#[path = "chromium/reading.rs"]
mod reading;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::time::{Duration, Instant, SystemTime};

use console_browser_renderer::{self as frame, MAX_SIDE, invalid};
use console_core_atomic_writes::whole;
use console_core_external_programs::Program;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_state_machine::SingleEffectMachine;
use console_core_web_engine::{
  DownloadState, Event, FrameLayout, READER_SCRIPT, Request, decoded_request, encoded_event,
};
use console_operating_system_linux::program_lifetime::{BoundToParent, TimedPipe, alongside};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{FileType, Mode, OFlags, fcntl_setfl, fstat, open};
use rustix::pipe::{PipeFlags, pipe_with};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use serde_json::value::{RawValue, to_raw_value};
use serde_json::{Map, Value, json};

use console_browser_downloads::{self as downloads, Id, Name};

const MESSAGE_LIMIT: u64 = 32 * 1024 * 1024;
const IO_WAIT: Duration = Duration::from_secs(15);
const INBOUND_IDLE: Timespec = Timespec {
  tv_sec: 5,
  tv_nsec: 0,
};
pub(crate) const POOL_FLAG: &str = "--chromium-pool";

struct Pipe {
  incoming: BufReader<File>,
  outgoing: TimedPipe,
  _process: BoundToParent,
  message: Vec<u8>,
  pending: VecDeque<Message>,
  next_id: u64,
}

type Preferences = BTreeMap<String, Box<RawValue>>;

fn write_preferences(profile: &Path) -> io::Result<()> {
  let defaults = profile.join("Default");
  std::fs::create_dir_all(&defaults)?;
  let path = defaults.join("Preferences");
  let mut preferences = match std::fs::read(&path) {
    Ok(bytes) => {
      let previous: &RawValue = serde_json::from_slice(&bytes)?;
      preference_object(previous)?
    }
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => Preferences::new(),
      None => return Err(fault),
    },
  };
  set_private_preferences(&mut preferences)?;
  let bytes = serde_json::to_vec(&preferences)?;

  whole(&path, &bytes).map_err(io::Error::other)
}

fn preference_object(raw: &RawValue) -> io::Result<Preferences> {
  match raw.get().trim_start().as_bytes().first() {
    Some(b'{') => serde_json::from_str(raw.get()).map_err(io::Error::from),
    Some(_) | None => Ok(Preferences::new()),
  }
}

fn preference_group(
  preferences: &mut Preferences,
  name: &str,
  update: impl FnOnce(&mut Preferences) -> io::Result<()>,
) -> io::Result<()> {
  let mut group = match preferences.get(name) {
    Some(previous) => preference_object(previous)?,
    None => Preferences::new(),
  };
  update(&mut group)?;
  let encoded = to_raw_value(&group)?;
  preferences.insert(name.to_owned(), encoded);
  Ok(())
}

fn set_private_preferences(preferences: &mut Preferences) -> io::Result<()> {
  preference_group(preferences, "intl", |intl| {
    let languages = to_raw_value("en-US")?;
    intl.insert("selected_languages".to_owned(), languages);
    Ok(())
  })?;
  preference_group(preferences, "profile", |profile| {
    let cookie_mode = to_raw_value(&1_u8)?;
    profile.insert("cookie_controls_mode".to_owned(), cookie_mode);

    preference_group(profile, "default_content_setting_values", |contents| {
      let blocked = to_raw_value(&2_u8)?;

      for permission in [
        "media_stream_camera",
        "media_stream_mic",
        "geolocation",
        "sensors",
        "hid_guard",
        "serial_guard",
        "usb_guard",
        "bluetooth_guard",
        "bluetooth_scanning",
        "clipboard",
        "local_fonts",
      ] {
        contents.insert(permission.to_owned(), blocked.clone());
      }

      Ok(())
    })
  })?;

  preference_group(preferences, "webrtc", |webrtc| {
    let policy = to_raw_value("disable_non_proxied_udp")?;
    webrtc.insert("ip_handling_policy".to_owned(), policy);
    Ok(())
  })
}

#[cfg(test)]
mod preference_tests {
  use super::*;

  #[test]
  fn privacy_updates_leave_other_chromium_preferences_intact() -> io::Result<()> {
    let mut preferences: Preferences = serde_json::from_str(
      r#"{"browser":[true,42],"profile":{"default_content_setting_values":{"font_size":14,"media_stream_camera":1}}}"#,
    )?;
    set_private_preferences(&mut preferences)?;
    let browser = preferences
      .get("browser")
      .ok_or_else(|| invalid("browser preferences lost"))?;
    assert_eq!(browser.get(), "[true,42]");
    let profile = preferences
      .get("profile")
      .ok_or_else(|| invalid("profile preferences lost"))?;
    let profile_groups = preference_object(profile)?;
    let contents = profile_groups
      .get("default_content_setting_values")
      .ok_or_else(|| invalid("content preferences lost"))?;
    let settings = preference_object(contents)?;
    assert_eq!(
      settings.get("font_size").map(|value| value.get()),
      Some("14")
    );
    assert_eq!(
      settings.get("media_stream_camera").map(|value| value.get()),
      Some("2")
    );

    Ok(())
  }
}

impl Pipe {
  fn start(profile: PathBuf, diagnostics: Diagnostics) -> io::Result<Self> {
    write_preferences(&profile)?;
    let (reading, outgoing) = pipe_with(PipeFlags::CLOEXEC)?;
    let (incoming, writing) = pipe_with(PipeFlags::CLOEXEC)?;
    let Ok(mut command) = Program::Sh.command();
    let Ok(chromium) = Program::Chromium.name();
    let stderr = match diagnostics {
      Diagnostics::Show => Stdio::inherit(),
      Diagnostics::Disabled => Stdio::null(),
    };
    let mut profile_flag = OsString::from("--user-data-dir=");
    profile_flag.push(&profile);
    command
      .args([
        "-c",
        r#"exec "$@" 3<&0 4>&1 </dev/null >/dev/null"#,
        "console-chromium",
        chromium,
      ])
      .args([
        "--headless",
        "--lang=en-US",
        "--remote-debugging-pipe",
        "--no-startup-window",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-background-networking",
        "--disable-notifications",
        "--disable-sync",
        "--disable-reading-from-canvas",
        "--disable-webgl",
        "--disable-blink-features=ComputePressure",
        "--disable-features=WebGPUService",
        "--no-pings",
        "--gcm-checkin-url=https://127.0.0.1:0/",
        "--gcm-registration-url=https://127.0.0.1:0/",
        "--gcm-mcs-endpoint=https://127.0.0.1:0/",
      ])
      .arg(profile_flag)
      .env("TZ", "UTC")
      .stdin(Stdio::from(reading))
      .stdout(Stdio::from(writing))
      .stderr(stderr);

    fcntl_setfl(&incoming, OFlags::NONBLOCK)?;
    let outgoing_2 = TimedPipe::new(outgoing)?;
    let process = alongside(&mut command)?;

    Ok(Self {
      incoming: BufReader::new(File::from(incoming)),
      outgoing: outgoing_2,
      _process: process,
      message: Vec::new(),
      pending: VecDeque::new(),
      next_id: 1,
    })
  }

  fn send(&mut self, session: Option<&str>, method: &str, parameters: Value) -> io::Result<u64> {
    let id = self.next_id;
    let checked_value = id
      .checked_add(1)
      .ok_or_else(|| invalid("DevTools request numbers exhausted"))?;
    self.next_id = checked_value;
    let mut message = Map::from_iter([
      ("id".to_string(), Value::from(id)),
      ("method".to_string(), Value::from(method)),
      ("params".to_string(), parameters),
    ]);

    match session {
      Some(target) => {
        message.insert("sessionId".to_string(), Value::from(target));
      }
      None => {}
    }

    let mut wire = serde_json::to_vec(&message)?;
    let Ok(length) = fitted::<_, u64>(wire.len());

    match (length >= MESSAGE_LIMIT).then_some(()) {
      Some(()) => {
        return Err(invalid(
          "the Chromium request is larger than the pipe message limit",
        ));
      }
      None => {}
    }

    wire.push(0);
    let until = deadline()?;
    let mut remaining = wire.as_slice();

    for _ in 0..MESSAGE_LIMIT {
      match remaining {
        [] => return Ok(id),
        [_, ..] => {}
      }

      let wait = remaining_time(until)?;
      let written = self.outgoing.write_some(remaining, wait)?;

      match written {
        0 => return Err(io::Error::from(io::ErrorKind::WriteZero)),
        _ => {}
      }

      let Ok(advanced) = index(written);
      let checked_value_2 = remaining
        .get(advanced..)
        .ok_or_else(|| invalid("the pipe reported an oversized write"))?;
      remaining = checked_value_2;
    }

    match remaining {
      [] => Ok(id),
      [_, ..] => Err(invalid("Chromium accepted too many partial writes")),
    }
  }

  fn read(&mut self, until: Instant) -> io::Result<Message> {
    self.message.clear();

    for _ in 0..MESSAGE_LIMIT {
      let Ok(budget) = message_room(&self.message);
      let room = match budget {
        MessageRoom::Available(room) => room,
        MessageRoom::Full => return Err(invalid("Chromium sent an oversized reply")),
      };
      let wait = remaining_time(until)?;

      match (&mut self.incoming)
        .take(room)
        .read_until(0, &mut self.message)
      {
        Ok(0) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
        Ok(_read) => match self.message.last() {
          Some(0) => {
            self.message.pop();

            return decode_message(&self.message);
          }
          Some(_) | None => {}
        },
        Err(fault) => match (
          fault.kind() == io::ErrorKind::WouldBlock,
          fault.kind() == io::ErrorKind::Interrupted,
        ) {
          (true, _) => ready_for_input(self.incoming.get_ref(), wait)?,
          (false, true) => continue,
          (false, false) => return Err(fault),
        },
      }
    }

    Err(invalid("Chromium sent an oversized reply"))
  }

  fn call(
    &mut self,
    session: Option<&str>,
    method: &str,
    parameters: Value,
  ) -> io::Result<Value> {
    let id = self.send(session, method, parameters)?;
    let until = deadline()?;

    for _ in 0..4096 {
      let message = self.read(until)?;
      let reply = match message {
        Message::Reply(reply) => match (reply.id == id).then_some(()) {
          Some(()) => reply,
          None => {
            self.pending.push_back(Message::Reply(reply));
            continue;
          }
        },
        Message::Ignored => continue,
        pending @ (Message::Event(_) | Message::PageEvent(..)) => {
          self.pending.push_back(pending);
          continue;
        }
      };

      return match reply.error {
        Some(error) => Err(io::Error::other(format!("{method}: {error}"))),
        None => reply
          .result
          .ok_or_else(|| invalid("Chromium returned a reply without a result")),
      };
    }

    Err(invalid("Chromium did not reply after 4096 messages"))
  }

  fn close(&mut self) -> io::Result<()> {
    let Ok(pid) = self._process.id();
    let checked_value = i32::try_from(pid).map_err(io::Error::other)?;
    let pid_2 = Pid::from_raw(checked_value).ok_or_else(|| invalid("Chromium did not start"))?;
    let exited = pidfd_open(pid_2, PidfdFlags::empty())?;
    self.send(None, "Browser.close", json!({}))?;
    let mut watched = [PollFd::new(&exited, PollFlags::IN)];
    let timeout = Timespec {
      tv_sec: 2,
      tv_nsec: 0,
    };

    match poll(&mut watched, Some(&timeout)) {
      Ok(0) => return Err(io::Error::from(io::ErrorKind::TimedOut)),
      Ok(_ready) => {}
      Err(fault) => return Err(io::Error::from(fault)),
    }

    let _status = self._process.wait()?;
    Ok(())
  }
}

#[derive(Clone, Copy)]
pub(crate) enum Diagnostics {
  Disabled,
  Show,
}

fn deadline() -> io::Result<Instant> {
  Instant::now()
    .checked_add(IO_WAIT)
    .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))
}

fn remaining_time(until: Instant) -> io::Result<Duration> {
  until
    .checked_duration_since(Instant::now())
    .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))
}

fn ready_for_input(pipe: &File, wait: Duration) -> io::Result<()> {
  let timeout = Timespec::try_from(wait).map_err(io::Error::other)?;
  let mut descriptors = [PollFd::new(pipe, PollFlags::IN)];

  match poll(&mut descriptors, Some(&timeout)) {
    Ok(0) => Err(io::Error::from(io::ErrorKind::TimedOut)),
    Ok(_) | Err(rustix::io::Errno::INTR) => Ok(()),
    Err(fault) => Err(io::Error::from(fault)),
  }
}

enum MessageRoom {
  Available(u64),
  Full,
}

fn message_room(bytes: &[u8]) -> Result<MessageRoom, Never> {
  let Ok(used) = fitted::<_, u64>(bytes.len());

  Ok(match MESSAGE_LIMIT.saturating_sub(used) {
    0 => MessageRoom::Full,
    remaining => MessageRoom::Available(remaining),
  })
}

enum Message {
  Reply(Reply),
  Event(DevtoolsEvent),
  PageEvent(String, DevtoolsEvent),
  Ignored,
}

struct Reply {
  id: u64,
  session: Option<String>,
  result: Option<Value>,
  error: Option<Value>,
}

enum DevtoolsEvent {
  Screencast(ScreencastFrame),
  Finished,
  Crashed(String),
  TargetChanged(TargetInfo),
  TargetCreated(TargetInfo),
  TargetClosed(String),
  DownloadStarted(Id, PendingDownload),
  DownloadEnded(Id, DownloadEnd),
  FileChooser(u64),
}

enum DownloadEnd {
  Completed,
  Canceled,
}

struct PendingDownload {
  name: Name,
  site: Option<String>,
}

struct ScreencastFrame {
  session: u64,
  image: String,
}

struct TargetInfo {
  id: String,
  context: Option<String>,
  opener: Option<String>,
  url: Option<String>,
  title: Option<String>,
}

fn decode_message(bytes: &[u8]) -> io::Result<Message> {
  let mut envelope: Map<String, Value> = serde_json::from_slice(bytes)?;
  let reply_id = envelope.get("id").and_then(Value::as_u64);
  let Ok(session) = take_string(&mut envelope, "sessionId");

  match reply_id {
    Some(id) => {
      return Ok(Message::Reply(Reply {
        id,
        session,
        result: envelope.remove("result"),
        error: envelope.remove("error"),
      }));
    }
    None => {}
  }

  let Ok(method) = take_string(&mut envelope, "method");
  let Ok(params) = take_object(&mut envelope, "params");

  let decoded = match (method.as_deref(), params) {
    (Some("Page.screencastFrame"), Some(params_2)) => decode_frame(params_2),
    (Some("Page.screencastFrame"), None) => {
      Err(invalid("Chromium sent a frame without parameters"))
    }
    (Some("Page.loadEventFired"), _) => Ok(Message::Event(DevtoolsEvent::Finished)),
    (Some("Page.fileChooserOpened"), Some(params_7)) => Ok(
      match params_7.get("backendNodeId").and_then(Value::as_u64) {
        Some(id) => Message::Event(DevtoolsEvent::FileChooser(id)),
        None => Message::Ignored,
      },
    ),
    (Some("Browser.downloadWillBegin" | "Page.downloadWillBegin"), Some(mut params_5)) => {
      let Ok(guid) = take_string(&mut params_5, "guid");
      let Ok(name) = take_string(&mut params_5, "suggestedFilename");
      let site = params_5
        .get("url")
        .and_then(Value::as_str)
        .and_then(|address| {
          let Ok(source) = downloads::source_site(address);

          source
        });

      Ok(match (guid.as_deref().map(Id::parse), name) {
        (Some(Ok(id)), Some(filename)) => {
          let Ok(suggested) = Name::suggested(&filename);

          Message::Event(DevtoolsEvent::DownloadStarted(
            id,
            PendingDownload {
              name: suggested,
              site: site.map(str::to_owned),
            },
          ))
        }
        (Some(Err(downloads::InvalidId)) | None, _) | (Some(Ok(_)), None) => Message::Ignored,
      })
    }
    (Some("Browser.downloadProgress" | "Page.downloadProgress"), Some(mut params_6)) => {
      let Ok(guid) = take_string(&mut params_6, "guid");
      let state = match params_6.get("state").and_then(Value::as_str) {
        Some("completed") => DownloadEnd::Completed,
        Some("canceled") => DownloadEnd::Canceled,
        Some(_) | None => return Ok(Message::Ignored),
      };

      Ok(match guid.as_deref().map(Id::parse) {
        Some(Ok(id)) => Message::Event(DevtoolsEvent::DownloadEnded(id, state)),
        Some(Err(downloads::InvalidId)) | None => Message::Ignored,
      })
    }
    (Some("Target.targetCrashed"), Some(mut params_3)) => {
      let Ok(id) = take_string(&mut params_3, "targetId");

      Ok(match id {
        Some(id_2) => Message::Event(DevtoolsEvent::Crashed(id_2)),
        None => Message::Ignored,
      })
    }
    (Some("Target.targetDestroyed"), Some(mut params_8)) => {
      let Ok(target) = take_string(&mut params_8, "targetId");
      Ok(target.map_or(Message::Ignored, |id| {
        Message::Event(DevtoolsEvent::TargetClosed(id))
      }))
    }
    (
      Some(method_2 @ ("Target.targetInfoChanged" | "Target.targetCreated")),
      Some(mut params_4),
    ) => {
      let Ok(info) = take_object(&mut params_4, "targetInfo");

      match info {
        Some(info_2) => {
          let Ok(event) = target_event(method_2, info_2);

          Ok(event)
        }
        None => Ok(Message::Ignored),
      }
    }
    _ => Ok(Message::Ignored),
  }?;
  Ok(match (session, decoded) {
    (Some(session_2), Message::Event(event)) => Message::PageEvent(session_2, event),
    (_, decoded_2) => decoded_2,
  })
}

fn take_string(fields: &mut Map<String, Value>, name: &str) -> Result<Option<String>, Never> {
  Ok(match fields.remove(name) {
    Some(Value::String(word)) => Some(word),
    Some(
      Value::Object(_) | Value::Array(_) | Value::Number(_) | Value::Bool(_) | Value::Null,
    )
    | None => None,
  })
}

fn take_object(
  fields: &mut Map<String, Value>,
  name: &str,
) -> Result<Option<Map<String, Value>>, Never> {
  Ok(match fields.remove(name) {
    Some(Value::Object(fields_2)) => Some(fields_2),
    Some(
      Value::String(_) | Value::Array(_) | Value::Number(_) | Value::Bool(_) | Value::Null,
    )
    | None => None,
  })
}

fn decode_frame(mut fields: Map<String, Value>) -> io::Result<Message> {
  let session = fields
    .get("sessionId")
    .and_then(Value::as_u64)
    .ok_or_else(|| invalid("Chromium sent a frame without a session"))?;
  let Ok(image) = take_string(&mut fields, "data");
  let image_2 = image.ok_or_else(|| invalid("Chromium sent a frame without pixels"))?;

  Ok(Message::Event(DevtoolsEvent::Screencast(ScreencastFrame {
    session,
    image: image_2,
  })))
}

fn target_event(method: &str, mut fields: Map<String, Value>) -> Result<Message, Never> {
  match {
    ::core::primitive::bool::then_some(
      fields.get("type").and_then(Value::as_str) != Some("page"),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Ok(Message::Ignored);
    }
    ::core::option::Option::None => {}
  }

  let Ok(target) = take_string(&mut fields, "targetId");
  let id = match target {
    Some(id) => id,
    None => return Ok(Message::Ignored),
  };
  let Ok(url) = take_string(&mut fields, "url");
  let Ok(title) = take_string(&mut fields, "title");
  let Ok(context) = take_string(&mut fields, "browserContextId");
  let Ok(opener) = take_string(&mut fields, "openerId");

  let target_2 = TargetInfo {
    id,
    context,
    opener,
    url,
    title,
  };
  Ok(Message::Event(match method {
    "Target.targetCreated" => DevtoolsEvent::TargetCreated(target_2),
    _ => DevtoolsEvent::TargetChanged(target_2),
  }))
}

#[derive(Clone, Copy)]
enum Direction {
  Previous,
  Next,
}

#[derive(Clone, Copy)]
struct HistoryRequest {
  id: u64,
  direction: Direction,
}

#[derive(Clone, Copy)]
enum Appearance {
  Foreground { snapshot: Option<u64> },
  Background,
}

enum Delivery {
  Available,
  WaitingForRelease(Option<String>),
}

#[derive(PartialEq, Eq)]
enum PageRoute {
  Known,
  Unknown,
}

enum PageIdentifier<'a> {
  Session(&'a str),
  Target(&'a str),
}

struct PageTarget {
  id: String,
  session: String,
  address: String,
}

fn attach(pipe: &mut Pipe, target: &str) -> io::Result<String> {
  let attached = pipe.call(
    None,
    "Target.attachToTarget",
    json!({ "targetId": target, "flatten": true }),
  )?;
  let session = attached
    .get("sessionId")
    .and_then(Value::as_str)
    .ok_or_else(|| invalid("Chromium did not attach its page"))?;
  pipe.call(Some(session), "Page.enable", json!({}))?;
  pipe.call(
    Some(session),
    "Page.setInterceptFileChooserDialog",
    json!({ "enabled": true }),
  )?;
  pipe.call(
    Some(session),
    "Emulation.setEmulatedMedia",
    json!({ "features": [{ "name": "prefers-color-scheme", "value": "dark" }] }),
  )?;
  Ok(session.to_owned())
}

struct Chromium {
  channel: OwnedFd,
  target: String,
  session: String,
  openers: Vec<PageTarget>,
  viewport: Size<u32>,
  pointer: Point<f64>,
  appearance: Appearance,
  frame: Delivery,
  history: Option<HistoryRequest>,
  reader: reading::State,
  title_request: Option<u64>,
  navigation_request: Option<u64>,
  file_chooser: Option<u64>,
  zoom: u16,
  address: String,
  staging: Option<PathBuf>,
  downloads: HashMap<Id, PendingDownload>,
}

impl Chromium {
  fn start(
    pipe: &mut Pipe,
    channel: OwnedFd,
    context: &str,
    staging: Option<PathBuf>,
  ) -> io::Result<Self> {
    let new_page = json!({ "url": "about:blank", "browserContextId": context });
    let (target, session) = {
      let devtools = &mut *pipe;
      let page = devtools.call(None, "Target.createTarget", new_page)?;
      let target_name = page
        .get("targetId")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Chromium did not identify its page target"))?;
      let target = target_name.to_owned();
      let session = attach(devtools, &target)?;
      let info = devtools.call(None, "Target.getTargetInfo", json!({ "targetId": target }))?;

      match {
        ::core::primitive::bool::then_some(
          info
            .get("targetInfo")
            .and_then(|info_2| info_2.get("browserContextId"))
            .and_then(Value::as_str)
            != Some(context),
          (),
        )
      } {
        ::core::option::Option::Some(()) => {
          return Err(invalid("Chromium placed a tab outside its private context"));
        }
        ::core::option::Option::None => {}
      }

      (target, session)
    };
    let download_settings = match staging.as_ref().and_then(|path| path.to_str()) {
      Some(path) => json!({
        "behavior": "allowAndName", "downloadPath": path, "eventsEnabled": true, "browserContextId": context
      }),
      None => json!({ "behavior": "deny", "eventsEnabled": true, "browserContextId": context }),
    };

    match pipe.call(None, "Browser.setDownloadBehavior", download_settings) {
      Ok(_) => {}
      Err(fault) => {
        console_core_logging::warn!("could not configure Chromium downloads: {fault}")
      }
    }

    let Ok(reading) = reading::Reading::initialize(&(), None);
    let mut browser = Self {
      channel,
      target,
      session,
      openers: Vec::new(),
      viewport: Size {
        width: 1,
        height: 1,
      },
      pointer: Point { x: 0.0, y: 0.0 },
      appearance: Appearance::Foreground { snapshot: None },
      frame: Delivery::Available,
      history: None,
      reader: reading.state,
      title_request: None,
      navigation_request: None,
      file_chooser: None,
      zoom: 100,
      address: "about:blank".to_owned(),
      staging,
      downloads: HashMap::new(),
    };
    browser.resize(
      pipe,
      Size {
        width: 1100,
        height: 700,
      },
    )?;
    browser.command(pipe, "Page.bringToFront", json!({}))?;
    browser.start_screencast(pipe)?;

    Ok(browser)
  }

  fn command(&mut self, pipe: &mut Pipe, method: &str, parameters: Value) -> io::Result<u64> {
    pipe.send(Some(&self.session), method, parameters)
  }

  fn start_screencast(&mut self, pipe: &mut Pipe) -> io::Result<()> {
    self.command(
      pipe,
      "Page.startScreencast",
      json!({ "format": "png", "maxWidth": MAX_SIDE, "maxHeight": MAX_SIDE }),
    )?;
    Ok(())
  }

  fn snapshot(&mut self, pipe: &mut Pipe) -> io::Result<()> {
    match (self.appearance, self.address.as_str()) {
      (Appearance::Background, _) | (Appearance::Foreground { .. }, "about:blank") => Ok(()),
      (Appearance::Foreground { .. }, _) => {
        let command = self.command(
          pipe,
          "Page.captureScreenshot",
          json!({ "format": "png", "captureBeyondViewport": false }),
        )?;
        self.appearance = Appearance::Foreground {
          snapshot: Some(command),
        };
        Ok(())
      }
    }
  }

  fn clear_snapshot(&mut self) -> Result<(), Never> {
    match &mut self.appearance {
      Appearance::Foreground { snapshot } => *snapshot = None,
      Appearance::Background => {}
    }

    Ok(())
  }

  fn show(&mut self, pipe: &mut Pipe) -> io::Result<()> {
    match self.appearance {
      Appearance::Foreground { .. } => Ok(()),
      Appearance::Background => {
        self.appearance = Appearance::Foreground { snapshot: None };
        self.command(pipe, "Page.bringToFront", json!({}))?;
        self.start_screencast(pipe)?;
        self.snapshot(pipe)
      }
    }
  }

  fn hide(&mut self, pipe: &mut Pipe) -> io::Result<()> {
    match self.appearance {
      Appearance::Background => return Ok(()),
      Appearance::Foreground { .. } => self.appearance = Appearance::Background,
    }

    self.command(pipe, "Page.stopScreencast", json!({}))?;

    match &mut self.frame {
      Delivery::WaitingForRelease(pending) => *pending = None,
      Delivery::Available => {}
    }

    Ok(())
  }

  fn resize(&mut self, pipe: &mut Pipe, size: Size<u32>) -> io::Result<()> {
    let viewport = Size {
      width: size.width.clamp(1, MAX_SIDE),
      height: size.height.clamp(1, MAX_SIDE),
    };

    match ::core::primitive::bool::then_some(viewport == self.viewport, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        self.viewport = viewport;
        self.command(pipe, "Emulation.setDeviceMetricsOverride", json!({
        "width": viewport.width, "height": viewport.height, "deviceScaleFactor": 1, "mobile": false
      }))?;
      }
    }

    Ok(())
  }

  fn event(&self, event: &Event, descriptor: Option<&OwnedFd>) -> io::Result<()> {
    let Ok(line) = encoded_event(event);

    frame::send(self.channel.as_fd(), &line, descriptor)
  }

  fn control(&mut self, pipe: &mut Pipe, request: Request) -> io::Result<()> {
    match request {
      Request::Open(address) => {
        let unchanged = address == "about:blank"
          && self.address == address
          && self.navigation_request.is_none();

        match unchanged.then_some(()) {
          Some(()) => self.event(&Event::Finished, None)?,
          None => {
            self.file_chooser = None;
            let Ok(()) = self.clear_snapshot();
            self.event(&Event::Started, None)?;
            let command = self.command(pipe, "Page.navigate", json!({ "url": address }))?;
            self.navigation_request = Some(command);
          }
        }
      }
      Request::Resize(size) => self.resize(pipe, size)?,
      Request::Back => self.request_history(pipe, Direction::Previous)?,
      Request::Forward => self.request_history(pipe, Direction::Next)?,
      Request::Reload => {
        self.command(pipe, "Page.reload", json!({}))?;
      }
      Request::Stop => {
        self.command(pipe, "Page.stopLoading", json!({}))?;
      }
      Request::Click(at) => {
        self.pointer = at;

        for state in ["mousePressed", "mouseReleased"] {
          self.command(
            pipe,
            "Input.dispatchMouseEvent",
            json!({
              "type": state, "button": "left", "clickCount": 1, "x": at.x, "y": at.y
            }),
          )?;
        }
      }
      Request::Scroll(by) => {
        self.command(pipe, "Input.dispatchMouseEvent", json!({
          "type": "mouseWheel", "x": self.pointer.x, "y": self.pointer.y, "deltaX": by.x, "deltaY": by.y
        }))?;
      }
      Request::Text(text) | Request::Paste(text) => {
        self.command(pipe, "Input.insertText", json!({ "text": text }))?;
      }
      Request::CopySelection => {
        let Ok(selection) = self.page_tool(
          pipe,
          r#"(() => {
          const field = document.activeElement;
          if (field?.type === 'password') return '';
          if (Number.isInteger(field?.selectionStart) && Number.isInteger(field?.selectionEnd))
            return field.value.slice(field.selectionStart, field.selectionEnd);
          return window.getSelection()?.toString() ?? '';
        })()"#,
        );
        let text = match selection
          .get("result")
          .and_then(|result| result.get("value"))
          .and_then(Value::as_str)
        {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => "",
        };
        self.event(&Event::Selection(text.to_owned()), None)?;
      }
      Request::Key(key) => self.key(pipe, &key)?,
      Request::Read(id) => self.advance_reader(pipe, reading::Request::Start(id))?,
      Request::Find { text, backward } => {
        let expression = format!("window.find({}, false, {backward}, true)", json!(text));
        let Ok(result) = self.page_tool(pipe, &expression);
        let found = match result
          .get("result")
          .and_then(|result_2| result_2.get("value"))
          .and_then(Value::as_bool)
        {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => false,
        };
        self.event(&Event::FindResult(found), None)?;
      }
      Request::Zoom(percent) => {
        self.zoom = percent.clamp(50, 200);
        let Ok(()) = self.apply_zoom(pipe);
      }
      Request::Upload { chooser, location } => self.upload(pipe, chooser, &location)?,
      Request::ClosePopup => {
        match { ::core::primitive::bool::then_some(!self.openers.is_empty(), ()) } {
          ::core::option::Option::Some(()) => {
            pipe.send(
              None,
              "Target.closeTarget",
              json!({ "targetId": self.target }),
            )?;
          }
          ::core::option::Option::None => {}
        }
      }
      Request::Visible(true) => self.show(pipe)?,
      Request::Visible(false) => self.hide(pipe)?,
    }

    Ok(())
  }

  fn request_history(&mut self, pipe: &mut Pipe, direction: Direction) -> io::Result<()> {
    let id = self.command(pipe, "Page.getNavigationHistory", json!({}))?;
    self.history = Some(HistoryRequest { id, direction });

    Ok(())
  }

  fn evaluate_privately(&mut self, pipe: &mut Pipe, expression: &str) -> io::Result<Value> {
    let devtools = pipe;
    let tree = devtools.call(Some(&self.session), "Page.getFrameTree", json!({}))?;
    let frame = tree
      .get("frameTree")
      .and_then(|tree_2| tree_2.get("frame"))
      .and_then(|frame| frame.get("id"))
      .and_then(Value::as_str)
      .ok_or_else(|| invalid("Chromium page has no main frame"))?;
    let world = devtools.call(
      Some(&self.session),
      "Page.createIsolatedWorld",
      json!({ "frameId": frame, "worldName": "console-page-tools" }),
    )?;
    let context = world
      .get("executionContextId")
      .and_then(Value::as_u64)
      .ok_or_else(|| invalid("Chromium could not open the page tools"))?;
    devtools.call(
      Some(&self.session),
      "Runtime.evaluate",
      json!({ "contextId": context, "expression": expression, "returnByValue": true }),
    )
  }

  fn page_tool(
    &mut self,
    pipe: &mut Pipe,
    expression: &str,
  ) -> core::result::Result<Value, console_core_never::Never> {
    Ok(match self.evaluate_privately(pipe, expression) {
      Ok(result) => result,
      Err(fault) => {
        console_core_logging::debug!("Chromium page tool unavailable: {}", fault.kind());
        Value::Null
      }
    })
  }

  fn apply_zoom(
    &mut self,
    pipe: &mut Pipe,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let zoom = self.zoom;
      let Ok(_discarded_reply) = self.page_tool(
        pipe,
        &format!(
          r#"(() => {{
      const style = document.documentElement?.style;
      if (!style) return;
      if ({zoom} === 100) {{
        const previous = window.consolePageZoom;
        if (!previous) return;
        style.setProperty('zoom', previous.value, previous.priority);
        delete window.consolePageZoom;
        return;
      }}
      window.consolePageZoom ??= {{
        value: style.getPropertyValue('zoom'),
        priority: style.getPropertyPriority('zoom')
      }};
      style.setProperty('zoom', '{zoom}%', 'important');
    }})()"#
        ),
      );
    };

    Ok(())
  }

  fn upload(&mut self, pipe: &mut Pipe, chooser: u64, location: &str) -> io::Result<()> {
    match { ::core::primitive::bool::then_some(self.file_chooser != Some(chooser), ()) } {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(decoded) = console_core_file_urls::path(location);
    let path = match decoded.filter(|candidate| candidate.is_absolute()) {
      Some(path) => path,
      None => return Ok(()),
    };
    let opened = match open(&path, OFlags::PATH | OFlags::CLOEXEC, Mode::empty()) {
      Ok(opened) => opened,
      Err(fault) => {
        console_core_logging::debug!("Chromium upload file could not be opened: {fault}");
        return Ok(());
      }
    };
    let attributes = match fstat(&opened) {
      Ok(attributes) => attributes,
      Err(fault) => {
        console_core_logging::debug!("Chromium upload file could not be inspected: {fault}");
        return Ok(());
      }
    };

    let regular = FileType::from_raw_mode(attributes.st_mode) == FileType::RegularFile;
    let name = match (regular, path.to_str()) {
      (true, Some(name)) => name,
      (false, _) | (true, None) => return Ok(()),
    };

    self.file_chooser = None;
    self.command(
      pipe,
      "DOM.setFileInputFiles",
      json!({ "backendNodeId": chooser, "files": [name] }),
    )?;
    Ok(())
  }

  fn key(&mut self, pipe: &mut Pipe, key: &str) -> io::Result<()> {
    let code = match key {
      "BackSpace" => ("Backspace", 8),
      "Tab" => ("Tab", 9),
      "Return" => ("Enter", 13),
      "Escape" => ("Escape", 27),
      "Page_Up" => ("PageUp", 33),
      "Page_Down" => ("PageDown", 34),
      "End" => ("End", 35),
      "Home" => ("Home", 36),
      "Left" => ("ArrowLeft", 37),
      "Up" => ("ArrowUp", 38),
      "Right" => ("ArrowRight", 39),
      "Down" => ("ArrowDown", 40),
      "Delete" => ("Delete", 46),
      _ => return Ok(()),
    };

    for state in ["rawKeyDown", "keyUp"] {
      self.command(
        pipe,
        "Input.dispatchKeyEvent",
        json!({
          "type": state, "key": code.0, "code": code.0, "windowsVirtualKeyCode": code.1
        }),
      )?;
    }

    Ok(())
  }

  fn reply(&mut self, pipe: &mut Pipe, reply: Reply) -> io::Result<()> {
    let snapshot = match self.appearance {
      Appearance::Foreground { snapshot } => snapshot,
      Appearance::Background => None,
    };

    match (snapshot == Some(reply.id)).then_some(()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.clear_snapshot();

        match reply
          .result
          .as_ref()
          .and_then(|result| result.get("data").and_then(Value::as_str))
        {
          Some(image) => {
            self.offer_frame(image.to_owned())?;
          }
          ::core::option::Option::None => {}
        }

        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(self.navigation_request == Some(reply.id), ()) {
      ::core::option::Option::Some(()) => {
        self.navigation_request = None;
        let download = reply
          .result
          .as_ref()
          .and_then(|result| result.get("isDownload"));

        match download.and_then(Value::as_bool) {
          Some(true) => {
            self.event(&Event::Address(self.address.clone()), None)?;
            self.event(&Event::Finished, None)?;
          }
          Some(false) | None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    let Ok(awaiting) = self.reader.awaiting();

    match (awaiting == Some(reply.id)).then_some(()) {
      ::core::option::Option::Some(()) => {
        return self.advance_reader(
          pipe,
          reading::Request::Reply {
            command: reply.id,
            result: reply.result,
          },
        );
      }
      ::core::option::Option::None => {}
    }

    match self.title_request {
      Some(asked) => match (asked == reply.id).then_some(()) {
        Some(()) => {
          self.title_request = None;
          let title = reply
            .result
            .as_ref()
            .and_then(|result| result.get("result"))
            .and_then(|result| result.get("value"))
            .and_then(Value::as_str);

          match title {
            Some(title_2) => self.event(&Event::Title(title_2.to_owned()), None)?,
            None => {}
          }
        }
        None => {}
      },
      None => {}
    }

    match self.history.filter(|request| request.id == reply.id) {
      Some(history) => {
        self.history = None;

        match reply.result.as_ref() {
          Some(value) => self.visit_history(pipe, history.direction, value)?,
          None => {}
        }
      }
      None => {}
    }

    Ok(())
  }

  fn advance_reader(&mut self, pipe: &mut Pipe, request: reading::Request) -> io::Result<()> {
    let Ok(turn) = reading::Reading::handle(std::mem::take(&mut self.reader), request);
    self.reader = turn.state;

    use reading::Effect;
    let command = match turn.effect {
      Some(Effect::FindMainFrame) => self.command(pipe, "Page.getFrameTree", json!({}))?,
      Some(Effect::CreateWorld(frame)) => self.command(
        pipe,
        "Page.createIsolatedWorld",
        json!({ "frameId": frame, "worldName": "console-reader" }),
      )?,
      Some(Effect::ReadText(context)) => self.command(
        pipe,
        "Runtime.evaluate",
        json!({ "contextId": context, "expression": READER_SCRIPT, "returnByValue": true }),
      )?,
      Some(Effect::Publish { id, text }) => return self.event(&Event::Reader(id, text), None),
      None => return Ok(()),
    };
    let Ok(sent) = reading::Reading::handle(
      std::mem::take(&mut self.reader),
      reading::Request::Sent(command),
    );
    self.reader = sent.state;

    Ok(())
  }

  fn visit_history(
    &mut self,
    pipe: &mut Pipe,
    direction: Direction,
    details: &Value,
  ) -> io::Result<()> {
    let current = match details.get("currentIndex").and_then(Value::as_i64) {
      Some(current) => current,
      None => return Ok(()),
    };
    let next = match direction {
      Direction::Previous => current.checked_sub(1),
      Direction::Next => current.checked_add(1),
    };
    let adjacent = match next {
      Some(adjacent) => adjacent,
      None => return Ok(()),
    };
    let page = match u32::try_from(adjacent) {
      Ok(page) => page,
      Err(_outside_history) => return Ok(()),
    };
    let Ok(position) = index(page);
    let entry = details
      .get("entries")
      .and_then(|entries| entries.get(position))
      .and_then(|entry| entry.get("id"));

    match entry {
      Some(id) => {
        self.command(
          pipe,
          "Page.navigateToHistoryEntry",
          json!({ "entryId": id }),
        )?;
      }
      None => {}
    }

    Ok(())
  }

  fn devtools_event(&mut self, pipe: &mut Pipe, event: DevtoolsEvent) -> io::Result<()> {
    match event {
      DevtoolsEvent::Screencast(frame) => self.frame(pipe, frame),
      DevtoolsEvent::Finished => self.load_completed(pipe),
      DevtoolsEvent::Crashed(target) => match (target == self.target).then_some(()) {
        Some(()) => Err(io::Error::other("the Chromium page process exited")),
        None => Ok(()),
      },
      DevtoolsEvent::TargetChanged(info) => self.target_info(info),
      DevtoolsEvent::TargetCreated(info) => self.open_popup(pipe, info),
      DevtoolsEvent::TargetClosed(id) => self.target_closed(pipe, &id),
      DevtoolsEvent::FileChooser(id) => {
        self.file_chooser = Some(id);
        self.event(&Event::FileChooser(id), None)
      }
      DevtoolsEvent::DownloadStarted(id, mut download) => {
        match { ::core::primitive::bool::then_some(download.site.is_none(), ()) } {
          ::core::option::Option::Some(()) => {
            let Ok(site) = downloads::source_site(&self.address);
            download.site = site.map(str::to_owned);
          }
          ::core::option::Option::None => {}
        }

        let Ok(filename) = download.name.as_str();
        self.event(
          &Event::Download(DownloadState::Started, filename.to_owned()),
          None,
        )?;
        self.downloads.insert(id, download);
        Ok(())
      }
      DevtoolsEvent::DownloadEnded(id, state) => self.download_ended(id, state),
    }
  }

  fn load_completed(&mut self, pipe: &mut Pipe) -> io::Result<()> {
    match self.zoom {
      100 => {}
      _ => {
        let Ok(()) = self.apply_zoom(pipe);
      }
    }

    self.event(&Event::Finished, None)?;
    let request = self.command(
      pipe,
      "Runtime.evaluate",
      json!({ "expression": "document.title", "returnByValue": true }),
    )?;
    self.title_request = Some(request);
    self.snapshot(pipe)
  }

  fn route(&self, identifier: PageIdentifier<'_>) -> Result<PageRoute, Never> {
    let known = match identifier {
      PageIdentifier::Session(session) => {
        session == self.session || self.openers.iter().any(|page| page.session == session)
      }
      PageIdentifier::Target(target) => {
        target == self.target || self.openers.iter().any(|page| page.id == target)
      }
    };
    Ok(match known.then_some(()) {
      Some(()) => PageRoute::Known,
      None => PageRoute::Unknown,
    })
  }

  fn download_ended(&mut self, id: Id, state: DownloadEnd) -> io::Result<()> {
    let PendingDownload { name, site } = match self.downloads.remove(&id) {
      Some(download) => download,
      None => return Ok(()),
    };
    let Ok(filename) = name.as_str();
    let outcome = match (state, self.staging.as_ref()) {
      (DownloadEnd::Completed, Some(staging)) => {
        let completed_at = SystemTime::now();

        match downloads::publish(staging, &id, &name, site.as_deref(), completed_at) {
          Ok(()) => DownloadState::Saved,
          Err(fault) => {
            console_core_logging::warn!("could not save browser download {filename:?}: {fault}");
            DownloadState::Failed
          }
        }
      }
      (DownloadEnd::Completed, None) | (DownloadEnd::Canceled, _) => DownloadState::Failed,
    };
    self.event(&Event::Download(outcome, filename.to_owned()), None)
  }

  fn target_info(&mut self, info: TargetInfo) -> io::Result<()> {
    match (info.id == self.target).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    match info
      .url
      .filter(|url| url != &self.address && !url.starts_with("chrome-error:"))
    {
      Some(url) => {
        self.file_chooser = None;
        self.address = url.clone();
        self.event(&Event::Address(url), None)?;
      }
      None => {}
    }

    match info.title {
      Some(title) => self.event(&Event::Title(title), None)?,
      None => {}
    }

    Ok(())
  }

  fn open_popup(&mut self, pipe: &mut Pipe, info: TargetInfo) -> io::Result<()> {
    match {
      ::core::primitive::bool::then_some(info.opener.as_deref() != Some(self.target.as_str()), ())
    } {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match { ::core::primitive::bool::then_some(self.openers.len() >= 5, ()) } {
      ::core::option::Option::Some(()) => {
        pipe.send(None, "Target.closeTarget", json!({ "targetId": info.id }))?;
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let attached = attach(pipe, &info.id);
    let session = match attached {
      Ok(session) => session,
      Err(fault) => {
        console_core_logging::debug!("Chromium popup unavailable: {}", fault.kind());
        return Ok(());
      }
    };
    self.command(pipe, "Page.stopScreencast", json!({}))?;
    let address = match info.url.filter(|url| !url.is_empty()) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => <::std::string::String as ::core::convert::From<
        &::core::primitive::str,
      >>::from("about:blank"),
    };
    self.openers.push(PageTarget {
      id: std::mem::replace(&mut self.target, info.id),
      session: std::mem::replace(&mut self.session, session),
      address: std::mem::replace(&mut self.address, address),
    });
    self.event(&Event::Popup(true), None)?;
    self.page_changed(pipe)
  }

  fn target_closed(&mut self, pipe: &mut Pipe, id: &str) -> io::Result<()> {
    match { ::core::primitive::bool::then_some(id != self.target, ()) } {
      ::core::option::Option::Some(()) => {
        self.openers.retain(|page| page.id != id);
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let previous = self
      .openers
      .pop()
      .ok_or_else(|| invalid("Chromium closed its page"))?;
    self.target = previous.id;
    self.session = previous.session;
    let info = pipe.call(
      None,
      "Target.getTargetInfo",
      json!({ "targetId": self.target }),
    )?;
    self.address = info
      .get("targetInfo")
      .and_then(|info_2| info_2.get("url"))
      .and_then(Value::as_str)
      .map_or(previous.address, str::to_owned);
    self.page_changed(pipe)?;
    self.event(&Event::Popup(!self.openers.is_empty()), None)
  }

  fn page_changed(&mut self, pipe: &mut Pipe) -> io::Result<()> {
    self.history = None;
    self.file_chooser = None;
    self.navigation_request = None;
    self.title_request = None;
    let Ok(()) = self.clear_snapshot();
    let Ok(reader) = reading::Reading::initialize(&(), None);
    self.reader = reader.state;

    match &mut self.frame {
      Delivery::WaitingForRelease(pending) => {
        *pending = None;
      }
      Delivery::Available => {}
    }

    let viewport = self.viewport;
    self.viewport = Size {
      width: 1,
      height: 1,
    };
    self.resize(pipe, viewport)?;

    match self.appearance {
      Appearance::Foreground { .. } => {
        self.command(pipe, "Page.bringToFront", json!({}))?;
        self.start_screencast(pipe)?;
      }
      Appearance::Background => {}
    }

    self.event(&Event::Started, None)?;
    self.event(&Event::Address(self.address.clone()), None)?;
    self.load_completed(pipe)
  }

  fn frame(&mut self, pipe: &mut Pipe, frame: ScreencastFrame) -> io::Result<()> {
    self.command(
      pipe,
      "Page.screencastFrameAck",
      json!({ "sessionId": frame.session }),
    )?;

    match self.appearance {
      Appearance::Foreground { .. } => {
        self.offer_frame(frame.image)?;
      }
      Appearance::Background => {}
    }

    Ok(())
  }

  fn offer_frame(&mut self, image: String) -> io::Result<()> {
    match &mut self.frame {
      Delivery::WaitingForRelease(pending) => {
        *pending = Some(image);
        Ok(())
      }
      Delivery::Available => self.publish_frame(image),
    }
  }

  fn publish_frame(&mut self, encoded: String) -> io::Result<()> {
    let png = console_core_urls::forgiving_base64::decode_to_vec(encoded.as_bytes())
      .map_err(io::Error::other)?;
    let size = console_core_png_files::measured(&png).map_err(io::Error::other)?;

    match (size.width > MAX_SIDE || size.height > MAX_SIDE || size != self.viewport).then_some(())
    {
      Some(()) => return Ok(()),
      None => {}
    }

    let picture = console_core_png_files::decoded(&png).map_err(io::Error::other)?;
    let layout = FrameLayout::rgba(size)
      .map_err(|_invalid_layout| invalid("invalid Chromium frame dimensions"))?;
    let _pixels = layout
      .pixels(&picture.bytes)
      .map_err(|_wrong_buffer_length| invalid("Chromium sent a short frame"))?;
    let frame_2 = frame::sealed(&picture.bytes)?;
    self.event(&Event::Frame(size), Some(&frame_2))?;
    self.frame = Delivery::WaitingForRelease(None);

    Ok(())
  }

  fn parent_request(&mut self, pipe: &mut Pipe) -> io::Result<()> {
    let packet = frame::receive(self.channel.as_fd())?;

    match packet.frame {
      Some(_) => return Err(invalid("an engine request carried memory")),
      None => {}
    }

    match packet.line.as_str() {
      frame::RELEASED => match std::mem::replace(&mut self.frame, Delivery::Available) {
        Delivery::WaitingForRelease(Some(frame)) => self.publish_frame(frame)?,
        Delivery::Available | Delivery::WaitingForRelease(None) => {}
      },
      line => {
        let Ok(request) = decoded_request(line);

        match request {
          Some(command) => self.control(pipe, command)?,
          None => {}
        }
      }
    }

    Ok(())
  }
}

#[must_use]
pub(crate) fn pool_main(
  profile: PathBuf,
  diagnostics: Diagnostics,
) -> core::result::Result<ExitCode, console_core_never::Never> {
  Ok(
    match host::shared(profile, diagnostics, std::io::stdin().as_fd()) {
      Ok(()) => ExitCode::SUCCESS,
      Err(fault) => {
        console_core_logging::error!("shared Chromium: {fault}");
        ExitCode::FAILURE
      }
    },
  )
}
