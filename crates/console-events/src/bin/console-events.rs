use std::process::ExitCode;
use std::time::Duration;

use console_core_arguments::{Command, Flag, Operands, Presence, Takes, read, run_main};
use console_core_logging::{Severity, Sink, install};
use console_events::ipc::{Client, Reply, Request, Trace};
use console_events::{place, serving, sources};

const TRACE: Flag = Flag {
  spelling: "--trace",
  takes: Takes::None,
  about: "log protocol versions, request names, serials, and frame sizes",
};

const PROBE: Flag = Flag {
  spelling: "--probe",
  takes: Takes::None,
  about: "send a versioned ping to the running relay",
};

const COMMAND: Command = Command {
  name: "console-events",
  about: "serve desktop events and shared workspace state",
  flags: &[TRACE, PROBE],
  operands: Operands::None,
};

const TRACE_LOG: Sink = Sink {
  minimum: Severity::Info,
  write: |record| eprintln!("{record}"),
};

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();
  let Ok(code) = run_main(
    &COMMAND,
    &words,
    |words_2| read(&COMMAND, words_2),
    |line| -> Result<(), Box<dyn std::error::Error>> {
      let Ok(tracing) = line.presence(TRACE);
      let trace = match tracing {
        Presence::Present => {
          install(&TRACE_LOG)?;

          Trace::Enabled
        }
        Presence::Absent => Trace::Quiet,
      };
      let Ok(probing) = line.presence(PROBE);

      match probing {
        Presence::Present => probe(trace),
        Presence::Absent => {
          let socket = place::socket()?;
          serving::serve_traced(&socket, sources::hold, trace)?;

          Ok(())
        }
      }
    },
  );

  code
}

fn probe(trace: Trace) -> Result<(), Box<dyn std::error::Error>> {
  let mut client = Client::session(trace)?;
  let reply = client.call(Request::Ping, Duration::from_secs(1))?;

  match reply {
    Reply::Pong => {
      println!("pong");

      Ok(())
    }
    Reply::Workspace(_) | Reply::Fault(_) => {
      Err(Box::from("the relay returned an unexpected reply"))
    }
  }
}
