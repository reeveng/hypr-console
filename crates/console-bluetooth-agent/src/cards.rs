//! What is put in front of somebody: the code to type, and what went wrong.
//!
//! Two things come out of here, and neither is raised by the program. The
//! code is the pairing's own -- a passkey or a PIN the device wants typed on
//! it -- and is one line the program prints while the pairing runs. The other
//! is an [`Error`] said as what somebody can do about it, a card of a summary
//! and a body, written on the program's error output as it ends.
//!
//! Both used to be notifications the program raised, and a notification raised
//! while a card is up is drawn behind the card -- so a keyboard's code was
//! under the panel it was being paired from, and a pairing that failed said so
//! to nobody until the panel was put away. The panel that ran the program says
//! both now: the line as the card's note while it runs, and the card, read
//! back with [`told`], when it has ended; each as a notification instead when
//! no card is up. [`written`] and [`read`] are the two halves of the card,
//! kept beside each other because they are one format: the summary on the
//! first line and the body under it.

use console_bluetooth::adapter::Power;
use console_core_never::Never;
use console_program_contract::{Answer, ExitStatus, Notification};

use crate::errors::Error;

const UNSAID: &str = "Bluetooth stopped without saying why";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
  pub summary: String,
  pub body: String,
}

pub fn card(error: &Error) -> Result<Card, Never> {
  Ok(match error {
    Error::Unreachable(why) => Card {
      summary: "Bluetooth is not answering".to_string(),
      body: why.clone(),
    },
    Error::Pair { name, reason } => Card {
      summary: format!("Couldn't pair with {name}"),
      body: reason.to_string(),
    },
    Error::Trust { name, reason } => Card {
      summary: format!("{name} paired, and will be a stranger again after a restart"),
      body: reason.to_string(),
    },
    Error::PairedNotConnected { name, reason } => Card {
      summary: format!("{name} paired, and did not connect"),
      body: reason.to_string(),
    },
    Error::Connect { name, reason } => Card {
      summary: format!("Couldn't connect to {name}"),
      body: reason.to_string(),
    },
    Error::Disconnect { name, reason } => Card {
      summary: format!("Couldn't disconnect {name}"),
      body: reason.to_string(),
    },
    Error::Forget { name, reason } => Card {
      summary: format!("Couldn't forget {name}"),
      body: reason.to_string(),
    },
    Error::Power {
      power: Power::On,
      reason,
    } => Card {
      summary: "Couldn't turn Bluetooth on".to_string(),
      body: reason.to_string(),
    },
    Error::Power {
      power: Power::Off,
      reason,
    } => Card {
      summary: "Couldn't turn Bluetooth off".to_string(),
      body: reason.to_string(),
    },
    Error::Search { reason } => Card {
      summary: "Couldn't search for devices".to_string(),
      body: reason.to_string(),
    },
  })
}

pub fn written(card: &Card) -> Result<String, Never> {
  Ok(format!("{}\n{}", card.summary, card.body))
}

pub fn read(said: &str) -> Result<Option<Card>, Never> {
  let whole = said.trim();

  Ok(match whole.split_once('\n') {
    Some((summary, body)) => Some(Card {
      summary: summary.trim().to_string(),
      body: body.trim().to_string(),
    }),
    None => match ::core::primitive::bool::then_some(whole.is_empty(), ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => Some(Card {
        summary: whole.to_string(),
        body: String::new(),
      }),
    },
  })
}

pub fn told(answer: &Answer) -> Result<Option<Notification>, Never> {
  match answer.status {
    ExitStatus::Success => return Ok(None),
    ExitStatus::Failure(_) | ExitStatus::Interrupt => {}
  }

  let Ok(read) = read(&answer.error_output);

  let card = match read {
    Some(card) => card,
    None => Card {
      summary: UNSAID.to_string(),
      body: String::new(),
    },
  };

  let body = match ::core::primitive::bool::then_some(card.body.is_empty(), ()) {
    ::core::option::Option::Some(()) => None,
    ::core::option::Option::None => Some(card.body),
  };

  Ok(Some(Notification {
    summary: card.summary,
    body,
  }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Code {
  Passkey(u32),
  Pin(String),
}

pub fn shown(code: &Code) -> Result<String, Never> {
  Ok(match code {
    Code::Passkey(passkey) => format!("{passkey:06}"),
    Code::Pin(pin) => pin.clone(),
  })
}

pub fn entering(name: &str, code: &Code) -> Result<String, Never> {
  let Ok(shown) = shown(code);

  Ok(format!("Type {shown} on {name}, then press Return"))
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::errors::Reason;

  #[test]
  fn a_passkey_is_six_digits_whatever_number_it_is() {
    assert_eq!(shown(&Code::Passkey(42)), Ok("000042".to_string()));
    assert_eq!(shown(&Code::Passkey(987654)), Ok("987654".to_string()));
    assert_eq!(
      shown(&Code::Pin("0815".to_string())),
      Ok("0815".to_string()),
      "a pin is shown as it was given"
    );
  }

  #[test]
  fn the_code_is_one_line_naming_the_device_and_the_code() {
    assert_eq!(
      entering("Lab31 - Keyboard", &Code::Passkey(1234)),
      Ok("Type 001234 on Lab31 - Keyboard, then press Return".to_string())
    );
  }

  #[test]
  fn a_card_that_did_not_work_names_the_device_rather_than_its_address() {
    let error = Error::Pair {
      name: "MINISO-MS218".to_string(),
      reason: Reason::OutOfRange,
    };
    let Ok(said) = card(&error);

    assert_eq!(said.summary, "Couldn't pair with MINISO-MS218");
    assert_eq!(
      said.body,
      "Make sure it is turned on, in pairing mode and nearby."
    );
  }

  fn ended(status: ExitStatus, error_output: &str) -> Result<Answer, Never> {
    Ok(Answer {
      job: None,
      command: console_program_contract::Command {
        program: console_program_contract::Executable::Internal(
          console_core_internal_programs::InternalProgram::Bluetooth,
        ),
        arguments: Vec::new(),
      },
      output: String::new(),
      error_output: error_output.to_string(),
      status,
    })
  }

  #[test]
  fn what_the_program_wrote_on_the_way_out_is_the_card_the_panel_says()
  -> Result<(), Box<dyn std::error::Error>> {
    let error = Error::Pair {
      name: "MINISO-MS218".to_string(),
      reason: Reason::OutOfRange,
    };
    let Ok(said) = card(&error);
    let Ok(on_the_way_out) = written(&said);
    let Ok(failed) = ended(ExitStatus::Failure(Some(1)), &on_the_way_out);

    let Ok(told) = told(&failed);
    let notification = told.ok_or("a failure is said")?;

    assert_eq!(notification.summary, said.summary);
    assert_eq!(notification.body, Some(said.body));

    Ok(())
  }

  #[test]
  fn a_program_that_worked_says_nothing_and_one_that_said_nothing_still_failed()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(worked) = ended(ExitStatus::Success, "");
    let Ok(mute) = ended(ExitStatus::Failure(None), "");

    assert_eq!(told(&worked), Ok(None));

    let Ok(told_of_mute) = told(&mute);
    let notification = told_of_mute.ok_or("a failure is said even with nothing written")?;

    assert_eq!(notification.summary, UNSAID);

    Ok(())
  }

  #[test]
  fn a_device_that_is_half_there_is_said_differently_from_one_that_is_not_there() {
    let reason = Reason::Message("br-connection-profile-unavailable".to_string());
    let Ok(half) = card(&Error::PairedNotConnected {
      name: "Pad".to_string(),
      reason: reason.clone(),
    });
    let Ok(gone) = card(&Error::Connect {
      name: "Pad".to_string(),
      reason,
    });

    assert_eq!(half.summary, "Pad paired, and did not connect");
    assert_eq!(gone.summary, "Couldn't connect to Pad");
    assert_eq!(
      gone.body, "br-connection-profile-unavailable",
      "bluez's own sentence is carried through"
    );
  }
}
