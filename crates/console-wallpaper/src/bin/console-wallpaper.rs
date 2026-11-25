//! Apply one wallpaper with --now, or follow the sky using the shared runtime.
//! The --run-scene command runs a user-enrolled executable from wallpaper-scenes.

use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::UNIX_EPOCH;

use console_core_arguments::{Presence, read};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_location::timezone::CachedLocation;
use console_operating_system_linux::program_runtime::{self, Clock, Interpreter};
use console_paper::{self as paper, Motion, Request};
use console_program_contract::{Answer, Event, ExitStatus};
use console_wallpaper::choose::{Selection, Set, Wanted};
use console_wallpaper::keeping::{
  self, Chosen, Going, Input, Observation, Rendered, Sun, WallpaperEffect, WallpaperEvent,
};
use console_wallpaper::{covered, place, rules, scene};
use console_weather::conditions::{self as weather, Weather};

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let line = match read(&keeping::COMMAND, &words) {
    Ok(line) => line,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };
  let Ok(launch) = line.presence(keeping::RUN_SCENE);

  match launch {
    Presence::Present => {
      let Ok(code) = run_scene();

      return code;
    }
    Presence::Absent => {}
  }

  let Ok(now) = line.presence(keeping::NOW);
  let Ok(going) = Going::of(now);

  match run(going) {
    Ok(code) => code,
    Err(fault) => {
      console_core_logging::warn!("{fault}");
      ExitCode::FAILURE
    }
  }
}

#[must_use]
fn run_scene() -> Result<ExitCode, Never> {
  let Ok(wanted) = Wanted::load();
  let Ok(scene) = scene::selected(&wanted);
  let path = match scene {
    Some(path) => path,
    None => return Ok(ExitCode::SUCCESS),
  };
  let fault = std::process::Command::new(&path)
    .env("CONSOLE_LIVE_WALLPAPER", "1")
    .exec();

  console_core_logging::error!("could not run live wallpaper {}: {fault}", path.display());
  Ok(ExitCode::FAILURE)
}

#[derive(Debug)]
enum Untabled {
  Read(std::path::PathBuf, std::io::Error),
  Unparsed(std::path::PathBuf, toml::de::Error),
}

impl std::fmt::Display for Untabled {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Untabled::Read(at, fault) => {
        write!(to, "{} could not be read: {fault}", at.display())
      }
      Untabled::Unparsed(at, fault) => {
        write!(to, "{} does not parse: {fault}", at.display())
      }
    }
  }
}

impl std::error::Error for Untabled {}

fn run(going: Going) -> Result<ExitCode, Untabled> {
  let table = read_table()?;
  let Ok(rule_directory) = rules::directory();
  let input = Input {
    going,
    pictures: &table.pictures,
    rule_directory: rule_directory.as_deref(),
  };
  let Ok(mut desktop) = Desktop::new(going);
  let Ok(code) = program_runtime::run::<Sun, _>("console-wallpaper", &input, &mut desktop);

  Ok(code)
}

struct Desktop {
  going: Going,
  location: CachedLocation,
  compositor: covered::Notified,
  forecast: weather::Notified,
  scene: Option<PathBuf>,
}

impl Interpreter for Desktop {
  type Event = WallpaperEvent;
  type Effect = WallpaperEffect;
  type Error = Never;

  fn connected(&mut self) -> Result<Option<WallpaperEvent>, Never> {
    Ok(Some(WallpaperEvent::Reconnected))
  }

  fn interpret(
    &mut self,
    acts: &WallpaperEffect,
    clock: &Clock,
  ) -> Result<Vec<Event<WallpaperEvent>>, Never> {
    let event = match acts {
      WallpaperEffect::Observe => {
        let Ok(observed) = self.observe(clock);

        WallpaperEvent::Observed(observed)
      }
      WallpaperEffect::Resolve {
        name,
        elapsed,
        covered,
      } => {
        let Ok(found) = place::picture(name);

        WallpaperEvent::Looked {
          elapsed: *elapsed,
          covered: *covered,
          chosen: found.map(|(moving, still)| Chosen {
            still: (place::file_status(&still) == Ok(place::FileStatus::File)).then_some(still),
            moving,
          }),
        }
      }
      WallpaperEffect::WeatherLocation => {
        let Ok(time) = clock.sample();
        let Ok(at) = self.location.here(time.monotonic);

        WallpaperEvent::WeatherLocation(at)
      }
      WallpaperEffect::ReadWeather(answer) => {
        let Ok(said) = self.read_weather(answer);

        WallpaperEvent::Weather(said)
      }
      WallpaperEffect::Paint(picture) => {
        let Ok(event) = paint(picture, Motion::Play);
        event
      }
      WallpaperEffect::Freeze(picture) => {
        let Ok(event) = paint(picture, Motion::Freeze);
        event
      }
    };

    Ok(vec![Event::Custom(event)])
  }
}

impl Desktop {
  #[must_use]
  fn new(going: Going) -> Result<Self, Never> {
    Ok(Self {
      going,
      location: CachedLocation::default(),
      compositor: covered::Notified::default(),
      forecast: weather::Notified::default(),
      scene: None,
    })
  }

  fn observe(&mut self, clock: &Clock) -> Result<Observation, Never> {
    let Ok(time) = clock.sample();
    let seconds = match time.wall.duration_since(UNIX_EPOCH) {
      Ok(since) => since.as_secs_f64(),
      Err(before) => {
        console_core_logging::warn!(
          "console-wallpaper: the clock is before 1970 ({before}), using the epoch"
        );

        0.0
      }
    };
    let Ok(at) = self.location.here(time.monotonic);
    let Ok(wanted) = Wanted::load();
    let Ok(scene) = scene::selected(&wanted);
    let covered = match &scene {
      Some(_) => covered::Covered::Yes,
      None => {
        let Ok(covered) = covered::now(&mut self.compositor);
        covered
      }
    };
    let Ok(()) = self.sync_scene(scene);
    let pinned_file = !matches!(wanted.selection, Selection::Following)
      && place::picture(&wanted.picture).is_ok_and(|found| found.is_some());
    let Ok(rules) = rules::installed();

    Ok(Observation {
      seconds: console_wallpaper::units::UnixSeconds(seconds),
      elapsed: time.elapsed,
      at,
      wanted,
      pinned_file,
      rules,
      covered,
    })
  }

  fn sync_scene(&mut self, desired: Option<PathBuf>) -> Result<(), Never> {
    let action = match (self.going, self.scene.as_deref(), desired.as_deref()) {
      (Going::Once, _, _) | (Going::KeepGoing, None, None) => return Ok(()),
      (Going::KeepGoing, Some(_), None) => "stop",
      (Going::KeepGoing, previous, Some(next)) => match (previous == Some(next)).then_some(()) {
        Some(()) => return Ok(()),
        None => "restart",
      },
    };

    match scene_service(action) {
      Ok(()) => self.scene = desired,
      Err(fault) => console_core_logging::warn!("could not {action} the live wallpaper: {fault}"),
    }

    Ok(())
  }

  fn read_weather(&mut self, answer: &Answer) -> Result<Option<Weather>, Never> {
    let said = match answer.status {
      ExitStatus::Success => weather::read(&answer.output)?,
      ExitStatus::Failure(_) | ExitStatus::Interrupt => {
        match ::core::primitive::bool::then_some(self.forecast == weather::Notified::NotYet, ()) {
          ::core::option::Option::Some(()) => {
            console_core_logging::warn!(
              "console-wallpaper: Open-Meteo request failed ({:?}) {}",
              answer.status,
              answer.error_output.trim()
            );
          }
          ::core::option::Option::None => {}
        }

        None
      }
    };
    self.forecast = match said {
      Some(_) => weather::Notified::NotYet,
      None => weather::Notified::Already,
    };

    Ok(said)
  }
}

fn scene_service(action: &str) -> io::Result<()> {
  let Ok(mut systemctl) = Program::Systemctl.command();
  let output = systemctl
    .args(["--user", action, "console-wallpaper-live.service"])
    .output()?;

  match output.status.success().then_some(()) {
    Some(()) => Ok(()),
    None => {
      let Ok(error) = console_wallpaper::diagnostic(&output.stderr);
      Err(io::Error::other(error))
    }
  }
}

fn read_table() -> Result<Set, Untabled> {
  let Ok(at) = place::table();

  let held = console_core_atomic_writes::read_text(&at)
    .map_err(|fault| Untabled::Read(at.clone(), fault))?;

  toml::from_str(&held).map_err(|fault| Untabled::Unparsed(at, fault))
}

#[must_use]
fn paint(picture: &Path, motion: Motion) -> Result<WallpaperEvent, Never> {
  let result =
    std::path::absolute(picture).and_then(|path| paper::ask(&Request::Show(path, motion)));

  let went = match result {
    Ok(_) => Rendered::Yes,
    Err(fault) => {
      console_core_logging::warn!(
        "the wallpaper would not take {}: {fault}",
        picture.display()
      );
      Rendered::No
    }
  };

  Ok(WallpaperEvent::Rendered {
    at: picture.to_path_buf(),
    went,
  })
}
