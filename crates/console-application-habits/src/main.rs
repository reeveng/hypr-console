//! Foreground activity history and a native Wayland dashboard.

#![forbid(unsafe_code)]

mod activity;
mod browser;
mod dashboard;
mod idle;
mod report;
mod setup;
mod stored;

use std::env;
use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use console_core_atomic_writes::whole;
use console_core_number_conversion::{Float, whole_u64};
use console_core_places::Base;

const APP_ID: &str = "console-habits";
const DEFAULT_INTERVAL: u64 = 60;
const DAY: u64 = 86_400;
const USAGE: &str = "Console Habits\n\n  console-application-habits [open]                 Open the dashboard\n  console-application-habits record                 Run the foreground recorder\n  console-application-habits interval [minutes]     Show or set the interval (0.25–10 minutes)\n  console-application-habits export [hours]         Write a Mermaid timeline to standard output\n  console-application-habits browser-setup BROWSER  Prepare the companion for brave, chromium, chrome, firefox, librewolf, or zen\n  console-application-habits install                Install the user launcher and start recording\n";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Paths {
  home: PathBuf,
  state: PathBuf,
  runtime: PathBuf,
}

impl Paths {
  fn new() -> Result<Self> {
    let Ok(user_home) = console_core_places::home();
    let home = user_home.ok_or("HOME is required")?;
    let Ok(user_runtime) = console_core_places::runtime();
    let runtime_base = user_runtime.ok_or("XDG_RUNTIME_DIR is required")?;
    let Ok(state_base) = xdg(Base::State, &home);
    let runtime = runtime_base.join("habits");
    let state = state_base.join("habits");
    Ok(Self {
      home,
      state,
      runtime,
    })
  }

  fn interval(&self) -> Result<u64> {
    match fs::read_to_string(self.state.join("interval-seconds")) {
      Ok(text) => {
        let parse = text.trim().parse::<u64>()?;

        Ok(parse.clamp(15, 600))
      }
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
          ::core::option::Option::Some(()) => Ok(DEFAULT_INTERVAL),
          ::core::option::Option::None => Err(Box::new(fault)),
        }
      }
    }
  }
}

#[must_use]
fn xdg(base: Base, home: &Path) -> core::result::Result<PathBuf, console_core_never::Never> {
  let Ok(configured) = base.configured();

  match configured.filter(|path| path.is_absolute()) {
    Some(path) => Ok(path),
    None => base.under(home),
  }
}

fn private_directory(path: &Path) -> io::Result<()> {
  DirBuilder::new().recursive(true).mode(0o700).create(path)
}

#[must_use]
fn now() -> core::result::Result<u64, console_core_never::Never> {
  Ok(match SystemTime::now().duration_since(UNIX_EPOCH) {
    Ok(elapsed) => elapsed.as_secs(),
    Err(_before_epoch) => 0,
  })
}

fn seconds(minutes: &str) -> Result<u64> {
  let minutes_2 = minutes.parse::<f64>()?;

  match bool::then_some(!(0.25..=10.0).contains(&minutes_2), ()) {
    Some(()) => {
      return Err(Box::from("choose an interval between 0.25 and 10 minutes"));
    }
    None => {}
  }

  let Ok(seconds) = whole_u64(minutes_2 * 60.0);
  Ok(seconds)
}

fn run(arguments: &[String]) -> Result<()> {
  let args: Vec<_> = arguments.iter().map(String::as_str).collect();

  match args.as_slice() {
    ["--help" | "-h"] => {
      print!("{USAGE}");
      return Ok(());
    }
    _command => {}
  }

  let paths = Paths::new()?;

  match args.as_slice() {
    [] | ["open"] => setup::open(&paths),
    ["record"] => activity::record(&paths),
    ["interval"] => {
      let paths_interval = paths.interval()?;
      let Ok(seconds) = paths_interval.float();
      println!("{} minute(s)", seconds / 60.0);
      Ok(())
    }
    ["interval", minutes] => {
      let interval = seconds(minutes)?;
      private_directory(&paths.state)?;
      whole(
        &paths.state.join("interval-seconds"),
        interval.to_string().as_bytes(),
      )?;
      println!("Sampling every {interval} seconds");
      Ok(())
    }
    ["export"] => report::export(&paths, 24),
    ["export", written] => {
      let hours = written.parse()?;
      report::export(&paths, hours)
    }
    ["browser-setup", name] => browser::setup(&paths, name),
    ["install"] => setup::install(&paths),
    ["native-messaging", ..] => browser::host(&paths),
    _ => Err(Box::from(format!("unknown arguments\n\n{USAGE}"))),
  }
}

fn main() -> ExitCode {
  let arguments: Vec<_> = env::args().skip(1).collect();

  match run(&arguments) {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      console_core_logging::warn!("{fault}");
      ExitCode::FAILURE
    }
  }
}
