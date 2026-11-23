//! Chromium owns the document and its sandboxed renderer processes. The
//! adapter reads compositor updates from its private DevTools pipe.

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use console_browser_renderer::{self as frame, MAX_SIDE, invalid};
use console_core_external_programs::Program;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_web_engine::{
  Event, FrameLayout, READER_SCRIPT, Request, decoded_request, encoded_event,
};
use console_operating_system_linux::program_lifetime::{BoundToParent, TimedPipe, alongside};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{OFlags, fcntl_setfl};
use rustix::io::fcntl_dupfd_cloexec;
use rustix::pipe::{PipeFlags, pipe_with};
use serde_json::{Map, Value, json};

pub const WORKER: &str = "--chromium-worker";
const MESSAGE_LIMIT: u64 = 32 * 1024 * 1024;
const IO_WAIT: Duration = Duration::from_secs(15);
const INBOUND_IDLE: Timespec = Timespec {
  tv_sec: 5,
  tv_nsec: 0,
};

unsafe extern "C" {
  fn dup2(from: i32, into: i32) -> i32;
}

struct Pipe {
  incoming: BufReader<File>,
  outgoing: TimedPipe,
  _process: BoundToParent,
  message: Vec<u8>,
  next_id: u64,
}

impl Pipe {
  fn start(profile: PathBuf, diagnostics: Diagnostics) -> io::Result<Self> {
    let preferences = profile.join("Default");
    std::fs::create_dir(&preferences)?;
    std::fs::write(
      preferences.join("Preferences"),
      serde_json::to_vec(&json!({
        "profile": {
          "default_content_setting_values": {
            "media_stream_camera": 2, "media_stream_mic": 2, "geolocation": 2,
            "sensors": 2, "hid_guard": 2, "serial_guard": 2, "usb_guard": 2,
            "bluetooth_guard": 2, "bluetooth_scanning": 2, "clipboard": 2
          }
        },
        "webrtc": { "ip_handling_policy": "disable_non_proxied_udp" }
      }))?,
    )?;
    let (reading, outgoing) = pipe_with(PipeFlags::CLOEXEC)?;
    let (incoming, writing) = pipe_with(PipeFlags::CLOEXEC)?;
    let child_reading = fcntl_dupfd_cloexec(&reading, 5)?;
    let child_writing = fcntl_dupfd_cloexec(&writing, 5)?;
    let ends = ChildPipes {
      reading: child_reading,
      writing: child_writing,
    };
    let Ok(mut command) = Program::Chromium.command();
    let stderr = match diagnostics {
      Diagnostics::Show => Stdio::inherit(),
      Diagnostics::Disabled => Stdio::null(),
    };
    let mut profile_flag = OsString::from("--user-data-dir=");
    profile_flag.push(&profile);
    command
      .args([
        "--headless",
        "--remote-debugging-pipe",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-background-networking",
        "--no-pings",
        "about:blank",
      ])
      .arg(profile_flag)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(stderr);

    let Ok(()) = inherit_pipes(&mut command, ends);
    fcntl_setfl(&incoming, OFlags::NONBLOCK)?;
    let outgoing_2 = TimedPipe::new(outgoing)?;
    let process = alongside(&mut command)?;

    Ok(Self {
      incoming: BufReader::new(File::from(incoming)),
      outgoing: outgoing_2,
      _process: process,
      message: Vec::new(),
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
        Message::Reply(reply) => reply,
        Message::Event(_) | Message::Ignored => continue,
      };

      match (reply.id == id).then_some(()) {
        Some(()) => {}
        None => continue,
      }

      return match reply.error {
        Some(error) => Err(io::Error::other(format!("{method}: {error}"))),
        None => reply
          .result
          .ok_or_else(|| invalid("Chromium returned a reply without a result")),
      };
    }

    Err(invalid("Chromium did not reply after 4096 messages"))
  }
}

#[derive(Clone, Copy)]
pub(crate) enum Diagnostics {
  Disabled,
  Show,
}

struct ChildPipes {
  reading: OwnedFd,
  writing: OwnedFd,
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "standard Command requires an unsafe pre-exec registration for the fixed Chromium descriptor handoff"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit095_no_unsafe_in_a_handler",
  expect(
    explicit095_no_unsafe_in_a_handler,
    reason = "install_child_pipes only calls async-signal-safe dup2 on owned descriptors and reports the immediate errno"
  )
)]
fn inherit_pipes(command: &mut Command, pipes: ChildPipes) -> Result<(), Never> {
  // SAFETY: the hook performs only two async-signal-safe dup2 calls. The source descriptors are above 4 and close on exec.
  unsafe {
    command.pre_exec(move || install_child_pipes(&pipes));
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit090_no_raw_descriptor",
  expect(
    explicit090_no_raw_descriptor,
    reason = "Chromium's pipe ABI requires inheriting descriptors 3 and 4 across exec, so their owned sources are borrowed as raw numbers at the dup2 boundary"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the Chromium protocol fixes the two descriptor numbers. The owned source descriptors remain live until the child execs"
  )
)]
fn install_child_pipes(pipes: &ChildPipes) -> io::Result<()> {
  // SAFETY: both source descriptors are owned by the exec hook. Descriptors 3 and 4 belong to the child after fork.
  let copied = unsafe {
    !dup2(pipes.reading.as_raw_fd(), 3).is_negative()
      && !dup2(pipes.writing.as_raw_fd(), 4).is_negative()
  };

  match copied.then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::last_os_error()),
  }
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
  Ignored,
}

struct Reply {
  id: u64,
  result: Option<Value>,
  error: Option<Value>,
}

enum DevtoolsEvent {
  Screencast(ScreencastFrame),
  Finished,
  Crashed(String),
  TargetChanged(TargetInfo),
}

struct ScreencastFrame {
  session: u64,
  image: String,
}

struct TargetInfo {
  id: String,
  url: Option<String>,
  title: Option<String>,
}

fn decode_message(bytes: &[u8]) -> io::Result<Message> {
  let mut envelope: Map<String, Value> = serde_json::from_slice(bytes)?;
  let reply_id = envelope.get("id").and_then(Value::as_u64);

  match reply_id {
    Some(id) => {
      return Ok(Message::Reply(Reply {
        id,
        result: envelope.remove("result"),
        error: envelope.remove("error"),
      }));
    }
    None => {}
  }

  let Ok(method) = take_string(&mut envelope, "method");
  let Ok(params) = take_object(&mut envelope, "params");

  match (method.as_deref(), params) {
    (Some("Page.screencastFrame"), Some(params_2)) => decode_frame(params_2),
    (Some("Page.screencastFrame"), None) => {
      Err(invalid("Chromium sent a frame without parameters"))
    }
    (Some("Page.loadEventFired"), _) => Ok(Message::Event(DevtoolsEvent::Finished)),
    (Some("Target.targetCrashed"), Some(mut params_3)) => {
      let Ok(id) = take_string(&mut params_3, "targetId");

      Ok(match id {
        Some(id_2) => Message::Event(DevtoolsEvent::Crashed(id_2)),
        None => Message::Ignored,
      })
    }
    (Some("Target.targetInfoChanged"), Some(mut params_4)) => {
      let Ok(info) = take_object(&mut params_4, "targetInfo");

      match info {
        Some(info_2) => {
          let Ok(event) = target_changed(info_2);

          Ok(event)
        }
        None => Ok(Message::Ignored),
      }
    }
    _ => Ok(Message::Ignored),
  }
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

fn target_changed(mut fields: Map<String, Value>) -> Result<Message, Never> {
  let Ok(target) = take_string(&mut fields, "targetId");
  let id = match target {
    Some(id) => id,
    None => return Ok(Message::Ignored),
  };
  let Ok(url) = take_string(&mut fields, "url");
  let Ok(title) = take_string(&mut fields, "title");

  Ok(Message::Event(DevtoolsEvent::TargetChanged(TargetInfo {
    id,
    url,
    title,
  })))
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

enum ReaderStage {
  MainFrame,
  IsolatedWorld,
  Text,
}

struct ReaderRequest {
  command: u64,
  id: u64,
  stage: ReaderStage,
}

enum Delivery {
  Available,
  WaitingForRelease(Option<ScreencastFrame>),
}

struct Chromium<'channel> {
  pipe: Pipe,
  channel: BorrowedFd<'channel>,
  target: String,
  session: String,
  viewport: Size<u32>,
  pointer: Point<f64>,
  frame: Delivery,
  history: Option<HistoryRequest>,
  reader: Option<ReaderRequest>,
  title_request: Option<u64>,
  address: String,
}

impl<'channel> Chromium<'channel> {
  fn start(mut pipe: Pipe, channel: BorrowedFd<'channel>) -> io::Result<Self> {
    let made = pipe.call(None, "Target.getTargets", json!({}))?;
    let page = made
      .get("targetInfos")
      .and_then(Value::as_array)
      .and_then(|targets| {
        targets
          .iter()
          .find(|target| target.get("type").and_then(Value::as_str) == Some("page"))
      });
    let identifier = page
      .and_then(|target| target.get("targetId"))
      .and_then(Value::as_str)
      .ok_or_else(|| invalid("Chromium has no page target"))?;
    let target = identifier.to_owned();
    let attached = pipe.call(
      None,
      "Target.attachToTarget",
      json!({ "targetId": target, "flatten": true }),
    )?;
    let session = attached
      .get("sessionId")
      .and_then(Value::as_str)
      .ok_or_else(|| invalid("Chromium did not attach its page"))?;
    let session_2 = session.to_owned();
    pipe.call(
      None,
      "Target.setDiscoverTargets",
      json!({ "discover": true }),
    )?;
    pipe.call(Some(&session_2), "Page.enable", json!({}))?;
    pipe.call(
      Some(&session_2),
      "Emulation.setEmulatedMedia",
      json!({ "features": [{ "name": "prefers-color-scheme", "value": "dark" }] }),
    )?;
    let mut browser = Self {
      pipe,
      channel,
      target,
      session: session_2,
      viewport: Size {
        width: 1,
        height: 1,
      },
      pointer: Point { x: 0.0, y: 0.0 },
      frame: Delivery::Available,
      history: None,
      reader: None,
      title_request: None,
      address: String::new(),
    };
    browser.resize(Size {
      width: 1100,
      height: 700,
    })?;
    browser.command(
      "Page.startScreencast",
      json!({ "format": "png", "maxWidth": MAX_SIDE, "maxHeight": MAX_SIDE }),
    )?;

    Ok(browser)
  }

  fn command(&mut self, method: &str, parameters: Value) -> io::Result<u64> {
    self.pipe.send(Some(&self.session), method, parameters)
  }

  fn resize(&mut self, size: Size<u32>) -> io::Result<()> {
    let viewport = Size {
      width: size.width.clamp(1, MAX_SIDE),
      height: size.height.clamp(1, MAX_SIDE),
    };

    match ::core::primitive::bool::then_some(viewport == self.viewport, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        self.viewport = viewport;
        self.command("Emulation.setDeviceMetricsOverride", json!({
        "width": viewport.width, "height": viewport.height, "deviceScaleFactor": 1, "mobile": false
      }))?;
      }
    }

    Ok(())
  }

  fn event(&self, event: &Event, descriptor: Option<&OwnedFd>) -> io::Result<()> {
    let Ok(line) = encoded_event(event);

    frame::send(self.channel, &line, descriptor)
  }

  fn control(&mut self, request: Request) -> io::Result<()> {
    match request {
      Request::Open(address) => {
        self.event(&Event::Started, None)?;
        self.command("Page.navigate", json!({ "url": address }))?;
      }
      Request::Resize(size) => self.resize(size)?,
      Request::Back => self.request_history(Direction::Previous)?,
      Request::Forward => self.request_history(Direction::Next)?,
      Request::Reload => {
        self.command("Page.reload", json!({}))?;
      }
      Request::Stop => {
        self.command("Page.stopLoading", json!({}))?;
      }
      Request::Click(at) => {
        self.pointer = at;

        for state in ["mousePressed", "mouseReleased"] {
          self.command(
            "Input.dispatchMouseEvent",
            json!({
              "type": state, "button": "left", "clickCount": 1, "x": at.x, "y": at.y
            }),
          )?;
        }
      }
      Request::Scroll(by) => {
        self.command("Input.dispatchMouseEvent", json!({
          "type": "mouseWheel", "x": self.pointer.x, "y": self.pointer.y, "deltaX": by.x, "deltaY": by.y
        }))?;
      }
      Request::Text(text) => {
        self.command("Input.insertText", json!({ "text": text }))?;
      }
      Request::Key(key) => self.key(&key)?,
      Request::Read(id) => {
        let command = self.command("Page.getFrameTree", json!({}))?;
        self.reader = Some(ReaderRequest {
          command,
          id,
          stage: ReaderStage::MainFrame,
        });
      }
    }

    Ok(())
  }

  fn request_history(&mut self, direction: Direction) -> io::Result<()> {
    let id = self.command("Page.getNavigationHistory", json!({}))?;
    self.history = Some(HistoryRequest { id, direction });

    Ok(())
  }

  fn key(&mut self, key: &str) -> io::Result<()> {
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
        "Input.dispatchKeyEvent",
        json!({
          "type": state, "key": code.0, "code": code.0, "windowsVirtualKeyCode": code.1
        }),
      )?;
    }

    Ok(())
  }

  fn reply(&mut self, reply: Reply) -> io::Result<()> {
    match self
      .reader
      .as_ref()
      .filter(|request| request.command == reply.id)
    {
      Some(_) => match self.reader.take() {
        Some(request) => return self.reader_reply(request, reply.result.as_ref()),
        None => {}
      },
      None => {}
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
          Some(value) => self.visit_history(history.direction, value)?,
          None => {}
        }
      }
      None => {}
    }

    Ok(())
  }

  fn reader_reply(&mut self, request: ReaderRequest, result: Option<&Value>) -> io::Result<()> {
    let next = match request.stage {
      ReaderStage::MainFrame => result
        .and_then(|result| result.pointer("/frameTree/frame/id"))
        .map(|frame| {
          (
            "Page.createIsolatedWorld",
            json!({
              "frameId": frame, "worldName": "console-reader"
            }),
            ReaderStage::IsolatedWorld,
          )
        }),
      ReaderStage::IsolatedWorld => result
        .and_then(|result| result.get("executionContextId"))
        .map(|context| {
          (
            "Runtime.evaluate",
            json!({
              "contextId": context, "expression": READER_SCRIPT, "returnByValue": true
            }),
            ReaderStage::Text,
          )
        }),
      ReaderStage::Text => {
        let text = result
          .and_then(|result| result.pointer("/result/value"))
          .and_then(Value::as_str)
          .unwrap_or_default();
        return self.event(&Event::Reader(request.id, text.to_owned()), None);
      }
    };
    match next {
      Some((method, parameters, stage)) => {
        let command = self.command(method, parameters)?;
        self.reader = Some(ReaderRequest {
          command,
          stage,
          ..request
        });
      }
      None => self.event(&Event::Reader(request.id, String::new()), None)?,
    }
    Ok(())
  }

  fn visit_history(&mut self, direction: Direction, details: &Value) -> io::Result<()> {
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
        self.command("Page.navigateToHistoryEntry", json!({ "entryId": id }))?;
      }
      None => {}
    }

    Ok(())
  }

  fn devtools_event(&mut self, event: DevtoolsEvent) -> io::Result<()> {
    match event {
      DevtoolsEvent::Screencast(frame) => self.frame(frame),
      DevtoolsEvent::Finished => {
        self.event(&Event::Finished, None)?;
        let request = self.command(
          "Runtime.evaluate",
          json!({
            "expression": "document.title", "returnByValue": true
          }),
        )?;
        self.title_request = Some(request);

        Ok(())
      }
      DevtoolsEvent::Crashed(target) => match (target == self.target).then_some(()) {
        Some(()) => Err(io::Error::other("the Chromium page process exited")),
        None => Ok(()),
      },
      DevtoolsEvent::TargetChanged(info) => self.target_info(info),
    }
  }

  fn target_info(&mut self, info: TargetInfo) -> io::Result<()> {
    match (info.id == self.target).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    match info.url.filter(|url| {
      url != &self.address && url != "about:blank" && !url.starts_with("chrome-error:")
    }) {
      Some(url) => {
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

  fn frame(&mut self, frame: ScreencastFrame) -> io::Result<()> {
    self.command(
      "Page.screencastFrameAck",
      json!({ "sessionId": frame.session }),
    )?;

    match &mut self.frame {
      Delivery::WaitingForRelease(pending) => {
        *pending = Some(frame);
        Ok(())
      }
      Delivery::Available => self.publish_frame(frame),
    }
  }

  fn publish_frame(&mut self, frame: ScreencastFrame) -> io::Result<()> {
    let encoded = frame.image;
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

  fn run(self) -> io::Result<()> {
    for step in self {
      step?;
    }

    Ok(())
  }

  fn poll_once(&mut self) -> io::Result<Connection> {
    let buffered = self.pipe.incoming.buffer().contains(&0);
    let mut descriptors = [
      PollFd::new(&self.channel, PollFlags::IN),
      PollFd::new(self.pipe.incoming.get_ref(), PollFlags::IN),
    ];
    let timeout = match buffered.then_some(()) {
      Some(()) => Timespec::default(),
      None => INBOUND_IDLE,
    };

    match poll(&mut descriptors, Some(&timeout)) {
      Err(rustix::io::Errno::INTR) => return Ok(Connection::Open),
      Ok(_) => {}
      Err(fault) => return Err(io::Error::from(fault)),
    }

    let [parent, devtools] = descriptors;
    let control = parent.revents();
    let replies = devtools.revents();

    match control
      .intersects(PollFlags::HUP | PollFlags::ERR)
      .then_some(())
    {
      Some(()) => return Ok(Connection::Closed),
      None => {}
    }

    match control.contains(PollFlags::IN).then_some(()) {
      Some(()) => self.parent_request()?,
      None => {}
    }

    match (buffered || replies.intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR))
      .then_some(())
    {
      Some(()) => {
        let until = deadline()?;
        let message = self.pipe.read(until)?;

        match message {
          Message::Reply(reply) => self.reply(reply)?,
          Message::Event(event) => self.devtools_event(event)?,
          Message::Ignored => {}
        }
      }
      None => {}
    }

    Ok(Connection::Open)
  }

  fn parent_request(&mut self) -> io::Result<()> {
    let packet = frame::receive(self.channel)?;

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
          Some(command) => self.control(command)?,
          None => {}
        }
      }
    }

    Ok(())
  }
}

impl Iterator for Chromium<'_> {
  type Item = io::Result<()>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.poll_once() {
      Ok(Connection::Open) => Some(Ok(())),
      Ok(Connection::Closed) => None,
      Err(fault) => Some(Err(fault)),
    }
  }
}

enum Connection {
  Open,
  Closed,
}

pub(crate) fn worker_main(profile: PathBuf, diagnostics: Diagnostics) -> Result<ExitCode, Never> {
  let channel = std::io::stdin();
  let started = Pipe::start(profile, diagnostics)
    .and_then(|pipe| Chromium::start(pipe, channel.as_fd()))
    .and_then(Chromium::run);
  let fault = match started {
    Ok(()) => return Ok(ExitCode::SUCCESS),
    Err(fault) => fault,
  };
  let mut socket = [PollFd::new(&channel, PollFlags::IN)];

  match poll(&mut socket, Some(&Timespec::default())) {
    Ok(_) => {
      let [parent] = socket;

      match parent.revents().contains(PollFlags::HUP).then_some(()) {
        Some(()) => return Ok(ExitCode::SUCCESS),
        None => {}
      }
    }
    Err(error) => {
      console_core_logging::debug!("unable to verify browser parent disconnect: {error}")
    }
  }

  eprintln!("console-browser-window: {fault}");
  Ok(ExitCode::FAILURE)
}
