//! What the Bluetooth tab asks of this machine, carried to bluez.
//!
//!     console-bluetooth on | off | search
//!     console-bluetooth introduce [--pin PIN] AA:BB:CC:DD:EE:FF
//!     console-bluetooth connect | disconnect | forget AA:BB:CC:DD:EE:FF
//!
//! What each of those asks of bluez and what its answers mean is the
//! library's, and can be pressed without a radio. This is the part that needs
//! one: a connection to the system bus, the calls carried down it, what comes
//! back handed back, a code said as a line while the pairing waits on it, and
//! the card when it did not work, written on the way out.
//!
//! It exists because a panel row runs one program with one list of words, and
//! meeting a device is three steps and a conversation. Someone pressing a
//! keyboard's name means the keyboard to work, and a machine that made them
//! press Pair, then Trust, then Connect would be asking them to know what bluez
//! calls the halves of that. The other rows came here after it, off
//! `bluetoothctl`, so that the tab speaks to bluez one way.
//!
//! Silent when it worked. The row it was pressed from redraws into the device
//! being there, which is the whole of what anyone wanted to see. Failure is the
//! other way round -- the row goes back to reading the way it did, which on its
//! own is indistinguishable from a press that never landed -- so what went
//! wrong is carried out to where someone can read it, and into the journal for
//! whoever reads it afterwards. Where someone can read it is the panel's to
//! decide, because the panel knows whether a card is up and a notification
//! raised from here does not: the card is written on the error output, and the
//! row that started this reads it back with `cards::told` and says it on the
//! card it was pressed on, or as a notification once there is no card. A row
//! also says what this is doing while it runs, and will not start it a second
//! time, because the panel keeps what its presses started.
//!
//! **An introduction waits once, and on no timer.** Each call goes out and the
//! serial it went under is written down beside its name; then the program
//! waits on the socket for the next whole message, which is a reply to one of
//! those calls or bluez calling the agent, and hands it to the machine. Every
//! call bluez is given ends in bluez's own time, with an answer or an error --
//! a page timeout, an authentication timeout -- and the bus answers for a bluez
//! that went away. A deadline here would be a second guess at how long a radio
//! takes. When the introduction ends the connection goes, the agent with it if
//! bluez had not already let it go, and the radio is unpairable again.
//!
//! **A search is the one action that lasts.** bluez searches for as long as
//! the connection that asked is on the bus and stops the moment it leaves, so
//! the program stays for the length of a search, asking now and then whether
//! the radio is still searching -- turning Bluetooth off ends every search, and
//! the program goes with it. Ten minutes is the length bluetoothctl was asked
//! for when the tab ran it, and longer than anybody stands over a tab waiting
//! for a pair of headphones to appear. The elapsing is what was asked for, so
//! it is `console-waiting`'s patience rather than a sleep.
//!
//! The tab runs a search as its watch, for as long as it is the tab in front,
//! and that is most of the searches there are -- including every time the tab
//! is opened with Bluetooth off. So a search with no radio on to search with is
//! one that has already ended, the way turning the radio off ends one, and it
//! says nothing: a card each time somebody looked at a switch that reads Off
//! would be news about the switch they were looking at.
//!
//! **The digits a keyboard is given are drawn here,** from the kernel, because
//! a machine handed them can be pressed in a test and one that drew them could
//! not. A kernel that will not give six random digits gives a keyboard none,
//! and the pairing says it wanted a PIN rather than going ahead with a guessable
//! one.

use std::process::ExitCode;
use std::time::Duration;

use console_bluetooth::adapter::{self, Discovery, Power};
use console_bluetooth::agent::PinCode;
use console_bluetooth::errors::Error as Refusal;
use console_bluetooth::objects::{self, Objects};
use console_bluetooth_agent::agent::{self, Pins};
use console_bluetooth_agent::calls::{self, Calls};
use console_bluetooth_agent::cards;
use console_bluetooth_agent::command::{self, Action, Once};
use console_bluetooth_agent::errors::Error;
use console_bluetooth_agent::introduction::{
  self, Effect, Input, Introduction, Method, Outcome, Request, State,
};
use console_bus::connection::{Bus, ConnectionError};
use console_bus::messages::Message;
use console_bus::properties::ObjectPath;
use console_bus::replies::Pending;
use console_core_iteration::{Endless, Step, iterate};
use console_core_media_access_control_addresses::Address;
use console_core_never::Never;
use console_core_state_machine::{Machine, Transition};
use console_notifications::saying::journal;
use console_waiting::{Ready, Schedule, until_handed};
use rustix::rand::{GetRandomFlags, getrandom};

const AGENT: &str = "/console/bluetooth/agent";

const SEARCH: Duration = Duration::from_secs(600);

const BETWEEN_LOOKS: Duration = Duration::from_secs(5);

const PIN_DIGITS: u32 = 1_000_000;

struct Running {
  bus: Bus,
  pending: Pending<Method>,
  state: State,
  lost: Option<ConnectionError>,
}

fn first(
  lost: Option<ConnectionError>,
  why: ConnectionError,
) -> Result<Option<ConnectionError>, Never> {
  Ok(match lost {
    Some(earlier) => Some(earlier),
    None => Some(why),
  })
}

fn carry(running: Running, effect: Effect) -> Result<Running, Never> {
  let Running {
    mut bus,
    mut pending,
    state,
    lost: earlier,
  } = running;

  let lost = match effect {
    Effect::Call { method, message } => match bus.say(&message) {
      Ok(serial) => {
        let Ok(()) = pending.sent(serial, method);

        earlier
      }
      Err(why) => {
        let Ok(kept) = first(earlier, why);

        kept
      }
    },
    Effect::Reply(message) => match bus.say(&message) {
      Ok(_nothing_answers_a_reply) => earlier,
      Err(why) => {
        let Ok(kept) = first(earlier, why);

        kept
      }
    },
    Effect::ShowCode(line) => {
      println!("{line}");

      earlier
    }
  };

  Ok(Running {
    bus,
    pending,
    state,
    lost,
  })
}

fn carried(running: Running, effects: Vec<Effect>) -> Result<Running, Never> {
  effects.into_iter().try_fold(running, carry)
}

fn turn(running: Running) -> Result<Step<Running, Outcome>, Never> {
  let Ok(ended) = introduction::ended(&running.state);

  match ended {
    Some(outcome) => Ok(Step::Halt(outcome)),
    None => {
      let Running {
        mut bus,
        mut pending,
        state: before,
        lost,
      } = running;

      let request = match lost {
        Some(why) => Request::Lost(why),
        None => match bus.receive() {
          Ok(message) => {
            let Ok(received) = pending.received(message);

            Request::Received(received)
          }
          Err(why) => Request::Lost(why),
        },
      };

      let Ok(Transition { state, effects }) = Introduction::transition(before, request);
      let Ok(next) = carried(
        Running {
          bus,
          pending,
          state,
          lost: None,
        },
        effects,
      );

      Ok(Step::Again(next))
    }
  }
}

fn introduce(bus: Bus, address: Address, pins: Pins) -> Result<Outcome, Never> {
  let input = Input {
    address,
    agent: AGENT.to_string(),
    pins,
  };
  let Ok(Transition { state, effects }) = Introduction::initial_transition(&input, None);
  let Ok(pending) = Pending::new();
  let Ok(running) = carried(
    Running {
      bus,
      pending,
      state,
      lost: None,
    },
    effects,
  );

  Ok(match iterate(running, turn) {
    Ok(outcome) => outcome,
    Err(Endless) => Err(Error::Unreachable(Endless.to_string())),
  })
}

#[cfg_attr(
  dylint_lib = "explicit077_no_ambient_randomness",
  allow(
    explicit077_no_ambient_randomness,
    reason = "this is the edge the rule asks for: the digits are drawn here, once, and handed into the introduction as its input, so the machine that answers bluez can be pressed with any digits a test likes"
  )
)]
fn chosen() -> Result<Option<PinCode>, Never> {
  let mut drawn = [0u8; 4];

  match getrandom(&mut drawn, GetRandomFlags::empty()) {
    Ok(_four_bytes_since_getrandom_fills_any_request_this_short_whole) => {}
    Err(_no_randomness) => return Ok(None),
  }

  let digits = match u32::from_le_bytes(drawn).checked_rem(PIN_DIGITS) {
    Some(digits) => digits,
    None => return Ok(None),
  };

  PinCode::read(&format!("{digits:06}"))
}

fn called(bus: &mut Bus, message: &Message) -> Result<Result<(), Refusal>, ConnectionError> {
  let answered = called_for(bus, message)?;

  Ok(answered.map(|_answer| ()))
}

fn known(bus: &mut Bus) -> Result<Result<Objects, Error>, Never> {
  let Ok(managed) = objects::managed();

  Ok(match called_for(bus, &managed) {
    Ok(Ok(answer)) => {
      let Ok(read) = objects::read(&answer);

      Ok(read)
    }
    Ok(Err(refusal)) => Err(Error::Unreachable(refusal.to_string())),
    Err(why) => Err(Error::Unreachable(why.to_string())),
  })
}

fn called_for(
  bus: &mut Bus,
  message: &Message,
) -> Result<Result<Message, Refusal>, ConnectionError> {
  let answer = bus.call(message)?;

  Ok(match &answer.fault {
    Some(_named) => {
      let Ok(refusal) = Refusal::of(&answer);

      Err(refusal)
    }
    None => Ok(answer),
  })
}

fn once_only(mut bus: Bus, once: &Once) -> Result<Outcome, Never> {
  let Ok(read) = known(&mut bus);

  let objects = match read {
    Ok(objects) => objects,
    Err(error) => return Ok(Err(error)),
  };

  let Ok(planned) = calls::calls(&objects, once);

  let (name, messages) = match planned {
    Ok(Calls::Needed { name, messages }) => (name, messages),
    Ok(Calls::Unneeded) => return Ok(Ok(())),
    Err(error) => return Ok(Err(error)),
  };

  let went = messages.iter().try_fold(bus, |mut asking, message| {
    match called(&mut asking, message) {
      Ok(reply) => {
        let Ok(outcome) = calls::answered(once, &name, reply);

        outcome.map(|()| asking)
      }
      Err(why) => Err(Error::Unreachable(why.to_string())),
    }
  });

  Ok(went.map(|_finished| ()))
}

fn still_searching(bus: &mut Bus) -> Result<Ready, Never> {
  let Ok(read) = known(bus);

  let objects = match read {
    Ok(objects) => objects,
    Err(_gone) => return Ok(Ready::Yes),
  };

  let Ok(discovery) = objects.discovery();

  Ok(match discovery {
    Discovery::Searching => Ready::NotYet,
    Discovery::NotSearching => Ready::Yes,
  })
}

fn search(mut bus: Bus) -> Result<Outcome, Never> {
  let Ok(read) = known(&mut bus);

  let objects = match read {
    Ok(objects) => objects,
    Err(error) => return Ok(Err(error)),
  };

  let radios: Vec<String> = objects
    .adapters
    .iter()
    .filter(|radio| radio.power == Power::On)
    .map(|radio| radio.path.clone())
    .collect();

  match ::core::primitive::bool::then_some(radios.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(Ok(())),
    ::core::option::Option::None => {}
  }

  let started = radios.iter().try_fold(bus, |mut asking, radio| {
    let Ok(starting) = adapter::start_discovery(ObjectPath(radio));

    match called(&mut asking, &starting) {
      Ok(Ok(())) => Ok(asking),
      Ok(Err(refusal)) => {
        let Ok(reason) = agent::reason(&refusal);

        Err(Error::Search { reason })
      }
      Err(why) => Err(Error::Unreachable(why.to_string())),
    }
  });

  let mut searching = match started {
    Ok(searching) => searching,
    Err(error) => return Ok(Err(error)),
  };

  let Ok(patience) = Schedule::asking_every(SEARCH, BETWEEN_LOOKS);
  let Ok(_ended_either_way) = until_handed(patience, &mut searching, still_searching);

  Ok(Ok(()))
}

fn said_wrong(error: &Error) -> Result<ExitCode, Never> {
  let card = cards::card(error)?;
  let Ok(written) = cards::written(&card);

  eprintln!("{written}");

  let Ok(()) = journal(&error.to_string());

  Ok(ExitCode::from(1))
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let action = match command::read(&words) {
    Ok(action) => action,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  let bus = match Bus::system() {
    Ok(bus) => bus,
    Err(why) => {
      let Ok(code) = said_wrong(&Error::Unreachable(why.to_string()));

      return code;
    }
  };

  let Ok(went) = match action {
    Action::Introduce { address, pin } => {
      let Ok(drawn) = chosen();

      introduce(
        bus,
        address,
        Pins {
          typed: pin,
          chosen: drawn,
        },
      )
    }
    Action::Search => search(bus),
    Action::Once(once) => once_only(bus, &once),
  };

  match went {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      let Ok(code) = said_wrong(&error);

      code
    }
  }
}
