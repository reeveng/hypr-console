//! The compositor's configuration, with the device's screen swapped for this one.
//! The bar starts after the set of outputs has held steady for 1.5 seconds,
//! since the stage replaces the initial output while it settles.
//! Whatever the session starts here, it starts the way the device starts it and
//! from the same place the device reads. The stage is the device's tree copied
//! somewhere else with every absolute path inside it pointed back into the
//! copy, so a line here that names `/usr/local` names the staged one by the
//! time it is written -- `staging::rewritten` is the whole of that, and this
//! file is written through it rather than around it.  The keyboard is the
//! reason that is a rule and not a habit. This script used to start it by its
//! bare name off the staged `PATH`, which is one program started the way
//! nothing on the device starts anything: `keyboard-toggle` signals a command
//! line, that command line was the name and not the path, and X did nothing on
//! this stage for as long as it existed. The command is read off
//! `console-input-keyboard.service` now, so the flags the device types with are
//! the flags this stage types with, and neither is written down twice.

use console_core_geometry::Size;
use console_core_ini_files::{Key, Under, field};
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_number_conversion::whole_u32;
use console_screen::Screen;

pub const UNIT: &str = "files/etc/systemd/user/console-input-keyboard.service";

pub fn started_by(unit: &str) -> Result<Option<String>, Never> {
  let started = field(unit, Under("Service"), Key("ExecStart"))?;

  Ok(
    started
      .filter(|command| command.starts_with('/'))
      .map(str::to_string),
  )
}

pub const START: &str = "session-start";
pub const KEYBOARD: &str = "session-start.keyboard";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wallpaper {
  Started,
  LeftOut,
}

pub fn start_session(at: &std::path::Path) -> std::io::Result<()> {
  use std::collections::BTreeSet;
  use std::io::ErrorKind;
  use std::process::{Command, Stdio};
  use std::time::Duration;

  let keyboard = match std::fs::read_to_string(at.with_file_name(KEYBOARD)) {
    Ok(unit) => unit,
    Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(()),
      None => return Err(fault),
    },
  };

  let Ok(schedule) =
    console_waiting::Schedule::asking_every(Duration::from_secs(8), Duration::from_millis(250));
  let previous = BTreeSet::new();
  let steady = 0_u32;
  let Ok(_) = console_waiting::until_some_handed(
    schedule,
    &mut (previous, steady),
    |(previous_2, steady_2)| {
      let screens: BTreeSet<_> = match console_compositor::ask(console_compositor::EveryMonitor) {
        Ok(monitors) => monitors.into_iter().map(|monitor| monitor.named).collect(),
        Err(_compositor_starting) => BTreeSet::new(),
      };
      *steady_2 = match ::core::primitive::bool::then_some(
        !screens.is_empty() && &screens == previous_2,
        (),
      ) {
        ::core::option::Option::Some(()) => steady_2.saturating_add(1),
        ::core::option::Option::None => 0,
      };
      *previous_2 = screens;

      Ok::<_, Never>((*steady_2 >= 6).then_some(()))
    },
  );

  let launch = |program: &str, arguments: &[&str]| {
    let mut child = Command::new(program);
    child.args(arguments).stdin(Stdio::null());

    match console_program_lifetime::let_go(&mut child) {
      Ok(_) => {}
      Err(fault) => console_core_logging::warn!("console-desktop: starting {program}: {fault}"),
    }
  };
  let Ok(events) = InternalProgram::Events.name();
  let Ok(bar) = InternalProgram::Bar.name();
  launch(events, &[]);
  launch(bar, &[]);
  let words: Vec<_> = keyboard.split_whitespace().collect();

  match words.split_first() {
    Some((program, arguments)) => launch(program, arguments),
    None => {}
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the compositor's output scale is a dimensionless ratio of device pixels to logical pixels"
  )
)]
#[derive(Debug, Clone, Copy)]
pub struct Scale(pub f64);

fn monitor(
  output: &str,
  mode: Size<u32>,
  screen: &Screen,
  Scale(scale): Scale,
) -> Result<String, Never> {
  console_compositor::lua::MonitorRule {
    output,
    size: mode,
    refresh: Some(screen.refresh),
    scale: console_compositor::setup::OutputScale(scale),
    transform: screen.transform,
  }
  .lua()
}

pub fn in_a_window(screen: &Screen, scale: Scale) -> Result<String, Never> {
  let (wide, tall) = (screen.mode.width, screen.mode.height);
  let shown = |size: u32| {
    let Ok(shown) = whole_u32(f64::from(size) * scale.0 / screen.scale);

    shown
  };

  monitor(
    "WAYLAND-1",
    Size {
      width: shown(wide),
      height: shown(tall),
    },
    screen,
    scale,
  )
}

pub fn headless(screen: &Screen) -> Result<String, Never> {
  let (wide, tall) = (screen.mode.width, screen.mode.height);
  let mode = Size {
    width: wide,
    height: tall,
  };
  let Ok(first) = monitor("HEADLESS-1", mode, screen, Scale(screen.scale));
  let Ok(second) = monitor("HEADLESS-2", mode, screen, Scale(screen.scale));
  let Ok(window) = monitor(
    "WAYLAND-1",
    Size {
      width: 320,
      height: 200,
    },
    screen,
    Scale(1.0),
  );

  Ok(format!("{first}\n{second}\n{window}"))
}

pub fn made_headless(screen: &Screen) -> Result<String, Never> {
  monitor("HEADLESS-1", screen.mode, screen, Scale(screen.scale))
}

const THE_COLOR_NOTHING_IS: &str = "\
-- On the device the bare background is the darkest color in the palette, so
-- that a wallpaper arriving a second after the compositor does not announce
-- itself with a flash of something else. Here that is the one color it must
-- not be: the wallpaper rests at that same color, so a screen nothing painted
-- and a screen the wallpaper painted would read alike, and a check comparing them
-- would pass with no wallpaper at all. Nothing is ever this.";

const THE_DEVICES_OWN: &str = "\
-- The ground is left the device's own, because this session starts no
-- wallpaper daemon to paint over it and no one here has to tell a screen the
-- wallpaper painted from a screen nothing did.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Names<'a> {
  pub screen: &'a str,
  pub device: &'a str,
}

pub fn configuration(said: Names<'_>, wallpaper: Wallpaper) -> Result<String, Never> {
  let Names {
    screen: screen_said,
    device: device_configuration,
  } = said;

  let ground = match wallpaper {
    Wallpaper::Started => {
      let Ok(color) = console_compositor::lua::background_color("rgb(ff00ff)");

      format!("{THE_COLOR_NOTHING_IS}\n{color}")
    }
    Wallpaper::LeftOut => THE_DEVICES_OWN.to_owned(),
  };
  let Ok(disable_device_panel) = console_compositor::lua::disable_monitor("eDP-1");
  let Ok(device_configuration_2) = console_compositor::quote(device_configuration);
  let hide_watchdog = console_compositor::lua::DISABLE_WATCHDOG_WARNING;

  Ok(format!(
    "\
-- The device's compositor configuration, with its screen swapped for this one.
--
-- Nothing else is changed. The window rules, the bindings, the look and the
-- absence of animation are all read from the file the device reads, so a
-- change tried here is a change tried there.

{screen_said}

dofile({device_configuration})

-- Said again after the device's configuration, because that one names the screen the
-- device is mounted on and this is not that screen.
{disable_device_panel}

{ground}

-- The bar Hyprland draws over the screen when it was not started by
-- start-hyprland, which is otherwise in every picture taken here.
{hide_watchdog}
",
    device_configuration = device_configuration_2
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  const GO: Screen = Screen {
    mode: console_core_geometry::Size {
      width: 2560,
      height: 1600,
    },
    refresh: 144,
    scale: 2.5,
    transform: console_screen::Transform::Degrees90,
  };

  #[test]
  fn a_picture_is_taken_at_the_devices_own_pixels() {
    let Ok(said) = headless(&GO);

    assert!(said.contains("2560x1600@144"), "{said}");
    assert!(said.contains("HEADLESS-1") && said.contains("HEADLESS-2"));
  }

  #[test]
  fn a_window_too_large_for_this_screen_gives_up_pixels_and_not_layout() {
    let Ok(said) = in_a_window(&GO, Scale(1.25));

    assert!(said.contains("1280x800@144"), "{said}");
    assert!(said.contains("scale = 1.25"), "{said}");
  }

  #[test]
  fn the_nested_config_reads_the_devices_own() {
    let Ok(said) = configuration(
      Names {
        screen: "-- a screen",
        device: "/somewhere/hyprland.lua",
      },
      Wallpaper::Started,
    );

    assert!(said.contains(r#"dofile("/somewhere/hyprland.lua")"#));
    assert!(
      said.contains("rgb(ff00ff)"),
      "the background has to be a color nothing is"
    );
  }

  #[test]
  fn a_panel_without_wallpaper_uses_the_device_background() {
    let Ok(said) = configuration(
      Names {
        screen: "-- a screen",
        device: "/somewhere/hyprland.lua",
      },
      Wallpaper::LeftOut,
    );

    assert!(
      !said.contains("ff00ff"),
      "nothing paints this ground, so a color nothing is is all anyone would see: {said}"
    );
  }
}
