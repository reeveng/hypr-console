//! Being the thing that answers `org.freedesktop.Notifications`.
//!
//! mako held this name for a year and drew the card, and it was the last
//! surface on the machine drawn in someone else's colors: a second
//! stylesheet, written out of the palette by `just theme` and read only when a
//! daemon started, for a card nothing here could decide anything about. What
//! the rest of this crate already was is the other three quarters of a
//! notification daemon -- `saying` is what raises one, `reading` and `rows` are
//! what keeps and draws it afterwards -- so what was missing was the middle:
//! hearing the call, giving it a number, holding it until it goes, and saying
//! when it went.
//!
//! That is what is here, and none of it touches a socket or a screen. A
//! message goes in and what to answer, what to say afterwards and whether the
//! screen changed come out, so every rule below -- which notification replaces
//! which, how long a fault stays, what a number means when it is handed back
//! -- can be asked in a test with no bus anywhere. `console-notify` is the
//! program that carries the answers out; `console-bus` is the wire under it.
//!
//! **A number is never handed out twice and never handed out as zero.** Zero
//! is what the specification uses to mean *this is not a replacement*, so a
//! server that ever answered zero would be handing a caller an id that means
//! `no id` -- and the caller's next notification would silently land beside
//! the one it meant to replace rather than over it. The count starts at one
//! and only goes up.
//!
//! **What replaces what is the caller's decision and is checked all the same.**
//! A rocker held down raises a card many times a second and every one of them
//! names the id it replaces, which is what makes it one card rather than
//! twenty. An id that names nothing waiting is not a fault and is not silently
//! dropped either: it is raised as a new one, because a card that arrived while
//! the last one was going out is still a card someone asked for.
//!
//! **Five seconds, or until it is seen.** A caller that says nothing about how
//! long is answered with five seconds, which is long enough to read a sentence
//! at arm's length; a caller that asks for none at all gets none. Everything
//! `console-say` raises is critical and critical stays, because the whole point
//! of it is that a thing which broke while no one was looking is still there
//! when someone looks.
//!
//! **Quiet is not deaf.** The mode holds the card back and changes nothing
//! else: what was sent is still held, the bell still counts it and still turns
//! coral for a fault. A handheld is held in front of a game as often as it is
//! worked on, and the thing worth stopping is the interruption rather than the
//! news.
//!
//! **Where the file goes can be value, for the reason the strip's can.** A check
//! brings a desktop up in a session of its own, and a daemon inside it writing
//! into this laptop's own runtime directory would take away the reading the
//! machine it is running on already had. `CONSOLE_NOTIFICATIONS_PATH` is how a stage
//! points it somewhere else; the device is never told, and the default is the
//! one place the panel and the bell look.

use console_bus::cli;
use console_bus::connection::{BUS, BUS_PATH};
use console_bus::messages::{Kind, Message, Signal, ValidationError, Value, Whom};
use console_core_never::Never;
use console_program_contract::Command;

use std::{collections::BTreeSet, path::PathBuf};

use crate::reading::{DoNotDisturb, Inbox, Notification, Urgency};
use crate::saying::Expiry;

pub use console_events::sources::{APPLICATION, NOTIFICATIONS};

pub const AT: &str = "/org/freedesktop/Notifications";

pub const KEPT: &str = "notifications.json";

pub const WHERE: &str = "CONSOLE_NOTIFICATIONS_PATH";

const PEER: &str = "org.freedesktop.DBus.Peer";

const LOOKING: &str = "org.freedesktop.DBus.Introspectable";

const CLOSE_NOTIFICATION: &str = "CloseNotification";
const CLEAR_ALL: &str = "ClearAll";
const QUIETEN: &str = "Quieten";
const HOLD_QUIET: &str = "HoldQuiet";
const NAME_OWNER_CHANGED: &str = "NameOwnerChanged";

pub const QUIET_OWNER_LOST: &str = "type='signal',sender='org.freedesktop.DBus',\
interface='org.freedesktop.DBus',member='NameOwnerChanged',path='/org/freedesktop/DBus',arg2=''";

pub fn hold_quiet(destination: &str) -> Result<Message, Never> {
  Message::call(&Whom {
    to: destination,
    at: AT,
    on: APPLICATION,
    calling: HOLD_QUIET,
  })
}

const A_WHILE: u32 = 5000;

pub const EARLIER: u32 = 20;

const URGENCY: &str = "urgency";

const VALUE: &str = "value";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Why {
  RanOut,
  #[default]
  Dismissed,
  Request,
}

impl Why {
  fn code(self) -> Result<u32, Never> {
    Ok(match self {
      Why::RanOut => 1,
      Why::Dismissed => 2,
      Why::Request => 3,
    })
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Modified {
  Yes,
  #[default]
  No,
}

impl From<bool> for Modified {
  fn from(changed: bool) -> Self {
    match bool::then_some(changed, ()) {
      Some(()) => Self::Yes,
      None => Self::No,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Closed {
  Yes,
  #[default]
  No,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
  pub application: String,
  pub replacing: u32,
  pub summary: String,
  pub body: String,
  pub urgency: Urgency,
  pub expiry: Expiry,
  pub value: Option<i64>,
  pub actions: Vec<Action>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
  pub key: String,
  pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveNotification {
  pub notification: Notification,
  pub expiry: Expiry,
  pub value: Option<i64>,
  pub raised: u64,
  pub actions: Vec<Action>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
  pub id: u32,
  pub raised: u64,
  pub choice: Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
  Dismiss,
  Invoke(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Armed {
  pub id: u32,
  pub raised: u64,
  pub expiry: Expiry,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Holding {
  pub waiting: Vec<ActiveNotification>,
  pub earlier: Vec<Notification>,
  pub do_not_disturb: DoNotDisturb,
  quiet_holders: BTreeSet<String>,
  counted: u32,
  raises: u64,
}

impl Holding {
  pub fn raised(&mut self, asked: &Request) -> Result<Armed, Never> {
    let id = match ::core::primitive::bool::then_some(
      self
        .waiting
        .iter()
        .any(|held| held.notification.id == asked.replacing),
      (),
    ) {
      ::core::option::Option::Some(()) => asked.replacing,
      ::core::option::Option::None => {
        let counted = self.counted.saturating_add(1);

        self.counted = counted;

        counted
      }
    };

    let raises = self.raises.saturating_add(1);

    self.raises = raises;

    let held = ActiveNotification {
      notification: Notification {
        id,
        application: asked.application.clone(),
        summary: asked.summary.clone(),
        body: asked.body.clone(),
        urgency: asked.urgency,
      },
      expiry: asked.expiry,
      value: asked.value,
      raised: raises,
      actions: asked.actions.clone(),
    };

    match self
      .waiting
      .iter_mut()
      .find(|held_2| held_2.notification.id == asked.replacing)
    {
      Some(room) => *room = held,
      None => self.waiting.push(held),
    }

    Ok(Armed {
      id,
      raised: raises,
      expiry: asked.expiry,
    })
  }

  pub fn ran_out(&mut self, armed: &Armed) -> Result<Closed, Never> {
    let standing = self
      .waiting
      .iter()
      .any(|held| held.notification.id == armed.id && held.raised == armed.raised);

    match ::core::primitive::bool::then_some(standing, ()) {
      ::core::option::Option::Some(()) => self.closed(armed.id),
      ::core::option::Option::None => Ok(Closed::No),
    }
  }

  pub fn activated(&mut self, target: Target) -> Result<Turn, Never> {
    let matching = self
      .waiting
      .iter()
      .find(|held| held.notification.id == target.id && held.raised == target.raised);
    let held = match matching {
      Some(held) => held,
      None => return Ok(Turn::default()),
    };
    let mut signals = match target.choice {
      Choice::Dismiss => Vec::new(),
      Choice::Invoke(ordinal) => {
        let Ok(index) = console_core_number_conversion::index(ordinal);

        match held.actions.get(index) {
          Some(action) => {
            let Ok(signal) = action_signal(target.id, &action.key);
            vec![signal]
          }
          None => return Ok(Turn::default()),
        }
      }
    };
    let Ok(_closed) = self.closed(target.id);
    let Ok(closed) = closed_signals(&[target.id], Why::Dismissed);
    signals.extend(closed);

    Ok(Turn {
      signals,
      changed: Modified::Yes,
      ..Turn::default()
    })
  }

  pub fn closed(&mut self, id: u32) -> Result<Closed, Never> {
    let held = match self
      .waiting
      .extract_if(.., |held| held.notification.id == id)
      .next()
    {
      Some(held) => held,
      None => return Ok(Closed::No),
    };

    self.earlier.insert(0, held.notification);

    let Ok(earlier) = console_core_number_conversion::index(EARLIER);

    self.earlier.truncate(earlier);

    Ok(Closed::Yes)
  }

  pub fn cleared(&mut self) -> Result<Vec<u32>, Never> {
    let every: Vec<u32> = self
      .waiting
      .iter()
      .map(|held| held.notification.id)
      .collect();

    for id in &every {
      let Ok(_gone) = self.closed(*id);
    }

    Ok(every)
  }

  pub fn toggled_do_not_disturb(&mut self) -> Result<DoNotDisturb, Never> {
    self.do_not_disturb = match self.do_not_disturb {
      DoNotDisturb::Off => DoNotDisturb::On,
      DoNotDisturb::On => DoNotDisturb::Off,
    };

    Ok(self.do_not_disturb)
  }

  pub fn visible(&self) -> Result<Vec<&ActiveNotification>, Never> {
    let Ok(held) = self.held_quiet();
    let Ok(quiet) = self.do_not_disturb.with_hold(held);

    Ok(match quiet {
      DoNotDisturb::On => Vec::new(),
      DoNotDisturb::Off => self.waiting.iter().rev().collect(),
    })
  }

  fn held_quiet(&self) -> Result<DoNotDisturb, Never> {
    Ok(match bool::then_some(self.quiet_holders.is_empty(), ()) {
      Some(()) => DoNotDisturb::Off,
      None => DoNotDisturb::On,
    })
  }
}

#[derive(Debug, Default)]
pub struct Turn {
  pub say: Option<Message>,
  pub signals: Vec<Message>,
  pub changed: Modified,
  pub arm: Option<Armed>,
}

pub fn handle_message(holding: &mut Holding, message: &Message) -> Result<Turn, Never> {
  match message.kind {
    Kind::Signal => return quiet_owner_lost(holding, message),
    Kind::Call => {}
    Kind::Answer | Kind::ErrorReply => return Ok(Turn::default()),
  }

  let asking = match (message.interface.as_deref(), message.member.as_deref()) {
    (Some(NOTIFICATIONS), Some(member)) => member,
    (Some(APPLICATION), Some(HOLD_QUIET)) => return hold_for_caller(holding, message),
    (Some(APPLICATION), Some(member)) => member,
    (Some(PEER), Some("Ping")) => {
      let Ok(answer) = message.answering();

      return Ok(Turn {
        say: Some(answer),
        ..Turn::default()
      });
    }
    (Some(LOOKING), Some("Introspect")) | (None, Some("Introspect")) => {
      let Ok(answer) = message.answering();
      let Ok(looked) = looked_at();
      let Ok(value) = Value::word(&looked);
      let Ok(answer_2) = answer.carrying("s", vec![value]);

      return Ok(Turn {
        say: Some(answer_2),
        ..Turn::default()
      });
    }
    (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) | (None, None) => {
      let Ok(complaint) = message.complaining(
        ValidationError::UnknownMethod,
        "this desktop answers notifications only",
      );

      return Ok(Turn {
        say: Some(complaint),
        ..Turn::default()
      });
    }
  };

  match asking {
    "Notify" => notifying(holding, message),
    CLOSE_NOTIFICATION => closing(holding, message),
    "GetCapabilities" => {
      let Ok(answer) = message.answering();
      let Ok(body) = Value::word("body");
      let Ok(persistence) = Value::word("persistence");
      let Ok(actions) = Value::word("actions");
      let Ok(answer_3) =
        answer.carrying("as", vec![Value::List(vec![body, persistence, actions])]);

      Ok(Turn {
        say: Some(answer_3),
        ..Turn::default()
      })
    }
    "GetServerInformation" => {
      let Ok(answer) = message.answering();
      let mut values: Vec<Value> = Vec::new();

      for word in ["console", "console", env!("CARGO_PKG_VERSION"), "1.2"] {
        let Ok(word_2) = Value::word(word);

        values.push(word_2);
      }

      let Ok(answer_4) = answer.carrying("ssss", values);

      Ok(Turn {
        say: Some(answer_4),
        ..Turn::default()
      })
    }
    CLEAR_ALL => {
      let Ok(every) = holding.cleared();
      let Ok(answer) = message.answering();
      let Ok(closed) = closed_signals(&every, Why::Dismissed);

      Ok(Turn {
        say: Some(answer),
        signals: closed,
        changed: Modified::Yes,
        arm: None,
      })
    }
    QUIETEN => {
      let Ok(quiet) = holding.toggled_do_not_disturb();
      let Ok(word) = quiet.said();
      let Ok(value) = Value::word(word);
      let Ok(answer) = message.answering();
      let Ok(answer_5) = answer.carrying("s", vec![value]);

      Ok(Turn {
        say: Some(answer_5),
        changed: Modified::Yes,
        ..Turn::default()
      })
    }
    _other => {
      let Ok(complaint) = message.complaining(
        ValidationError::UnknownMethod,
        "nothing here answers to that",
      );

      Ok(Turn {
        say: Some(complaint),
        ..Turn::default()
      })
    }
  }
}

fn hold_for_caller(holding: &mut Holding, message: &Message) -> Result<Turn, Never> {
  let owner = message
    .sender
    .as_deref()
    .filter(|name| name.starts_with(':'));
  let owner_name = match (owner, message.values.as_slice()) {
    (Some(name), []) => name,
    _invalid => {
      let Ok(complaint) = message.complaining(
        ValidationError::InvalidArgs,
        "a quiet hold takes no arguments and belongs to the calling connection",
      );
      return Ok(Turn {
        say: Some(complaint),
        ..Turn::default()
      });
    }
  };
  let changed = Modified::from(holding.quiet_holders.insert(owner_name.to_owned()));
  let Ok(answer) = message.answering();
  Ok(Turn {
    say: Some(answer),
    changed,
    ..Turn::default()
  })
}

fn quiet_owner_lost(holding: &mut Holding, message: &Message) -> Result<Turn, Never> {
  let changed = match (
    message.sender.as_deref(),
    message.path.as_deref(),
    message.interface.as_deref(),
    message.member.as_deref(),
    message.values.as_slice(),
  ) {
    (
      Some(BUS),
      Some(BUS_PATH),
      Some(BUS),
      Some(NAME_OWNER_CHANGED),
      [Value::Word(name), Value::Word(old), Value::Word(new)],
    ) => match bool::then_some(name == old && new.is_empty(), ()) {
      Some(()) => Modified::from(holding.quiet_holders.remove(name)),
      None => Modified::No,
    },
    _unrelated => Modified::No,
  };

  Ok(Turn {
    changed,
    ..Turn::default()
  })
}

fn notifying(holding: &mut Holding, message: &Message) -> Result<Turn, Never> {
  let Ok(reading) = parse_request(&message.values);

  let asked = match reading {
    Some(asked) => asked,
    None => {
      let Ok(complaint) = message.complaining(
        ValidationError::InvalidArgs,
        "a notification is susssasa{sv}i and this was not",
      );

      return Ok(Turn {
        say: Some(complaint),
        ..Turn::default()
      });
    }
  };

  let Ok(armed) = holding.raised(&asked);
  let Ok(answer) = message.answering();
  let Ok(answer_2) = answer.carrying("u", vec![Value::Unsigned32(armed.id)]);

  Ok(Turn {
    say: Some(answer_2),
    signals: Vec::new(),
    changed: Modified::Yes,
    arm: Some(armed),
  })
}

fn closing(holding: &mut Holding, message: &Message) -> Result<Turn, Never> {
  let counted = match message.values.first() {
    Some(value) => {
      let Ok(counted) = value.counted();

      counted
    }
    None => None,
  };

  let id = match counted {
    Some(id) => match u32::try_from(id) {
      Ok(id_2) => id_2,
      Err(_too_big) => 0,
    },
    None => 0,
  };

  let Ok(closed) = holding.closed(id);
  let Ok(answer) = message.answering();

  Ok(match closed {
    Closed::No => Turn {
      say: Some(answer),
      ..Turn::default()
    },
    Closed::Yes => {
      let Ok(messages) = closed_signals(&[id], Why::Request);

      Turn {
        say: Some(answer),
        signals: messages,
        changed: Modified::Yes,
        arm: None,
      }
    }
  })
}

pub fn closed_signals(every: &[u32], why: Why) -> Result<Vec<Message>, Never> {
  let mut value: Vec<Message> = Vec::new();

  for id in every {
    let Ok(signal) = Message::signal(&Signal {
      at: AT,
      on: NOTIFICATIONS,
      name: "NotificationClosed",
    });
    let Ok(code) = why.code();
    let Ok(signal_2) =
      signal.carrying("uu", vec![Value::Unsigned32(*id), Value::Unsigned32(code)]);

    value.push(signal_2);
  }

  Ok(value)
}

fn action_signal(id: u32, key: &str) -> Result<Message, Never> {
  let Ok(signal) = Message::signal(&Signal {
    at: AT,
    on: NOTIFICATIONS,
    name: "ActionInvoked",
  });
  let Ok(word) = Value::word(key);
  signal.carrying("us", vec![Value::Unsigned32(id), word])
}

fn actions(value: Option<&Value>) -> Result<Vec<Action>, Never> {
  let listed = match value {
    Some(present) => {
      let Ok(items) = present.listed();
      items
    }
    None => None,
  };

  Ok(
    listed
      .into_iter()
      .flat_map(|values| values.chunks_exact(2))
      .filter_map(|pair| match pair {
        [Value::Word(key), Value::Word(label)] => (!key.is_empty()).then(|| Action {
          key: key.clone(),
          label: label.clone(),
        }),
        _ => None,
      })
      .take(3)
      .collect(),
  )
}

pub fn parse_request(values: &[Value]) -> Result<Option<Request>, Never> {
  let Ok(application) = text_of(values.first());
  let Ok(summary) = text_of(values.get(3));
  let Ok(body) = text_of(values.get(4));

  let enough = match (values.first(), values.get(3), values.get(4), values.get(7)) {
    (Some(_), Some(_), Some(_), Some(_)) => Enough::Yes,
    (_, _, _, _) => Enough::No,
  };

  match enough {
    Enough::No => return Ok(None),
    Enough::Yes => {}
  }

  let hints = values.get(6);
  let Ok(saying) = find_hint(hints, URGENCY);
  let Ok(valued) = find_hint(hints, VALUE);
  let Ok(urgency) = urgently(saying);
  let Ok(value) = numbered(valued);
  let Ok(asking) = numbered(values.get(7));
  let Ok(expiry) = lasting(asking, urgency);
  let Ok(replacing) = counted(values.get(1));
  let Ok(actions) = actions(values.get(5));

  Ok(Some(Request {
    application,
    replacing,
    summary,
    body,
    urgency,
    expiry,
    value,
    actions,
  }))
}

enum Enough {
  Yes,
  No,
}

fn urgently(value: Option<&Value>) -> Result<Urgency, Never> {
  let Ok(counted) = numbered(value);

  Ok(match counted {
    Some(0) => Urgency::Low,
    Some(2) => Urgency::Critical,
    Some(_) | None => Urgency::Normal,
  })
}

fn numbered(value: Option<&Value>) -> Result<Option<i64>, Never> {
  match value {
    Some(value_2) => value_2.counted(),
    None => Ok(None),
  }
}

fn counted(value: Option<&Value>) -> Result<u32, Never> {
  let Ok(counted) = numbered(value);

  Ok(match counted {
    Some(counted_2) => match u32::try_from(counted_2) {
      Ok(counted_3) => counted_3,
      Err(_too_big) => 0,
    },
    None => 0,
  })
}

fn text_of(value: Option<&Value>) -> Result<String, Never> {
  let saying = match value {
    Some(value_2) => {
      let Ok(saying) = value_2.text();

      saying
    }
    None => None,
  };

  Ok(match saying {
    Some(saying_2) => saying_2.to_string(),
    None => String::new(),
  })
}

fn lasting(asking: Option<i64>, urgency: Urgency) -> Result<Expiry, Never> {
  Ok(match asking {
    Some(0) => Expiry::Stays,
    Some(asking_2) => match u32::try_from(asking_2) {
      Ok(asking_3) => Expiry::Milliseconds(asking_3),
      Err(_it_is_not_a_length_of_time) => match urgency {
        Urgency::Critical => Expiry::Stays,
        Urgency::Low | Urgency::Normal => Expiry::Milliseconds(A_WHILE),
      },
    },
    None => match urgency {
      Urgency::Critical => Expiry::Stays,
      Urgency::Low | Urgency::Normal => Expiry::Milliseconds(A_WHILE),
    },
  })
}

fn find_hint<'a>(hints: Option<&'a Value>, wanted: &str) -> Result<Option<&'a Value>, Never> {
  let listed = match hints {
    Some(hints_2) => {
      let Ok(listed) = hints_2.listed();

      listed
    }
    None => None,
  };

  let held = match listed {
    Some(held) => held,
    None => return Ok(None),
  };

  for hint in held {
    let Ok(pair) = hint.pair();

    let (name, value) = match pair {
      Some((name, value)) => (name, value),
      None => continue,
    };

    let Ok(name_2) = name.text();

    match ::core::primitive::bool::then_some(name_2 == Some(wanted), ()) {
      ::core::option::Option::Some(()) => return Ok(Some(value)),
      ::core::option::Option::None => {}
    }
  }

  Ok(None)
}

fn looked_at() -> Result<String, Never> {
  Ok(format!(
    "<!DOCTYPE node PUBLIC \"-//freedesktop//DTD D-BUS Object Introspection 1.0//EN\" \
\"http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd\">
<node>
  <interface name=\"{NOTIFICATIONS}\">
    <method name=\"Notify\">
      <arg type=\"s\" name=\"app_name\" direction=\"in\"/>
      <arg type=\"u\" name=\"replaces_id\" direction=\"in\"/>
      <arg type=\"s\" name=\"app_icon\" direction=\"in\"/>
      <arg type=\"s\" name=\"summary\" direction=\"in\"/>
      <arg type=\"s\" name=\"body\" direction=\"in\"/>
      <arg type=\"as\" name=\"actions\" direction=\"in\"/>
      <arg type=\"a{{sv}}\" name=\"hints\" direction=\"in\"/>
      <arg type=\"i\" name=\"expire_timeout\" direction=\"in\"/>
      <arg type=\"u\" name=\"id\" direction=\"out\"/>
    </method>
    <method name=\"CloseNotification\">
      <arg type=\"u\" name=\"id\" direction=\"in\"/>
    </method>
    <method name=\"GetCapabilities\">
      <arg type=\"as\" name=\"capabilities\" direction=\"out\"/>
    </method>
    <method name=\"GetServerInformation\">
      <arg type=\"s\" name=\"name\" direction=\"out\"/>
      <arg type=\"s\" name=\"vendor\" direction=\"out\"/>
      <arg type=\"s\" name=\"version\" direction=\"out\"/>
      <arg type=\"s\" name=\"spec_version\" direction=\"out\"/>
    </method>
    <signal name=\"NotificationClosed\">
      <arg type=\"u\" name=\"id\"/>
      <arg type=\"u\" name=\"reason\"/>
    </signal>
    <signal name=\"ActionInvoked\">
      <arg type=\"u\" name=\"id\"/>
      <arg type=\"s\" name=\"action_key\"/>
    </signal>
  </interface>
  <interface name=\"{APPLICATION}\">
    <method name=\"ClearAll\"/>
    <method name=\"{HOLD_QUIET}\"/>
    <method name=\"Quieten\">
      <arg type=\"s\" name=\"quiet\" direction=\"out\"/>
    </method>
  </interface>
</node>
"
  ))
}

pub fn keeps(told: Option<&str>) -> Result<PathBuf, Never> {
  match told.map(str::trim).filter(|value| !value.is_empty()) {
    Some(value) => Ok(PathBuf::from(value)),
    None => {
      let Ok(under) = crate::saying::under();

      Ok(under.join(KEPT))
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  allow(
    explicit026_env_read_once,
    reason = "CONSOLE_NOTIFICATIONS_PATH belongs to this crate, and the const beside it is the only spelling of the name"
  )
)]
pub fn inbox_path() -> Result<PathBuf, Never> {
  let told = match std::env::var(WHERE) {
    Ok(value) => Some(value),
    Err(std::env::VarError::NotPresent) => None,
    Err(fault) => {
      eprintln!("console-notify: {WHERE}: {fault}");

      None
    }
  };

  keeps(told.as_deref())
}

pub fn load_inbox() -> Result<Inbox, Never> {
  let Ok(at) = inbox_path();
  let Ok(read) = console_core_atomic_writes::read(&at);

  let value = match read {
    console_core_atomic_writes::Stored::Text(text) => text,
    console_core_atomic_writes::Stored::Absent => return Ok(Inbox::default()),
    console_core_atomic_writes::Stored::Failed(why) => {
      eprintln!("{}: {why}", at.display());

      return Ok(Inbox::default());
    }
  };

  crate::reading::whole(&value)
}

pub fn to_inbox(holding: &Holding) -> Result<Inbox, Never> {
  let waiting: Vec<Notification> = holding
    .waiting
    .iter()
    .map(|held| held.notification.clone())
    .collect();

  let Ok(stream_quiet) = holding.held_quiet();
  Ok(Inbox {
    waiting,
    earlier: holding.earlier.clone(),
    do_not_disturb: holding.do_not_disturb,
    stream_quiet,
  })
}

pub fn closing_one(id: u32) -> Result<Command, Never> {
  cli::effect(&[
    "--user",
    "call",
    NOTIFICATIONS,
    AT,
    NOTIFICATIONS,
    CLOSE_NOTIFICATION,
    "u",
    &id.to_string(),
  ])
}

pub fn clear_all() -> Result<Command, Never> {
  call(CLEAR_ALL)
}

pub fn quieten() -> Result<Command, Never> {
  call(QUIETEN)
}

fn call(member: &str) -> Result<Command, Never> {
  cli::effect(&["--user", "call", NOTIFICATIONS, AT, APPLICATION, member])
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_bus::messages::{Kind, Whom};

  #[derive(Debug, PartialEq)]
  enum NotFound {
    Answer,
    Number,
    Waiting,
    Armed,
    Closed,
  }

  fn calling(member: &str) -> Result<Message, Never> {
    Message::call(&Whom {
      to: NOTIFICATIONS,
      at: AT,
      on: NOTIFICATIONS,
      calling: member,
    })
  }

  #[test]
  fn a_notification_action_is_sent_once_and_cannot_apply_to_a_replacement() -> Result<(), NotFound>
  {
    let mut holding = Holding::default();
    let Ok(message) = notify("Screenshot saved", 0, Vec::new(), -1);
    let Ok(parsed) = parse_request(&message.values);
    let mut asked = parsed.ok_or(NotFound::Waiting)?;
    asked.actions.push(Action {
      key: "send".to_owned(),
      label: "Send to device".to_owned(),
    });
    let Ok(first) = holding.raised(&asked);
    asked.replacing = first.id;
    let Ok(replaced) = holding.raised(&asked);
    let target = Target {
      id: first.id,
      raised: first.raised,
      choice: Choice::Invoke(0),
    };
    let Ok(stale) = holding.activated(target);
    assert!(stale.signals.is_empty());
    assert_eq!(stale.changed, Modified::No);

    let current = Target {
      raised: replaced.raised,
      ..target
    };
    let Ok(turn) = holding.activated(current);
    let invoked = turn.signals.first().ok_or(NotFound::Answer)?;
    assert_eq!(invoked.member.as_deref(), Some("ActionInvoked"));
    assert_eq!(
      invoked.values,
      vec![
        Value::Unsigned32(current.id),
        Value::Word("send".to_owned())
      ]
    );
    let last = turn.signals.last().ok_or(NotFound::Closed)?;
    assert_eq!(last.member.as_deref(), Some("NotificationClosed"));
    assert_eq!(turn.changed, Modified::Yes);
    let Ok(again) = holding.activated(current);
    assert!(again.signals.is_empty());
    Ok(())
  }

  fn notify(
    summary: &str,
    replacing: u32,
    hints: Vec<Value>,
    lasting: i32,
  ) -> Result<Message, Never> {
    let Ok(call) = calling("Notify");

    call.carrying(
      "susssasa{sv}i",
      vec![
        Value::Word("Console".to_string()),
        Value::Unsigned32(replacing),
        Value::Word(String::new()),
        Value::Word(summary.to_string()),
        Value::Word("the body".to_string()),
        Value::List(Vec::new()),
        Value::List(hints),
        Value::Signed32(lasting),
      ],
    )
  }

  fn hint((named, shape): (&str, &str), value: Value) -> Result<Value, Never> {
    Ok(Value::Group(vec![
      Value::Word(named.to_string()),
      Value::Variant {
        shape: shape.to_string(),
        value: Box::new(value),
      },
    ]))
  }

  fn urgently(urgency: u8) -> Result<Vec<Value>, Never> {
    let Ok(hint) = hint(("urgency", "y"), Value::Byte(urgency));

    Ok(vec![hint])
  }

  fn told(holding: &mut Holding, message: Result<Message, Never>) -> Result<Turn, Never> {
    let Ok(message_2) = message;

    handle_message(holding, &message_2)
  }

  fn raising(holding: &mut Holding, summary: &str) -> Result<u32, NotFound> {
    let Ok(turn) = told(holding, notify(summary, 0, Vec::new(), -1));
    let answer = turn.say.ok_or(NotFound::Answer)?;

    match answer.values.first() {
      Some(Value::Unsigned32(id)) => Ok(*id),
      Some(_) | None => Err(NotFound::Number),
    }
  }

  fn first(holding: &Holding) -> Result<&ActiveNotification, NotFound> {
    holding.waiting.first().ok_or(NotFound::Waiting)
  }

  #[test]
  fn a_notification_is_answered_with_a_number_and_the_number_is_never_nothing() {
    let mut holding = Holding::default();

    assert_eq!(raising(&mut holding, "one"), Ok(1));
    assert_eq!(raising(&mut holding, "two"), Ok(2));
    assert_eq!(holding.waiting.len(), 2);
  }

  #[test]
  fn what_a_card_says_is_what_the_call_carried() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let _id = raising(&mut holding, "Notifications fell over")?;
    let held = first(&holding)?;

    assert_eq!(held.notification.summary, "Notifications fell over");
    assert_eq!(held.notification.body, "the body");
    assert_eq!(held.notification.application, "Console");

    Ok(())
  }

  #[test]
  fn a_notification_that_replaces_one_that_is_waiting_takes_its_place_and_its_number()
  -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let replaced = raising(&mut holding, "40%")?;
    let Ok(turn) = told(&mut holding, notify("45%", replaced, Vec::new(), -1));
    let said = turn.say.ok_or(NotFound::Answer)?;

    assert_eq!(said.values, vec![Value::Unsigned32(replaced)]);
    let held = first(&holding)?;

    assert_eq!(holding.waiting.len(), 1);
    assert_eq!(held.notification.summary, "45%");

    Ok(())
  }

  #[test]
  fn a_notification_that_replaces_one_that_has_gone_is_raised_as_a_new_one()
  -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let replaced = raising(&mut holding, "40%")?;
    let Ok(_gone) = holding.closed(replaced);
    let Ok(turn) = told(&mut holding, notify("45%", replaced, Vec::new(), -1));
    let said = turn.say.ok_or(NotFound::Answer)?;

    assert_eq!(said.values, vec![Value::Unsigned32(2)]);
    assert_eq!(holding.waiting.len(), 1);

    Ok(())
  }

  #[test]
  fn five_seconds_is_what_a_caller_that_says_nothing_gets() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let _id = raising(&mut holding, "listening")?;
    let held = first(&holding)?;

    assert_eq!(held.expiry, Expiry::Milliseconds(A_WHILE));

    Ok(())
  }

  #[test]
  fn a_fault_stays_until_someone_has_seen_it() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(urgent) = urgently(2);
    let Ok(_turn) = told(&mut holding, notify("it broke", 0, urgent, -1));
    let held = first(&holding)?;

    assert_eq!(held.expiry, Expiry::Stays);
    assert_eq!(held.notification.urgency, Urgency::Critical);

    Ok(())
  }

  #[test]
  fn a_caller_that_asks_for_a_length_is_given_the_one_it_asked_for() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(_turn) = told(&mut holding, notify("a moment", 0, Vec::new(), 1200));
    let held = first(&holding)?;

    assert_eq!(held.expiry, Expiry::Milliseconds(1200));

    Ok(())
  }

  #[test]
  fn a_caller_that_asks_for_no_length_at_all_is_asking_for_it_to_stay() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(_turn) = told(&mut holding, notify("standing", 0, Vec::new(), 0));
    let held = first(&holding)?;

    assert_eq!(held.expiry, Expiry::Stays);

    Ok(())
  }

  #[test]
  fn a_reading_carries_the_number_it_was_sent_with() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(value) = hint(("value", "i"), Value::Signed32(40));
    let Ok(_turn) = told(&mut holding, notify("Volume", 0, vec![value], -1));
    let held = first(&holding)?;

    assert_eq!(held.value, Some(40));

    Ok(())
  }

  #[test]
  fn the_length_a_card_was_given_runs_out_and_takes_that_card_down() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(turn) = told(&mut holding, notify("40%", 0, Vec::new(), 400));
    let armed = turn.arm.ok_or(NotFound::Armed)?;

    assert_eq!(armed.expiry, Expiry::Milliseconds(400));
    assert_eq!(holding.ran_out(&armed), Ok(Closed::Yes));
    assert!(holding.waiting.is_empty());

    Ok(())
  }

  #[test]
  fn a_card_replaced_before_its_length_ran_out_is_not_taken_down_by_the_old_length()
  -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(before) = told(&mut holding, notify("40%", 0, Vec::new(), 400));
    let armed = before.arm.ok_or(NotFound::Armed)?;
    let Ok(again) = told(&mut holding, notify("45%", armed.id, Vec::new(), 400));
    let armed_again = again.arm.ok_or(NotFound::Armed)?;

    assert_eq!(holding.ran_out(&armed), Ok(Closed::No));
    assert_eq!(holding.waiting.len(), 1);
    assert_eq!(holding.ran_out(&armed_again), Ok(Closed::Yes));
    assert!(holding.waiting.is_empty());

    Ok(())
  }

  #[test]
  fn a_card_that_stays_is_armed_with_nothing_to_run_out() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(urgent) = urgently(2);
    let Ok(turn) = told(&mut holding, notify("it broke", 0, urgent, -1));
    let armed = turn.arm.ok_or(NotFound::Armed)?;

    assert_eq!(armed.expiry, Expiry::Stays);

    Ok(())
  }

  #[test]
  fn closing_one_moves_it_to_earlier_and_says_which_and_why() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let id = raising(&mut holding, "one")?;
    let Ok(close) = calling("CloseNotification");
    let Ok(turn) = told(
      &mut holding,
      close.carrying("u", vec![Value::Unsigned32(id)]),
    );

    assert_eq!(turn.changed, Modified::Yes);
    assert!(holding.waiting.is_empty());
    assert_eq!(holding.earlier.len(), 1);
    assert_eq!(turn.signals.len(), 1);

    let gone = turn.signals.first().ok_or(NotFound::Closed)?;

    assert_eq!(gone.kind, Kind::Signal);
    assert_eq!(gone.member.as_deref(), Some("NotificationClosed"));
    assert_eq!(
      gone.values,
      vec![Value::Unsigned32(id), Value::Unsigned32(3)]
    );

    Ok(())
  }

  #[test]
  fn closing_one_that_is_not_there_says_nothing_and_is_not_a_fault() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(close) = calling("CloseNotification");
    let Ok(turn) = told(
      &mut holding,
      close.carrying("u", vec![Value::Unsigned32(9)]),
    );
    let said = turn.say.ok_or(NotFound::Answer)?;

    assert_eq!(turn.changed, Modified::No);
    assert!(turn.signals.is_empty());
    assert_eq!(said.kind, Kind::Answer);

    Ok(())
  }

  #[test]
  fn clearing_them_all_says_one_of_each_and_keeps_them_all() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let one = raising(&mut holding, "one")?;
    let two = raising(&mut holding, "two")?;
    let Ok(turn) = told(
      &mut holding,
      Message::call(&Whom {
        to: NOTIFICATIONS,
        at: AT,
        on: APPLICATION,
        calling: "ClearAll",
      }),
    );

    assert!(holding.waiting.is_empty());
    assert_eq!(holding.earlier.len(), 2);

    let values: Vec<Vec<Value>> = turn
      .signals
      .iter()
      .map(|gone| gone.values.clone())
      .collect();

    assert_eq!(
      values,
      [
        vec![Value::Unsigned32(one), Value::Unsigned32(2)],
        vec![Value::Unsigned32(two), Value::Unsigned32(2)]
      ]
    );

    Ok(())
  }

  #[test]
  fn what_is_cleared_is_in_earlier_a_moment_later_and_the_newest_is_first() -> Result<(), NotFound>
  {
    let mut holding = Holding::default();
    let _one = raising(&mut holding, "one")?;
    let _two = raising(&mut holding, "two")?;
    let Ok(_every) = holding.cleared();

    assert_eq!(
      holding.earlier.first().map(|was| was.summary.as_str()),
      Some("two")
    );

    Ok(())
  }

  #[test]
  fn earlier_holds_what_a_screenful_holds_and_lets_the_rest_go() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let last = EARLIER.saturating_add(4);

    for many in 0..=last {
      let id = raising(&mut holding, &format!("one of {many}"))?;
      let Ok(_gone) = holding.closed(id);
    }

    let Ok(kept) = console_core_number_conversion::fitted::<_, u32>(holding.earlier.len());

    assert_eq!(kept, EARLIER);
    assert_eq!(
      holding.earlier.first().map(|was| was.summary.clone()),
      Some(format!("one of {last}"))
    );

    Ok(())
  }

  #[test]
  fn quiet_holds_follow_their_connections_and_preserve_the_users_choice() -> Result<(), NotFound>
  {
    let mut holding = Holding::default();
    let _id = raising(&mut holding, "one")?;

    for owner in [":1.42", ":1.43", ":1.42"] {
      let Ok(mut request) = hold_quiet(NOTIFICATIONS);
      request.sender = Some(owner.to_owned());
      let Ok(turn) = handle_message(&mut holding, &request);
      let say = turn.say.ok_or(NotFound::Answer)?;
      assert_eq!(say.kind, Kind::Answer);
    }

    assert_eq!(holding.do_not_disturb, DoNotDisturb::Off);
    let Ok(visible) = holding.visible();
    assert!(visible.is_empty());

    let Ok(_changed) = holding.toggled_do_not_disturb();
    let lost = |owner: &str| Message {
      kind: Kind::Signal,
      sender: Some(BUS.to_owned()),
      path: Some(BUS_PATH.to_owned()),
      interface: Some(BUS.to_owned()),
      member: Some(NAME_OWNER_CHANGED.to_owned()),
      values: vec![
        Value::Word(owner.to_owned()),
        Value::Word(owner.to_owned()),
        Value::Word(String::new()),
      ],
      ..Message::default()
    };
    let Ok(_first) = handle_message(&mut holding, &lost(":1.42"));
    let Ok(held) = to_inbox(&holding);
    assert_eq!(held.stream_quiet, DoNotDisturb::On);

    let Ok(last) = handle_message(&mut holding, &lost(":1.43"));
    assert_eq!(last.changed, Modified::Yes);
    assert_eq!(holding.do_not_disturb, DoNotDisturb::On);
    let Ok(held_2) = to_inbox(&holding);
    assert_eq!(held_2.stream_quiet, DoNotDisturb::Off);
    let Ok(still_hidden) = holding.visible();
    assert!(still_hidden.is_empty());
    let Ok(_changed) = holding.toggled_do_not_disturb();
    let Ok(visible_2) = holding.visible();
    assert_eq!(visible_2.len(), 1);

    Ok(())
  }

  #[test]
  fn quiet_holds_the_card_back_and_holds_nothing_else_back() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let _id = raising(&mut holding, "one")?;
    let Ok(quietening) = Message::call(&Whom {
      to: NOTIFICATIONS,
      at: AT,
      on: APPLICATION,
      calling: "Quieten",
    });
    let Ok(turn) = handle_message(&mut holding, &quietening);
    let said = turn.say.ok_or(NotFound::Answer)?;
    let Ok(held_back) = holding.visible();

    assert_eq!(said.values, vec![Value::Word("held-back".to_string())]);
    assert_eq!(holding.do_not_disturb, DoNotDisturb::On);
    assert_eq!(holding.waiting.len(), 1);
    assert!(held_back.is_empty());

    let Ok(_turn) = handle_message(&mut holding, &quietening);
    let Ok(shown) = holding.visible();

    assert_eq!(holding.do_not_disturb, DoNotDisturb::Off);
    assert_eq!(shown.len(), 1);

    Ok(())
  }

  #[test]
  fn the_newest_card_is_the_one_drawn_first() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let _one = raising(&mut holding, "one")?;
    let _two = raising(&mut holding, "two")?;
    let Ok(visible) = holding.visible();

    assert_eq!(
      visible
        .first()
        .map(|held| held.notification.summary.as_str()),
      Some("two")
    );

    Ok(())
  }

  #[test]
  fn what_this_can_do_is_said_when_it_is_asked() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(turn) = told(&mut holding, calling("GetCapabilities"));
    let said = turn.say.ok_or(NotFound::Answer)?;

    assert_eq!(
      said.values,
      vec![Value::List(vec![
        Value::Word("body".to_string()),
        Value::Word("persistence".to_string()),
        Value::Word("actions".to_string()),
      ])]
    );

    Ok(())
  }

  #[test]
  fn what_this_is_is_said_when_it_is_asked() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(turn) = told(&mut holding, calling("GetServerInformation"));
    let said = turn.say.ok_or(NotFound::Answer)?;

    assert_eq!(
      said.values.first(),
      Some(&Value::Word("console".to_string()))
    );
    assert_eq!(said.values.len(), 4);

    Ok(())
  }

  #[test]
  fn a_call_nothing_here_answers_is_complained_about_rather_than_ignored() -> Result<(), NotFound>
  {
    let mut holding = Holding::default();
    let Ok(turn) = told(&mut holding, calling("Whatever"));
    let said = turn.say.ok_or(NotFound::Answer)?;

    assert_eq!(said.kind, Kind::ErrorReply);
    assert_eq!(
      said.fault.as_deref(),
      Some("org.freedesktop.DBus.Error.UnknownMethod")
    );

    Ok(())
  }

  #[test]
  fn a_notification_that_is_not_a_notification_is_complained_about() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(call) = calling("Notify");
    let Ok(turn) = told(
      &mut holding,
      call.carrying("s", vec![Value::Word("only this".to_string())]),
    );
    let said = turn.say.ok_or(NotFound::Answer)?;

    assert_eq!(said.kind, Kind::ErrorReply);
    assert!(holding.waiting.is_empty());

    Ok(())
  }

  #[test]
  fn a_hint_nothing_here_reads_is_walked_past_rather_than_stopping_the_card()
  -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let Ok(picture) = hint(
      ("image-path", "s"),
      Value::Word("/a/picture.png".to_string()),
    );
    let Ok(urgency) = hint(("urgency", "y"), Value::Byte(2));
    let Ok(_turn) = told(
      &mut holding,
      notify("it broke", 0, vec![picture, urgency], -1),
    );
    let held = first(&holding)?;

    assert_eq!(held.notification.urgency, Urgency::Critical);

    Ok(())
  }

  #[test]
  fn what_the_panel_reads_is_what_the_daemon_is_holding() -> Result<(), NotFound> {
    let mut holding = Holding::default();
    let id = raising(&mut holding, "one")?;
    let _two = raising(&mut holding, "two")?;
    let Ok(_gone) = holding.closed(id);
    let Ok(to_inbox) = to_inbox(&holding);

    assert_eq!(to_inbox.waiting.len(), 1);
    assert_eq!(to_inbox.earlier.len(), 1);
    assert_eq!(to_inbox.do_not_disturb, DoNotDisturb::Off);

    Ok(())
  }
}
