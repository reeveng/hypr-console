//! An introduction, as a machine: what has been asked of bluez, what it
//! answered, and what to ask next.
//!
//! Introducing a device is a run of calls in an order and a conversation going
//! the other way at the same time. bluez is asked where the device is and what
//! it is called, the agent is registered and made the one asked first, the
//! device is paired, trusted and connected, and the agent goes again. While
//! the pairing is awaited bluez calls back -- a code to show, a device asking
//! to be let in -- and those are answered without the pairing being any less
//! awaited.
//!
//! That is what an async runtime is for, and there is none here. The state
//! says which call is awaited; a reply is a request handed in with the call it
//! answers, and so is a call from bluez; the next call to make and the answer
//! to send are effects offered on the queue. Nothing in this file waits, reads
//! a socket or raises a card, so a whole introduction is a list of requests
//! and the transcript they produce, pressed in a test the same way the device
//! presses it. The loop that carries the effects to the bus and hands back what
//! arrived is the program, at the edge.
//!
//! A reply is only heard in the stage that awaits it. The bus matches a reply
//! to the call it answers by serial and hands in the call's name with it, so a
//! reply the state is not waiting on is one from a stage already left, and
//! leaves the state as it was.
//!
//! The code is the pairing's. It is said, as a line, when bluez says to show
//! one for the device being introduced, and it lasts as long as the
//! introduction does: the panel that ran it shows the line on the card that
//! was pressed and takes it down when the program ends, whichever way it
//! ended. This machine used to put a notification up and take it down again
//! itself, and kept whether one was up in its state for that; a notification
//! raised while a card is up is drawn behind the card, so the code a keyboard
//! needed typed was under the panel it was being paired from.
//!
//! A PIN is answered the same way as everything else bluez asks, from what
//! the machine was handed: the PIN somebody typed on the tab, if they typed
//! one, and six digits drawn at the edge for a keyboard to be given. Which of
//! them a device gets is `agent::pin_answer`'s, and a device that wanted one
//! and was given none fails saying so.

use console_bluetooth::agent::{self as bluez, Capability, Rejection, Request as AgentRequest};
use console_bluetooth::device::{self, Device};
use console_bluetooth::errors::Error as Refusal;
use console_bluetooth::objects;
use console_bus::connection::ConnectionError;
use console_bus::messages::Message;
use console_bus::properties::ObjectPath;
use console_bus::replies::Received;
use console_core_media_access_control_addresses::Address;
use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};

use crate::agent::{self, Answer, PinAnswer, Pins, Went};
use crate::cards::{self, Code};
use crate::errors::{Error, Reason};

pub type Outcome = Result<(), Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
  GetManagedObjects,
  RegisterAgent,
  RequestDefaultAgent,
  Pair,
  SetTrusted,
  Connect,
  UnregisterAgent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
  pub address: Address,
  pub agent: String,
  pub pins: Pins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pin {
  NotRequested,
  Given,
  Denied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
  Finding,
  Registering(Device),
  RequestingDefault(Device),
  Pairing { found: Device, pin: Pin },
  Trusting(Device),
  Connecting(Device),
  Unregistering(Outcome),
  Finished(Outcome),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
  pub address: Address,
  pub agent: String,
  pub pins: Pins,
  pub stage: Stage,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
  Received(Received<Method>),
  Lost(ConnectionError),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
  Call { method: Method, message: Message },
  Reply(Message),
  ShowCode(String),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Call { .. } | Self::Reply(_) => &[Service::Bluetooth],
      Self::ShowCode(_) => &[Service::Panels],
    })
  }
}

pub struct Introduction;

impl Machine for Introduction {
  type Input = Input;
  type State = State;
  type Request = Request;
  type Effect = Effect;

  fn initialize(
    input: &Input,
    _previous: Option<State>,
    effects: &mut Queue<Effect>,
  ) -> Result<State, Never> {
    let Ok(asking) = objects::managed();
    let Ok(()) = call(effects, Method::GetManagedObjects, asking);

    Ok(State {
      address: input.address,
      agent: input.agent.clone(),
      pins: input.pins.clone(),
      stage: Stage::Finding,
    })
  }

  fn handle(state: State, request: Request, effects: &mut Queue<Effect>) -> Result<State, Never> {
    match request {
      Request::Lost(why) => lost(state, &why),
      Request::Received(received) => match received {
        Received::Reply { to, reply } => replied(state, to, Ok(reply), effects),
        Received::Error { to, error } => {
          let Ok(refusal) = Refusal::of(&error);

          replied(state, to, Err(refusal), effects)
        }
        Received::Call(asked) => asked_of(state, &asked, effects),
        Received::Signal(_unasked) | Received::Unmatched(_unasked) => Ok(state),
      },
    }
  }
}

pub fn ended(state: &State) -> Result<Option<Outcome>, Never> {
  Ok(match &state.stage {
    Stage::Finished(outcome) => Some(outcome.clone()),
    Stage::Finding
    | Stage::Registering(_)
    | Stage::RequestingDefault(_)
    | Stage::Pairing { .. }
    | Stage::Trusting(_)
    | Stage::Connecting(_)
    | Stage::Unregistering(_) => None,
  })
}

fn awaiting(stage: &Stage) -> Result<Option<Method>, Never> {
  Ok(match stage {
    Stage::Finding => Some(Method::GetManagedObjects),
    Stage::Registering(_) => Some(Method::RegisterAgent),
    Stage::RequestingDefault(_) => Some(Method::RequestDefaultAgent),
    Stage::Pairing { .. } => Some(Method::Pair),
    Stage::Trusting(_) => Some(Method::SetTrusted),
    Stage::Connecting(_) => Some(Method::Connect),
    Stage::Unregistering(_) => Some(Method::UnregisterAgent),
    Stage::Finished(_) => None,
  })
}

fn call(effects: &mut Queue<Effect>, method: Method, message: Message) -> Result<(), Never> {
  effects.offer(Effect::Call { method, message })
}

fn reply(effects: &mut Queue<Effect>, message: Message) -> Result<(), Never> {
  effects.offer(Effect::Reply(message))
}

fn unreachable(said: &impl ToString) -> Result<Outcome, Never> {
  Ok(Err(Error::Unreachable(said.to_string())))
}

fn replied(
  state: State,
  method: Method,
  reply: Result<Message, Refusal>,
  effects: &mut Queue<Effect>,
) -> Result<State, Never> {
  let Ok(awaited) = awaiting(&state.stage);

  match ::core::primitive::bool::then_some(awaited == Some(method), ()) {
    ::core::option::Option::None => Ok(state),
    ::core::option::Option::Some(()) => {
      let State {
        address,
        agent,
        pins,
        stage,
      } = state;
      let Ok(next) = advanced(address, &agent, stage, reply, effects);

      Ok(State {
        address,
        agent,
        pins,
        stage: next,
      })
    }
  }
}

fn advanced(
  address: Address,
  agent: &str,
  stage: Stage,
  reply: Result<Message, Refusal>,
  effects: &mut Queue<Effect>,
) -> Result<Stage, Never> {
  match stage {
    Stage::Finding => located(address, agent, reply, effects),
    Stage::Registering(found) => registered(found, agent, reply, effects),
    Stage::RequestingDefault(found) => defaulted(found, agent, reply, effects),
    Stage::Pairing { found, pin } => paired(found, pin, agent, reply, effects),
    Stage::Trusting(found) => trusted(found, agent, reply, effects),
    Stage::Connecting(found) => connected(found, agent, reply, effects),
    Stage::Unregistering(outcome) | Stage::Finished(outcome) => Ok(Stage::Finished(outcome)),
  }
}

fn located(
  address: Address,
  agent: &str,
  reply: Result<Message, Refusal>,
  effects: &mut Queue<Effect>,
) -> Result<Stage, Never> {
  let objects = match reply {
    Ok(answered) => answered,
    Err(refusal) => {
      let Ok(outcome) = unreachable(&refusal);

      return Ok(Stage::Finished(outcome));
    }
  };

  let Ok(looked_up) = device::find(&objects, address);

  match looked_up {
    Some(found) => {
      let Ok(asking) = bluez::register(ObjectPath(agent), Capability::DisplayOnly);
      let Ok(()) = call(effects, Method::RegisterAgent, asking);

      Ok(Stage::Registering(found))
    }
    None => Ok(Stage::Finished(Err(Error::Pair {
      name: address.to_string(),
      reason: Reason::OutOfRange,
    }))),
  }
}

fn registered(
  found: Device,
  agent: &str,
  reply: Result<Message, Refusal>,
  effects: &mut Queue<Effect>,
) -> Result<Stage, Never> {
  match reply {
    Ok(_registered) => {
      let Ok(asking) = bluez::request_default(ObjectPath(agent));
      let Ok(()) = call(effects, Method::RequestDefaultAgent, asking);

      Ok(Stage::RequestingDefault(found))
    }
    Err(refusal) => {
      let Ok(outcome) = unreachable(&refusal);

      Ok(Stage::Finished(outcome))
    }
  }
}

fn defaulted(
  found: Device,
  agent: &str,
  reply: Result<Message, Refusal>,
  effects: &mut Queue<Effect>,
) -> Result<Stage, Never> {
  match reply {
    Ok(_defaulted) => {
      let Ok(asking) = device::pair(ObjectPath(&found.path));
      let Ok(()) = call(effects, Method::Pair, asking);

      Ok(Stage::Pairing {
        found,
        pin: Pin::NotRequested,
      })
    }
    Err(refusal) => {
      let Ok(outcome) = unreachable(&refusal);

      unregistering(agent, outcome, effects)
    }
  }
}

fn paired(
  found: Device,
  pin: Pin,
  agent: &str,
  reply: Result<Message, Refusal>,
  effects: &mut Queue<Effect>,
) -> Result<Stage, Never> {
  let Ok(went) = agent::paired(reply.map(|_paired| ()));

  match went {
    Went::Succeeded => {
      let Ok(asking) = device::trust(ObjectPath(&found.path));
      let Ok(()) = call(effects, Method::SetTrusted, asking);

      Ok(Stage::Trusting(found))
    }
    Went::Failed(said) => {
      let reason = match pin {
        Pin::Denied => Reason::NeedsPin,
        Pin::NotRequested | Pin::Given => said,
      };

      unregistering(
        agent,
        Err(Error::Pair {
          name: found.alias,
          reason,
        }),
        effects,
      )
    }
  }
}

fn trusted(
  found: Device,
  agent: &str,
  reply: Result<Message, Refusal>,
  effects: &mut Queue<Effect>,
) -> Result<Stage, Never> {
  match reply {
    Ok(_trusted) => {
      let Ok(asking) = device::connect(ObjectPath(&found.path));
      let Ok(()) = call(effects, Method::Connect, asking);

      Ok(Stage::Connecting(found))
    }
    Err(refusal) => {
      let Ok(reason) = agent::reason(&refusal);

      unregistering(
        agent,
        Err(Error::Trust {
          name: found.alias,
          reason,
        }),
        effects,
      )
    }
  }
}

fn connected(
  found: Device,
  agent: &str,
  reply: Result<Message, Refusal>,
  effects: &mut Queue<Effect>,
) -> Result<Stage, Never> {
  let Ok(went) = agent::connected(reply.map(|_connected| ()));

  match went {
    Went::Succeeded => unregistering(agent, Ok(()), effects),
    Went::Failed(reason) => unregistering(
      agent,
      Err(Error::PairedNotConnected {
        name: found.alias,
        reason,
      }),
      effects,
    ),
  }
}

fn unregistering(
  agent: &str,
  outcome: Outcome,
  effects: &mut Queue<Effect>,
) -> Result<Stage, Never> {
  let Ok(asking) = bluez::unregister(ObjectPath(agent));
  let Ok(()) = call(effects, Method::UnregisterAgent, asking);

  Ok(Stage::Unregistering(outcome))
}

fn lost(state: State, why: &ConnectionError) -> Result<State, Never> {
  let State {
    address,
    agent,
    pins,
    stage,
  } = state;
  let Ok(unreached) = unreachable(why);

  let next = match stage {
    Stage::Pairing { .. }
    | Stage::Finding
    | Stage::Registering(_)
    | Stage::RequestingDefault(_)
    | Stage::Trusting(_)
    | Stage::Connecting(_) => Stage::Finished(unreached),
    Stage::Unregistering(outcome) | Stage::Finished(outcome) => Stage::Finished(outcome),
  };

  Ok(State {
    address,
    agent,
    pins,
    stage: next,
  })
}

fn asked_of(state: State, asked: &Message, effects: &mut Queue<Effect>) -> Result<State, Never> {
  let request = match bluez::request(asked) {
    Ok(request) => request,
    Err(unrecognised) => {
      let Ok(refusal) = bluez::refused(asked, &unrecognised);
      let Ok(()) = reply(effects, refusal);

      return Ok(state);
    }
  };

  match request {
    AgentRequest::Release => {
      let Ok(accepted) = bluez::accepted(asked);
      let Ok(()) = reply(effects, accepted);

      Ok(state)
    }
    AgentRequest::Cancel => {
      let Ok(accepted) = bluez::accepted(asked);
      let Ok(()) = reply(effects, accepted);

      Ok(state)
    }
    AgentRequest::DisplayPasskey {
      device, passkey, ..
    } => {
      let Ok(digits) = passkey.number_for_display();

      shown(state, asked, device, Code::Passkey(digits), effects)
    }
    AgentRequest::DisplayPinCode { device, pin_code } => shown(
      state,
      asked,
      device,
      Code::Pin(pin_code.to_string()),
      effects,
    ),
    AgentRequest::RequestPinCode { device } => pin_asked(state, asked, device, effects),
    AgentRequest::RequestPasskey { .. } | AgentRequest::RequestConfirmation { .. } => {
      let Ok(rejected) = bluez::rejected(asked, Rejection::Rejected);
      let Ok(()) = reply(effects, rejected);

      Ok(state)
    }
    AgentRequest::RequestAuthorization { device }
    | AgentRequest::AuthorizeService { device, .. } => {
      let Ok(answer) = agent::answering(device, state.address);
      let Ok(answered) = answering(asked, answer);
      let Ok(()) = reply(effects, answered);

      Ok(state)
    }
  }
}

fn answering(asked: &Message, answer: Answer) -> Result<Message, Never> {
  match answer {
    Answer::Allow => bluez::accepted(asked),
    Answer::Reject => bluez::rejected(asked, Rejection::Rejected),
  }
}

fn shown(
  state: State,
  asked: &Message,
  device: Address,
  code: Code,
  effects: &mut Queue<Effect>,
) -> Result<State, Never> {
  let State {
    address,
    agent,
    pins,
    stage,
  } = state;
  let Ok(answer) = agent::answering(device, address);

  let next = match stage {
    Stage::Pairing { found, pin } => match answer {
      Answer::Allow => {
        let Ok(line) = cards::entering(&found.alias, &code);
        let Ok(()) = effects.offer(Effect::ShowCode(line));
        let Ok(accepted) = bluez::accepted(asked);
        let Ok(()) = reply(effects, accepted);

        Stage::Pairing { found, pin }
      }
      Answer::Reject => {
        let Ok(rejected) = bluez::rejected(asked, Rejection::Rejected);
        let Ok(()) = reply(effects, rejected);

        Stage::Pairing { found, pin }
      }
    },
    elsewhere @ (Stage::Finding
    | Stage::Registering(_)
    | Stage::RequestingDefault(_)
    | Stage::Trusting(_)
    | Stage::Connecting(_)
    | Stage::Unregistering(_)
    | Stage::Finished(_)) => {
      let Ok(rejected) = bluez::rejected(asked, Rejection::Rejected);
      let Ok(()) = reply(effects, rejected);

      elsewhere
    }
  };

  Ok(State {
    address,
    agent,
    pins,
    stage: next,
  })
}

fn pin_asked(
  state: State,
  asked: &Message,
  device: Address,
  effects: &mut Queue<Effect>,
) -> Result<State, Never> {
  let State {
    address,
    agent,
    pins,
    stage,
  } = state;
  let Ok(answer) = agent::answering(device, address);

  let next = match (stage, answer) {
    (
      Stage::Pairing {
        found,
        pin: _asked_before,
      },
      Answer::Allow,
    ) => {
      let Ok(pin_answer) = agent::pin_answer(&found, &pins);

      match pin_answer {
        PinAnswer::Give(code) => {
          let Ok(given) = bluez::pin_code(asked, &code);
          let Ok(()) = reply(effects, given);

          Stage::Pairing {
            found,
            pin: Pin::Given,
          }
        }
        PinAnswer::Show(code) => {
          let Ok(line) = cards::entering(&found.alias, &Code::Pin(code.to_string()));
          let Ok(()) = effects.offer(Effect::ShowCode(line));
          let Ok(given) = bluez::pin_code(asked, &code);
          let Ok(()) = reply(effects, given);

          Stage::Pairing {
            found,
            pin: Pin::Given,
          }
        }
        PinAnswer::Refuse => {
          let Ok(rejected) = bluez::rejected(asked, Rejection::Rejected);
          let Ok(()) = reply(effects, rejected);

          Stage::Pairing {
            found,
            pin: Pin::Denied,
          }
        }
      }
    }
    (
      elsewhere @ (Stage::Pairing { .. }
      | Stage::Finding
      | Stage::Registering(_)
      | Stage::RequestingDefault(_)
      | Stage::Trusting(_)
      | Stage::Connecting(_)
      | Stage::Unregistering(_)
      | Stage::Finished(_)),
      Answer::Reject,
    )
    | (
      elsewhere @ (Stage::Finding
      | Stage::Registering(_)
      | Stage::RequestingDefault(_)
      | Stage::Trusting(_)
      | Stage::Connecting(_)
      | Stage::Unregistering(_)
      | Stage::Finished(_)),
      Answer::Allow,
    ) => {
      let Ok(rejected) = bluez::rejected(asked, Rejection::Rejected);
      let Ok(()) = reply(effects, rejected);

      elsewhere
    }
  };

  Ok(State {
    address,
    agent,
    pins,
    stage: next,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  use console_bluetooth::agent::PinCode;
  use console_bus::messages::{Kind, Value, Whom};
  use console_bus::properties::PropertyName;
  use console_core_state_machine::run;

  const AGENT: &str = "/console/bluetooth/agent";

  const KEYBOARD: Address = Address([0xE0, 0xC1, 0x02, 0x00, 0xB0, 0xC3]);

  const PATH: &str = "/org/bluez/hci0/dev_E0_C1_02_00_B0_C3";

  const STRANGER_PATH: &str = "/org/bluez/hci0/dev_00_75_B2_D0_78_E6";

  const NAME: &str = "Lab31 - Keyboard";

  type Failure = Box<dyn std::error::Error>;

  const CHOSEN: &str = "482913";

  fn input() -> Result<Input, Never> {
    let Ok(chosen) = PinCode::read(CHOSEN);

    Ok(Input {
      address: KEYBOARD,
      agent: AGENT.to_string(),
      pins: Pins {
        typed: None,
        chosen,
      },
    })
  }

  fn typed(entered: PinCode) -> Result<Input, Never> {
    let Ok(input) = input();

    Ok(Input {
      pins: Pins {
        typed: Some(entered),
        ..input.pins
      },
      ..input
    })
  }

  fn entry(name: &str, value: Value) -> Result<Value, Never> {
    let Ok(word) = Value::word(name);

    Ok(Value::Group(vec![word, value]))
  }

  fn said(name: PropertyName<'_>, text: &str) -> Result<Value, Never> {
    let Ok(word) = Value::word(text);
    let Ok(variant) = Value::variant("s", word);

    entry(name.0, variant)
  }

  fn answered(to: Method, values: Vec<Value>) -> Result<Request, Never> {
    let reply = Message {
      kind: Kind::Answer,
      values,
      ..Message::default()
    };

    Ok(Request::Received(Received::Reply { to, reply }))
  }

  fn known_as(icon: Option<&str>) -> Result<Request, Never> {
    let Ok(address) = said(PropertyName(device::ADDRESS), "E0:C1:02:00:B0:C3");
    let Ok(alias) = said(PropertyName(device::ALIAS), NAME);
    let drawn = icon.map(|named| {
      let Ok(drawn) = said(PropertyName(device::ICON), named);

      drawn
    });
    let held: Vec<Value> = [address, alias].into_iter().chain(drawn).collect();
    let Ok(properties) = entry(device::INTERFACE, Value::List(held));
    let keyboard = Value::Group(vec![
      Value::Path(PATH.to_string()),
      Value::List(vec![properties]),
    ]);

    answered(Method::GetManagedObjects, vec![Value::List(vec![keyboard])])
  }

  fn failed(to: Method, name: &str) -> Result<Request, Never> {
    let Ok(said) = Value::word("from bluez");
    let error = Message {
      kind: Kind::ErrorReply,
      fault: Some(name.to_string()),
      values: vec![said],
      ..Message::default()
    };

    Ok(Request::Received(Received::Error { to, error }))
  }

  fn bluez_asks(method: &str, values: Vec<Value>) -> Result<(Message, Request), Never> {
    let shape = match method {
      "DisplayPasskey" => "ouq",
      "AuthorizeService" => "os",
      _taking_only_the_device => "o",
    };

    let Ok(mut asked) = Message::call(&Whom {
      to: ":1.40",
      at: AGENT,
      on: bluez::INTERFACE,
      calling: method,
    });

    asked.serial = 30;
    asked.sender = Some(":1.3".to_string());

    let Ok(carried) = asked.carrying(shape, values);

    Ok((carried.clone(), Request::Received(Received::Call(carried))))
  }

  fn up_to_pairing() -> Result<Vec<Request>, Never> {
    up_to_pairing_as(None)
  }

  fn up_to_pairing_as(icon: Option<&str>) -> Result<Vec<Request>, Never> {
    let Ok(known) = known_as(icon);
    let Ok(registered) = answered(Method::RegisterAgent, Vec::new());
    let Ok(defaulted) = answered(Method::RequestDefaultAgent, Vec::new());

    Ok(vec![known, registered, defaulted])
  }

  fn pin(written: &str) -> Result<PinCode, Failure> {
    let Ok(read) = PinCode::read(written);

    let pin = read.ok_or("the test's PIN is one bluez would take")?;

    Ok(pin)
  }

  fn calls(effects: &[Effect]) -> Result<Vec<Method>, Never> {
    Ok(
      effects
        .iter()
        .filter_map(|effect| match effect {
          Effect::Call { method, .. } => Some(*method),
          Effect::Reply(_) | Effect::ShowCode(_) => None,
        })
        .collect(),
    )
  }

  #[test]
  fn headphones_that_ask_nothing_are_paired_trusted_connected_and_let_go_of()
  -> Result<(), Failure> {
    let Ok(mut requests) = up_to_pairing();
    let Ok(paired) = answered(Method::Pair, Vec::new());
    let Ok(trusted) = answered(Method::SetTrusted, Vec::new());
    let Ok(connected) = answered(Method::Connect, Vec::new());
    let Ok(unregistered) = answered(Method::UnregisterAgent, Vec::new());

    requests.extend([paired, trusted, connected, unregistered]);

    let Ok(input) = input();
    let trace = run::<Introduction>(&input, &requests)?;
    let Ok(effects) = trace.effects();
    let Ok(made) = calls(&effects);
    let Ok(pairing) = device::pair(ObjectPath(PATH));

    assert_eq!(
      calls(&trace.initialized),
      Ok(vec![Method::GetManagedObjects])
    );
    assert_eq!(
      made,
      vec![
        Method::RegisterAgent,
        Method::RequestDefaultAgent,
        Method::Pair,
        Method::SetTrusted,
        Method::Connect,
        Method::UnregisterAgent,
      ]
    );
    assert!(
      effects.contains(&Effect::Call {
        method: Method::Pair,
        message: pairing
      }),
      "the pairing is asked of the device that was found"
    );
    assert_eq!(trace.state.stage, Stage::Finished(Ok(())));

    Ok(())
  }

  #[test]
  fn a_keyboard_s_code_is_said_when_bluez_asks_and_the_pairing_goes_on() -> Result<(), Failure> {
    let Ok(mut requests) = up_to_pairing();
    let Ok((shown, asked)) = bluez_asks(
      "DisplayPasskey",
      vec![
        Value::Path(PATH.to_string()),
        Value::Unsigned32(1234),
        Value::Unsigned16(0),
      ],
    );
    let Ok(paired) = answered(Method::Pair, Vec::new());

    requests.extend([asked, paired]);

    let Ok(input) = input();
    let trace = run::<Introduction>(&input, &requests)?;
    let Ok(line) = cards::entering(NAME, &Code::Passkey(1234));
    let Ok(accepted) = bluez::accepted(&shown);
    let Ok(trusting) = device::trust(ObjectPath(PATH));

    let Ok(asked_to_show) = trace.on(3);
    let Ok(pairing_done) = trace.on(4);

    assert_eq!(
      asked_to_show,
      Some([Effect::ShowCode(line), Effect::Reply(accepted)].as_slice())
    );
    assert_eq!(
      pairing_done,
      Some(
        [Effect::Call {
          method: Method::SetTrusted,
          message: trusting
        }]
        .as_slice()
      )
    );

    Ok(())
  }

  #[test]
  fn a_stranger_asking_in_while_a_device_pairs_is_turned_away() -> Result<(), Failure> {
    let Ok(mut requests) = up_to_pairing();
    let Ok((theirs, asked)) = bluez_asks(
      "AuthorizeService",
      vec![
        Value::Path(STRANGER_PATH.to_string()),
        Value::Word("0000110b-0000-1000-8000-00805f9b34fb".to_string()),
      ],
    );
    let Ok((ours, ours_asked)) =
      bluez_asks("RequestAuthorization", vec![Value::Path(PATH.to_string())]);
    let Ok((code, code_asked)) = bluez_asks(
      "DisplayPasskey",
      vec![
        Value::Path(STRANGER_PATH.to_string()),
        Value::Unsigned32(1),
        Value::Unsigned16(0),
      ],
    );

    requests.extend([asked, ours_asked, code_asked]);

    let Ok(input) = input();
    let trace = run::<Introduction>(&input, &requests)?;
    let Ok(turned_away) = bluez::rejected(&theirs, Rejection::Rejected);
    let Ok(let_in) = bluez::accepted(&ours);
    let Ok(not_shown) = bluez::rejected(&code, Rejection::Rejected);

    assert_eq!(answering(&theirs, Answer::Reject), Ok(turned_away.clone()));
    let Ok(stranger_asked) = trace.on(3);
    let Ok(ours_answered) = trace.on(4);
    let Ok(stranger_code) = trace.on(5);

    assert_eq!(
      stranger_asked,
      Some([Effect::Reply(turned_away)].as_slice())
    );
    assert_eq!(ours_answered, Some([Effect::Reply(let_in)].as_slice()));
    assert_eq!(
      stranger_code,
      Some([Effect::Reply(not_shown)].as_slice()),
      "no card is raised with a stranger's code on it"
    );

    Ok(())
  }

  #[test]
  fn an_old_keyboard_is_given_the_drawn_digits_and_they_are_put_up_to_type() -> Result<(), Failure>
  {
    let Ok(mut requests) = up_to_pairing_as(Some("input-keyboard"));
    let Ok((pin_asked, asked)) =
      bluez_asks("RequestPinCode", vec![Value::Path(PATH.to_string())]);

    requests.push(asked);

    let Ok(input) = input();
    let trace = run::<Introduction>(&input, &requests)?;
    let Ok(line) = cards::entering(NAME, &Code::Pin(CHOSEN.to_string()));
    let drawn = pin(CHOSEN)?;
    let Ok(given) = bluez::pin_code(&pin_asked, &drawn);
    let Ok(answered_pin) = trace.on(3);

    assert_eq!(
      answered_pin,
      Some([Effect::ShowCode(line), Effect::Reply(given)].as_slice())
    );

    Ok(())
  }

  #[test]
  fn old_headphones_are_given_the_pin_printed_in_their_manual() -> Result<(), Failure> {
    let Ok(mut requests) = up_to_pairing_as(Some("audio-headset"));
    let Ok((pin_asked, asked)) =
      bluez_asks("RequestPinCode", vec![Value::Path(PATH.to_string())]);

    requests.push(asked);

    let Ok(input) = input();
    let trace = run::<Introduction>(&input, &requests)?;
    let printed = pin(agent::HEADPHONES_PIN)?;
    let Ok(given) = bluez::pin_code(&pin_asked, &printed);
    let Ok(answered_pin) = trace.on(3);

    assert_eq!(
      answered_pin,
      Some([Effect::Reply(given)].as_slice()),
      "nothing is put up for headphones to type"
    );

    Ok(())
  }

  #[test]
  fn a_pin_somebody_typed_is_the_pin_given() -> Result<(), Failure> {
    let Ok(mut requests) = up_to_pairing();
    let Ok((pin_asked, asked)) =
      bluez_asks("RequestPinCode", vec![Value::Path(PATH.to_string())]);
    let Ok(paired) = answered(Method::Pair, Vec::new());

    requests.extend([asked, paired]);

    let entered = pin("1234")?;
    let Ok(input) = typed(entered.clone());
    let trace = run::<Introduction>(&input, &requests)?;
    let Ok(given) = bluez::pin_code(&pin_asked, &entered);
    let Ok(answered_pin) = trace.on(3);
    let Ok(effects) = trace.effects();
    let Ok(made) = calls(&effects);

    assert_eq!(answered_pin, Some([Effect::Reply(given)].as_slice()));
    assert!(
      made.contains(&Method::SetTrusted),
      "a pairing the PIN let through goes on to be trusted"
    );

    Ok(())
  }

  #[test]
  fn a_stranger_asking_for_a_pin_is_given_none() -> Result<(), Failure> {
    let Ok(mut requests) = up_to_pairing_as(Some("audio-headset"));
    let Ok((pin_asked, asked)) = bluez_asks(
      "RequestPinCode",
      vec![Value::Path(STRANGER_PATH.to_string())],
    );

    requests.push(asked);

    let entered = pin("1234")?;
    let Ok(input) = typed(entered);
    let trace = run::<Introduction>(&input, &requests)?;
    let Ok(turned_away) = bluez::rejected(&pin_asked, Rejection::Rejected);
    let Ok(answered_pin) = trace.on(3);

    assert_eq!(
      answered_pin,
      Some([Effect::Reply(turned_away)].as_slice()),
      "the PIN typed for one device is not given to another"
    );

    Ok(())
  }

  #[test]
  fn a_device_that_wants_a_pin_typed_is_said_to_want_one() -> Result<(), Failure> {
    let Ok(mut requests) = up_to_pairing();
    let Ok((_pin, asked)) = bluez_asks("RequestPinCode", vec![Value::Path(PATH.to_string())]);
    let Ok(refused) = failed(Method::Pair, "org.bluez.Error.AuthenticationFailed");
    let Ok(unregistered) = answered(Method::UnregisterAgent, Vec::new());

    requests.extend([asked, refused, unregistered]);

    let Ok(input) = input();
    let trace = run::<Introduction>(&input, &requests)?;

    assert_eq!(
      trace.state.stage,
      Stage::Finished(Err(Error::Pair {
        name: NAME.to_string(),
        reason: Reason::NeedsPin
      }))
    );

    Ok(())
  }

  #[test]
  fn a_device_bluez_has_not_heard_is_out_of_range_and_no_agent_is_registered()
  -> Result<(), Failure> {
    let Ok(nothing) = answered(Method::GetManagedObjects, vec![Value::List(Vec::new())]);
    let Ok(input) = input();
    let trace = run::<Introduction>(&input, &[nothing])?;
    let Ok(effects) = trace.effects();

    assert_eq!(effects, Vec::new());
    assert_eq!(
      trace.state.stage,
      Stage::Finished(Err(Error::Pair {
        name: "E0:C1:02:00:B0:C3".to_string(),
        reason: Reason::OutOfRange
      }))
    );

    Ok(())
  }

  #[test]
  fn a_device_that_will_not_connect_still_lets_the_agent_go() -> Result<(), Failure> {
    let Ok(mut requests) = up_to_pairing();
    let Ok(paired) = answered(Method::Pair, Vec::new());
    let Ok(trusted) = answered(Method::SetTrusted, Vec::new());
    let Ok(refused) = failed(Method::Connect, "org.bluez.Error.Failed");
    let Ok(unregistered) = failed(Method::UnregisterAgent, "org.bluez.Error.DoesNotExist");

    requests.extend([paired, trusted, refused, unregistered]);

    let Ok(input) = input();
    let trace = run::<Introduction>(&input, &requests)?;

    let Ok(refusal_heard) = trace.on(5);

    assert_eq!(
      refusal_heard.map(calls),
      Some(Ok(vec![Method::UnregisterAgent]))
    );
    assert_eq!(
      trace.state.stage,
      Stage::Finished(Err(Error::PairedNotConnected {
        name: NAME.to_string(),
        reason: Reason::Message("from bluez".to_string())
      }))
    );

    Ok(())
  }

  #[test]
  fn the_bus_going_away_mid_pairing_ends_the_introduction_and_its_code_with_it()
  -> Result<(), Failure> {
    let Ok(mut requests) = up_to_pairing();
    let Ok((_shown, asked)) = bluez_asks(
      "DisplayPasskey",
      vec![
        Value::Path(PATH.to_string()),
        Value::Unsigned32(42),
        Value::Unsigned16(0),
      ],
    );

    requests.extend([asked, Request::Lost(ConnectionError::Ended)]);

    let Ok(input) = input();
    let trace = run::<Introduction>(&input, &requests)?;

    assert_eq!(
      trace.state.stage,
      Stage::Finished(Err(Error::Unreachable("the bus hung up".to_string())))
    );

    Ok(())
  }

  #[test]
  fn a_reply_the_stage_is_not_waiting_on_changes_nothing() -> Result<(), Failure> {
    let Ok(late) = answered(Method::Connect, Vec::new());
    let Ok(input) = input();
    let trace = run::<Introduction>(&input, &[late])?;
    let Ok(effects) = trace.effects();

    assert_eq!(effects, Vec::new());
    assert_eq!(trace.state.stage, Stage::Finding);

    Ok(())
  }
}
