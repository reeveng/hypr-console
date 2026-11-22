//! What the program is told, and the line a row on the tab runs to tell it.
//!
//! Every row on the Bluetooth tab is a command line, and every one of them was
//! `bluetoothctl` until the tab moved onto `console-bluetooth`: a program that
//! holds a connection to bluez for the length of one word, and that registers
//! no agent when its command is on its own line, which is why pairing was the
//! one row it could not do. They are all this program now, so what the tab can
//! ask of the machine is one list, in one file, in this desktop's words.
//!
//! An action is written and read here and nowhere else. `command_line` is what
//! a row runs and `read` is what the program makes of it, and a test sends
//! every action through both, so the tab and the program cannot come to
//! disagree about a word the way the tab and bluetoothctl did -- the tab began
//! its pairing line with the program's name a second time, and bluetoothctl
//! answered with its help.
//!
//! A PIN rides on the introduction that needs one, as `--pin`, because the tab
//! asks for it before the pairing starts: bluez says ahead of time which
//! devices can only pair by one.
//!
//! A row hands the panel `of`, the action as the contract's `Command`, rather
//! than the words: the panel keeps what its presses started by that value, so
//! a pairing that is already running is one the row draws as in progress and
//! a second press does not start. `command_line` is the same action as the
//! words a watch is started with, and the program's name in both is
//! `InternalProgram::Bluetooth`'s.

use console_bluetooth::adapter::Power;
use console_bluetooth::agent::PinCode;
use console_core_arguments::{
  Command, CommandLine, Flag, Operands, Reason as Refused, Subcommand, Takes, ValidationError,
  read_with,
};
use console_core_internal_programs::InternalProgram;
use console_core_media_access_control_addresses::Address;
use console_core_never::Never;
use console_core_words::Words;
use console_program_contract::Executable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Bluetooth {
  #[words(word = "on", about = "turn Bluetooth on")]
  On,
  #[words(word = "off", about = "turn Bluetooth off")]
  Off,
  #[words(
    word = "search",
    about = "look for devices nearby until the search ends or the radio goes off"
  )]
  Search,
  #[words(
    word = "introduce",
    about = "pair with the device at ADDRESS, trust it and connect it"
  )]
  Introduce,
  #[words(word = "connect", about = "connect the paired device at ADDRESS")]
  Connect,
  #[words(word = "disconnect", about = "disconnect the device at ADDRESS")]
  Disconnect,
  #[words(word = "forget", about = "unpair the device at ADDRESS and forget it")]
  Forget,
}

impl Subcommand for Bluetooth {
  fn variants() -> Result<impl Iterator<Item = Self>, Never> {
    Ok(Bluetooth::VARIANTS.iter().copied())
  }

  fn spelling(self) -> Result<&'static str, Never> {
    self.word()
  }

  fn about(self) -> Result<&'static str, Never> {
    Bluetooth::about(self)
  }
}

pub const ADDRESS: [&str; 1] = ["ADDRESS"];

pub const PIN: Flag = Flag {
  spelling: "--pin",
  takes: Takes::Value("PIN"),
  about: "the PIN to give a device that can only pair by one",
};

pub const COMMAND: Command = Command {
  name: "console-bluetooth",
  about: "what the Bluetooth tab asks of this machine",
  flags: &[PIN],
  operands: Operands::Optional("ADDRESS"),
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Once {
  Power(Power),
  Connect(Address),
  Disconnect(Address),
  Forget(Address),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
  Search,
  Introduce {
    address: Address,
    pin: Option<PinCode>,
  },
  Once(Once),
}

pub fn of(action: &Action) -> Result<console_program_contract::Command, Never> {
  let Ok(arguments) = arguments(action);

  Ok(console_program_contract::Command {
    program: Executable::Internal(InternalProgram::Bluetooth),
    arguments,
  })
}

pub fn command_line(action: &Action) -> Result<Vec<String>, Never> {
  let Ok(name) = InternalProgram::Bluetooth.name();
  let Ok(arguments) = arguments(action);

  Ok(std::iter::once(name.to_string()).chain(arguments).collect())
}

fn arguments(action: &Action) -> Result<Vec<String>, Never> {
  let (subcommand, rest) = match action {
    Action::Search => (Bluetooth::Search, Vec::new()),
    Action::Introduce {
      address,
      pin: Some(pin),
    } => (
      Bluetooth::Introduce,
      vec![
        PIN.spelling.to_string(),
        pin.to_string(),
        address.to_string(),
      ],
    ),
    Action::Introduce { address, pin: None } => (Bluetooth::Introduce, vec![address.to_string()]),
    Action::Once(Once::Power(Power::On)) => (Bluetooth::On, Vec::new()),
    Action::Once(Once::Power(Power::Off)) => (Bluetooth::Off, Vec::new()),
    Action::Once(Once::Connect(address)) => (Bluetooth::Connect, vec![address.to_string()]),
    Action::Once(Once::Disconnect(address)) => (Bluetooth::Disconnect, vec![address.to_string()]),
    Action::Once(Once::Forget(address)) => (Bluetooth::Forget, vec![address.to_string()]),
  };

  let Ok(word) = subcommand.word();

  Ok(std::iter::once(word.to_string()).chain(rest).collect())
}

pub fn read(words: &[String]) -> Result<Action, ValidationError> {
  let read = read_with::<Bluetooth, String>(&COMMAND, words);
  let line = read?;
  let required = line.require_subcommand();
  let subcommand = required?;

  match subcommand {
    Bluetooth::On => alone(&line, Action::Once(Once::Power(Power::On))),
    Bluetooth::Off => alone(&line, Action::Once(Once::Power(Power::Off))),
    Bluetooth::Search => alone(&line, Action::Search),
    Bluetooth::Introduce => introduced(&line),
    Bluetooth::Connect => {
      let address = about(&line)?;

      Ok(Action::Once(Once::Connect(address)))
    }
    Bluetooth::Disconnect => {
      let address = about(&line)?;

      Ok(Action::Once(Once::Disconnect(address)))
    }
    Bluetooth::Forget => {
      let address = about(&line)?;

      Ok(Action::Once(Once::Forget(address)))
    }
  }
}

fn unpinned(line: &CommandLine<Bluetooth>) -> Result<(), ValidationError> {
  let Ok(given) = line.value(PIN);

  match given {
    Some(_pin) => {
      let Ok(refusal) = line.refusal(Refused::ExtraArgument(PIN.spelling.to_string()));

      Err(refusal)
    }
    None => Ok(()),
  }
}

fn alone(line: &CommandLine<Bluetooth>, action: Action) -> Result<Action, ValidationError> {
  unpinned(line)?;

  let operands = line.exactly([]);
  let [] = operands?;

  Ok(action)
}

fn address(line: &CommandLine<Bluetooth>) -> Result<Address, ValidationError> {
  let operands = line.exactly(ADDRESS);
  let [written] = operands?;

  match written.parse::<Address>() {
    Ok(address) => Ok(address),
    Err(_not_an_address) => {
      let Ok(refusal) = line.refusal(Refused::InvalidValue {
        of: "ADDRESS",
        value: written.clone(),
      });

      Err(refusal)
    }
  }
}

fn about(line: &CommandLine<Bluetooth>) -> Result<Address, ValidationError> {
  unpinned(line)?;

  address(line)
}

fn introduced(line: &CommandLine<Bluetooth>) -> Result<Action, ValidationError> {
  let address = address(line)?;
  let Ok(given) = line.value(PIN);

  let pin = match given {
    Some(written) => {
      let Ok(read) = PinCode::read(written);

      match read {
        Some(pin) => Some(pin),
        None => {
          let Ok(refusal) = line.refusal(Refused::InvalidValue {
            of: "PIN",
            value: written.to_string(),
          });

          return Err(refusal);
        }
      }
    }
    None => None,
  };

  Ok(Action::Introduce { address, pin })
}

#[cfg(test)]
mod tests {
  use super::*;

  const MS218: Address = Address([0xE0, 0xC1, 0x02, 0x00, 0xB0, 0xC3]);

  fn words(said: &[&str]) -> Result<Vec<String>, Never> {
    Ok(said.iter().map(|word| (*word).to_string()).collect())
  }

  fn refused(said: &[&str]) -> Result<Option<Refused>, Never> {
    let Ok(line) = words(said);

    Ok(match read(&line) {
      Ok(_understood) => None,
      Err(refusal) => Some(refusal.reason),
    })
  }

  #[test]
  fn every_action_a_row_can_run_is_read_back_as_the_same_action()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(pin) = PinCode::read("0000");
    let actions = vec![
      Action::Search,
      Action::Introduce {
        address: MS218,
        pin: None,
      },
      Action::Introduce {
        address: MS218,
        pin,
      },
      Action::Once(Once::Power(Power::On)),
      Action::Once(Once::Power(Power::Off)),
      Action::Once(Once::Connect(MS218)),
      Action::Once(Once::Disconnect(MS218)),
      Action::Once(Once::Forget(MS218)),
    ];

    for action in actions {
      let Ok(line) = command_line(&action);
      let (program, rest) = line.split_first().ok_or("a row's line is never empty")?;

      assert_eq!(
        program, COMMAND.name,
        "the tab runs the program by the name it is built under"
      );
      assert_eq!(read(rest), Ok(action));
    }

    Ok(())
  }

  #[test]
  fn the_program_a_row_runs_is_the_one_this_reads_for_and_the_one_the_line_names()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(name) = InternalProgram::Bluetooth.name();
    let action = Action::Once(Once::Connect(MS218));
    let Ok(pressed) = of(&action);
    let Ok(line) = command_line(&action);
    let (program, rest) = line.split_first().ok_or("a row's line is never empty")?;

    assert_eq!(
      COMMAND.name, name,
      "the help names the program the panel runs"
    );
    assert_eq!(program, name);
    assert_eq!(
      pressed.program,
      Executable::Internal(InternalProgram::Bluetooth)
    );
    assert_eq!(
      pressed.arguments, rest,
      "a press and a watch are the same words"
    );

    Ok(())
  }

  #[test]
  fn a_device_action_wants_exactly_one_address() {
    assert_eq!(
      refused(&["connect"]),
      Ok(Some(Refused::MissingOperands(vec!["ADDRESS"])))
    );
    assert_eq!(
      refused(&["forget", "E0:C1:02:00:B0:C3", "11:22:33:44:55:66"]),
      Ok(Some(Refused::ExtraArgument(
        "11:22:33:44:55:66".to_string()
      )))
    );
    assert_eq!(
      refused(&["introduce", "E0-C1-02"]),
      Ok(Some(Refused::InvalidValue {
        of: "ADDRESS",
        value: "E0-C1-02".to_string()
      })),
      "an operand that is not an address is refused before bluez is asked"
    );
  }

  #[test]
  fn the_radio_s_actions_take_no_address() {
    assert_eq!(
      refused(&["on", "E0:C1:02:00:B0:C3"]),
      Ok(Some(Refused::ExtraArgument(
        "E0:C1:02:00:B0:C3".to_string()
      )))
    );
    assert_eq!(refused(&["search"]), Ok(None));
  }

  #[test]
  fn a_pin_is_for_an_introduction_and_is_one_bluez_would_take() {
    assert_eq!(
      refused(&["connect", "--pin", "0000", "E0:C1:02:00:B0:C3"]),
      Ok(Some(Refused::ExtraArgument("--pin".to_string())))
    );
    assert_eq!(
      refused(&[
        "introduce",
        "--pin",
        "0123456789abcdefg",
        "E0:C1:02:00:B0:C3"
      ]),
      Ok(Some(Refused::InvalidValue {
        of: "PIN",
        value: "0123456789abcdefg".to_string()
      }))
    );
  }

  #[test]
  fn a_word_this_program_does_not_have_is_refused_by_name() {
    assert_eq!(
      refused(&["pair", "E0:C1:02:00:B0:C3"]),
      Ok(Some(Refused::NoSuchSubcommand("pair".to_string())))
    );
    assert_eq!(refused(&[]), Ok(Some(Refused::MissingSubcommand)));
  }
}
