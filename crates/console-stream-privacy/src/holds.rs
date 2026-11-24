use std::io;
use std::time::{Duration, Instant};

use console_bus::connection::{BUS, BUS_PATH, Bus};
use console_bus::messages::{Kind, Message, Value, Whom};
use console_core_state_machine::Never;
use console_notifications::serving;

const SCREENSAVER: &str = "org.freedesktop.ScreenSaver";
const TIMEOUT: Duration = Duration::from_millis(500);

pub struct Hold {
  bus: Bus,
  owner: String,
}

#[derive(Default)]
pub struct Holds {
  notifications: Option<Hold>,
  screen: Option<Hold>,
}

pub enum Protection {
  Held,
  Limited,
}

impl Holds {
  pub fn notifications(&mut self) -> io::Result<()> {
    acquire(&mut self.notifications, serving::NOTIFICATIONS, |owner| {
      let Ok(request) = serving::hold_quiet(owner);
      (request, "")
    })
  }

  pub fn screen(&mut self) -> io::Result<()> {
    acquire(&mut self.screen, SCREENSAVER, |owner| {
      let Ok(request) = Message::call(&Whom {
        to: owner,
        at: "/org/freedesktop/ScreenSaver",
        on: SCREENSAVER,
        calling: "Inhibit",
      });
      let Ok(app) = Value::word("Console");
      let Ok(reason) = Value::word("OBS is streaming");
      let Ok(request_2) = request.carrying("ss", vec![app, reason]);
      (request_2, "u")
    })
  }

  pub fn protection(&self) -> Result<Protection, Never> {
    Ok(match (&self.notifications, &self.screen) {
      (Some(_notifications), Some(_screen)) => Protection::Held,
      (None, _) | (_, None) => Protection::Limited,
    })
  }
}

fn acquire(
  held: &mut Option<Hold>,
  service: &str,
  request: impl FnOnce(&str) -> (Message, &'static str),
) -> io::Result<()> {
  let (mut bus, previous) = match held.take() {
    Some(Hold { bus, owner }) => (bus, Some(owner)),
    None => {
      let bus = Bus::session().map_err(io::Error::other)?;

      (bus, None)
    }
  };
  let owner = owner(&mut bus, service)?;

  match { ::core::primitive::bool::then_some(previous.as_deref() != Some(&owner), ()) } {
    ::core::option::Option::Some(()) => {
      let (request_2, shape) = request(&owner);
      let reply = call(&mut bus, &request_2)?;

      match (reply.shape == shape).then_some(()) {
        Some(()) => {}
        None => return Err(io::Error::other("invalid reply to the hold request")),
      }
    }
    ::core::option::Option::None => {}
  }

  *held = Some(Hold { bus, owner });

  Ok(())
}

fn owner(bus: &mut Bus, service: &str) -> io::Result<String> {
  let Ok(request) = Message::call(&Whom {
    to: BUS,
    at: BUS_PATH,
    on: BUS,
    calling: "GetNameOwner",
  });
  let Ok(service_2) = Value::word(service);
  let Ok(request_2) = request.carrying("s", vec![service_2]);
  let reply = call(bus, &request_2)?;

  match reply.values.as_slice() {
    [Value::Word(owner)] => match owner.starts_with(':').then_some(()) {
      Some(()) => Ok(owner.clone()),
      None => Err(io::Error::other("invalid reply to GetNameOwner")),
    },
    _ => Err(io::Error::other("invalid reply to GetNameOwner")),
  }
}

fn call(bus: &mut Bus, request: &Message) -> io::Result<Message> {
  let deadline = Instant::now()
    .checked_add(TIMEOUT)
    .ok_or_else(|| io::Error::other("D-Bus deadline exceeds the clock range"))?;
  let reply = bus
    .call_before(request, deadline)
    .map_err(io::Error::other)?;

  match reply.kind {
    Kind::Answer => Ok(reply),
    Kind::Call | Kind::ErrorReply | Kind::Signal => Err(io::Error::other(match reply.fault {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => "unexpected D-Bus reply".to_owned(),
    })),
  }
}
