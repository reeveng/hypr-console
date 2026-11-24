use std::process::ExitCode;
use std::time::Instant;

use console_application_clipboard::{
  COMMAND, READ_TEXT, WATCH, WHO, card, copy_png, copy_text, door, read_system_text, watch,
};
use console_core_internal_programs::{CLIPBOARD_COPY_PNG, CLIPBOARD_COPY_TEXT};
use console_core_number_conversion::fitted;
use console_panel::card::{Panel, opened};

const COPY_TEXT: &str = CLIPBOARD_COPY_TEXT.spelling;
const COPY_PNG: &str = CLIPBOARD_COPY_PNG.spelling;

fn main() -> ExitCode {
  let asked: Vec<String> = std::env::args().skip(1).collect();

  match { ::core::primitive::bool::then_some(asked == [READ_TEXT.spelling], ()) } {
    ::core::option::Option::Some(()) => {
      use std::io::Write;
      return match read_system_text(Instant::now) {
        Ok(Some(text)) => match std::io::stdout().write_all(text.as_bytes()) {
          Ok(()) => ExitCode::SUCCESS,
          Err(fault) => {
            console_core_logging::debug!("writing the active clipboard: {fault}");
            ExitCode::FAILURE
          }
        },
        Ok(None) => ExitCode::FAILURE,
        Err(fault) => {
          console_core_logging::debug!("reading the active clipboard: {fault}");
          ExitCode::FAILURE
        }
      };
    }
    ::core::option::Option::None => {}
  }

  let copying = match asked.as_slice() {
    [flag] => match flag.as_str() {
      COPY_TEXT => Some(copy_text(std::io::stdin().lock())),
      COPY_PNG => Some(copy_png(std::io::stdin().lock())),
      _ => None,
    },
    [] | [_, _, ..] => None,
  };

  match copying {
    Some(result) => {
      return match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
          console_core_logging::error!("clipboard copy: {fault}");
          ExitCode::FAILURE
        }
      };
    }
    None => {}
  }

  match (asked == [WATCH.spelling]).then_some(()) {
    ::core::option::Option::Some(()) => {
      return match watch_session() {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
          console_core_logging::error!("clipboard service: {fault}");
          ExitCode::FAILURE
        }
      };
    }
    ::core::option::Option::None => {}
  }

  let Ok(code) = opened(
    &asked,
    Panel {
      who: WHO,
      command: COMMAND,
      door,
      card,
    },
  );

  code
}

fn watch_session() -> Result<(), Box<dyn std::error::Error>> {
  let seed = session_seed()?;

  watch(seed, Instant::now)
}

#[cfg_attr(
  dylint_lib = "explicit077_no_ambient_randomness",
  expect(
    explicit077_no_ambient_randomness,
    reason = "the binary reads one session seed so IDs from a restarted clipboard service do not address older panel items"
  )
)]
fn session_seed() -> std::io::Result<u64> {
  let mut seed = [0; size_of::<u64>()];
  let read =
    rustix::rand::getrandom(&mut seed, rustix::rand::GetRandomFlags::empty()).map(|length| {
      let Ok(portable) = fitted::<_, u32>(length);
      portable
    });
  let count = read?;
  let Ok(expected) = fitted::<_, u32>(seed.len());

  match count.cmp(&expected) {
    std::cmp::Ordering::Equal => Ok(u64::from_le_bytes(seed)),
    std::cmp::Ordering::Less | std::cmp::Ordering::Greater => {
      Err(std::io::Error::other("short clipboard history session ID"))
    }
  }
}
