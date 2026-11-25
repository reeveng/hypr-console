use std::collections::BTreeMap;
use std::io;

use console_bus::connection::{Bus, NameRequestResult, Receiver, Sender};
use console_bus::messages::{
  ErrorName, Kind, Message, Signal, Truth, ValidationError, Value, Whom,
};
use console_core_never::Never;
use console_lock_screen::idle::policy::{Inhibition, Inhibitor, Request, Session};
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use super::{Failure, Result};

const LOGIN: &str = "org.freedesktop.login1";
const LOGIN_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";
const PROPERTIES: &str = console_bus::properties::INTERFACE;
const BLOCK_INHIBITED: &str = "BlockInhibited";

pub struct Login {
  pub incoming: Receiver,
  outgoing: Sender,
  path: String,
}

impl Login {
  pub fn connect() -> Result<(Self, Inhibition)> {
    let mut watch = Bus::system()?;
    watch.subscribe("type='signal',sender='org.freedesktop.login1'")?;
    let mut query = Bus::system()?;
    let Ok(session) = Message::call(&Whom {
      to: LOGIN,
      at: LOGIN_PATH,
      on: MANAGER,
      calling: "GetSession",
    });
    let Ok(resolve_session) = session.carrying("s", vec![Value::Word("auto".to_owned())]);
    let resolved = query.call(&resolve_session)?;
    let path = match resolved.values.as_slice() {
      [Value::Path(path)] => path.clone(),
      _ => {
        return Err(Failure::from(io::Error::other(format!(
          "logind could not resolve the session: {resolved:?}"
        ))));
      }
    };
    let Ok(property) = Message::call(&Whom {
      to: LOGIN,
      at: LOGIN_PATH,
      on: PROPERTIES,
      calling: "Get",
    });
    let Ok(read_inhibitors) = property.carrying(
      "ss",
      vec![
        Value::Word(MANAGER.to_owned()),
        Value::Word(BLOCK_INHIBITED.to_owned()),
      ],
    );
    let current = query.call(&read_inhibitors)?;
    let Ok(reported) = match current.values.as_slice() {
      [value] => inhibition(value),
      _ => Ok(None),
    };
    let idle_inhibition = reported.ok_or_else(|| {
      io::Error::other(format!(
        "logind could not read idle inhibitors: {current:?}"
      ))
    })?;
    let Ok((incoming, outgoing)) = watch.split();

    Ok((
      Self {
        incoming,
        outgoing,
        path,
      },
      idle_inhibition,
    ))
  }

  pub fn next(&mut self) -> Result<Option<Request>> {
    let received = ready(&mut self.incoming)?;
    let Ok(request) = match received.as_ref() {
      Some(message) => self.request(message),
      None => Ok(None),
    };

    Ok(request)
  }

  fn request(&self, message: &Message) -> core::result::Result<Option<Request>, Never> {
    Ok(
      match (
        message.kind,
        message.path.as_deref(),
        message.interface.as_deref(),
        message.member.as_deref(),
        message.values.as_slice(),
      ) {
        (
          Kind::Signal,
          Some(LOGIN_PATH),
          Some(MANAGER),
          Some("PrepareForSleep"),
          [Value::Truth(Truth::Yes)],
        ) => Some(Request::PreparingForSleep),
        (
          Kind::Signal,
          Some(LOGIN_PATH),
          Some(MANAGER),
          Some("PrepareForSleep"),
          [Value::Truth(Truth::No)],
        ) => Some(Request::ResumedFromSleep),
        (
          Kind::Signal,
          Some(LOGIN_PATH),
          Some(PROPERTIES),
          Some("PropertiesChanged"),
          [Value::Word(interface), changed, _],
        ) => match (interface == MANAGER).then_some(()) {
          Some(()) => {
            let Ok(value) = changed.keyed(BLOCK_INHIBITED);
            let Ok(now) = value.map(inhibition).transpose();
            now
              .flatten()
              .map(|state| Request::Inhibit(Inhibitor::Logind, state))
          }
          None => None,
        },
        (Kind::Signal, Some(path), Some(SESSION), Some("Lock"), []) => {
          (path == self.path).then_some(Request::LockRequested)
        }
        _ => None,
      },
    )
  }

  pub fn report(&self, session: Session) -> Result<()> {
    let Ok(call) = Message::call(&Whom {
      to: LOGIN,
      at: &self.path,
      on: SESSION,
      calling: "SetLockedHint",
    });
    let Ok(active) = truth(session);
    let Ok(hint) = call.carrying("b", vec![Value::Truth(active)]);
    let _serial = self.outgoing.say(&hint)?;

    Ok(())
  }
}

fn inhibition(value: &Value) -> core::result::Result<Option<Inhibition>, Never> {
  let Ok(word) = value.text();

  Ok(word.map(|inhibited| {
    match inhibited
      .split(':')
      .any(|entry| entry == "idle")
      .then_some(())
    {
      Some(()) => Inhibition::Held,
      None => Inhibition::Clear,
    }
  }))
}

fn truth(session: Session) -> core::result::Result<Truth, Never> {
  Ok(match session {
    Session::Locked => Truth::Yes,
    Session::Unlocked => Truth::No,
  })
}

fn ready(receiver: &mut Receiver) -> Result<Option<Message>> {
  let mut watched = [PollFd::new(receiver, PollFlags::IN)];

  match poll(&mut watched, Some(&Timespec::default())) {
    Ok(0) | Err(rustix::io::Errno::INTR) => Ok(None),
    Ok(_ready) => receiver.receive().map(Some).map_err(Failure::from),
    Err(fault) => Err(Failure::from(fault)),
  }
}

const SCREEN_SAVER: &str = "org.freedesktop.ScreenSaver";
const SCREEN_PATH: &str = "/org/freedesktop/ScreenSaver";
const DBUS: &str = "org.freedesktop.DBus";

pub struct ScreenSaver {
  pub incoming: Receiver,
  outgoing: Sender,
  cookies: BTreeMap<u32, String>,
  last_cookie: u32,
}

impl ScreenSaver {
  pub fn connect() -> Result<Option<Self>> {
    let mut bus = Bus::session()?;
    bus.subscribe("type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg2=''")?;

    let named = bus.request_name(SCREEN_SAVER)?;

    match named {
      NameRequestResult::PrimaryOwner | NameRequestResult::AlreadyOwner => {
        let Ok((incoming, outgoing)) = bus.split();

        Ok(Some(Self {
          incoming,
          outgoing,
          cookies: BTreeMap::new(),
          last_cookie: 0,
        }))
      }
      NameRequestResult::InQueue | NameRequestResult::Exists => {
        console_core_logging::warn!("The session already has a ScreenSaver service");

        Ok(None)
      }
    }
  }

  pub fn next(&mut self, session: Session) -> Result<Option<Request>> {
    let before = self.cookies.is_empty();
    let received = ready(&mut self.incoming)?;

    match received {
      Some(message) => self.handle(message, session)?,
      None => return Ok(None),
    }

    Ok(match (before, self.cookies.is_empty()) {
      (true, false) => Some(Request::Inhibit(Inhibitor::ScreenSaver, Inhibition::Held)),
      (false, true) => Some(Request::Inhibit(Inhibitor::ScreenSaver, Inhibition::Clear)),
      (false, false) | (true, true) => None,
    })
  }

  fn handle(&mut self, message: Message, session: Session) -> Result<()> {
    let answered = match (
      message.kind,
      message.path.as_deref(),
      message.interface.as_deref(),
      message.member.as_deref(),
      message.values.as_slice(),
      message.sender.as_deref(),
    ) {
      (
        Kind::Signal,
        Some("/org/freedesktop/DBus"),
        Some(DBUS),
        Some("NameOwnerChanged"),
        [_, Value::Word(owner), Value::Word(replacement)],
        Some(DBUS),
      ) => {
        match replacement.is_empty().then_some(()) {
          Some(()) => self.cookies.retain(|_, client| client != owner),
          None => {}
        }

        return Ok(());
      }
      (
        Kind::Call,
        Some(SCREEN_PATH | "/ScreenSaver"),
        Some(SCREEN_SAVER),
        Some("Inhibit"),
        [Value::Word(_app), Value::Word(_reason)],
        Some(client),
      ) => match self.last_cookie.checked_add(1) {
        Some(cookie) => {
          self.last_cookie = cookie;
          let _vacant = self.cookies.insert(cookie, client.to_owned());
          let Ok(answer) = message.answering();
          answer.carrying("u", vec![Value::Unsigned32(cookie)])
        }
        None => message.failing(
          ErrorName("org.freedesktop.DBus.Error.LimitsExceeded"),
          "No more inhibitor cookies are available",
        ),
      },
      (
        Kind::Call,
        Some(SCREEN_PATH | "/ScreenSaver"),
        Some(SCREEN_SAVER),
        Some("UnInhibit"),
        [Value::Unsigned32(cookie)],
        Some(client),
      ) => {
        match (self.cookies.get(cookie).map(String::as_str) == Some(client)).then_some(()) {
          Some(()) => {
            let _removed = self.cookies.remove(cookie);
          }
          None => {}
        }

        message.answering()
      }
      (
        Kind::Call,
        Some(SCREEN_PATH | "/ScreenSaver"),
        Some(SCREEN_SAVER),
        Some("GetActive"),
        [],
        _,
      ) => {
        let Ok(answer) = message.answering();
        let Ok(active) = truth(session);
        answer.carrying("b", vec![Value::Truth(active)])
      }
      (
        Kind::Call,
        Some(SCREEN_PATH | "/ScreenSaver"),
        Some("org.freedesktop.DBus.Introspectable"),
        Some("Introspect"),
        [],
        _,
      ) => {
        let Ok(answer) = message.answering();
        let Ok(xml) = Value::word(INTROSPECTION);
        answer.carrying("s", vec![xml])
      }
      (
        Kind::Call,
        Some(SCREEN_PATH | "/ScreenSaver"),
        Some(SCREEN_SAVER),
        Some("Inhibit" | "UnInhibit" | "GetActive"),
        _,
        _,
      ) => message.complaining(
        ValidationError::InvalidArgs,
        "Invalid ScreenSaver arguments",
      ),
      (Kind::Call, _, _, _, _, _) => {
        message.complaining(ValidationError::UnknownMethod, "Unknown ScreenSaver method")
      }
      (Kind::Signal | Kind::Answer | Kind::ErrorReply, _, _, _, _, _) => return Ok(()),
    };
    let Ok(reply) = answered;
    let _serial = self.outgoing.say(&reply)?;

    Ok(())
  }

  pub fn report(&self, session: Session) -> Result<()> {
    let Ok(signal) = Message::signal(&Signal {
      at: SCREEN_PATH,
      on: SCREEN_SAVER,
      name: "ActiveChanged",
    });
    let Ok(active) = truth(session);
    let Ok(changed) = signal.carrying("b", vec![Value::Truth(active)]);
    let _serial = self.outgoing.say(&changed)?;

    Ok(())
  }
}

const INTROSPECTION: &str = r#"<node>
  <interface name="org.freedesktop.ScreenSaver">
    <method name="Inhibit">
      <arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="u" direction="out"/>
    </method>
    <method name="UnInhibit"><arg type="u" direction="in"/></method>
    <method name="GetActive"><arg type="b" direction="out"/></method>
    <signal name="ActiveChanged"><arg type="b"/></signal>
  </interface>
</node>"#;
