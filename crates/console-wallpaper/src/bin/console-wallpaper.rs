//! Apply one wallpaper with --now, or follow the sky using the shared runtime.

use std::process::ExitCode;
use std::time::UNIX_EPOCH;

use console_core_arguments::read;
use console_core_never::Never;
use console_location::timezone::CachedLocation;
use console_operating_system_linux::program_runtime::{self, Clock, Interpreter};
use console_paper::{self as paper, Motion, Request};
use console_program_contract::{Answer, Event, ExitStatus};
use console_wallpaper::choose::{Set, Wanted};
use console_wallpaper::keeping::{
  self, Chosen, Going, Input, Observation, Rendered, Sun, WallpaperEffect, WallpaperEvent,
};
use console_wallpaper::{covered, place, rules};
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
  let mut desktop = Desktop::default();
  let Ok(code) = program_runtime::run::<Sun, _>("console-wallpaper", &input, &mut desktop);

  Ok(code)
}

#[derive(Default)]
struct Desktop {
  location: CachedLocation,
  compositor: covered::Notified,
  forecast: weather::Notified,
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
    let Ok(covered) = covered::now(&mut self.compositor);
    let pinned_file =
      !wanted.follow && place::picture(&wanted.picture).is_ok_and(|found| found.is_some());
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

fn read_table() -> Result<Set, Untabled> {
  let Ok(at) = place::table();

  let held = console_core_atomic_writes::read_text(&at)
    .map_err(|fault| Untabled::Read(at.clone(), fault))?;

  toml::from_str(&held).map_err(|fault| Untabled::Unparsed(at, fault))
}

#[must_use]
fn paint(picture: &std::path::Path, motion: Motion) -> Result<WallpaperEvent, Never> {
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
