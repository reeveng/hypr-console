//! The wallpaper is painted, and it is one of the pictures the table names.
//!
//! Two things put something on this screen and the check is different for each.
//! `console-paper.service` brings the wallpaper daemon up and fills the screen
//! with the deepest ground, which is all a machine with no pressed pictures ever
//! shows. `console-wallpaper` then paints a picture over it, which is what the
//! device shows and what the nested desktop has nothing to show.
//!
use console_core_geometry::Point;
use console_core_never::Never;
use console_test_stages::Error;
use console_test_stages::checking::{Body, Check, CheckResult, cannot, expect_ready, same};
use console_test_stages::desktop::{Desktop, Installed};
use console_test_stages::device::{A_MOMENT, Device, Ready};

use crate::Unchecked;

const NO_DIGITS_THERE: &str = "";

const NOTHING_SAID: &str = "";

const WITHIN: i32 = 4;

const LOOKING: u32 = 6;

pub const WALLPAPER: Check = Check {
  name: "150-the-wallpaper",
  about: "The wallpaper is painted, and it is one of the pictures the table names.",
  feature: "wallpaper",
  since: "2026-08-28",
  bodies: &[Body::Desktop(desktop), Body::Device(device)],
};

pub fn picture_names() -> Result<Vec<String>, Unchecked> {
  let Ok(root) = console_test_stages::root();

  let at = root.join("theme/sky.toml");
  let said = console_core_atomic_writes::read_text(&at).map_err(Unchecked::Machine)?;
  let table: console_wallpaper::choose::Set =
    toml::from_str(&said).map_err(Unchecked::Unparsed)?;
  Ok(
    table
      .pictures
      .into_iter()
      .map(|picture| picture.name)
      .collect(),
  )
}

pub fn ground() -> Result<String, Unchecked> {
  let Ok(root) = console_test_stages::root();

  let at = root.join("files/etc/systemd/user/console-paper.service");
  let said = std::fs::read_to_string(&at).map_err(Unchecked::Machine)?;
  let Ok(after) = console_core_ini_files::field(
    &said,
    console_core_ini_files::Under("Service"),
    console_core_ini_files::Key("ExecStart"),
  );

  after
    .and_then(|after_2| after_2.rsplit_once("console-paper serve "))
    .map(|(_, color)| color.trim().to_string())
    .ok_or_else(|| Unchecked::NoGround(at.clone()))
}

pub trait Viewport {
  fn background(&mut self) -> Result<String, Error>;

  fn patch(&mut self, at: Point<f64>) -> Result<String, Error>;
}

impl Viewport for Desktop {
  fn background(&mut self) -> Result<String, Error> {
    Desktop::background(self)
  }

  fn patch(&mut self, at: Point<f64>) -> Result<String, Error> {
    Desktop::patch(self, at)
  }
}

impl Viewport for Device<'_> {
  fn background(&mut self) -> Result<String, Error> {
    Device::background(self)
  }

  fn patch(&mut self, at: Point<f64>) -> Result<String, Error> {
    Device::patch(self, at)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shade {
  Same,
  Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shades<'a> {
  pub one: &'a str,
  pub other: &'a str,
}

pub fn near(shades: Shades<'_>) -> Result<Shade, Never> {
  let Shades { one, other } = shades;
  let band = |said: &str, at: u32| {
    let Ok(at_2) = console_core_number_conversion::index(at);
    let two = match said.get(at_2..at_2.saturating_add(2)) {
      Some(two) => two,
      None => NO_DIGITS_THERE,
    };

    i32::from_str_radix(two, 16)
  };
  let alike = (0..3u32).map(|band_2| band_2.saturating_mul(2)).all(|at| {
    match (band(one, at), band(other, at)) {
      (Ok(one_2), Ok(other_2)) => one_2.saturating_sub(other_2).abs() <= WITHIN,
      _ => false,
    }
  });

  Ok(match ::core::primitive::bool::then_some(alike, ()) {
    ::core::option::Option::Some(()) => Shade::Same,
    ::core::option::Option::None => Shade::Other,
  })
}

pub fn grounded(screen: &mut impl Viewport) -> CheckResult {
  let ground = ground()?;
  let behind = screen.background()?;
  let Ok(near) = near(Shades {
    one: &behind,
    other: &ground,
  });

  same(&near, &Shade::Same, || {
    format!("the screen is #{behind} where the unit fills it with #{ground}")
  })
}

pub fn showing_a_picture(said: &str, names: &[String]) -> Result<String, Unchecked> {
  let path = said
    .rsplit_once("image: ")
    .map(|(_, path)| path.trim())
    .filter(|path| !path.is_empty())
    .ok_or_else(|| {
      let said_2 = said.trim();
      let showing = match ::core::primitive::bool::then_some(said_2.is_empty(), ()) {
        ::core::option::Option::Some(()) => "nothing",
        ::core::option::Option::None => said_2,
      };

      Unchecked::ShowingInstead(showing.to_string())
    })?;
  let named = path
    .rsplit('/')
    .next()
    .and_then(|file| file.strip_suffix(".webp"))
    .map(|name| match name.strip_suffix(".still") {
      Some(moving) => moving,
      None => name,
    });

  let name = match named {
    Some(name) => name,
    None => NOTHING_SAID,
  };

  match ::core::primitive::bool::then_some(names.iter().any(|named_2| named_2 == name), ()) {
    ::core::option::Option::Some(()) => Ok(path.to_string()),
    ::core::option::Option::None => {
      Err(Unchecked::NotInTheTable(path.to_string(), names.to_vec()))
    }
  }
}

fn desktop(stage: &mut Desktop) -> CheckResult {
  let Ok(paper) = console_core_internal_programs::InternalProgram::Paper.name();
  let Ok(installed) = stage.installed(paper);

  match installed {
    Installed::No => cannot("console-paper is not installed on this machine"),
    Installed::Yes => grounded(stage),
  }
}

fn device(stage: &mut Device) -> CheckResult {
  let names = picture_names()?;
  let said = stage.wallpaper()?;

  let picture = showing_a_picture(&said, &names)?;
  let mut clear = Ready::NotYet;

  for _ in 0..LOOKING {
    let Ok(here) = stage.windows_here();

    match ::core::primitive::bool::then_some(here == 0, ()) {
      ::core::option::Option::Some(()) => {
        clear = Ready::Yes;
        break;
      }
      ::core::option::Option::None => {}
    }

    let Ok(was) = stage.workspace();
    let Ok(()) = stage.press("r1");
    let Ok(_) = stage.changed(Device::workspace, &was, A_MOMENT);
  }

  expect_ready(clear, || {
    "could not get to a workspace with nothing on it".to_string()
  })?;

  let ground = ground()?;
  let behind = stage.background()?;
  let Ok(near) = near(Shades {
    one: &behind,
    other: &ground,
  });

  same(&near, &Shade::Other, || {
    format!(
      "the daemon says it is showing {picture}, but the screen is still #{behind}, which \
             is the color the unit fills it with before anything is chosen."
    )
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_color_the_encoder_moved_is_still_the_color() {
    assert_eq!(
      near(Shades {
        one: "65647f",
        other: "656580"
      }),
      Ok(Shade::Same)
    );
    assert_eq!(
      near(Shades {
        one: "65647f",
        other: "302937"
      }),
      Ok(Shade::Other)
    );
  }

  #[test]
  fn the_still_of_a_picture_the_table_names_is_a_picture_the_table_names() -> Result<(), Unchecked>
  {
    let names = vec!["campfire".to_string(), "lazy-river".to_string()];
    let said =
      "eDP-1: currently displaying: image: /usr/share/backgrounds/console/campfire.still.webp";

    showing_a_picture(said, &names)?;
    Ok(())
  }

  #[test]
  fn a_picture_the_table_does_not_name_is_a_failure() {
    let names = vec!["campfire".to_string()];
    let said = "eDP-1: currently displaying: image: /usr/share/backgrounds/console.webp";
    let fault = showing_a_picture(said, &names).expect_err("not in the table");
    assert!(fault.to_string().contains("console.webp"), "{fault}");
  }

  #[test]
  fn a_daemon_showing_no_picture_is_a_failure_that_says_so() {
    let names = vec!["campfire".to_string()];

    for said in [
      "",
      "no daemon is running",
      "eDP-1: currently displaying: color: #110b12",
    ] {
      let fault = showing_a_picture(said, &names).expect_err("no picture");
      assert!(fault.to_string().contains("showing"), "{fault}");
    }
  }

  #[test]
  fn the_ground_is_the_color_the_unit_fills_the_screen_with() -> Result<(), Unchecked> {
    let ground = ground()?;

    assert_eq!(ground.len(), 6, "{ground:?}");
    assert!(
      ground.chars().all(|digit| digit.is_ascii_hexdigit()),
      "{ground:?}"
    );

    Ok(())
  }

  #[test]
  fn the_table_names_some_pictures() -> Result<(), Unchecked> {
    let names = picture_names()?;

    assert!(!names.is_empty(), "theme/sky.toml names no pictures");

    Ok(())
  }
}
