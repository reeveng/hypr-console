//! The settings, drawn.
//!
//! What is here is the reading of the machine. What each tab holds once it has
//! been read is `crate::rows`, where it can be asked without a
//! machine to ask.
//!
//! Anything that takes a moment, connecting above all, is done off to one side
//! so the panel keeps answering the buttons while it happens.

use console_books::appearance::Appearance;
use console_core_arguments::{Command, Operands};
use console_core_external_programs::Program;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_number_conversion::fitted;

pub const WHO: &str = "settings-panel";

pub const COMMAND: Command = Command {
  name: "settings-panel",
  about: "the settings, each kind in its own place",
  flags: &[],
  operands: Operands::Optional("TAB"),
};

pub fn door(arguments: &[String]) -> Result<Door, Never> {
  Door::closing_at("settings", arguments.first().map(String::as_str))
}

pub fn card(arguments: &[String]) -> Result<Card, Never> {
  card_with_monitors(arguments, read_monitors)
}

pub fn card_with_monitors(
  arguments: &[String],
  read: impl Fn() -> Result<Vec<Monitor>, HyprctlError> + Send + Sync + 'static,
) -> Result<Card, Never> {
  let Ok(card) = Card::supervised::<Settings, _>((), move |looking| pages(looking, read));

  card.opening_at(arguments.first().map(String::as_str))
}

use crate::choosing::{
  Closes, Deeper, Destination, Meeting, Settings, SettingsEffect, SettingsEvent, Under, closes,
  under,
};
use crate::defaults;
use crate::languages::{self, Language, Names};
use crate::learned::{self, Following};
use crate::level::{Muted, Step, apply_step};
use crate::light;
use crate::network_code;
use crate::rocker::{preview, sound_server};
use crate::rows::{
  Chosen, Languages, Opens, adjustment, alphabet_rows, answer, battery_rows, bluetooth_rows,
  called_row, clock_rows, dictation_rows, dictation_says, engine_says, language_picker_rows,
  language_rows, meeting_rows, notifications_rows, place_rows, region_rows, screen_rows,
  search_rows, security_rows, sound_effects, sound_rows, tabs, wifi_rows, zone_rows,
};
use crate::wallpaper::{Found, Offered, wallpaper_rows};
use crate::warm::{self, NightShift};
use crate::{bluetooth, screen, size, sound, turning, wifi};
use crate::{hours, named};
use console_actor::Actor;
use console_battery as battery;
use console_bluetooth::device::Device;
use console_bluetooth_agent::command;
use console_compositor::{HyprctlError, Monitor, Monitors};
use console_core_atomic_writes as writes;
use console_default_applications::editors::{self, Application};
use console_home_screen::shape::{self, Shape};
use console_input_alphabets as alphabets;
use console_panel::before;
use console_panel::card::{Card, Door};
use console_panel::page::{ActionOutcome, Aside, Handler, Level, Page, Rows, Showing};
use console_panel::running::{Notification, run_output, say};
use console_panel::tasks::Task;
use console_program_contract::{Effect, Event};
use console_sound_effects::SoundEffects;
use console_wallpaper::choose::{Set, Wanted};
use console_wallpaper::place;
use std::path::Path;
use std::sync::OnceLock;

const NONE_AT_ALL: u32 = 0;

fn pactl(arguments: &[&str]) -> Result<String, Never> {
  run_output(Program::Pactl, arguments)
}

fn of_kind(kind: &str) -> Result<Vec<sound::Thing>, Never> {
  let Ok(said) = pactl(&["-f", "json", "list", &format!("{kind}s")]);

  sound::read(&said)
}

fn hush(index: i64, kind: &'static str) -> Result<Handler, Never> {
  Handler::and_stay(move |_| {
    let Ok(_) = pactl(&[&format!("set-{kind}-mute"), &index.to_string(), "toggle"]);
  })
}

fn turn_to(index: i64, kind: &'static str) -> Result<Level, Never> {
  adjustment(move |step| {
    let Ok(of_kind) = of_kind(kind);

    let Ok(one) = sound::one(&of_kind, index);

    let thing = match one.cloned() {
      Some(thing) => thing,
      None => return,
    };

    let Ok(level) = thing.level();
    let Ok(going) = apply_step(level, Step(step));

    let changed = sound_server(&[
      &format!("set-{kind}-volume"),
      &index.to_string(),
      &format!("{going}%"),
    ]);

    match changed {
      Ok(_) if kind == "sink" && !thing.mute && going != level => {
        let Ok(()) = preview(&thing.name, Muted::No, Some(i64::from(going)));
      }
      Ok(_) => {}
      Err(fault) => console_core_logging::warn!("could not set the volume: {fault}"),
    }
  })
}

const SINKS: &str = "sinks";
const SPEAKERS: &str = "default sink";

fn run_and_note(note: &str, program: Program, arguments: &[&str]) -> Result<String, Never> {
  before::run_and_note(note, program, arguments)
}

fn last_output(note: &str) -> Result<String, Never> {
  before::last(note)
}

fn pactl_kept(note: &str, arguments: &[&str]) -> Result<String, Never> {
  run_and_note(note, Program::Pactl, arguments)
}

fn sound_tab() -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(sinks) = pactl_kept(SINKS, &["-f", "json", "list", "sinks"]);
  let Ok(mut playing) = of_kind("sink-input");
  playing.retain(|stream| {
    !stream.properties.contains_key("console.balance.output")
      && !stream.properties.contains_key("console.balance.preview")
  });
  let Ok(speakers) = pactl_kept(SPEAKERS, &["get-default-sink"]);

  let Ok(sinks_2) = sound::read(&sinks);
  let Ok(mut rows) = sound_rows(&sinks_2, &playing, &speakers, hush, turn_to);
  let Ok(effects) = SoundEffects::current();
  let Ok(switch) = sound_effects(effects);

  rows.push(switch);

  Ok(rows)
}

fn sound_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(sinks) = last_output(SINKS);
  let Ok(speakers) = last_output(SPEAKERS);

  let Ok(sinks_2) = sound::read(&sinks);
  let Ok(mut rows) = sound_rows(&sinks_2, &[], &speakers, hush, turn_to);
  let Ok(effects) = SoundEffects::current();
  let Ok(switch) = sound_effects(effects);

  rows.push(switch);

  Ok(rows)
}

fn brightness() -> Result<i32, Never> {
  let Ok(panel) = screen::here();

  let points = match panel {
    Some(panel_2) => {
      let Ok(now) = panel_2.now();

      match now {
        Some(now_2) => panel_2.as_points(now_2)?,
        None => 0,
      }
    }
    None => 0,
  };

  fitted(points)
}

fn dim() -> Result<Level, Never> {
  adjustment(|step| {
    let way = match ::core::primitive::bool::then_some(step > 0, ()) {
      ::core::option::Option::Some(()) => screen::Way::Up,
      ::core::option::Option::None => screen::Way::Down,
    };

    let Ok(panel) = screen::here();

    match panel {
      Some(panel_2) => {
        let Ok(now) = panel_2.now();

        match now {
          Some(now_2) => {
            let Ok(going) = panel_2.step_from(now_2, way);

            let ::core::result::Result::Ok(_) = panel_2.set(going);
          }
          None => {}
        }
      }
      None => {}
    }
  })
}

fn guard(step: battery::Step) -> Result<Level, Never> {
  adjustment(move |way| {
    let Ok(levels) = battery::Levels::here();
    let Ok(at) = levels.at(step);
    let Ok(going) = apply_step(at, Step(way));
    let Ok(_) = levels.set(step, going);
  })
}

const PROFILE: &str = "power profile";

fn battery_tab() -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(profile) = run_and_note(PROFILE, Program::Powerprofilesctl, &["get"]);

  let Ok(levels) = battery::Levels::here();

  battery_rows(Some(&profile), levels, guard)
}

fn warmth() -> Result<NightShift, Never> {
  let Ok(said) = console_core_places::home();

  let home = match said {
    Some(home) => home,
    None => return Ok(NightShift::Scheduled),
  };

  let Ok(standing) = warm::load(&home);

  match standing {
    warm::Standing::Loaded(warmth) => Ok(warmth),
    warm::Standing::Invalid(fault) => {
      let Ok(at) = warm::at(&home);

      console_core_logging::warn!(
        "settings-panel: {} says whether the screen follows the clock, and it will not \
                 be read: {fault}. The row is drawn as though it were following, which is what a \
                 machine no one has told does.",
        at.display()
      );

      Ok(NightShift::Scheduled)
    }
  }
}

fn battery_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(profile) = last_output(PROFILE);

  let Ok(levels) = battery::Levels::here();

  battery_rows(Some(&profile), levels, guard)
}

const SCREENS: &str = "screens";

fn read_monitors() -> Result<Vec<Monitor>, HyprctlError> {
  console_compositor::ask_recorded(Monitors, |answer| {
    let Ok(()) = before::keep(SCREENS, answer.as_bytes());
  })
}

fn refreshed_monitors(
  read: impl FnOnce() -> Result<Vec<Monitor>, HyprctlError>,
) -> Result<Vec<Monitor>, Never> {
  Ok(match read() {
    Ok(monitors) => monitors,
    Err(fault) => {
      console_core_logging::warn!("settings-panel: reading screens: {fault}");

      Vec::new()
    }
  })
}

fn screen_orientation(
  monitors: &[Monitor],
) -> Result<(Option<turning::Turn>, console_screen::Shape), Never> {
  let Ok(shown) = console_screen::shown(monitors);

  Ok(match shown {
    Some(screen) => {
      let Ok(turn) = turning::current_turn(&screen);
      let Ok(shape) = screen.shape();

      (turn, shape)
    }
    None => (None, console_screen::Shape::Wider),
  })
}

fn room() -> Result<Option<Following>, Never> {
  let Ok(found) = light::here();

  match found {
    Some(_a_machine_with_eyes) => {}
    None => return Ok(None),
  }

  let Ok(home) = console_core_places::home();

  let home_3 = match home {
    Some(home_2) => home_2,
    None => return Ok(None),
  };

  let Ok(following) = learned::following(&home_3);

  Ok(Some(following))
}

fn size_rows(
  held: &ActorAddress,
  monitors: &[Monitor],
) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(brightness) = brightness();
  let Ok(warmth) = warmth();
  let Ok((turned, shape)) = screen_orientation(monitors);
  let Ok(home) = home_rows(held, shape);
  let Ok(standing) = size::current_size(monitors);

  let Ok(dim) = dim();

  let Ok(room) = room();

  screen_rows(Some(brightness), dim, room, warmth, standing, turned, home)
}

fn home_standing() -> Result<console_screen::Shape, Never> {
  Ok(match console_home_screen::standing() {
    Ok(standing) => standing,
    Err(fault) => {
      console_core_logging::warn!("settings-panel: which way up the screen stands: {fault}");

      console_screen::Shape::Wider
    }
  })
}

fn home_at(standing: console_screen::Shape) -> Result<Option<std::path::PathBuf>, Never> {
  let Ok(said) = console_core_places::home();

  Ok(match said {
    Some(home) => {
      let Ok(at) = shape::at(&home, standing);

      Some(at)
    }
    None => None,
  })
}

fn home_shape() -> Result<Shape, Never> {
  let Ok(standing) = home_standing();

  home_shape_at(standing)
}

fn home_shape_at(standing: console_screen::Shape) -> Result<Shape, Never> {
  let Ok(usual) = Shape::usual(standing);
  let Ok(home) = home_at(standing);

  let at = match home {
    Some(at) => at,
    None => return Ok(usual),
  };

  let Ok(held) = console_core_atomic_writes::read(&at);

  Ok(match held {
    console_core_atomic_writes::Stored::Text(said) => {
      let Ok(shape) = Shape::read(&said, standing);

      shape
    }
    console_core_atomic_writes::Stored::Absent => usual,
    console_core_atomic_writes::Stored::Failed(fault) => {
      console_core_logging::warn!("settings-panel: {}: {fault}", at.display());

      usual
    }
  })
}

fn home_set(shape: Shape) -> Result<(), Never> {
  let Ok(standing) = home_standing();
  let Ok(home) = home_at(standing);

  let at = match home {
    Some(at) => at,
    None => {
      console_core_logging::warn!(
        "settings-panel: this machine will not say whose home to write the grid in"
      );

      return Ok(());
    }
  };

  match at.parent().map(std::fs::create_dir_all) {
    Some(Err(fault)) => {
      console_core_logging::warn!("settings-panel: {}: {fault}", at.display());

      return Ok(());
    }
    Some(Ok(())) | None => {}
  }

  let Ok(written) = shape.serialize();

  match console_core_atomic_writes::whole(&at, written.as_bytes()) {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!("settings-panel: {}: {fault}", at.display());

      return Ok(());
    }
  }

  match console_panel::door::send_to_home(console_panel::door::PadInput::Again) {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!("settings-panel: the home screen was not told: {fault}")
    }
  }

  Ok(())
}

fn home_rows(
  held: &ActorAddress,
  standing: console_screen::Shape,
) -> Result<Vec<console_panel::page::Row>, Never> {
  let across = held.clone();
  let down = held.clone();
  let sized = held.clone();
  let Ok(shape) = home_shape_at(standing);

  let Ok(cross) = adjustment(move |step| {
    let Ok(shape_2) = home_shape();
    let Ok(()) = grid(
      &across,
      SettingsEvent::Across {
        shape: shape_2,
        step,
      },
    );
  });
  let Ok(descend) = adjustment(move |step| {
    let Ok(shape_3) = home_shape();
    let Ok(()) = grid(
      &down,
      SettingsEvent::Down {
        shape: shape_3,
        step,
      },
    );
  });
  let Ok(resize) = adjustment(move |step| {
    let Ok(shape_4) = home_shape();
    let Ok(()) = grid(
      &sized,
      SettingsEvent::Sized {
        shape: shape_4,
        step,
      },
    );
  });

  crate::rows::home_rows(shape, cross, descend, resize)
}

fn grid(held: &ActorAddress, heard: SettingsEvent) -> Result<(), Never> {
  let effects = match held.send(Event::Custom(heard)) {
    Ok(effects) => effects,
    Err(_the_actor_has_gone) => {
      console_core_logging::warn!(
        "settings-panel: the panel's own state is missing, so the grid did not move"
      );

      Vec::new()
    }
  };

  for shape in effects.iter().filter_map(|effect| {
    let Ok(shape) = shaped(effect);

    shape
  }) {
    let Ok(()) = home_set(*shape);
  }

  Ok(())
}

fn shaped(effect: &Effect<SettingsEffect>) -> Result<Option<&shape::Shape>, Never> {
  Ok(match effect {
    Effect::Custom(SettingsEffect::HomeScreen(shape)) => Some(shape),

    Effect::Custom(
      SettingsEffect::Replace(_) | SettingsEffect::NetworkShown | SettingsEffect::NetworkHidden,
    )
    | Effect::Run(_)
    | Effect::Stream(_)
    | Effect::Fork(_)
    | Effect::Interrupt(_)
    | Effect::Prompt(_)
    | Effect::Spawn(_)
    | Effect::Subscribe(_)
    | Effect::Unsubscribe(_)
    | Effect::Write(_)
    | Effect::Notify(_)
    | Effect::Print(_)
    | Effect::Stop(_) => None,
  })
}

pub(crate) fn join(network: wifi::Network, known: wifi::Known) -> Result<Handler, Never> {
  Handler::and_stay(move |showing| {
    let name = network.name.clone();

    match (known, network.locked) {
      (wifi::Known::Yes, true | false) | (wifi::Known::No, false) => {
        let Ok(plain) = wifi::joining(&name, known);
        let Ok(joins) = joining(&name, plain);

        showing.run(joins);
      }
      (wifi::Known::No, true) => {
        let asking = name.clone();

        let Ok(respond) = answer(move |showing_2, word| {
          let Ok(typed) = wifi::joining_with(&asking, wifi::Password(word));
          let Ok(joins) = joining(&asking, typed);

          showing_2.run(joins);
        });
        showing.ask(&format!("Password for {name}"), respond);
      }
    }
  })
}

fn joining(name: &str, command: console_program_contract::Command) -> Result<Task, Never> {
  let name_2 = name.to_string();
  let Ok(task) = Task::of(command);

  task.then(move |answer| {
    let Ok(said) = wifi::joined(&name_2, answer);

    Ok(Some(said))
  })
}

fn notifications_tab() -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(held) = console_notifications::serving::load_inbox();

  notifications_rows(held.do_not_disturb)
}

const WIFI_RADIO: &str = "wifi radio";
const KNOWN: &str = "wifi known";
const IN_RANGE: &str = "wifi in range";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Wifi<'a> {
  radio: &'a str,
  known: &'a str,
  in_range: &'a str,
}

fn wifi_at(
  said: Wifi<'_>,
  looking: &ActorAddress,
) -> Result<Vec<console_panel::page::Row>, Never> {
  let on = wifi::on(said.radio)?;
  let networks = wifi::networks(said.in_range)?;
  let saved = wifi::saved(said.known)?;

  wifi_rows(on, networks, &saved, join, |network| {
    let held = looking.clone();
    Ok(move |showing: &dyn Showing| {
      let Ok(()) = press(&held, SettingsEvent::ShowNetwork(network.clone()), showing);
    })
  })
}

fn wifi_tab(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(shown) = shared_network(looking);

  match shown {
    Some(network) => return network_code::rows(&network),
    None => {}
  }

  let Ok(radio) = run_and_note(WIFI_RADIO, Program::Nmcli, &["radio", "wifi"]);
  let Ok(known) = run_and_note(KNOWN, Program::Nmcli, &wifi::KNOWN);
  let Ok(in_range) = run_and_note(
    IN_RANGE,
    Program::Nmcli,
    &["-t", "-f", wifi::FIELDS, "device", "wifi", "list"],
  );

  wifi_at(
    Wifi {
      radio: &radio,
      known: &known,
      in_range: &in_range,
    },
    looking,
  )
}

fn wifi_meanwhile(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(shown) = shared_network(looking);

  match shown {
    Some(_) => return Ok(Vec::new()),
    None => {}
  }

  let Ok(radio) = last_output(WIFI_RADIO);
  let Ok(known) = last_output(KNOWN);
  let Ok(in_range) = last_output(IN_RANGE);

  wifi_at(
    Wifi {
      radio: &radio,
      known: &known,
      in_range: &in_range,
    },
    looking,
  )
}

fn look_again(showing: &dyn Showing) -> Result<(), Never> {
  let Ok(rescanning) =
    console_program_contract::Command::external(Program::Nmcli, &["device", "wifi", "rescan"]);
  let Ok(rescans) = Task::of(rescanning);

  showing.run(rescans);

  Ok(())
}

const BLUETOOTH: &str = "bluetooth";

fn meeting(looking: &ActorAddress) -> Result<impl Opens, Never> {
  let held = looking.clone();

  Ok(move |device: &Device, at: u32, showing: &dyn Showing| {
    let meeting = Meeting {
      address: device.address,
      at,
    };
    let Ok(()) = press(
      &held,
      SettingsEvent::Opened(Destination::Meeting(meeting)),
      showing,
    );
  })
}

fn bluetooth_at(
  looking: &ActorAddress,
  answer: &[u8],
) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(objects) = bluetooth::read(answer);
  let Ok(shown) = bluetooth::shown(&objects);
  let Ok(power) = objects.power();
  let Ok(discovery) = objects.discovery();
  let Ok(onto) = looking_at(looking);

  let standing = match onto {
    Destination::Meeting(meeting) => shown
      .iter()
      .find(|device| device.address == meeting.address)
      .cloned(),
    Destination::Settings
    | Destination::Search
    | Destination::Dictation
    | Destination::Kind(_)
    | Destination::Languages
    | Destination::Language(_)
    | Destination::Alphabets
    | Destination::Zones
    | Destination::Zone(_)
    | Destination::Clock => None,
  };

  match standing {
    Some(device) => {
      let Ok(back) = back_up(looking);

      meeting_rows(&device, back)
    }
    None => {
      let Ok(opens) = meeting(looking);

      bluetooth_rows(power, discovery, shown, opens)
    }
  }
}

fn bluetooth_tab(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(answer) = bluetooth::asked();
  let Ok(()) = before::keep(BLUETOOTH, &answer);

  bluetooth_at(looking, &answer)
}

fn bluetooth_meanwhile(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(kept) = before::kept(BLUETOOTH);

  bluetooth_at(looking, &kept)
}

const UP: &str = "wallpaper";

fn on_the_screen() -> Result<String, Never> {
  let Ok(said) = run_and_note(UP, Program::Awww, &["query"]);

  place::current_picture(&said)
}

fn was_on_the_screen() -> Result<String, Never> {
  let Ok(said) = last_output(UP);

  place::current_picture(&said)
}

#[derive(Debug)]
enum Unchosen {
  NoOnes,
  Holding(std::path::PathBuf, std::io::Error),
  Writing(console_core_atomic_writes::Unwritten),
}

impl std::fmt::Display for Unchosen {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Unchosen::NoOnes => {
        write!(to, "This machine will not say whose home to write it in.")
      }
      Unchosen::Holding(at, fault) => {
        write!(to, "{} could not be made: {fault}", at.display())
      }
      Unchosen::Writing(fault) => write!(to, "{fault}"),
    }
  }
}

impl std::error::Error for Unchosen {}

fn write_down(wanted: &Wanted) -> Result<(), Unchosen> {
  let Ok(asked) = place::config_path();

  let at = asked.ok_or(Unchosen::NoOnes)?;

  match at.parent() {
    Some(holding) => std::fs::create_dir_all(holding)
      .map_err(|fault| Unchosen::Holding(holding.to_path_buf(), fault))?,
    None => {}
  }

  let Ok(written) = wanted.serialize();

  console_core_atomic_writes::whole(&at, written.as_bytes()).map_err(Unchosen::Writing)
}

fn ask_for(showing: &dyn Showing, wanted: &Wanted, going_on: &str) -> Result<(), Never> {
  match write_down(wanted) {
    Ok(()) => {
      let Ok(keeping) = console_program_contract::Command::internal(
        InternalProgram::Wallpaper,
        &[console_wallpaper::keeping::NOW.spelling],
      );
      let Ok(keeps) = Task::of(keeping);

      showing.note(going_on);
      showing.run(keeps);
    }
    Err(why) => {
      let Ok(()) = say(
        "wallpaper-choice",
        Notification {
          summary: "Couldn't change the wallpaper",
          body: &why.to_string(),
        },
      );
    }
  }

  Ok(())
}

fn offered() -> Result<Vec<Offered>, Never> {
  let Ok(table_at) = place::table();

  let held = std::fs::read_to_string(table_at);

  let table = match held.as_deref() {
    Ok(said) => {
      let Ok(set) = Set::read(said);

      set
    }
    Err(_unreadable) => None,
  };
  let written_down = |name: &str| {
    let set = table.as_ref()?;

    set
      .pictures
      .iter()
      .find(|picture| picture.name == name)
      .map(|picture| Offered {
        name: picture.name.clone(),
        says: picture.says.clone(),
        by: picture.by.clone(),
      })
  };
  let Ok(every) = place::every();

  Ok(
    every
      .into_iter()
      .map(|name| match written_down(&name) {
        Some(offered) => offered,
        None => {
          let Ok(offered) = Offered::of(&name);

          offered
        }
      })
      .collect(),
  )
}

fn dropped() -> Result<u32, Never> {
  let Ok(location) = place::dropped();
  let path = match location {
    Some(path) => path,
    None => return Ok(NONE_AT_ALL),
  };
  let entries = match std::fs::read_dir(&path) {
    Ok(entries) => entries,
    Err(fault) => match (fault.kind() == std::io::ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(NONE_AT_ALL),
      None => {
        console_core_logging::warn!(
          "could not read dropped wallpapers at {}: {fault}",
          path.display()
        );
        return Ok(NONE_AT_ALL);
      }
    },
  };

  fitted(
    entries
      .filter_map(|entry| match entry {
        Ok(found) => match std::fs::metadata(found.path()) {
          Ok(metadata) => metadata.is_file().then_some(()),
          Err(fault) => match (fault.kind() == std::io::ErrorKind::NotFound).then_some(()) {
            Some(()) => None,
            None => {
              console_core_logging::warn!(
                "could not check dropped wallpaper {}: {fault}",
                found.path().display()
              );
              None
            }
          },
        },
        Err(fault) => {
          console_core_logging::warn!("could not read a dropped wallpaper entry: {fault}");
          None
        }
      })
      .count(),
  )
}

fn wallpaper_tab() -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(up) = on_the_screen();

  wallpaper_at(&up)
}

fn wallpaper_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(up) = was_on_the_screen();

  wallpaper_at(&up)
}

fn picture_label(pictures: &[Offered], name: &str) -> Result<String, Never> {
  let found = pictures
    .iter()
    .find(|picture| picture.name == name)
    .map(|one| one.says.clone());

  Ok(match found {
    Some(says) => says,
    None => {
      let Ok(offered) = Offered::of(name);

      offered.says
    }
  })
}

fn wallpaper_at(up: &str) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(asked) = Wanted::load();

  let up_2 = up.to_string();
  let Ok(pictures) = offered();
  let Ok(waiting) = dropped();
  let found = Found {
    pictures: &pictures,
    following: asked.follow,
    up: &up_2,
    dropped: waiting,
  };
  let Ok(rendering) = console_program_contract::Command::internal(
    InternalProgram::WallpaperRender,
    &[console_core_internal_programs::WALLPAPER_DROPPED.spelling],
  );
  let Ok(renders) = Task::of(rendering);
  let Ok(taking) = Handler::and_stay(move |showing| {
    showing.note(&match waiting {
      1 => "Adding the picture, about a minute".to_string(),
      many => format!("Adding {many} pictures, a minute each"),
    });
    showing.run(renders.clone());
  });
  let Ok(finding) = Handler::run(&["files", "Pictures"]);

  wallpaper_rows(
    &found,
    |following| {
      let picture = up_2.clone();
      let Ok(said) = picture_label(&pictures, &up_2);
      let going_on = match (following, up_2.is_empty()) {
        (true, _) => "Wallpaper follows the weather".to_string(),
        (false, true) => "Wallpaper stays as it is".to_string(),
        (false, false) => format!("Wallpaper stays on {said}"),
      };
      let Ok(does) = Handler::and_stay(move |showing| {
        let Ok(()) = ask_for(
          showing,
          &Wanted {
            follow: following,
            picture: picture.clone(),
          },
          &going_on,
        );

        showing.refresh();
      });

      does
    },
    |name| {
      let picture = name.to_string();
      let Ok(said) = picture_label(&pictures, name);
      let going_on = format!("{said} is going up");
      let Ok(does) = Handler::and_stay(move |showing| {
        let Ok(()) = ask_for(
          showing,
          &Wanted {
            follow: false,
            picture: picture.clone(),
          },
          &going_on,
        );

        showing.refresh();
      });

      does
    },
    taking,
    finding,
  )
}

fn opening(kind: &defaults::Kind, applications: &[Application]) -> Result<String, Never> {
  match kind.editor {
    Some(editor) => {
      let Ok(app) = editor.preferred(kind.mime, applications);

      Ok(match app.map(|app_2| app_2.id.clone()) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => ::std::string::String::new(),
      })
    }
    None => run_output(Program::XdgMime, &["query", "default", kind.mime]),
  }
}

fn use_it(
  looking: &ActorAddress,
  kind: &defaults::Kind,
  application: &Application,
) -> Result<Handler, Never> {
  let Ok(every) = kind.every();

  let every_2: Vec<String> = every.map(str::to_string).collect();
  let scheme = kind.mime.starts_with("x-scheme-handler/");
  let editor = kind.editor;
  let id = application.id.clone();
  let looking_2 = looking.clone();

  Handler::and_stay(move |showing| {
    match editor {
      Some(editor_2) => {
        let Ok(()) = editor_2.choose(&id);
      }
      None => {
        for mime in &every_2 {
          let Ok(_) = run_output(Program::XdgMime, &["default", &id, mime]);
        }

        match ::core::primitive::bool::then_some(scheme, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(_) = run_output(Program::XdgSettings, &["set", "default-web-browser", &id]);
          }
          ::core::option::Option::None => {}
        }
      }
    }

    let Ok(()) = went_up(&looking_2, showing);
  })
}

type ActorAddress = Actor<Settings>;

fn looking_at(held: &ActorAddress) -> Result<Destination, Never> {
  Ok(match held.get() {
    Ok(state) => state.destination,
    Err(_the_actor_has_gone) => Destination::Settings,
  })
}

fn shared_network(held: &ActorAddress) -> Result<Option<wifi::Network>, Never> {
  Ok(match held.get() {
    Ok(state) => state.shared_network,
    Err(fault) => {
      console_core_logging::warn!("could not read the active Wi-Fi share: {fault}");
      None
    }
  })
}

fn press(held: &ActorAddress, heard: SettingsEvent, showing: &dyn Showing) -> Result<(), Never> {
  let effects = match held.send(Event::Custom(heard)) {
    Ok(effects) => effects,
    Err(_the_actor_has_gone) => {
      console_core_logging::warn!(
        "settings-panel: the panel's own state is missing, so the press did nothing"
      );

      Vec::new()
    }
  };

  for effect in effects {
    match effect {
      Effect::Custom(SettingsEffect::Replace(row)) => {
        let Ok(row_2) = console_core_number_conversion::fitted(row);

        showing.replace(row_2)
      }
      Effect::Custom(SettingsEffect::HomeScreen(shape)) => {
        let Ok(()) = home_set(shape);
      }
      Effect::Custom(SettingsEffect::NetworkShown | SettingsEffect::NetworkHidden) => {
        showing.refresh()
      }

      Effect::Run(_)
      | Effect::Stream(_)
      | Effect::Fork(_)
      | Effect::Interrupt(_)
      | Effect::Prompt(_)
      | Effect::Spawn(_)
      | Effect::Subscribe(_)
      | Effect::Unsubscribe(_)
      | Effect::Write(_)
      | Effect::Notify(_)
      | Effect::Print(_)
      | Effect::Stop(_) => {}
    }
  }

  Ok(())
}

fn went_up(held: &ActorAddress, showing: &dyn Showing) -> Result<(), Never> {
  press(held, SettingsEvent::Back, showing)
}

fn back_up(held: &ActorAddress) -> Result<impl Chosen, Never> {
  let address = held.clone();

  Ok(move |showing: &dyn Showing| {
    let Ok(()) = went_up(&address, showing);
  })
}

fn open(held: &ActorAddress, onto: Destination) -> Result<Handler, Never> {
  let held_2 = held.clone();

  Handler::and_stay(move |showing| {
    let Ok(()) = press(&held_2, SettingsEvent::Opened(onto.clone()), showing);
  })
}

fn defaults_tab(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(onto) = looking_at(looking);
  let Ok(under) = under(&onto);

  match under {
    Under::Configuration => {}
    Under::Language => return setting_rows(looking),
  }

  match onto {
    Destination::Settings => setting_rows(looking),
    Destination::Search => {
      let Ok(back) = back_up(looking);

      let Ok(chosen) = console_default_applications::engines::current();

      search_rows(&chosen, back)
    }
    Destination::Zones => {
      let Ok(back) = back_up(looking);
      let Ok(zones) = zones();
      let Ok(regions) = hours::regions(&zones);
      let Ok(here) = here();

      region_rows(&regions, here.as_deref(), back, |at, region| {
        open(
          looking,
          Destination::Zone(Deeper {
            name: region.to_string(),
            at,
          }),
        )
      })
    }
    Destination::Zone(deeper) => {
      let Ok(back) = back_up(looking);
      let Ok(zones) = zones();
      let Ok(places) = hours::places(&zones, &deeper.name);
      let Ok(here) = here();

      zone_rows(&deeper.name, &places, here.as_deref(), back)
    }
    Destination::Clock => {
      let Ok(back) = back_up(looking);
      let Ok(now) = console_default_applications::clock::clock();

      clock_rows(now, back)
    }
    Destination::Meeting(_)
    | Destination::Dictation
    | Destination::Languages
    | Destination::Language(_)
    | Destination::Alphabets => setting_rows(looking),

    Destination::Kind(at) => {
      let leaving = looking.clone();
      let chosen = looking.clone();

      let Ok(at_2) = console_core_number_conversion::index(at);

      let kind = match defaults::KINDS.get(at_2) {
        Some(kind) => kind,
        None => return Ok(Vec::new()),
      };

      let Ok(applications) = editors::installed();

      defaults::choice_rows(
        kind,
        &applications,
        &|kind_3| opening(kind_3, &applications),
        move |showing| {
          let Ok(()) = went_up(&leaving, showing);
        },
        move |kind_2, application| use_it(&chosen, kind_2, application),
      )
    }
  }
}

fn defaults_meanwhile(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(onto) = looking_at(looking);

  match onto {
    Destination::Settings => {
      let Ok(zone) = last_output(TIMEZONE);
      let Ok(here) = hours::parse_zone(&zone);
      let Ok(name) = last_output(HOSTNAME);
      let Ok(called) = named::read(&name);
      let Ok(mut rows) = device_rows(looking, here.as_deref(), &called);
      let Ok(kinds) = defaults::meanwhile_rows(|at| open(looking, Destination::Kind(at)));

      rows.extend(kinds);

      Ok(rows)
    }
    Destination::Search
    | Destination::Dictation
    | Destination::Kind(_)
    | Destination::Meeting(_)
    | Destination::Languages
    | Destination::Language(_)
    | Destination::Alphabets
    | Destination::Zones
    | Destination::Zone(_)
    | Destination::Clock => Ok(Vec::new()),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Description<'a>(&'a str);

fn run(at: &str, what: Description<'_>) -> Result<String, Never> {
  let Ok(held) = writes::read(Path::new(at));

  Ok(match held {
    writes::Stored::Text(said) => said,
    writes::Stored::Absent => String::new(),
    writes::Stored::Failed(fault) => {
      console_core_logging::warn!(
        "settings-panel: {at} is {}, and it will not be read: {fault}. The list is \
                 drawn empty, which is a machine that says it can do nothing rather than one \
                 that quietly offers half.",
        what.0
      );

      String::new()
    }
  })
}

fn names() -> Result<&'static Names, Never> {
  #[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
      explicit044_no_ambient_value,
      reason = "the names glibc gives the languages it ships, which are part of the installed system and do not change while a panel is up; the panel is an actor whose state is the row someone is looking at, and a table of language names is not that"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit068_no_global_mutable_state",
    expect(
      explicit068_no_global_mutable_state,
      reason = "the process parses the installed ISO name tables once and shares an immutable lookup table"
    )
  )]
  static ASKED: OnceLock<Names> = OnceLock::new();

  Ok(ASKED.get_or_init(|| {
    let Ok(names) = Names::here();

    names
  }))
}

fn supported_languages() -> Result<&'static Vec<Language>, Never> {
  #[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
      explicit044_no_ambient_value,
      reason = "what glibc can be asked to make, read off the installed system and unchanged while a panel is up; which of them has been made is the half that changes, and `generated` reads that again at every draw"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit068_no_global_mutable_state",
    expect(
      explicit068_no_global_mutable_state,
      reason = "the process discovers installed locales once and only borrows the resulting snapshot"
    )
  )]
  static ASKED: OnceLock<Vec<Language>> = OnceLock::new();

  Ok(ASKED.get_or_init(|| {
    let Ok(said) = run(
      languages::SUPPORTED,
      Description("every language glibc can make"),
    );
    let Ok(supported) = languages::supported(&said);
    let Ok(names) = names();
    let Ok(spoken) = languages::languages(&supported, names);

    spoken
  }))
}

fn generated() -> Result<Vec<String>, Never> {
  let Ok(said) = run_output(Program::Locale, &["-a"]);

  languages::generated(&said)
}

fn lang() -> Result<Option<String>, Never> {
  let Ok(said) = run_output(Program::Localectl, &["status"]);

  languages::parse_lang(&said)
}

const ZONES: &str = "zones";
const TIMEZONE: &str = "timezone";
const HOSTNAME: &str = "hostname";

fn zones() -> Result<Vec<String>, Never> {
  let Ok(said) = run_and_note(ZONES, Program::Timedatectl, &["list-timezones"]);

  hours::zones(&said)
}

fn here() -> Result<Option<String>, Never> {
  let Ok(said) = run_and_note(
    TIMEZONE,
    Program::Timedatectl,
    &["show", "--property=Timezone", "--value"],
  );

  hours::parse_zone(&said)
}

fn called() -> Result<String, Never> {
  let Ok(said) = run_and_note(HOSTNAME, Program::Hostnamectl, &["--static"]);

  named::read(&said)
}

fn language_top(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(lang) = lang();
  let Ok(spoken) = supported_languages();
  let Ok(names) = names();
  let Ok(standing) = languages::current_locale(spoken, lang.as_deref());

  let says = match &standing {
    Some(locale) => {
      let Ok(says) = names.says(locale);

      says
    }
    None => match lang {
      Some(lang_2) => lang_2,
      None => String::new(),
    },
  };

  let Ok(chosen) = alphabets::current();
  let Ok(types) = alphabets::describe(&chosen);

  let Ok(heard) = console_input_dictation::languages::current();
  let Ok(listens) = dictation_says(&heard);

  let Ok(words) = open(looking, Destination::Languages);
  let Ok(keyboard) = open(looking, Destination::Alphabets);
  let Ok(dictation) = open(looking, Destination::Dictation);

  language_rows(
    Languages {
      says: &says,
      types: &types,
      listens: &listens,
    },
    [words, keyboard, dictation],
  )
}

fn language_tab(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(onto) = looking_at(looking);
  let Ok(under) = under(&onto);

  match under {
    Under::Language => {}
    Under::Configuration => return language_top(looking),
  }

  match onto {
    Destination::Languages => {
      let Ok(back) = back_up(looking);
      let Ok(spoken) = supported_languages();
      let Ok(lang) = lang();
      let Ok(standing) = languages::current_locale(spoken, lang.as_deref());

      language_picker_rows(spoken, standing.as_ref(), back, |at, language| {
        open(
          looking,
          Destination::Language(Deeper {
            name: language.to_string(),
            at,
          }),
        )
      })
    }
    Destination::Language(deeper) => {
      let Ok(back) = back_up(looking);
      let Ok(spoken) = supported_languages();
      let Ok(names) = names();
      let Ok(lang) = lang();
      let Ok(standing) = languages::current_locale(spoken, lang.as_deref());
      let Ok(generated) = generated();

      let language = match spoken
        .iter()
        .find(|language| language.language == deeper.name)
      {
        Some(language) => language,
        None => return language_top(looking),
      };

      place_rows(language, names, &generated, standing.as_ref(), back)
    }
    Destination::Alphabets => {
      let Ok(back) = back_up(looking);
      let Ok(chosen) = alphabets::current();

      alphabet_rows(&chosen, back)
    }
    Destination::Dictation => {
      let Ok(back) = back_up(looking);
      let Ok(chosen) = console_input_dictation::languages::current();

      dictation_rows(&chosen, back)
    }
    Destination::Settings
    | Destination::Search
    | Destination::Kind(_)
    | Destination::Meeting(_)
    | Destination::Zones
    | Destination::Zone(_)
    | Destination::Clock => language_top(looking),
  }
}

fn where_row(
  looking: &ActorAddress,
  here: Option<&str>,
) -> Result<console_panel::page::Row, Never> {
  let Ok(says) = crate::rows::where_you_are();

  let zone = match here {
    Some(zone) => zone.replace('_', " "),
    None => String::new(),
  };

  let Ok(opens) = open(looking, Destination::Zones);
  let Ok(row) = console_panel::page::Row::new(&says, Aside(&zone), opens);

  row.opening()
}

fn clock_row(looking: &ActorAddress) -> Result<console_panel::page::Row, Never> {
  let Ok(now) = console_default_applications::clock::clock();
  let Ok(reads) = now.says();
  let Ok(says) = crate::rows::the_clock();
  let Ok(opens) = open(looking, Destination::Clock);
  let Ok(row) = console_panel::page::Row::new(&says, Aside(reads), opens);

  row.opening()
}

fn search_row(looking: &ActorAddress) -> Result<console_panel::page::Row, Never> {
  let Ok(engine) = console_default_applications::engines::current();
  let Ok(says) = engine_says(&engine);
  let Ok(opens) = open(looking, Destination::Search);
  let Ok(row) = console_panel::page::Row::new("Search Engine", Aside(&says), opens);

  row.opening()
}

fn buttons_row() -> Result<console_panel::page::Row, Never> {
  let Ok(mapping) = InternalProgram::MappingPanel.path();
  let Ok(runs) = Handler::run(&[mapping]);
  let Ok(row) = console_panel::page::Row::new("Buttons", Aside(""), runs);

  row.opening()
}

fn device_rows(
  looking: &ActorAddress,
  here: Option<&str>,
  called: &str,
) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(search) = search_row(looking);
  let Ok(where_) = where_row(looking, here);
  let Ok(clock) = clock_row(looking);
  let Ok(called) = called_row(called);
  let Ok(naming) = console_panel::page::Row::naming("On this device", Aside(""));
  let Ok(buttons) = buttons_row();
  let Ok(game) = crate::rows::game_row();

  Ok(vec![search, where_, clock, called, naming, buttons, game])
}

fn setting_rows(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(here) = here();
  let Ok(called) = called();
  let Ok(mut rows) = device_rows(looking, here.as_deref(), &called);
  let Ok(applications) = editors::installed();
  let Ok(kinds) =
    defaults::defaults_rows(&applications, &|kind| opening(kind, &applications), |at| {
      open(looking, Destination::Kind(at))
    });
  rows.extend(kinds);

  Ok(rows)
}

fn tab(
  of: impl Fn() -> Result<Vec<console_panel::page::Row>, Never> + Send + Sync + 'static,
) -> Result<Rows, Never> {
  Rows::computed(move || {
    let Ok(rows) = of();

    rows
  })
}

fn pages(
  looking: &ActorAddress,
  read_monitors: impl Fn() -> Result<Vec<Monitor>, HyprctlError> + Send + Sync + 'static,
) -> Result<Vec<Page>, Never> {
  let Ok(
    [
      sound,
      bluetooth,
      wifi,
      battery,
      notifications,
      size,
      status_bar,
      wallpaper,
      language,
      configuration,
      books,
      security,
    ],
  ) = tabs();

  let Ok(sound_2) = sound_page(&sound);
  let Ok(bluetooth_2) = bluetooth_page(&bluetooth, looking);
  let Ok(wifi_2) = wifi_page(&wifi, looking);
  let Ok(battery_2) = battery_page(&battery);

  let Ok(asked) = tab(notifications_tab);
  let Ok(notifications_2) = Page::new(&notifications, asked);

  let Ok(size_2) = size_page(&size, looking, read_monitors);
  let Ok(status_bar_rows) = tab(crate::status_bar::rows);
  let Ok(status_bar_page) = Page::new(&status_bar, status_bar_rows);
  let Ok(wallpaper_2) = wallpaper_page(&wallpaper);
  let Ok(language_2) = language_page(&language, looking);
  let Ok(configuration_2) = configuration_page(&configuration, looking);

  let Ok(asked_2) = tab(|| {
    let Ok(now) = Appearance::current();

    crate::rows::books_rows(now)
  });
  let Ok(books_2) = Page::new(&books, asked_2);

  let Ok(asked_3) = tab(security_rows);
  let Ok(security_2) = Page::new(&security, asked_3);

  Ok(vec![
    sound_2,
    bluetooth_2,
    wifi_2,
    battery_2,
    notifications_2,
    size_2,
    status_bar_page,
    wallpaper_2,
    language_2,
    configuration_2,
    books_2,
    security_2,
  ])
}

fn sound_page(title: &str) -> Result<Page, Never> {
  let Ok(asked) = tab(sound_tab);
  let Ok(page) = Page::new(title, asked);
  let Ok(page_2) = page.meanwhile(move || {
    let Ok(rows) = sound_meanwhile();

    rows
  });

  page_2.with_subscription(
    console_program_contract::EventGroup::Sound,
    console_events::again::sound,
  )
}

fn bluetooth_page(title: &str, looking: &ActorAddress) -> Result<Page, Never> {
  let drawing_bluetooth = looking.clone();
  let waiting_bluetooth = looking.clone();
  let backing_bluetooth = looking.clone();

  let Ok(asked) = tab(move || bluetooth_tab(&drawing_bluetooth));
  let Ok(page) = Page::new(title, asked);
  let Ok(page_2) = page.meanwhile(move || {
    let Ok(rows) = bluetooth_meanwhile(&waiting_bluetooth);

    rows
  });
  let Ok(searching) = command::command_line(&command::Action::Search);
  let words: Vec<&str> = searching.iter().map(String::as_str).collect();
  let Ok(watch) = console_panel::page::Watch::command(&words);
  let Ok(page_3) = page_2.with_watch(watch);

  page_3.on_back(move |showing| {
    let Ok(was) = looking_at(&backing_bluetooth);

    match was {
      Destination::Meeting(_) => {
        let Ok(()) = press(&backing_bluetooth, SettingsEvent::Back, showing);

        ActionOutcome::Stay
      }
      Destination::Settings
      | Destination::Search
      | Destination::Dictation
      | Destination::Kind(_)
      | Destination::Languages
      | Destination::Language(_)
      | Destination::Alphabets
      | Destination::Zones
      | Destination::Zone(_)
      | Destination::Clock => ActionOutcome::Close,
    }
  })
}

fn wifi_page(title: &str, looking: &ActorAddress) -> Result<Page, Never> {
  let drawing = looking.clone();
  let waiting = looking.clone();
  let sharing = looking.clone();
  let Ok(asked) = tab(move || wifi_tab(&drawing));
  let Ok(page) = Page::new(title, asked);
  let Ok(page_2) = page.meanwhile(move || {
    let Ok(rows) = wifi_meanwhile(&waiting);

    rows
  });
  let Ok(page_3) = page_2.on_back(move |showing| {
    let closed = match sharing.send(Event::Custom(SettingsEvent::HideNetwork)) {
      Ok(effects) => effects
        .iter()
        .any(|effect| matches!(effect, Effect::Custom(SettingsEffect::NetworkHidden))),
      Err(fault) => {
        console_core_logging::warn!("could not close the active Wi-Fi share: {fault}");
        false
      }
    };
    let Ok(()) = network_code::put_away();

    match closed.then_some(()) {
      Some(()) => {
        showing.refresh();
        ActionOutcome::Stay
      }
      None => ActionOutcome::Close,
    }
  });

  page_3.on_arriving(|showing| {
    let Ok(()) = look_again(showing);
  })
}

fn battery_page(title: &str) -> Result<Page, Never> {
  let Ok(asked) = tab(battery_tab);
  let Ok(page) = Page::new(title, asked);

  page.meanwhile(move || {
    let Ok(rows) = battery_meanwhile();

    rows
  })
}

fn size_page(
  title: &str,
  looking: &ActorAddress,
  read_monitors: impl Fn() -> Result<Vec<Monitor>, HyprctlError> + Send + Sync + 'static,
) -> Result<Page, Never> {
  let sizing = looking.clone();
  let drawing_size = looking.clone();

  let Ok(asked) = tab(move || {
    let Ok(monitors) = refreshed_monitors(&read_monitors);

    size_rows(&drawing_size, &monitors)
  });
  let Ok(page) = Page::new(title, asked);

  page.meanwhile(move || {
    let Ok(screens) = last_output(SCREENS);
    let monitors = match console_compositor::read(Monitors, &screens) {
      Ok(monitors) => monitors,
      Err(_no_readable_cache) => Vec::new(),
    };
    let Ok(rows) = size_rows(&sizing, &monitors);

    rows
  })
}

fn wallpaper_page(title: &str) -> Result<Page, Never> {
  let Ok(asked) = tab(wallpaper_tab);
  let Ok(page) = Page::new(title, asked);

  page.meanwhile(move || {
    let Ok(rows) = wallpaper_meanwhile();

    rows
  })
}

fn backed(page: Page, looking: &ActorAddress) -> Result<Page, Never> {
  let backing = looking.clone();

  page.on_back(move |showing| {
    let Ok(was) = looking_at(&backing);
    let Ok(()) = press(&backing, SettingsEvent::Back, showing);

    let Ok(closes) = closes(&was);

    match closes {
      Closes::Yes => ActionOutcome::Close,
      Closes::No => ActionOutcome::Stay,
    }
  })
}

fn language_page(title: &str, looking: &ActorAddress) -> Result<Page, Never> {
  let drawing_language = looking.clone();

  let Ok(asked) = tab(move || language_tab(&drawing_language));
  let Ok(page) = Page::new(title, asked);

  backed(page, looking)
}

fn configuration_page(title: &str, looking: &ActorAddress) -> Result<Page, Never> {
  let drawing = looking.clone();
  let waiting = looking.clone();

  let Ok(asked) = tab(move || defaults_tab(&drawing));
  let Ok(page) = Page::new(title, asked);
  let Ok(page_2) = page.meanwhile(move || {
    let Ok(rows) = defaults_meanwhile(&waiting);

    rows
  });

  backed(page_2, looking)
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_compositor::{Query, socket};

  #[test]
  fn refreshed_monitors_use_the_supplied_reading() -> Result<(), Never> {
    let supplied = Monitor {
      named: String::from("eDP-1"),
      size: Some((1920, 1200)),
      scale: Some(1.171875),
      refresh: Some(60.0),
      transform: Some(1),
    };
    let monitors = refreshed_monitors(|| Ok(vec![supplied]))?;

    assert_eq!(size::current_size(&monitors), Ok(Some(size::Size::Normal)));
    assert_eq!(
      screen_orientation(&monitors),
      Ok((Some(turning::Turn::Left), console_screen::Shape::Taller))
    );

    Ok(())
  }

  #[test]
  fn an_unavailable_compositor_leaves_live_size_and_turn_unknown() -> Result<(), Never> {
    let monitors = refreshed_monitors(|| {
      Err(HyprctlError::Socket(
        Query::Monitors,
        socket::SocketError::Unplaced(socket::Unplaced::Compositorless),
      ))
    })?;

    assert_eq!(size::current_size(&monitors), Ok(None));
    assert_eq!(
      screen_orientation(&monitors),
      Ok((None, console_screen::Shape::Wider))
    );

    Ok(())
  }
}
