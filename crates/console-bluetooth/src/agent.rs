//! The agent: the program bluez asks what the radio cannot decide by itself.
//!
//! A pairing raises questions only somebody in front of the machine can
//! answer -- here is a code, show it; this device wants in, may it -- and
//! bluez asks them of an agent, an object some program registers at a path
//! of its own choosing and answers calls on. With no agent registered bluez
//! decides for itself, and what it decides is that the machine has no way to
//! show anything and no way to say yes, which is the pairing nothing could
//! finish.
//!
//! So here are both halves. Registering, becoming the agent bluez asks first,
//! and going away again are calls to bluez's agent manager. What bluez asks
//! back is a call on the agent's path, read into a [`Request`] that names the
//! device it is about by its address; the answer is the method's reply, or
//! the error bluez reads as a no.
//!
//! The capability an agent claims is what decides which questions arrive. One
//! that says `DisplayOnly` is shown codes and is never asked to compare two
//! numbers, which is the honest claim for a panel that started a pairing and
//! has nobody standing at a keyboard to answer it.
//!
//! The one question a capability does not keep away is a PIN. A device from
//! before Bluetooth 2.1 pairs only by both ends being given the same code,
//! and bluez asks the agent for it whatever the agent claimed. bluez takes
//! one to sixteen bytes, which is what a [`PinCode`] holds and all it holds;
//! which code to give is the agent's question rather than bluez's.

use std::fmt;

use console_bus::messages::{ErrorName, Message, ValidationError, Value, Whom};
use console_bus::properties::ObjectPath;
use console_core_media_access_control_addresses::Address;
use console_core_never::Never;
use console_core_words::Words;

use crate::device;
use crate::errors::{ErrorKind, PREFIX};
use crate::{ROOT, SERVICE};

pub const INTERFACE: &str = "org.bluez.Agent1";

pub const MANAGER: &str = "org.bluez.AgentManager1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Capability {
  #[words(name = "DisplayOnly")]
  DisplayOnly,
  #[words(name = "DisplayYesNo")]
  DisplayYesNo,
  #[words(name = "KeyboardOnly")]
  KeyboardOnly,
  #[words(name = "NoInputNoOutput")]
  NoInputNoOutput,
  #[words(name = "KeyboardDisplay")]
  KeyboardDisplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Method {
  #[words(name = "Release")]
  Release,
  #[words(name = "RequestPinCode")]
  RequestPinCode,
  #[words(name = "DisplayPinCode")]
  DisplayPinCode,
  #[words(name = "RequestPasskey")]
  RequestPasskey,
  #[words(name = "DisplayPasskey")]
  DisplayPasskey,
  #[words(name = "RequestConfirmation")]
  RequestConfirmation,
  #[words(name = "RequestAuthorization")]
  RequestAuthorization,
  #[words(name = "AuthorizeService")]
  AuthorizeService,
  #[words(name = "Cancel")]
  Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
  Release,
  RequestPinCode {
    device: Address,
  },
  DisplayPinCode {
    device: Address,
    pin_code: PinCode,
  },
  RequestPasskey {
    device: Address,
  },
  DisplayPasskey {
    device: Address,
    passkey: Passkey,
    entered: u16,
  },
  RequestConfirmation {
    device: Address,
    passkey: Passkey,
  },
  RequestAuthorization {
    device: Address,
  },
  AuthorizeService {
    device: Address,
    service: String,
  },
  Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestError {
  UnknownInterface(String),
  UnknownMethod(String),
  InvalidArgs(String),
}

impl fmt::Display for RequestError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      RequestError::UnknownInterface(named) => {
        write!(to, "this is a bluez agent and has nothing called {named}")
      }
      RequestError::UnknownMethod(named) => write!(to, "a bluez agent is not asked {named}"),
      RequestError::InvalidArgs(named) => {
        write!(to, "{named} was asked with arguments bluez does not send")
      }
    }
  }
}

impl std::error::Error for RequestError {}

#[derive(Clone, PartialEq, Eq)]
pub struct PinCode(String);

impl fmt::Debug for PinCode {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str("PinCode([redacted])")
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Passkey(u32);

impl Passkey {
  pub fn number_for_display(self) -> Result<u32, Never> {
    Ok(self.0)
  }
}

impl fmt::Debug for Passkey {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str("Passkey([redacted])")
  }
}

pub const LONGEST_PIN: u32 = 16;

impl PinCode {
  pub fn read(written: &str) -> Result<Option<PinCode>, Never> {
    let length = u32::try_from(written.len());

    Ok(match length {
      Ok(0) => None,
      Ok(bytes) => match ::core::primitive::bool::then_some(bytes > LONGEST_PIN, ()) {
        ::core::option::Option::None => Some(PinCode(written.to_string())),
        ::core::option::Option::Some(()) => None,
      },
      Err(_longer_than_any_pin) => None,
    })
  }

  pub fn as_str(&self) -> Result<&str, Never> {
    Ok(&self.0)
  }
}

impl fmt::Display for PinCode {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "{}", self.0)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
  Rejected,
  Canceled,
}

fn managing(agent: ObjectPath<'_>, method: &str) -> Result<Message, Never> {
  let Ok(asking) = Message::call(&Whom {
    to: SERVICE,
    at: ROOT,
    on: MANAGER,
    calling: method,
  });

  asking.carrying("o", vec![Value::Path(agent.0.to_string())])
}

pub fn register(agent: ObjectPath<'_>, capability: Capability) -> Result<Message, Never> {
  let Ok(asking) = Message::call(&Whom {
    to: SERVICE,
    at: ROOT,
    on: MANAGER,
    calling: "RegisterAgent",
  });
  let Ok(word) = capability.name();
  let Ok(claimed) = Value::word(word);

  asking.carrying("os", vec![Value::Path(agent.0.to_string()), claimed])
}

pub fn request_default(agent: ObjectPath<'_>) -> Result<Message, Never> {
  managing(agent, "RequestDefaultAgent")
}

pub fn unregister(agent: ObjectPath<'_>) -> Result<Message, Never> {
  managing(agent, "UnregisterAgent")
}

pub fn request(asked: &Message) -> Result<Request, RequestError> {
  let interface = match &asked.interface {
    Some(interface) => interface.as_str(),
    None => "",
  };

  let member = match &asked.member {
    Some(member) => member.as_str(),
    None => "",
  };

  match ::core::primitive::bool::then_some(interface == INTERFACE, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(RequestError::UnknownInterface(interface.to_string())),
  }

  let Ok(named) = Method::from_name(member);

  let method = match named {
    Some(known) => known,
    None => return Err(RequestError::UnknownMethod(member.to_string())),
  };

  let Ok(read) = arguments(method, &asked.values);

  match read {
    Some(request) => Ok(request),
    None => Err(RequestError::InvalidArgs(member.to_string())),
  }
}

fn arguments(method: Method, values: &[Value]) -> Result<Option<Request>, Never> {
  let Ok(asking) = about(values.first());
  let Ok(second_text) = text(values.get(1));
  let Ok(second_number) = number::<u32>(values.get(1));
  let Ok(third_number) = number::<u16>(values.get(2));

  Ok(match method {
    Method::Release => Some(Request::Release),
    Method::Cancel => Some(Request::Cancel),
    Method::RequestPinCode => asking.map(|from| Request::RequestPinCode { device: from }),
    Method::RequestPasskey => asking.map(|from| Request::RequestPasskey { device: from }),
    Method::RequestAuthorization => {
      asking.map(|from| Request::RequestAuthorization { device: from })
    }
    Method::DisplayPinCode => asking.zip(second_text).and_then(|(device, shown)| {
      let Ok(code) = PinCode::read(&shown);

      code.map(|pin_code| Request::DisplayPinCode { device, pin_code })
    }),
    Method::AuthorizeService => {
      asking
        .zip(second_text)
        .map(|(from, uuid)| Request::AuthorizeService {
          device: from,
          service: uuid,
        })
    }
    Method::RequestConfirmation => {
      asking
        .zip(second_number)
        .map(|(from, compared)| Request::RequestConfirmation {
          device: from,
          passkey: Passkey(compared),
        })
    }
    Method::DisplayPasskey => {
      asking
        .zip(second_number)
        .zip(third_number)
        .map(|((from, shown), typed)| Request::DisplayPasskey {
          device: from,
          passkey: Passkey(shown),
          entered: typed,
        })
    }
  })
}

fn about(value: Option<&Value>) -> Result<Option<Address>, Never> {
  let Ok(path) = match value {
    Some(held) => held.text(),
    None => Ok(None),
  };

  match path {
    Some(written) => device::address(written),
    None => Ok(None),
  }
}

fn text(value: Option<&Value>) -> Result<Option<String>, Never> {
  let Ok(said) = match value {
    Some(held) => held.text(),
    None => Ok(None),
  };

  Ok(said.map(str::to_string))
}

fn number<Wanted: TryFrom<i64>>(value: Option<&Value>) -> Result<Option<Wanted>, Never> {
  let Ok(counted) = match value {
    Some(held) => held.counted(),
    None => Ok(None),
  };

  Ok(match counted {
    Some(whole) => match Wanted::try_from(whole) {
      Ok(fitted) => Some(fitted),
      Err(_out_of_range) => None,
    },
    None => None,
  })
}

pub fn accepted(asked: &Message) -> Result<Message, Never> {
  asked.answering()
}

pub fn pin_code(asked: &Message, pin: &PinCode) -> Result<Message, Never> {
  let Ok(answer) = asked.answering();
  let Ok(code) = Value::word(&pin.0);

  answer.carrying("s", vec![code])
}

pub fn rejected(asked: &Message, rejection: Rejection) -> Result<Message, Never> {
  let (kind, why) = match rejection {
    Rejection::Rejected => (ErrorKind::Rejected, "turned away by the agent"),
    Rejection::Canceled => (ErrorKind::Canceled, "canceled by the agent"),
  };

  let Ok(name) = kind.name();

  let named = format!("{PREFIX}{name}");

  asked.failing(ErrorName(&named), why)
}

pub fn refused(asked: &Message, error: &RequestError) -> Result<Message, Never> {
  let complaint = match error {
    RequestError::UnknownInterface(_) => ValidationError::UnknownInterface,
    RequestError::UnknownMethod(_) => ValidationError::UnknownMethod,
    RequestError::InvalidArgs(_) => ValidationError::InvalidArgs,
  };

  asked.complaining(complaint, &error.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  use console_bus::messages::Kind;

  const AGENT: &str = "/console/bluetooth/agent";

  const KEYBOARD: Address = Address([0xE0, 0xC1, 0x02, 0x00, 0xB0, 0xC3]);

  const PATH: &str = "/org/bluez/hci0/dev_E0_C1_02_00_B0_C3";

  fn asking(method: Method, shape: &str, values: Vec<Value>) -> Result<Message, Never> {
    let Ok(calling) = method.name();
    let Ok(mut asked) = Message::call(&Whom {
      to: ":1.40",
      at: AGENT,
      on: INTERFACE,
      calling,
    });

    asked.serial = 12;
    asked.sender = Some(":1.3".to_string());

    asked.carrying(shape, values)
  }

  #[test]
  fn a_pin_is_one_to_sixteen_bytes_and_nothing_else_is_one() {
    let Ok(four) = PinCode::read("0000");
    let Ok(sixteen) = PinCode::read("0123456789abcdef");
    let Ok(empty) = PinCode::read("");
    let Ok(seventeen) = PinCode::read("0123456789abcdefg");

    assert_eq!(four.map(|pin| pin.to_string()), Some("0000".to_string()));
    assert_eq!(
      sixteen.map(|pin| pin.to_string()),
      Some("0123456789abcdef".to_string())
    );
    assert_eq!(empty, None, "bluez refuses an empty PIN, so it is not one");
    assert_eq!(seventeen, None, "bluez refuses a seventeenth byte");
  }

  #[test]
  fn a_pin_is_given_as_the_reply_to_the_call_that_asked_for_it()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(asked) = asking(
      Method::RequestPinCode,
      "o",
      vec![Value::Path(PATH.to_string())],
    );
    let Ok(read) = PinCode::read("0000");
    let pin = read.ok_or("0000 is a PIN")?;
    let Ok(given) = pin_code(&asked, &pin);

    assert_eq!(given.kind, Kind::Answer);
    assert_eq!(given.reply_to, Some(12));
    assert_eq!(given.shape, "s");
    assert_eq!(given.values, vec![Value::Word("0000".to_string())]);

    Ok(())
  }

  #[test]
  fn registering_claims_the_capability_by_bluez_s_own_word_for_it() {
    let Ok(asked) = register(ObjectPath(AGENT), Capability::DisplayOnly);

    assert_eq!(asked.member.as_deref(), Some("RegisterAgent"));
    assert_eq!(asked.interface.as_deref(), Some(MANAGER));
    assert_eq!(
      asked.values,
      vec![
        Value::Path(AGENT.to_string()),
        Value::Word("DisplayOnly".to_string())
      ]
    );
  }

  #[test]
  fn a_code_to_show_is_read_with_the_device_it_is_for() {
    let Ok(shown) = asking(
      Method::DisplayPasskey,
      "ouq",
      vec![
        Value::Path(PATH.to_string()),
        Value::Unsigned32(1234),
        Value::Unsigned16(2),
      ],
    );

    assert_eq!(
      request(&shown),
      Ok(Request::DisplayPasskey {
        device: KEYBOARD,
        passkey: Passkey(1234),
        entered: 2
      })
    );
  }

  #[test]
  fn a_device_asking_to_reach_a_service_is_read_with_the_service() {
    let Ok(asked) = asking(
      Method::AuthorizeService,
      "os",
      vec![
        Value::Path(PATH.to_string()),
        Value::Word("00001124-0000-1000-8000-00805f9b34fb".to_string()),
      ],
    );

    assert_eq!(
      request(&asked),
      Ok(Request::AuthorizeService {
        device: KEYBOARD,
        service: "00001124-0000-1000-8000-00805f9b34fb".to_string()
      })
    );
  }

  #[test]
  fn a_call_that_is_not_one_bluez_makes_is_refused_by_what_was_wrong_with_it() {
    let Ok(garbled) = asking(
      Method::DisplayPasskey,
      "o",
      vec![Value::Path(PATH.to_string())],
    );
    let Ok(mut unknown) = asking(Method::Release, "", Vec::new());
    let Ok(mut elsewhere) = asking(Method::Release, "", Vec::new());

    unknown.member = Some("Introduce".to_string());
    elsewhere.interface = Some("org.freedesktop.DBus.Peer".to_string());
    elsewhere.member = Some("Ping".to_string());

    assert_eq!(
      request(&garbled),
      Err(RequestError::InvalidArgs("DisplayPasskey".to_string()))
    );
    assert_eq!(
      request(&unknown),
      Err(RequestError::UnknownMethod("Introduce".to_string()))
    );
    assert_eq!(
      request(&elsewhere),
      Err(RequestError::UnknownInterface(
        "org.freedesktop.DBus.Peer".to_string()
      ))
    );

    let Ok(refusal) = refused(
      &garbled,
      &RequestError::InvalidArgs("DisplayPasskey".to_string()),
    );

    assert_eq!(
      refusal.fault.as_deref(),
      Some("org.freedesktop.DBus.Error.InvalidArgs")
    );
  }

  #[test]
  fn a_no_is_bluez_s_own_error_sent_back_to_whoever_asked() {
    let Ok(asked) = asking(
      Method::RequestAuthorization,
      "o",
      vec![Value::Path(PATH.to_string())],
    );
    let Ok(no) = rejected(&asked, Rejection::Rejected);
    let Ok(yes) = accepted(&asked);

    assert_eq!(no.kind, Kind::ErrorReply);
    assert_eq!(no.fault.as_deref(), Some("org.bluez.Error.Rejected"));
    assert_eq!(no.reply_to, Some(12));
    assert_eq!(no.destination.as_deref(), Some(":1.3"));
    assert_eq!(yes.kind, Kind::Answer);
    assert_eq!(yes.reply_to, Some(12));
  }
}
