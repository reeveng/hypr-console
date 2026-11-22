//! Say that a piece of the desktop died. Run by every console unit as it stops.
//!
//!     ExecStopPost=-/usr/local/bin/console-report-crash %n
//!
//! systemd puts the reason in the environment, and a clean stop is most of what
//! this sees: the target stopping at logout stops all of them. Only a service
//! that fell over says anything.
//!
//! The card is a name and a sentence: what fell, and that it was not asked to.
//! The reason systemd gives for the fall is a word like `core-dump`, which is a
//! thing to look up rather than a thing to read, so it goes to the journal line
//! and not to the screen.
//!
//! The service still comes back on its own, because every one of them restarts.
//! What this adds is that someone knows it happened. A daemon that dies and
//! repairs itself every few minutes looks exactly like a daemon that is
//! running, right up to `systemctl --user show -p NRestarts`, which no one
//! thinks to ask for because nothing ever suggested it.

use std::process::ExitCode;

use console_core_arguments::{Command, Operands, read};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_notifications::saying::{Content, StatePath, fault, for_the_journal, journal, raise};

const SOMETHING_OF_THE_DESKTOPS: &str = "Part of the desktop";

const SUCCESS: &str = "success";

const DECLINED: &str = "exec-condition";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stopped<'a> {
  pub unit: &'a str,
  pub said: &'a str,
}

pub fn crashed(
  stopped: Stopped<'_>,
  result: Option<&str>,
) -> Result<Option<(String, String, String)>, Never> {
  let Stopped { unit, said } = stopped;
  let how = match result {
    Some(how) => how,
    None => SUCCESS,
  };

  match ::core::primitive::bool::then_some(how == SUCCESS || how == DECLINED, ()) {
    ::core::option::Option::Some(()) => return Ok(None),
    ::core::option::Option::None => {}
  }

  let what = match ::core::primitive::bool::then_some(said.trim().is_empty(), ()) {
    ::core::option::Option::Some(()) => unit.to_string(),
    ::core::option::Option::None => said.trim().to_string(),
  };

  Ok(Some((
    format!("unit-{unit}"),
    format!("{what} restarted"),
    "It quit unexpectedly.".to_string(),
  )))
}

fn described(unit: &str) -> Result<String, Never> {
  let Ok(mut asking) = Program::Systemctl.command();

  let said = match asking
    .args(["--user", "show", "-p", "Description", "--value", unit])
    .output()
  {
    Ok(message) => message,
    Err(_would_not_start) => return Ok(String::new()),
  };

  Ok(match std::str::from_utf8(&said.stdout) {
    Ok(description) => description.trim().to_string(),
    Err(_invalid_description_uses_the_unit_name) => String::new(),
  })
}

const COMMAND: Command = Command {
  name: "console-report-crash",
  about: "say that a piece of the desktop dies; every console unit runs this as it stops, with its own name",
  flags: &[],
  operands: Operands::Optional("UNIT"),
};

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let line = match read(&COMMAND, &words) {
    Ok(line) => line,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  let Ok(operands) = line.operands();
  let unit = match operands.first() {
    Some(unit) => unit.as_str(),
    None => SOMETHING_OF_THE_DESKTOPS,
  };
  let Ok(()) = report(unit);

  ExitCode::SUCCESS
}

fn report(unit: &str) -> Result<(), Never> {
  #[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
      explicit026_env_read_once,
      reason = "SERVICE_RESULT is what systemd hands an OnFailure unit, and this binary is that unit. Nothing else in this tree is started that way"
    )
  )]
  let result = match std::env::var("SERVICE_RESULT") {
    Ok(result) => result,
    Err(_unset) => return Ok(()),
  };

  let Ok(described) = described(unit);
  let Ok(crashed) = crashed(
    Stopped {
      unit,
      said: &described,
    },
    Some(&result),
  );

  let (kind, summary, body) = match crashed {
    Some((kind, summary, body)) => (kind, summary, body),
    None => return Ok(()),
  };

  let reasoned = format!("{body} ({result})");
  let Ok(said) = for_the_journal(
    &kind,
    Content {
      summary: &summary,
      body: &reasoned,
    },
  );
  let Ok(()) = journal(&said);
  #[cfg(target_os = "linux")]
  let Ok(()) = console_core_logging::orbit::keep_crash(unit, format_args!("{said}"));
  let Ok(counting) = StatePath::counting(&kind);
  let Ok(again) = counting.again();
  let Ok(fault) = fault(
    Content {
      summary: &summary,
      body: &reasoned,
    },
    again,
  );

  match fault {
    Some(notification) => {
      let Ok(_) = raise(&notification);
    }
    None => {}
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::{Stopped, crashed};

  const BAR: Stopped<'static> = Stopped {
    unit: "console-bar.service",
    said: "Status bar",
  };

  #[test]
  fn a_unit_that_was_stopped_on_purpose_says_nothing() {
    assert_eq!(crashed(BAR, Some("success")), Ok(None));
  }

  #[test]
  fn a_stop_with_no_reason_given_is_taken_as_a_clean_one() {
    assert_eq!(crashed(BAR, None), Ok(None));
  }

  #[test]
  fn a_unit_whose_condition_said_not_to_start_says_nothing() {
    assert_eq!(
      crashed(
        Stopped {
          unit: "console-warm.service",
          said: "The color of the screen, on a clock",
        },
        Some("exec-condition")
      ),
      Ok(None),
    );
  }

  #[test]
  fn a_unit_that_fell_over_says_so_in_a_line_anyone_can_read() -> Result<(), &'static str> {
    let Ok(fell) = crashed(BAR, Some("exit-code"));
    let (kind, summary, body) = fell.ok_or("it did not fall")?;

    assert_eq!(kind, "unit-console-bar.service");
    assert_eq!(summary, "Status bar restarted");
    assert!(
      !summary.contains("console-bar.service"),
      "the top line names a unit: {summary}"
    );
    assert_eq!(body, "It quit unexpectedly.");

    Ok(())
  }

  #[test]
  fn a_unit_with_no_description_is_said_by_its_name() -> Result<(), &'static str> {
    let untitled = Stopped {
      unit: "something-else.service",
      said: "",
    };
    let Ok(fell) = crashed(untitled, Some("exit-code"));
    let (_, summary, _) = fell.ok_or("it did not fall")?;

    assert!(summary.starts_with("something-else.service"), "{summary}");

    Ok(())
  }

  #[test]
  fn one_unit_is_one_kind_however_many_ways_it_falls() -> Result<(), &'static str> {
    let Ok(exited) = crashed(BAR, Some("exit-code"));
    let Ok(signalled) = crashed(BAR, Some("signal"));
    let (one, _, _) = exited.ok_or("it did not fall")?;
    let (other, _, _) = signalled.ok_or("it did not fall")?;

    assert_eq!(one, other);

    Ok(())
  }
}
