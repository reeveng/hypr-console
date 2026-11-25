//! Pointer settings shared by the settings panel and the Console Hyprland session.

use std::path::Path;

use console_compositor::{DispatchResult, Request};
use console_core_atomic_writes::{self as writes, Stored};
use console_core_never::Never;
use console_core_places::Base;
use console_panel::page::{Aside, Handler, Initial, Row, Showing};

use crate::rows::{answer, choices};

const SPEED: &str = "pointer-speed";
const PROFILE: &str = "pointer-acceleration";

#[derive(Debug, Default, PartialEq, Eq)]
struct Speed(i32);

impl Speed {
  fn parse(value: &str) -> Result<Self, InvalidSpeed> {
    let speed = value.trim().parse::<i32>().map_err(|_| InvalidSpeed)?;

    match (-100..=100).contains(&speed).then_some(()) {
      Some(()) => Ok(Self(speed)),
      None => Err(InvalidSpeed),
    }
  }
}

#[derive(Debug, PartialEq, Eq)]
struct InvalidSpeed;

#[derive(Clone, Copy)]
enum Profile {
  Default,
  Adaptive,
  Flat,
}

impl Profile {
  const ALL: [Self; 3] = [Self::Default, Self::Adaptive, Self::Flat];

  fn name(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Default => "Device default",
      Self::Adaptive => "Adaptive",
      Self::Flat => "Flat",
    })
  }

  fn value(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Default => "",
      Self::Adaptive => "adaptive",
      Self::Flat => "flat",
    })
  }
}

pub fn rows() -> Result<Vec<Row>, Never> {
  let Ok(configuration) = Base::Configuration.ours();
  let folder = match configuration {
    Some(folder) => folder,
    None => return Ok(Vec::new()),
  };
  let Ok(heading) = Row::naming("Mouse and touchpad", Aside(""));
  let Ok(speed) = speed_row(&folder.join(SPEED));
  let Ok(profile) = profile_row(&folder.join(PROFILE));

  Ok(vec![heading, speed, profile])
}

fn preference(at: &Path) -> Result<String, Never> {
  let Ok(stored) = writes::read(at);

  Ok(match stored {
    Stored::Text(value) => value.trim().to_owned(),
    Stored::Absent => String::new(),
    Stored::Failed(fault) => {
      console_core_logging::warn!("settings-panel: reading {}: {fault}", at.display());
      String::new()
    }
  })
}

fn speed_row(at: &Path) -> Result<Row, Never> {
  let Ok(raw) = preference(at);
  let speed = match Speed::parse(&raw) {
    Ok(speed) => speed,
    Err(InvalidSpeed) => Speed::default(),
  };
  let initial = speed.0.to_string();
  let aside = format!("{:+}%", speed.0);
  let at_2 = at.to_path_buf();
  let Ok(edit) = Handler::and_stay(move |showing| {
    let at_3 = at_2.clone();
    let Ok(respond) = answer(move |showing_2, entered| match Speed::parse(entered) {
      Ok(next) => {
        let sensitivity = f64::from(next.0) / 100.0;
        let Ok(()) = save(
          showing_2,
          &at_3,
          Preference {
            value: &next.0.to_string(),
            option: &format!("sensitivity = {sensitivity:.2}"),
          },
        );
      }
      Err(InvalidSpeed) => {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Settings"),
          "Enter a whole number from -100 to +100",
          None,
        );
      }
    });
    showing.ask_aloud("Pointer speed (%)", Initial(&initial), respond);
  });

  Row::new("Pointer speed", Aside(&aside), edit)
}

fn profile_row(at: &Path) -> Result<Row, Never> {
  let Ok(raw) = preference(at);
  let chosen = match raw.as_str() {
    "adaptive" => 1,
    "flat" => 2,
    _ => 0,
  };
  let names = Profile::ALL
    .into_iter()
    .map(|profile| {
      let Ok(name) = profile.name();

      name.to_owned()
    })
    .collect();
  let at_2 = at.to_path_buf();
  let Ok(control) = choices(names, chosen, move |showing, choice| {
    let Ok(index) = console_core_number_conversion::index(choice);

    match Profile::ALL.get(index) {
      Some(profile) => {
        let Ok(value) = profile.value();
        let Ok(()) = save(
          showing,
          &at_2,
          Preference {
            value,
            option: &format!("accel_profile = {value:?}"),
          },
        );
      }
      None => {}
    }
  });

  Row::picking("Pointer acceleration", control)
}

struct Preference<'a> {
  value: &'a str,
  option: &'a str,
}

fn save(
  showing: &dyn Showing,
  at: &Path,
  Preference { value, option }: Preference<'_>,
) -> Result<(), Never> {
  match writes::whole_with_folders(at, value.as_bytes()) {
    Ok(()) => {
      let Ok(result) = console_compositor::request(
        Request::Script,
        &format!("hl.config({{ input = {{ {option} }} }})"),
      );

      match result {
        DispatchResult::Success => {}
        DispatchResult::Failure(fault) => {
          console_core_logging::warn!("settings-panel: applying pointer setting: {fault}");
          let Ok(()) = console_notifications::notify(
            console_notifications::Application("Settings"),
            "Pointer setting saved",
            Some("It will apply when Console reloads."),
          );
        }
      }

      showing.refresh();
    }
    Err(fault) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Settings"),
        "Could not save pointer setting",
        Some(&fault.to_string()),
      );
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pointer_speed_accepts_the_full_hyprland_range_in_hundredths() {
    for (entered, expected) in [("-100", -100), ("0", 0), (" +69 ", 69), ("100", 100)] {
      assert_eq!(Speed::parse(entered), Ok(Speed(expected)));
    }

    for entered in ["-101", "101", "NaN", "0.69", ""] {
      assert!(matches!(Speed::parse(entered), Err(InvalidSpeed)));
    }
  }
}
