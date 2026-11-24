//! One DevTools pipe drives the private tabs in a Console Browser window.
//! Page events identify the tab when a download starts. Browser events keep
//! reporting download progress even after the originating popup closes.
//! Shut down the control socket before closing tabs, so a reload cannot reuse
//! a failed host while it is still unwinding.

use std::io;
use std::os::fd::{BorrowedFd, OwnedFd};
use std::path::PathBuf;

use console_browser_renderer::{self as frame, invalid};
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use console_core_state_machine::SingleEffectMachine;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{OFlags, fcntl_setfl};
use rustix::net::{Shutdown, shutdown};
use serde_json::{Value, json};

use super::context::{Effect, Lifecycle, Request, State};
use super::{
  Chromium, DevtoolsEvent, Diagnostics, INBOUND_IDLE, Message, PageIdentifier, PageRoute, Pipe,
  deadline,
};

const CLOSED: PollFlags = PollFlags::HUP.union(PollFlags::RDHUP).union(PollFlags::ERR);
const INPUT: PollFlags = PollFlags::IN.union(PollFlags::RDHUP);

struct Host {
  pipe: Pipe,
  tabs: Vec<HostedTab>,
}

struct HostedTab {
  page: Chromium,
  context: State,
}

impl Host {
  fn start(profile: PathBuf, diagnostics: Diagnostics) -> io::Result<Self> {
    let mut pipe = Pipe::start(profile, diagnostics)?;
    pipe.call(
      None,
      "Target.setDiscoverTargets",
      json!({ "discover": true }),
    )?;
    Ok(Self {
      pipe,
      tabs: Vec::new(),
    })
  }

  fn add(&mut self, channel: OwnedFd, staging: Option<PathBuf>) -> io::Result<()> {
    let Ok(initial) = Lifecycle::initialize(&(), None);
    let starting = OpeningTab {
      context: initial.state,
      request: Request::Open,
      arguments: Some((channel, staging)),
      page: None,
    };
    let outcome = iterate((&mut self.pipe, starting), |(pipe, opening)| {
      Ok(match opening.advance(pipe) {
        Ok(Step::Again(next)) => Step::Again((pipe, next)),
        Ok(Step::Halt(tab)) => Step::Halt(Ok(tab)),
        Err(fault) => Step::Halt(Err(fault)),
      })
    })
    .map_err(io::Error::other)?;
    let opened = outcome?;

    match opened {
      Some(tab) => self.tabs.push(tab),
      None => {}
    }

    Ok(())
  }

  fn remove(&mut self, position: u64) -> io::Result<()> {
    let Ok(slot_index) = index(position);

    match bool::then_some(slot_index < self.tabs.len(), ()) {
      Some(()) => {}
      None => return Err(invalid("Chromium has no tab at that position")),
    }

    let last = self
      .tabs
      .pop()
      .ok_or_else(|| invalid("Chromium host has no tabs to close"))?;
    let tab = match self.tabs.get_mut(slot_index) {
      Some(slot) => std::mem::replace(slot, last),
      None => last,
    };
    let Ok(turn) = Lifecycle::handle(tab.context, Request::Close);

    let id = match turn.effect {
      Some(Effect::Dispose(id)) => id,
      Some(Effect::Create | Effect::Attach(_)) | None => {
        return Err(invalid("Chromium tab has no context to dispose"));
      }
    };

    dispose(&mut self.pipe, &id)?;
    let Ok(_) = Lifecycle::handle(turn.state, Request::Disposed);
    Ok(())
  }

  fn tab_failed(&mut self, position: u64, fault: io::Error) -> io::Result<()> {
    match {
      ::core::primitive::bool::then_some(
        !matches!(
          fault.kind(),
          io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
        ),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        console_core_logging::debug!("Chromium tab stopped: {fault}");
      }
      ::core::option::Option::None => {}
    }

    self.remove(position)
  }

  fn receive_tab(&mut self, control: BorrowedFd<'_>) -> io::Result<()> {
    let packet = frame::receive(control)?;
    let channel = packet
      .frame
      .ok_or_else(|| invalid("Chromium tab has no channel"))?;
    let location = packet
      .line
      .strip_prefix("tab ")
      .ok_or_else(|| invalid("unrecognized Chromium host request"))?;
    let staging = (!location.is_empty()).then(|| PathBuf::from(location));
    fcntl_setfl(&channel, OFlags::NONBLOCK)?;
    self.add(channel, staging)
  }

  fn owner(&self, message: &Message) -> Result<Option<u64>, Never> {
    Ok(
      self
        .tabs
        .iter()
        .position(|tab| match message {
          Message::Reply(reply) => reply.session.as_deref() == Some(tab.page.session.as_str()),
          Message::PageEvent(session, DevtoolsEvent::DownloadStarted(..)) => {
            let Ok(route) = tab.page.route(PageIdentifier::Session(session));
            route == PageRoute::Known
          }
          Message::PageEvent(_, DevtoolsEvent::DownloadEnded(..)) => false,
          Message::PageEvent(session, _) => session == &tab.page.session,
          Message::Event(DevtoolsEvent::TargetCreated(info)) => {
            let Ok(context) = tab.context.id();
            info.context.as_deref() == context
              && info.opener.as_deref() == Some(tab.page.target.as_str())
          }
          Message::Event(DevtoolsEvent::TargetChanged(info)) => info.id == tab.page.target,
          Message::Event(DevtoolsEvent::TargetClosed(id)) => {
            let Ok(route) = tab.page.route(PageIdentifier::Target(id));
            route == PageRoute::Known
          }
          Message::Event(DevtoolsEvent::Crashed(id)) => id == &tab.page.target,
          Message::Event(DevtoolsEvent::DownloadEnded(id, _)) => {
            tab.page.downloads.contains_key(id)
          }
          Message::Event(_) | Message::Ignored => false,
        })
        .map(|position| {
          let Ok(fixed) = fixed_index(position);
          fixed
        }),
    )
  }

  fn deliver(&mut self, message: Message) -> io::Result<()> {
    let Ok(owner) = self.owner(&message);
    let position = match owner {
      Some(position) => position,
      None => return Ok(()),
    };
    let Ok(slot_index) = index(position);
    let slot = self
      .tabs
      .get_mut(slot_index)
      .ok_or_else(|| invalid("Chromium reply has no live tab"))?;
    let tab = &mut slot.page;
    let result = match message {
      Message::Reply(reply) => tab.reply(&mut self.pipe, reply),
      Message::Event(event) | Message::PageEvent(_, event) => {
        tab.devtools_event(&mut self.pipe, event)
      }
      Message::Ignored => Ok(()),
    };

    match result {
      Ok(()) => Ok(()),
      Err(fault) => self.tab_failed(position, fault),
    }
  }

  fn run(&mut self, control: BorrowedFd<'_>) -> io::Result<()> {
    let result = iterate(self, |host| {
      Ok(match host.turn(control) {
        Ok(HostTurn::Pending) => Step::Again(host),
        Ok(HostTurn::Closed) => Step::Halt(Ok(())),
        Err(fault) => Step::Halt(Err(fault)),
      })
    })
    .map_err(io::Error::other)?;
    result
  }

  fn turn(&mut self, control: BorrowedFd<'_>) -> io::Result<HostTurn> {
    let polled = self.poll_ready(control)?;
    let Readiness {
      replies,
      parent,
      tabs,
    } = match polled {
      Some(ready) => ready,
      None => return Ok(HostTurn::Pending),
    };

    match parent.intersects(CLOSED).then_some(()) {
      Some(()) => return Ok(HostTurn::Closed),
      None => {}
    }

    for (position, flags) in tabs.into_iter().enumerate().rev() {
      let Ok(fixed) = fixed_index(position);
      self.handle_tab(fixed, flags)?;
    }

    match parent.contains(PollFlags::IN).then_some(()) {
      Some(()) => self.receive_tab(control)?,
      None => {}
    }

    let next = self.receive_reply(replies)?;

    match next {
      Some(message) => self.deliver(message)?,
      None => {}
    }

    Ok(HostTurn::Pending)
  }

  fn poll_ready(&self, control: BorrowedFd<'_>) -> io::Result<Option<Readiness>> {
    let mut watched = Vec::with_capacity(self.tabs.len().saturating_add(2));
    watched.push(PollFd::new(self.pipe.incoming.get_ref(), PollFlags::IN));
    watched.push(PollFd::new(&control, INPUT));
    watched.extend(
      self
        .tabs
        .iter()
        .map(|tab| PollFd::new(&tab.page.channel, INPUT)),
    );
    let timeout = match self.pipe.pending.is_empty().then_some(()) {
      Some(()) => match self.pipe.incoming.buffer().contains(&0).then_some(()) {
        Some(()) => Timespec::default(),
        None => INBOUND_IDLE,
      },
      None => Timespec::default(),
    };

    match poll(&mut watched, Some(&timeout)) {
      Ok(_ready) => {}
      Err(rustix::io::Errno::INTR) => return Ok(None),
      Err(fault) => return Err(io::Error::from(fault)),
    }

    let (reply, parent, tabs) = match watched.as_slice() {
      [reply, parent, tabs @ ..] => (reply, parent, tabs),
      [] | [_] => return Err(invalid("Chromium host event sources disappeared")),
    };
    Ok(Some(Readiness {
      replies: reply.revents(),
      parent: parent.revents(),
      tabs: tabs.iter().map(PollFd::revents).collect(),
    }))
  }

  fn handle_tab(&mut self, position: u64, ready: PollFlags) -> io::Result<()> {
    match (ready.intersects(CLOSED), ready.contains(PollFlags::IN)) {
      (true, _) => return self.remove(position),
      (false, false) => return Ok(()),
      (false, true) => {}
    }

    let Ok(slot_index) = index(position);
    let tab = self
      .tabs
      .get_mut(slot_index)
      .ok_or_else(|| invalid("Chromium request has no live tab"))?;

    match tab.page.parent_request(&mut self.pipe) {
      Ok(()) => Ok(()),
      Err(fault) => self.tab_failed(position, fault),
    }
  }

  fn receive_reply(&mut self, ready: PollFlags) -> io::Result<Option<Message>> {
    match self.pipe.pending.pop_front() {
      Some(message) => return Ok(Some(message)),
      None => {}
    }

    match (
      self.pipe.incoming.buffer().contains(&0),
      ready.intersects(PollFlags::IN | CLOSED),
    ) {
      (false, false) => Ok(None),
      (true, _) | (false, true) => {
        let until = deadline()?;
        self.pipe.read(until).map(Some)
      }
    }
  }
}

enum HostTurn {
  Pending,
  Closed,
}

struct Readiness {
  replies: PollFlags,
  parent: PollFlags,
  tabs: Vec<PollFlags>,
}

struct OpeningTab {
  context: State,
  request: Request,
  arguments: Option<(OwnedFd, Option<PathBuf>)>,
  page: Option<Chromium>,
}

impl OpeningTab {
  fn advance(self, pipe: &mut Pipe) -> io::Result<Step<Self, Option<HostedTab>>> {
    let Self {
      context,
      request,
      mut arguments,
      mut page,
    } = self;
    let Ok(turn) = Lifecycle::handle(context, request);
    let next = match turn.effect {
      Some(Effect::Create) => {
        let result = pipe.call(
          None,
          "Target.createBrowserContext",
          json!({ "disposeOnDetach": true }),
        )?;
        let id = result
          .get("browserContextId")
          .and_then(Value::as_str)
          .ok_or_else(|| invalid("Chromium did not identify its private browser context"))?;
        Request::Created(id.to_owned())
      }
      Some(Effect::Attach(id)) => {
        let (channel, staging) = arguments
          .take()
          .ok_or_else(|| invalid("Chromium tab was already attached"))?;

        match Chromium::start(pipe, channel, &id, staging) {
          Ok(opened) => {
            page = Some(opened);
            Request::Attached(true)
          }
          Err(fault) => {
            console_core_logging::warn!("could not open a private Chromium tab: {fault}");
            Request::Attached(false)
          }
        }
      }
      Some(Effect::Dispose(id)) => {
        dispose(pipe, &id)?;
        Request::Disposed
      }
      None => {
        return Ok(Step::Halt(page.map(|opened| HostedTab {
          page: opened,
          context: turn.state,
        })));
      }
    };
    Ok(Step::Again(Self {
      context: turn.state,
      request: next,
      arguments,
      page,
    }))
  }
}

fn dispose(pipe: &mut Pipe, context: &str) -> io::Result<()> {
  pipe.call(
    None,
    "Target.disposeBrowserContext",
    json!({ "browserContextId": context }),
  )?;
  Ok(())
}

pub(super) fn shared(
  profile: PathBuf,
  diagnostics: Diagnostics,
  control: BorrowedFd<'_>,
) -> io::Result<()> {
  let mut host = Host::start(profile, diagnostics)?;
  let result = host.run(control);
  let disconnected = shutdown(control, Shutdown::Both);

  match disconnected {
    Ok(()) => {}
    Err(fault) => console_core_logging::debug!("closing Chromium host control: {fault}"),
  }

  let () = result?;
  host.pipe.close()
}
