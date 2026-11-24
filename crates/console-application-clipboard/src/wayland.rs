//! Wayland offers, history, and pending pipe transfers belong to one event queue.

use std::collections::{HashMap, HashSet};
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_state_machine::Machine;
use console_wayland::{client as wayland_client, protocols as wayland_protocols, registry};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::Errno;
use rustix::pipe::{PipeFlags, pipe_with};
use rustix::process::{DumpableBehavior, set_dumpable_behavior};
use wayland_client::backend::{ObjectId, WaylandError};
use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{
  Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop, event_created_child,
};
use wayland_protocols::ext::data_control::v1::client::{
  ext_data_control_device_v1 as device, ext_data_control_manager_v1 as manager,
  ext_data_control_offer_v1 as offer, ext_data_control_source_v1 as source,
};

use crate::copying::{self, Copying};
use crate::history::{Capture, Contents, Format, History, OWN_OFFER, PNG, format};
use crate::ipc::{self, Reply, Request};

const TRANSFER_TIME: Duration = Duration::from_secs(2);
const PASTE_TIME: Duration = Duration::from_millis(150);
const MAX_WRITES: u32 = 8;

struct Incoming {
  fd: OwnedFd,
  contents: Contents,
  until: Instant,
}

enum Received {
  Pending(Incoming),
  Complete(Contents),
  Discarded,
}

struct Outgoing {
  fd: OwnedFd,
  source: ObjectId,
  written: u32,
  until: Instant,
}

enum Sent {
  Pending,
  Finished,
}

struct Published {
  contents: Contents,
  status: SourceStatus,
}

enum SourceStatus {
  Active,
  Cancelled,
}

enum Session {
  Connected,
  Disconnected,
}

enum Readiness {
  Ready,
  Waiting,
}

impl From<bool> for Readiness {
  fn from(ready: bool) -> Self {
    match ready {
      true => Self::Ready,
      false => Self::Waiting,
    }
  }
}

struct Clipboard {
  history: History,
  selection: Option<offer::ExtDataControlOfferV1>,
  primary: Option<offer::ExtDataControlOfferV1>,
  offered: HashMap<ObjectId, Vec<String>>,
  incoming: Option<Incoming>,
  published: HashMap<ObjectId, Published>,
  local_source: Option<ObjectId>,
  outgoing: Vec<Outgoing>,
  session: Session,
  now: Instant,
}

fn nonblocking(fd: &OwnedFd) -> rustix::io::Result<()> {
  let flags = fcntl_getfl(fd)?;
  fcntl_setfl(fd, flags | OFlags::NONBLOCK)
}

fn deadline(now: Instant) -> io::Result<Instant> {
  now
    .checked_add(TRANSFER_TIME)
    .ok_or_else(|| io::Error::other("clipboard transfer deadline overflow"))
}

fn paste_text(contents: &Contents) -> Result<Reply, Never> {
  let Ok(length) = contents.length();

  match (contents.format, (length <= ipc::MAX_PASTE).then_some(())) {
    (Format::Text, Some(())) => match std::str::from_utf8(&contents.bytes) {
      Ok(text) => Ok(Reply::Text(text.to_owned())),
      Err(fault) => {
        console_core_logging::debug!("decoding clipboard text: {fault}");
        Ok(Reply::Unavailable)
      }
    },
    (Format::Text, None) | (Format::Image(_), _) => Ok(Reply::Unavailable),
  }
}

fn receive(
  offer: &offer::ExtDataControlOfferV1,
  offered: &[String],
  now: Instant,
) -> io::Result<Option<Incoming>> {
  let Ok(selected) = format(offered);
  let (kind, mime) = match selected {
    Some(format) => format,
    None => return Ok(None),
  };
  receive_mime(offer, kind, mime, {
    let checked_value = deadline(now)?;
    checked_value
  })
  .map(Some)
}

fn receive_mime(
  offer: &offer::ExtDataControlOfferV1,
  kind: Format,
  mime: &str,
  until: Instant,
) -> io::Result<Incoming> {
  let (read, write) = pipe_with(PipeFlags::CLOEXEC)?;
  nonblocking(&read)?;
  offer.receive(mime.to_string(), write.as_fd());

  Ok(Incoming {
    fd: read,
    contents: Contents {
      format: kind,
      bytes: Vec::new(),
    },
    until,
  })
}

impl Incoming {
  fn read(self, now: Instant) -> Result<Received, Never> {
    match self.until.cmp(&now) {
      std::cmp::Ordering::Less | std::cmp::Ordering::Equal => return Ok(Received::Discarded),
      std::cmp::Ordering::Greater => {}
    }

    let mut buffer = [0_u8; 64 * 1024];

    Ok(match rustix::io::read(&self.fd, &mut buffer) {
      Ok(0) => Received::Complete(self.contents),
      Ok(count) => {
        let Ok(filled) = self.append(buffer.get(..count));
        filled
      }
      Err(Errno::AGAIN | Errno::INTR) => Received::Pending(self),
      Err(fault) => {
        console_core_logging::debug!("clipboard read stopped: {fault}");
        Received::Discarded
      }
    })
  }

  fn append(mut self, bytes: Option<&[u8]>) -> Result<Received, Never> {
    let Ok(written) = self.contents.length();
    let Ok(limit) = self.contents.format.limit();
    let remaining = limit.saturating_sub(written);

    let chunk = match bytes {
      Some(chunk) => chunk,
      None => return Ok(Received::Discarded),
    };
    let Ok(count) = fitted::<_, u32>(chunk.len());

    Ok(match count.cmp(&remaining) {
      std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {
        self.contents.bytes.extend_from_slice(chunk);
        Received::Pending(self)
      }
      std::cmp::Ordering::Greater => Received::Discarded,
    })
  }
}

impl Outgoing {
  fn write(&mut self, now: Instant, contents: &Contents) -> Result<Sent, Never> {
    match self.until.cmp(&now) {
      std::cmp::Ordering::Less | std::cmp::Ordering::Equal => return Ok(Sent::Finished),
      std::cmp::Ordering::Greater => {}
    }

    let Ok(offset) = index(self.written);

    let remaining = match contents.bytes.get(offset..) {
      Some(bytes) => bytes,
      None => return Ok(Sent::Finished),
    };

    Ok(match rustix::io::write(&self.fd, remaining) {
      Ok(count) => {
        let Ok(written) = fitted::<_, u32>(count);
        self.written = self.written.saturating_add(written);
        let Ok(length) = contents.length();

        match self.written.cmp(&length) {
          std::cmp::Ordering::Less => Sent::Pending,
          std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => Sent::Finished,
        }
      }
      Err(Errno::AGAIN | Errno::INTR) => Sent::Pending,
      Err(fault) => {
        console_core_logging::debug!("clipboard write stopped: {fault}");
        Sent::Finished
      }
    })
  }
}

impl Clipboard {
  fn new(history: History, now: Instant) -> Result<Self, Never> {
    Ok(Self {
      history,
      selection: None,
      primary: None,
      offered: HashMap::new(),
      incoming: None,
      published: HashMap::new(),
      local_source: None,
      outgoing: Vec::new(),
      session: Session::Connected,
      now,
    })
  }

  fn retire_offer(&mut self, retired: Option<offer::ExtDataControlOfferV1>) -> Result<(), Never> {
    match retired {
      Some(previous) => {
        self.offered.remove(&previous.id());
        previous.destroy();
      }
      None => {}
    }

    Ok(())
  }

  fn select(&mut self, new: Option<offer::ExtDataControlOfferV1>) -> Result<(), Never> {
    let previous = self.selection.take();
    let Ok(()) = self.retire_offer(previous);

    self.incoming = match (self.history.capture, new.as_ref()) {
      (Capture::Paused, _) | (Capture::Recording, None) => None,
      (Capture::Recording, Some(offered)) => {
        let mimes: &[String] = self.offered.get(&offered.id()).map_or(&[], Vec::as_slice);

        match receive(offered, mimes, self.now) {
          Ok(transfer) => transfer,
          Err(fault) => {
            console_core_logging::debug!("couldn’t receive clipboard offer: {fault}");
            None
          }
        }
      }
    };
    self.selection = new;

    Ok(())
  }

  fn ingest(&mut self) -> Result<(), Never> {
    let transfer = match self.incoming.take() {
      Some(pending) => pending,
      None => return Ok(()),
    };
    let Ok(received) = transfer.read(self.now);

    match received {
      Received::Complete(contents) => self.history.remember(contents),
      Received::Pending(transfer_2) => {
        self.incoming = Some(transfer_2);
        Ok(())
      }
      Received::Discarded => Ok(()),
    }
  }

  fn deliver(&mut self) -> Result<(), Never> {
    self
      .outgoing
      .retain_mut(|transfer| match self.published.get(&transfer.source) {
        Some(published) => {
          let Ok(sent) = transfer.write(self.now, &published.contents);

          matches!(sent, Sent::Pending)
        }
        None => false,
      });

    let pending: HashSet<_> = self
      .outgoing
      .iter()
      .map(|transfer| &transfer.source)
      .collect();
    self
      .published
      .retain(|id, published| match published.status {
        SourceStatus::Active => true,
        SourceStatus::Cancelled => pending.contains(id),
      });

    Ok(())
  }

  fn start_send(&mut self, id: ObjectId, mime: &str, fd: OwnedFd) -> io::Result<()> {
    let Ok(count) = fitted::<_, u32>(self.outgoing.len());

    match count {
      MAX_WRITES.. => return Ok(()),
      0..MAX_WRITES => {}
    }

    let published = match self.published.get(&id) {
      Some(Published {
        status: SourceStatus::Active,
        contents,
      }) => contents,
      Some(Published {
        status: SourceStatus::Cancelled,
        ..
      })
      | None => return Ok(()),
    };
    let Ok(offered) = published.format.offered();

    match offered.contains(&mime).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    nonblocking(&fd)?;
    let until = deadline(self.now)?;
    self.outgoing.push(Outgoing {
      fd,
      source: id,
      written: 0,
      until,
    });

    Ok(())
  }

  fn publish(
    &mut self,
    contents: Contents,
    service: &Service<'_>,
    queue: &QueueHandle<Self>,
  ) -> Result<Reply, Never> {
    let Ok(offered) = contents.format.offered();
    let source = service.manager.create_data_source(queue, ());
    self.local_source = Some(source.id());
    self.published.insert(
      source.id(),
      Published {
        contents,
        status: SourceStatus::Active,
      },
    );

    for mime in offered.into_iter().chain([OWN_OFFER]) {
      source.offer(mime.to_string());
    }

    service.device.set_selection(Some(&source));
    self.incoming = None;

    Ok(match service.connection.flush() {
      Ok(()) => Reply::Finished,
      Err(fault) => {
        console_core_logging::debug!("couldn’t publish the clipboard selection: {fault}");

        Reply::Unavailable
      }
    })
  }

  fn cancel(&mut self, id: ObjectId) -> Result<(), Never> {
    match { ::core::primitive::bool::then_some(self.local_source.as_ref() == Some(&id), ()) } {
      ::core::option::Option::Some(()) => {
        self.local_source = None;
      }
      ::core::option::Option::None => {}
    }

    match self.published.get_mut(&id) {
      Some(published) => published.status = SourceStatus::Cancelled,
      None => {}
    }

    Ok(())
  }

  fn paste(&self, connection: &Connection, clock: &impl Fn() -> Instant) -> io::Result<Reply> {
    match self
      .local_source
      .as_ref()
      .and_then(|id| self.published.get(id))
    {
      Some(local) => {
        let Ok(reply) = paste_text(&local.contents);
        return Ok(reply);
      }
      ::core::option::Option::None => {}
    }

    let selection = match &self.selection {
      Some(matched_selection) => matched_selection,
      ::core::option::Option::None => return Ok(Reply::Unavailable),
    };

    let offered = match self.offered.get(&selection.id()) {
      Some(mimes) => mimes.iter().map(String::as_str).collect::<HashSet<_>>(),
      None => return Ok(Reply::Unavailable),
    };
    let Ok(text_types) = Format::Text.offered();

    let mime = {
      let optional_mime = text_types.into_iter().find(|mime| offered.contains(mime));

      match optional_mime {
        Some(matched_mime) => matched_mime,
        ::core::option::Option::None => return Ok(Reply::Unavailable),
      }
    };

    let until = clock()
      .checked_add(PASTE_TIME)
      .ok_or_else(|| io::Error::other("clipboard paste deadline overflow"))?;
    let mut transfer = receive_mime(selection, Format::Text, mime, until)?;
    connection.flush().map_err(io::Error::other)?;

    for _ in 0..64 {
      let now = clock();

      let remaining = {
        let optional_remaining = until.checked_duration_since(now);

        match optional_remaining {
          Some(matched_remaining) => matched_remaining,
          ::core::option::Option::None => break,
        }
      };

      let timeout = Timespec::try_from(remaining).map_err(io::Error::other)?;
      let mut watched = [PollFd::new(&transfer.fd, PollFlags::IN)];

      match poll(&mut watched, Some(&timeout)) {
        Err(Errno::INTR) => continue,
        Err(fault) => return Err(io::Error::from(fault)),
        Ok(0) => break,
        Ok(_) => {}
      }

      let Ok(received) = transfer.read(clock());

      match received {
        Received::Complete(contents) => {
          let Ok(reply) = paste_text(&contents);
          return Ok(reply);
        }
        Received::Pending(next) => {
          let Ok(length) = next.contents.length();

          match (length <= ipc::MAX_PASTE).then_some(()) {
            Some(()) => transfer = next,
            None => break,
          }
        }
        Received::Discarded => break,
      }
    }

    Ok(Reply::Unavailable)
  }

  fn request(
    &mut self,
    request: Request,
    service: &Service<'_>,
    queue: &QueueHandle<Self>,
    clock: &impl Fn() -> Instant,
  ) -> Result<Reply, Never> {
    match request {
      Request::ReadText => Ok(match self.paste(service.connection, clock) {
        Ok(reply) => reply,
        Err(fault) => {
          console_core_logging::debug!("couldn’t read clipboard text: {}", fault.kind());
          Reply::Unavailable
        }
      }),
      Request::List => {
        let Ok(entries) = self.history.previews();
        Ok(Reply::List {
          paused: self.history.capture,
          entries,
        })
      }
      Request::Restore(id) => {
        let Ok(restored) = self.history.restore(id);

        match restored.cloned() {
          Some(contents) => self.publish(contents, service, queue),
          None => Ok(Reply::Unavailable),
        }
      }
      Request::CopyText(text) => self.copy(
        Contents {
          format: Format::Text,
          bytes: text.into_bytes(),
        },
        service,
        queue,
      ),
      Request::CopyPng(bytes) => self.copy(
        Contents {
          format: Format::Image(PNG),
          bytes,
        },
        service,
        queue,
      ),
      Request::Clear => {
        self.incoming = None;
        let Ok(()) = self.history.clear();
        Ok(Reply::Finished)
      }
      Request::Pause(capture) => self.change_capture(capture),
    }
  }

  fn copy(
    &mut self,
    contents: Contents,
    service: &Service<'_>,
    queue: &QueueHandle<Self>,
  ) -> Result<Reply, Never> {
    let Ok(initial) = Copying::initial_transition(&self.history.capture, None);
    let Ok(first) = Copying::transition(initial.state, copying::Request::Copy(contents));
    let mut state = first.state;

    for effect in first.effects {
      let contents_2 = match effect {
        copying::Effect::Publish(contents_2) => contents_2,
        copying::Effect::Remember(_) => continue,
      };

      let Ok(reply) = self.publish(contents_2, service, queue);
      let Ok(finished) = Copying::transition(state, copying::Request::Published(reply));
      state = finished.state;

      for effect_2 in finished.effects {
        match effect_2 {
          copying::Effect::Remember(contents_3) => {
            let Ok(()) = self.history.remember(contents_3);
          }
          copying::Effect::Publish(_) => {}
        }
      }
    }

    Ok(match state {
      copying::State::Finished(reply) => reply,
      copying::State::Ready(_) | copying::State::Publishing(_) => Reply::Unavailable,
    })
  }

  fn change_capture(&mut self, capture: Capture) -> Result<Reply, Never> {
    match ipc::set_capture(capture) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::warn!("couldn’t save clipboard privacy setting: {fault}");
        return Ok(Reply::Unavailable);
      }
    }

    self.history.capture = capture;
    self.incoming = None;

    Ok(Reply::Finished)
  }
}

type WatchError = Box<dyn std::error::Error>;

pub fn read_system_text(clock: impl Fn() -> Instant) -> Result<Option<String>, WatchError> {
  let connection = console_wayland::connect()?;
  let (globals, mut queue) = registry::queue::<Clipboard>(&connection)?;
  let hand = queue.handle();
  let manager: manager::ExtDataControlManagerV1 = globals.bind(&hand, 1..=1, ())?;
  let seat = registry::seat(&globals, &hand, 1..=1)?;
  let _device = manager.get_data_device(&seat, &hand, ());
  let Ok(history) = History::new(Capture::Paused, 0);
  let Ok(mut clipboard) = Clipboard::new(history, clock());
  queue.roundtrip(&mut clipboard)?;
  let paste = clipboard.paste(&connection, &clock)?;

  Ok(match paste {
    Reply::Text(text) => Some(text),
    Reply::Finished | Reply::Unavailable | Reply::List { .. } => None,
  })
}

pub fn watch(seed: u64, clock: impl Fn() -> Instant) -> Result<(), WatchError> {
  set_dumpable_behavior(DumpableBehavior::NotDumpable)?;
  let connection = console_wayland::connect()?;
  let (globals, queue) = registry::queue::<Clipboard>(&connection)?;
  let hand = queue.handle();
  let manager: manager::ExtDataControlManagerV1 = globals.bind(&hand, 1..=1, ())?;
  let seat = registry::seat(&globals, &hand, 1..=1)?;
  let device = manager.get_data_device(&seat, &hand, ());
  let listener = ipc::listen()?;
  let capture = ipc::capture()?;
  let Ok(history) = History::new(capture, seed);
  let Ok(clipboard) = Clipboard::new(history, clock());
  let service = Service {
    connection: &connection,
    listener: &listener,
    manager: &manager,
    device: &device,
  };

  service.run(queue, clipboard, &clock)
}

struct Service<'a> {
  connection: &'a Connection,
  listener: &'a UnixListener,
  manager: &'a manager::ExtDataControlManagerV1,
  device: &'a device::ExtDataControlDeviceV1,
}

impl Service<'_> {
  #[cfg_attr(
    dylint_lib = "explicit053_no_while",
    expect(
      explicit053_no_while,
      reason = "the clipboard service lives for its Wayland session and exits on a disconnect or an I/O failure rather than after a fixed number of events"
    )
  )]
  fn run(
    &self,
    mut queue: EventQueue<Clipboard>,
    mut clipboard: Clipboard,
    clock: &impl Fn() -> Instant,
  ) -> Result<(), WatchError> {
    while let Session::Connected = clipboard.session {
      self.tick(&mut queue, &mut clipboard, clock)?;
    }

    Err(Box::new(io::Error::other("clipboard device disconnected")))
  }

  fn tick(
    &self,
    queue: &mut EventQueue<Clipboard>,
    clipboard: &mut Clipboard,
    clock: &impl Fn() -> Instant,
  ) -> Result<(), WatchError> {
    clipboard.now = clock();
    queue.dispatch_pending(clipboard)?;

    match clipboard.session {
      Session::Disconnected => return Ok(()),
      Session::Connected => {}
    }

    let Ok(()) = clipboard.ingest();
    let Ok(()) = clipboard.deliver();
    let flags = flush(self.connection)?;

    let read = match self.connection.prepare_read() {
      Some(guard) => guard,
      None => return Ok(()),
    };

    let (wayland, panel) = readiness(self.connection, self.listener, clipboard, flags)?;
    clipboard.now = clock();

    match wayland {
      Readiness::Ready => {
        read_events(read.read().map(|_count| ()))?;
        queue.dispatch_pending(clipboard)?;
        let Ok(()) = clipboard.ingest();
      }
      Readiness::Waiting => drop(read),
    }

    let Ok(()) = self.answer_panel(panel, clipboard, &queue.handle(), clock);

    Ok(())
  }

  fn answer_panel(
    &self,
    panel: Readiness,
    clipboard: &mut Clipboard,
    hand: &QueueHandle<Clipboard>,
    clock: &impl Fn() -> Instant,
  ) -> Result<(), Never> {
    match (panel, &clipboard.session) {
      (Readiness::Ready, Session::Connected) => {
        let answered = ipc::answer(self.listener, clipboard, |state, request| {
          state.request(request, self, hand, clock)
        });

        match answered {
          Ok(()) => {}
          Err(fault) => {
            console_core_logging::debug!("couldn’t answer clipboard panel: {:?}", fault.kind())
          }
        }
      }
      (Readiness::Waiting, _) | (Readiness::Ready, Session::Disconnected) => {}
    }

    Ok(())
  }
}

fn flush(connection: &Connection) -> Result<PollFlags, WaylandError> {
  match connection.flush() {
    Ok(()) => Ok(PollFlags::IN),
    Err(WaylandError::Io(fault)) => {
      match (fault.kind() == io::ErrorKind::WouldBlock).then_some(()) {
        Some(()) => Ok(PollFlags::IN | PollFlags::OUT),
        None => Err(WaylandError::Io(fault)),
      }
    }
    Err(fault @ WaylandError::Protocol(_)) => Err(fault),
  }
}

fn read_events(read: Result<(), WaylandError>) -> Result<(), WaylandError> {
  match read {
    Ok(()) => Ok(()),
    Err(WaylandError::Io(fault)) => {
      match (fault.kind() == io::ErrorKind::WouldBlock).then_some(()) {
        Some(()) => Ok(()),
        None => Err(WaylandError::Io(fault)),
      }
    }
    Err(fault @ WaylandError::Protocol(_)) => Err(fault),
  }
}

fn readiness(
  connection: &Connection,
  listener: &UnixListener,
  clipboard: &Clipboard,
  flags: PollFlags,
) -> io::Result<(Readiness, Readiness)> {
  let mut fds = vec![
    PollFd::new(connection, flags),
    PollFd::new(listener, PollFlags::IN),
  ];

  match &clipboard.incoming {
    Some(incoming) => fds.push(PollFd::new(&incoming.fd, PollFlags::IN)),
    None => {}
  }

  for outgoing in &clipboard.outgoing {
    fds.push(PollFd::new(&outgoing.fd, PollFlags::OUT));
  }

  let deadline = (fds.len() > 2).then_some(Timespec {
    tv_sec: 1,
    tv_nsec: 0,
  });

  match poll(&mut fds, deadline.as_ref()) {
    Ok(_count) => ready_fds(&fds),
    Err(Errno::INTR) => Ok((Readiness::Waiting, Readiness::Waiting)),
    Err(fault) => Err(io::Error::from(fault)),
  }
}

fn ready_fds(fds: &[PollFd<'_>]) -> io::Result<(Readiness, Readiness)> {
  match fds {
    [wayland, panel, ..] => Ok((
      Readiness::from(
        wayland
          .revents()
          .intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR),
      ),
      Readiness::from(panel.revents().contains(PollFlags::IN)),
    )),
    [] | [_] => Err(io::Error::other(
      "clipboard poll is missing its session descriptors",
    )),
  }
}

delegate_noop!(Clipboard: ignore manager::ExtDataControlManagerV1);
delegate_noop!(Clipboard: ignore wl_seat::WlSeat);

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Clipboard {
  fn event(
    _: &mut Self,
    _: &wl_registry::WlRegistry,
    _: wl_registry::Event,
    _: &GlobalListContents,
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
  }
}

impl Dispatch<offer::ExtDataControlOfferV1, ()> for Clipboard {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    expect(
      explicit016_no_wildcard_arm,
      reason = "the compositor binds version 1 and this foreign event enum may contain events from newer versions"
    )
  )]
  fn event(
    clipboard: &mut Self,
    offer: &offer::ExtDataControlOfferV1,
    event: offer::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match event {
      offer::Event::Offer { mime_type } => clipboard
        .offered
        .entry(offer.id())
        .or_default()
        .push(mime_type),
      _newer_protocol_event => {}
    }
  }
}

impl Dispatch<device::ExtDataControlDeviceV1, ()> for Clipboard {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    expect(
      explicit016_no_wildcard_arm,
      reason = "the compositor binds version 1 and this foreign event enum may contain events from newer versions"
    )
  )]
  fn event(
    clipboard: &mut Self,
    device: &device::ExtDataControlDeviceV1,
    event: device::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match event {
      device::Event::Selection { id } => {
        let Ok(()) = clipboard.select(id);
      }
      device::Event::PrimarySelection { id } => {
        let previous = std::mem::replace(&mut clipboard.primary, id);
        let Ok(()) = clipboard.retire_offer(previous);
      }
      device::Event::Finished => {
        clipboard.session = Session::Disconnected;
        device.destroy();
      }
      device::Event::DataOffer { .. } => {}
      _newer_protocol_event => {}
    }
  }

  event_created_child!(Clipboard, device::ExtDataControlDeviceV1, [
    device::EVT_DATA_OFFER_OPCODE => (offer::ExtDataControlOfferV1, ()),
  ]);
}

impl Dispatch<source::ExtDataControlSourceV1, ()> for Clipboard {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    expect(
      explicit016_no_wildcard_arm,
      reason = "the compositor binds version 1 and this foreign event enum may contain events from newer versions"
    )
  )]
  fn event(
    clipboard: &mut Self,
    source: &source::ExtDataControlSourceV1,
    event: source::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match event {
      source::Event::Send { mime_type, fd } => {
        match clipboard.start_send(source.id(), &mime_type, fd) {
          Ok(()) => {}
          Err(fault) => {
            console_core_logging::debug!("couldn’t start sending clipboard contents: {fault}")
          }
        }
      }
      source::Event::Cancelled => {
        let Ok(()) = clipboard.cancel(source.id());
        source.destroy();
      }
      _newer_protocol_event => {}
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_cancelled_source_keeps_the_payload_until_its_pending_pipe_finishes() -> io::Result<()> {
    let now = Instant::now();
    let Ok(mut clipboard) = Clipboard::new(History::default(), now);
    let source = ObjectId::null();
    let payload = b"the active selection";
    clipboard.published.insert(
      source.clone(),
      Published {
        contents: Contents {
          format: Format::Text,
          bytes: payload.to_vec(),
        },
        status: SourceStatus::Active,
      },
    );

    let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)?;
    clipboard.start_send(source.clone(), "text/plain", writer)?;
    let Ok(()) = clipboard.cancel(source.clone());
    assert!(clipboard.published.contains_key(&source));
    assert_eq!(clipboard.outgoing.len(), 1);

    let Ok(()) = clipboard.deliver();
    let mut received = [0_u8; 64];
    assert_eq!(rustix::io::read(&reader, &mut received), Ok(payload.len()));
    assert_eq!(received.get(..payload.len()), Some(payload.as_slice()));
    assert!(!clipboard.published.contains_key(&source));
    assert!(clipboard.outgoing.is_empty());

    Ok(())
  }
}
