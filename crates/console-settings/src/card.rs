//! The settings, drawn.
//!
//! What is here is the reading of the machine. What each tab holds once it has
//! been read is `crate::rows`, where it can be asked without a
//! machine to ask.
//!
//! Anything that takes a moment, connecting above all, is done off to one side
//! so the panel keeps answering the buttons while it happens.

use console_application_books::appearance::Appearance;
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
  card_with_outputs(arguments, read_outputs)
}

fn card_with_outputs(
  arguments: &[String],
  read: impl Fn() -> Result<Vec<Output>, HyprctlError> + Send + Sync + 'static,
) -> Result<Card, Never> {
  let Ok(card) = Card::supervised::<Settings, _>((), move |looking| pages(looking, read));

  card.opening_at(arguments.first().map(String::as_str))
}

use crate::choosing::{
  Closes, Deeper, Destination, KeyboardMenu, Meeting, Settings, SettingsEffect, SettingsEvent,
  Under, WallpaperView, closes, under,
};
use crate::defaults;
use crate::display;
use crate::languages::{self, Language, Names};
use crate::learned::{self, Following};
use crate::level::{Muted, Step, apply_step};
use crate::light;
use crate::network_code;
use crate::rocker::{preview, sound_server};
use crate::rows::{
  Chosen, Languages, Opens, ScreenScale, SoundControls, adjustment, answer, battery_rows,
  bluetooth_rows, called_row, clock_rows, dictation_rows, dictation_says, engine_says,
  language_picker_rows, language_rows, meeting_rows, notifications_rows, place_rows,
  playback_switch, region_rows, screen_rows, search_rows, security_rows, sound_effects,
  sound_rows, tabs, wifi_rows, zone_rows,
};
use crate::wallpaper::{self, ActiveWallpaper, Choice, Edit, Found, Offered, wallpaper_rows};
use crate::warm::{self, NightShift};
use crate::{bluetooth, screen, size, sound, turning, wifi};
use crate::{hours, named};
use console_actor::Actor;
use console_battery as battery;
use console_bluetooth::device::Device;
use console_bluetooth_agent::command;
use console_compositor::HyprctlError;
use console_compositor::setup::{Output, Outputs};
use console_core_atomic_writes as writes;
use console_core_controls::Switch;
use console_default_applications::editors::{self, Application};
use console_home_screen::shape::{self, Shape};
use console_input_alphabets as alphabets;
use console_notifications::saying::Content as Notification;
use console_panel::before;
use console_panel::card::{Card, Door};
use console_panel::page::{ActionOutcome, Aside, Handler, Level, Page, Rows, Showing};
use console_panel::running::run_output;
use console_panel::tasks::Task;
use console_program_contract::{Effect, Event};
use console_sound_effects::SoundEffects;
use console_wallpaper::choose::{Selection, Set, Wanted};
use console_wallpaper::{place, rules};
use std::io;
use std::path::Path;
use std::process::Output as CommandOutput;
use std::sync::OnceLock;

const NONE_AT_ALL: u32 = 0;

fn pactl(arguments: &[&str]) -> Result<String, Never> {
  run_output(Program::Pactl, arguments)
}

fn of_kind(kind: &str) -> Result<Vec<sound::Thing>, Never> {
  let Ok(said) = pactl(&["-f", "json", "list", &format!("{kind}s")]);

  sound::read(&said)
}

fn hush(target: sound::VolumeTarget) -> Result<Handler, Never> {
  Handler::and_stay(move |showing| {
    let Ok(kind) = target.kind();
    let Ok(items) = of_kind(kind);
    let items_2: Vec<_> = items
      .iter()
      .filter(|item| target.matches(item) == Ok(sound::TargetMatch::Selected))
      .collect();
    let mute = match {
      ::core::primitive::bool::then_some(items_2.iter().all(|item| item.mute == Muted::Yes), ())
    } {
      ::core::option::Option::Some(()) => "0",
      ::core::option::Option::None => "1",
    };

    for item in items_2 {
      let Ok(_) = pactl(&[&format!("set-{kind}-mute"), &item.index.to_string(), mute]);
    }

    showing.refresh();
  })
}

fn select_sound_output(name: &str) -> core::result::Result<(), console_core_never::Never> {
  {
    match sound_server(&["set-default-sink", name]) {
      Ok(_) => {}
      Err(fault) => console_core_logging::warn!("could not select the audio output: {fault}"),
    }
  };

  Ok(())
}

#[derive(PartialEq)]
enum VolumeChange {
  Applied,
  Failed,
}

struct VolumeSetting<'a> {
  kind: &'a str,
  index: i64,
  volume: &'a str,
}

fn set_volume(setting: VolumeSetting<'_>) -> core::result::Result<VolumeChange, Never> {
  let VolumeSetting {
    kind,
    index,
    volume,
  } = setting;
  Ok(
    match sound_server(&[&format!("set-{kind}-volume"), &index.to_string(), volume]) {
      Ok(_) => VolumeChange::Applied,
      Err(fault) => {
        console_core_logging::warn!("could not set the volume: {fault}");
        VolumeChange::Failed
      }
    },
  )
}

fn turn_to(target: sound::VolumeTarget) -> Result<Level, Never> {
  adjustment(move |step| {
    let Ok(kind) = target.kind();
    let Ok(items) = of_kind(kind);
    let items_2: Vec<_> = items
      .iter()
      .filter(|item| target.matches(item) == Ok(sound::TargetMatch::Selected))
      .collect();
    let Ok(level) = sound::loudest(&items_2);
    let Ok(going) = apply_step(level, Step(step));

    match { ::core::primitive::bool::then_some(going == level, ()) } {
      ::core::option::Option::Some(()) => {
        return;
      }
      ::core::option::Option::None => {}
    }

    let Ok(volume) = sound::volume_argument(sound::VolumeStep {
      from: level,
      to: going,
    });

    for item in items_2 {
      match {
        ::core::primitive::bool::then_some(
          ({
            let Ok(explicit_value) = set_volume(VolumeSetting {
              kind,
              index: item.index,
              volume: &volume,
            });
            explicit_value
          }) == VolumeChange::Applied
            && matches!(target, sound::VolumeTarget::Output(_))
            && item.mute == Muted::No,
          (),
        )
      } {
        ::core::option::Option::Some(()) => {
          let Ok(()) = preview(&item.name, Muted::No, Some(i64::from(going)));
        }
        ::core::option::Option::None => {}
      }
    }
  })
}

fn reset_app_volumes(
  app: Option<&sound::App>,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let Ok(playing) = of_kind("sink-input");
    let default = format!("{}%", sound::DEFAULT_APP_VOLUME);

    for stream in playing.iter().filter(|stream| {
      stream.node() == Ok(sound::AudioNode::Application)
        && sound::adjusted(stream) == Ok(sound::StreamVolume::Adjusted)
        && app.is_none_or(|filter| {
          let Ok(identity) = stream.app();
          &identity == filter
        })
    }) {
      match set_volume(VolumeSetting {
        kind: "sink-input",
        index: stream.index,
        volume: &default,
      }) {
        Ok(VolumeChange::Applied | VolumeChange::Failed) => {}
      }
    }
  };

  Ok(())
}

fn set_automatic_leveling(next: Switch) -> Result<(), Never> {
  set_playback_switch(sound::AUTOMATIC_LEVELING, next)
}

fn set_night_mode(next: Switch) -> Result<(), Never> {
  set_playback_switch(sound::NIGHT_MODE, next)
}

fn set_playback_switch(setting: &str, next: Switch) -> Result<(), Never> {
  let Ok(mut wpctl) = Program::Wpctl.command();
  let value = match next {
    Switch::On => "true",
    Switch::Off => "false",
  };
  let response = wpctl.args(["settings", "--save", setting, value]).output();

  report_leveling(setting, response)
}

fn set_app_leveling(key: &str, next: Switch) -> Result<(), Never> {
  let Ok(mut metadata) = Program::PwMetadata.command();
  let value = match next {
    Switch::On => "true",
    Switch::Off => "false",
  };
  let written = metadata
    .args([
      "-n",
      sound::APP_LEVELING,
      "0",
      key,
      value,
      "Spa:String:JSON",
    ])
    .output();

  report_leveling("app leveling", written)
}

fn report_leveling(setting: &str, response: io::Result<CommandOutput>) -> Result<(), Never> {
  match response {
    Ok(output) => match output.status.success().then_some(()) {
      Some(()) => {}
      None => match std::str::from_utf8(&output.stderr) {
        Ok(text) => console_core_logging::warn!("could not change {setting}: {}", text.trim()),
        Err(fault) => console_core_logging::warn!(
          "could not change {setting}: {} with invalid diagnostic text ({fault})",
          output.status,
        ),
      },
    },
    Err(fault) => console_core_logging::warn!("could not change {setting}: {fault}"),
  }

  Ok(())
}

const SINKS: &str = "sinks";
const SPEAKERS: &str = "default sink";
const LEVELING: &str = "automatic leveling";
const NIGHT_MODE: &str = "night mode";

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
  let Ok(playing) = of_kind("sink-input");
  let Ok(speakers) = pactl_kept(SPEAKERS, &["get-default-sink"]);
  let Ok(leveling) = run_and_note(
    LEVELING,
    Program::Wpctl,
    &["settings", sound::AUTOMATIC_LEVELING],
  );
  let Ok(night) = run_and_note(NIGHT_MODE, Program::Wpctl, &["settings", sound::NIGHT_MODE]);
  sound_settings(SoundReadings {
    sinks: &sinks,
    playing: &playing,
    speakers: &speakers,
    leveling: &leveling,
    night: &night,
  })
}

fn sound_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(sinks) = last_output(SINKS);
  let Ok(speakers) = last_output(SPEAKERS);
  let Ok(leveling) = last_output(LEVELING);
  let Ok(night) = last_output(NIGHT_MODE);
  sound_settings(SoundReadings {
    sinks: &sinks,
    playing: &[],
    speakers: &speakers,
    leveling: &leveling,
    night: &night,
  })
}

struct SoundReadings<'a> {
  sinks: &'a str,
  playing: &'a [sound::Thing],
  speakers: &'a str,
  leveling: &'a str,
  night: &'a str,
}

fn sound_settings(readings: SoundReadings<'_>) -> Result<Vec<console_panel::page::Row>, Never> {
  let SoundReadings {
    sinks,
    playing,
    speakers,
    leveling,
    night,
  } = readings;
  let Ok(sinks_2) = sound::read(sinks);
  let bypassed = match {
    ::core::primitive::bool::then_some(
      playing.is_empty()
        || ({
          let Ok(explicit_value) = sound::automatic_leveling(leveling);
          explicit_value
        })
        .is_none(),
      (),
    )
  } {
    ::core::option::Option::Some(()) => None,
    ::core::option::Option::None => {
      let Ok(metadata) = run_output(Program::PwMetadata, &["-n", sound::APP_LEVELING, "0"]);

      {
        let Ok(explicit_value) = sound::bypassed_apps(&metadata);
        explicit_value
      }
    }
  };
  let controls = SoundControls {
    select: (|explicit_argument_0| {
      let Ok(explicit_value) = select_sound_output(explicit_argument_0);
      explicit_value
    }),
    mute: hush,
    volume: turn_to,
    reset: (|explicit_argument_0| {
      let Ok(explicit_value) = reset_app_volumes(explicit_argument_0);
      explicit_value
    }),
    bypassed,
    leveling: set_app_leveling,
  };
  let Ok(mut rows) = sound_rows(&sinks_2, playing, speakers, controls);

  match sound::automatic_leveling(leveling) {
    Ok(Some(now)) => {
      let Ok(row) = playback_switch("Automatic Leveling", now, set_automatic_leveling);
      rows.push(row);

      match (now, sound::automatic_leveling(night)) {
        (Switch::On, Ok(Some(night_switch))) => {
          let Ok(night_row) = playback_switch("Night Mode", night_switch, set_night_mode);
          rows.push(night_row);
        }
        (Switch::Off, _) | (Switch::On, Ok(None)) => {}
      }
    }
    Ok(None) => {}
  }

  let Ok(effects) = SoundEffects::current();
  let Ok(switch) = sound_effects(effects);

  rows.push(switch);

  Ok(rows)
}

fn brightness() -> Result<Option<i32>, Never> {
  let Ok(panel) = screen::here();

  let panel_2 = match panel {
    Some(matched_panel) => matched_panel,
    ::core::option::Option::None => return Ok(None),
  };

  let Ok(now) = panel_2.now();

  Ok(now.map(|now_2| {
    let Ok(points) = panel_2.as_points(now_2);
    let Ok(percent) = fitted(points);
    percent
  }))
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

  battery_controls(&profile)
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

  battery_controls(&profile)
}

fn battery_controls(profile: &str) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(levels) = battery::Levels::here();
  let Ok(mut rows) = battery_rows(Some(profile), levels, guard);
  let Ok(limits) = crate::rows::charge_limit_rows();
  rows.extend(limits);
  Ok(rows)
}

const SCREENS: &str = "screens";

fn read_outputs() -> Result<Vec<Output>, HyprctlError> {
  console_compositor::ask_recorded(Outputs, |answer| {
    let Ok(()) = before::keep(SCREENS, answer.as_bytes());
  })
}

fn refreshed_outputs(
  read: impl FnOnce() -> Result<Vec<Output>, HyprctlError>,
) -> Result<Vec<Output>, Never> {
  Ok(match read() {
    Ok(monitors) => monitors,
    Err(fault) => {
      console_core_logging::warn!("settings-panel: reading screens: {fault}");

      Vec::new()
    }
  })
}

fn screen_orientation(
  screen: Option<&console_screen::Screen>,
) -> Result<(Option<turning::Turn>, console_screen::Shape), Never> {
  Ok(match screen {
    Some(screen_2) => {
      let Ok(turn) = turning::current_turn(screen_2);
      let Ok(shape) = screen_2.shape();

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

fn primary_screen(outputs: &[Output]) -> Result<Option<console_screen::Screen>, Never> {
  let Ok(primary) = display::selected(outputs, None);

  match primary {
    Some(output) => display::screen(output),
    None => Ok(None),
  }
}

fn display_rows(
  held: &ActorAddress,
  outputs: &[Output],
) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(brightness) = brightness();
  let Ok(warmth) = warmth();
  let Ok(primary) = primary_screen(outputs);
  let Ok((_, shape)) = screen_orientation(primary.as_ref());
  let Ok(home) = home_rows(held, shape);
  let Ok(dim) = dim();
  let Ok(room) = room();

  let selected = match held.get() {
    Ok(state) => state.display,
    Err(_stopped) => None,
  };

  let output = {
    let Ok(chosen) = display::selected(outputs, selected.as_deref());
    let optional_output = chosen.or_else(|| {
      let Ok(fallback) = display::selected(outputs, None);
      fallback
    });

    match optional_output {
      Some(matched_output) => matched_output,
      ::core::option::Option::None => {
        return screen_rows(brightness, dim, room, warmth, None, home);
      }
    }
  };

  let address = held.clone();
  let Ok(selector) = display::rows::selector(outputs, output, move |showing, connector| {
    let Ok(()) = press(&address, SettingsEvent::SelectDisplay(connector), showing);
  });
  let mut rows = vec![selector];
  let Ok(settings) = display::rows::settings(output, outputs);
  rows.extend(settings);

  let Ok(screen) = display::screen(output);
  let scale = screen.map(|current| {
    let Ok(zoom) = size::for_screen(&current);
    let Ok(turn) = turning::current_turn(&current);
    ScreenScale {
      connector: Some(&output.connector),
      zoom,
      turn,
    }
  });
  let Ok(controls) = screen_rows(brightness, dim, room, warmth, scale, home);
  rows.extend(controls);

  Ok(rows)
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
      SettingsEffect::Replace(_)
      | SettingsEffect::DisplaySelected
      | SettingsEffect::NetworkShown
      | SettingsEffect::NetworkHidden,
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

  let Ok(attributed) = task.from_application(console_notifications::Application("Settings"));
  attributed.then(move |answer| {
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
    | Destination::Alphabets(_)
    | Destination::Zones
    | Destination::Zone(_)
    | Destination::Clock
    | Destination::Wallpaper(_)
    | Destination::QuickWorkspace => None,
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
  let said = match console_paper::ask(&console_paper::Request::Query) {
    Ok(said) => {
      let Ok(()) = before::keep(UP, said.as_bytes());
      said
    }
    Err(fault) => {
      console_core_logging::warn!("could not query the wallpaper: {fault}");
      let Ok(kept) = last_output(UP);
      kept
    }
  };

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

      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Settings"),
        going_on,
        None,
      );
      showing.run(keeps);
    }
    Err(why) => {
      let Ok(()) = console_notifications::saying::report(
        console_notifications::Application("Settings"),
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

fn offered(table: Option<&Set>) -> Result<Vec<Offered>, Never> {
  let written_down = |name: &str| {
    let set = table?;

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

#[derive(Clone, Copy)]
enum WallpaperReading {
  Live,
  Cached,
}

fn wallpaper_tab(
  looking: &ActorAddress,
  reading: WallpaperReading,
) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(destination) = looking_at(looking);
  let Ok(table) = Set::load();

  match destination {
    Destination::Wallpaper(WallpaperView::Pictures) => wallpaper_sets(looking, table.as_ref()),
    Destination::Wallpaper(WallpaperView::Picture(picture)) => {
      wallpaper_rule(looking, table.as_ref(), &picture.name)
    }
    Destination::Settings
    | Destination::Search
    | Destination::Dictation
    | Destination::Kind(_)
    | Destination::QuickWorkspace
    | Destination::Meeting(_)
    | Destination::Languages
    | Destination::Language(_)
    | Destination::Alphabets(_)
    | Destination::Zones
    | Destination::Zone(_)
    | Destination::Clock => {
      let Ok(up) = match reading {
        WallpaperReading::Cached => was_on_the_screen(),
        WallpaperReading::Live => on_the_screen(),
      };

      wallpaper_at(looking, table.as_ref(), &up)
    }
  }
}

fn wallpaper_sets(
  looking: &ActorAddress,
  table: Option<&Set>,
) -> Result<Vec<console_panel::page::Row>, Never> {
  use console_panel::page::Row;

  let Ok(back) = back_up(looking);
  let Ok(back_2) = Row::back("Wallpaper", back);
  let Ok(title) = Row::naming(wallpaper::SETS, Aside(""));
  let Ok(pictures) = offered(table);
  let bundled: std::collections::HashSet<_> = table
    .into_iter()
    .flat_map(|set| &set.pictures)
    .map(|entry| entry.name.as_str())
    .collect();
  let mut rows = vec![back_2, title];

  match { ::core::primitive::bool::then_some(pictures.is_empty(), ()) } {
    ::core::option::Option::Some(()) => {
      let Ok(empty) = Row::placeholder("There are no pictures on this machine");
      rows.push(empty);
    }
    ::core::option::Option::None => {}
  }

  for picture in pictures {
    let Ok(at) = fitted(rows.len());
    let Ok(custom) = rules::load(&picture.name);
    let status = match custom {
      Some(rule) => match rule.automatic.then_some(()) {
        Some(()) => "Custom",
        None => "Off",
      },
      None => match bundled.contains(picture.name.as_str()).then_some(()) {
        Some(()) => "Default",
        None => "Off",
      },
    };
    let Ok(handler) = open(
      looking,
      Destination::Wallpaper(WallpaperView::Picture(Deeper {
        name: picture.name,
        at,
      })),
    );
    let Ok(row) = Row::new(&picture.says, Aside(status), handler);
    let Ok(row_2) = row.opening();
    rows.push(row_2);
  }

  Ok(rows)
}

fn wallpaper_rule(
  looking: &ActorAddress,
  table: Option<&Set>,
  name: &str,
) -> Result<Vec<console_panel::page::Row>, Never> {
  let bundled = table.and_then(|set| set.pictures.iter().find(|picture| picture.name == name));
  let label = match bundled {
    Some(picture) => picture.says.clone(),
    None => {
      let Ok(picture) = Offered::of(name);
      picture.says
    }
  };
  let default = match bundled {
    Some(picture) => {
      let Ok(rule) = rules::Rule::bundled(picture);
      rule
    }
    None => rules::Rule::default(),
  };
  let Ok(custom) = rules::load(name);
  let (rule, source) = match custom.as_ref() {
    Some(rule) => (rule, wallpaper::RuleSource::Customized),
    None => (&default, wallpaper::RuleSource::Default),
  };
  let Ok(back) = back_up(looking);

  wallpaper::rule_rows(&label, rule, source, back, |edit| {
    let name_2 = name.to_string();
    let default_2 = default.clone();
    let Ok(handler) = Handler::and_stay(move |showing| {
      let Ok(custom_2) = rules::load(&name_2);
      let mut rule_2 = match custom_2 {
        Some(rule_3) => rule_3,
        None => default_2.clone(),
      };
      let saved = match edit {
        Edit::Reset => rules::reset(&name_2),
        Edit::Automatic => {
          rule_2.automatic = !rule_2.automatic;
          rules::save(&name_2, &rule_2)
        }
        Edit::Condition(category, tag) => {
          let Ok(()) = rule_2.toggle(category, tag);
          rules::save(&name_2, &rule_2)
        }
      };

      match saved {
        Ok(()) => showing.refresh(),
        Err(fault) => {
          let Ok(()) = console_notifications::saying::report(
            console_notifications::Application("Settings"),
            "wallpaper-rules",
            Notification {
              summary: "Couldn't save the wallpaper rules",
              body: &fault.to_string(),
            },
          );
        }
      }
    });

    handler
  })
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

fn wallpaper_at(
  looking: &ActorAddress,
  table: Option<&Set>,
  up: &str,
) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(asked) = Wanted::load();

  let up_2 = up.to_string();
  let Ok(pictures) = offered(table);
  let Ok(scenes) = console_wallpaper::scene::every();
  let Ok(scenes_2): Result<Vec<_>, Never> = scenes.iter().map(|name| Offered::of(name)).collect();
  let selection = match &asked.selection {
    Selection::Following => ActiveWallpaper::Following,
    Selection::Still => ActiveWallpaper::Still,
    Selection::Live(name) => match scenes_2.iter().any(|one| one.name == *name).then_some(()) {
      Some(()) => ActiveWallpaper::Live(name),
      None => ActiveWallpaper::Still,
    },
  };
  let Ok(waiting) = dropped();
  let found = Found {
    pictures: &pictures,
    scenes: &scenes_2,
    selection,
    up: &up_2,
    dropped: waiting,
  };
  let Ok(rendering) = console_program_contract::Command::internal(
    InternalProgram::WallpaperRender,
    &[console_core_internal_programs::WALLPAPER_DROPPED.spelling],
  );
  let Ok(renders) = Task::of(rendering);
  let Ok(taking) = Handler::and_stay(move |showing| {
    let Ok(()) = console_notifications::notify(
      console_notifications::Application("Settings"),
      &match waiting {
        1 => "Adding the picture, about a minute".to_string(),
        many => format!("Adding {many} pictures, a minute each"),
      },
      None,
    );
    showing.run(renders.clone());
  });
  let Ok(finding) = Handler::run(&["files", "Pictures"]);
  let Ok(sets) = open(looking, Destination::Wallpaper(WallpaperView::Pictures));

  wallpaper_rows(
    &found,
    |following| {
      let picture = up_2.clone();
      let Ok(said) = picture_label(&pictures, &up_2);
      let requested = match following.then_some(()) {
        Some(()) => Selection::Following,
        None => Selection::Still,
      };
      let going_on = match (following, up_2.is_empty()) {
        (true, _) => "Wallpaper follows time and weather".to_string(),
        (false, true) => "Wallpaper stays as it is".to_string(),
        (false, false) => format!("Wallpaper stays on {said}"),
      };
      let Ok(does) = Handler::and_stay(move |showing| {
        let Ok(()) = ask_for(
          showing,
          &Wanted {
            selection: requested.clone(),
            picture: picture.clone(),
          },
          &going_on,
        );

        showing.refresh();
      });

      does
    },
    |choice| {
      let (wanted, said) = match choice {
        Choice::Picture(name) => {
          let Ok(said) = picture_label(&pictures, name);
          (
            Wanted {
              selection: Selection::Still,
              picture: name.to_string(),
            },
            said,
          )
        }
        Choice::Scene(name) => {
          let Ok(said) = picture_label(&scenes_2, name);
          (
            Wanted {
              selection: Selection::Live(name.to_string()),
              picture: up_2.clone(),
            },
            said,
          )
        }
      };
      let going_on = format!("{said} is going up");
      let Ok(does) = Handler::and_stay(move |showing| {
        let Ok(()) = ask_for(showing, &wanted, &going_on);

        showing.refresh();
      });

      does
    },
    sets,
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
    None => Ok(
      match console_default_applications::opening::default_for(kind.mime) {
        Ok(application) => application,
        Err(_default_unavailable) => String::new(),
      },
    ),
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
          use console_default_applications::opening::{Association, set_default};

          match set_default(Association { mime, desktop: &id }) {
            Ok(()) => {}
            Err(fault) => {
              console_core_logging::warn!("could not associate {mime} with {id}: {fault}")
            }
          }
        }

        match ::core::primitive::bool::then_some(scheme, ()) {
          ::core::option::Option::Some(()) => {
            match console_default_applications::browsers::set_default(&id) {
              Ok(()) => {}
              Err(fault) => {
                console_core_logging::warn!("could not select {id} as default browser: {fault}")
              }
            }
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
      Effect::Custom(
        SettingsEffect::DisplaySelected
        | SettingsEffect::NetworkShown
        | SettingsEffect::NetworkHidden,
      ) => showing.refresh(),

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
    Under::Language | Under::Wallpaper => return setting_rows(looking),
  }

  match onto {
    Destination::Settings => setting_rows(looking),
    Destination::QuickWorkspace => {
      let Ok(back) = back_up(looking);

      crate::quick_workspace::rows(back)
    }
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
    | Destination::Wallpaper(_)
    | Destination::Alphabets(_) => setting_rows(looking),

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
    Destination::Settings | Destination::Wallpaper(_) => {
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
    | Destination::Alphabets(_)
    | Destination::Zones
    | Destination::Zone(_)
    | Destination::Clock
    | Destination::QuickWorkspace => Ok(Vec::new()),
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

  let Ok(active) = console_input_language::active();

  let Ok(heard) = console_input_dictation::languages::current();
  let Ok(listens) = dictation_says(&heard);

  let Ok(words) = open(looking, Destination::Languages);
  let Ok(keyboard) = open(looking, Destination::Alphabets(KeyboardMenu::Closed));
  let Ok(dictation) = open(looking, Destination::Dictation);

  language_rows(
    Languages {
      says: &says,
      types: active.says,
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
    Under::Configuration | Under::Wallpaper => return language_top(looking),
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
    Destination::Alphabets(menu) => {
      let Ok(back) = back_up(looking);
      let Ok(chosen) = alphabets::current();
      let Ok(active) = console_input_language::active();
      let panel = looking.clone();

      crate::keyboard_layouts::rows(menu, &chosen, active, back, move |menu_2, showing| {
        let Ok(()) = press(
          &panel,
          SettingsEvent::Opened(Destination::Alphabets(menu_2)),
          showing,
        );
      })
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
    | Destination::Clock
    | Destination::Wallpaper(_)
    | Destination::QuickWorkspace => language_top(looking),
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
  let Ok(called_2) = called_row(called);
  let Ok(naming) = console_panel::page::Row::naming("On this device", Aside(""));
  let Ok(buttons) = buttons_row();
  let Ok(game) = crate::rows::game_row();
  let Ok(quick) = open(looking, Destination::QuickWorkspace);
  let Ok(quick_row) =
    console_panel::page::Row::new(crate::quick_workspace::TITLE, Aside(""), quick);
  let Ok(quick_row_2) = quick_row.opening();

  Ok(vec![
    search,
    where_,
    clock,
    called_2,
    naming,
    buttons,
    game,
    quick_row_2,
  ])
}

fn setting_rows(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
  let Ok(here) = here();
  let Ok(called) = called();
  let Ok(mut rows) = device_rows(looking, here.as_deref(), &called);
  let Ok(placement) = crate::window_placement::row();
  rows.extend(placement);
  let Ok(pointer) = crate::pointer::rows();
  rows.extend(pointer);
  let Ok(about) = crate::about::rows();
  rows.extend(about);

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
  read_outputs: impl Fn() -> Result<Vec<Output>, HyprctlError> + Send + Sync + 'static,
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
      theme,
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

  let Ok(size_2) = display_page(&size, looking, read_outputs);
  let Ok(status_bar_rows) = tab(crate::status_bar::rows);
  let Ok(status_bar_page) = Page::new(&status_bar, status_bar_rows);
  let Ok(theme_rows) = tab(crate::theme::rows);
  let Ok(theme_page) = Page::new(&theme, theme_rows);
  let Ok(wallpaper_2) = wallpaper_page(&wallpaper, looking);
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
    theme_page,
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
    console_events::again::playback,
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
      | Destination::Alphabets(_)
      | Destination::Zones
      | Destination::Zone(_)
      | Destination::Clock
      | Destination::Wallpaper(_)
      | Destination::QuickWorkspace => ActionOutcome::Close,
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

fn display_page(
  title: &str,
  looking: &ActorAddress,
  read_outputs: impl Fn() -> Result<Vec<Output>, HyprctlError> + Send + Sync + 'static,
) -> Result<Page, Never> {
  let sizing = looking.clone();
  let drawing = looking.clone();

  let Ok(asked) = tab(move || {
    let Ok(outputs) = refreshed_outputs(&read_outputs);

    display_rows(&drawing, &outputs)
  });
  let Ok(page) = Page::new(title, asked);

  let Ok(page_2) = page.meanwhile(move || {
    let Ok(screens) = last_output(SCREENS);
    let outputs = match console_compositor::read(Outputs, &screens) {
      Ok(outputs) => outputs,
      Err(_missing_or_invalid_cache) => Vec::new(),
    };
    let Ok(rows) = display_rows(&sizing, &outputs);

    rows
  });
  page_2.with_subscription(console_program_contract::EventGroup::Compositor, |line| {
    use console_compositor::events::OutputChange;
    use console_events::again::Worth;
    let Ok(event) = console_compositor::events::read(line);
    let Ok(outputs) = console_compositor::events::outputs_changed(line);
    Ok(
      match {
        ::core::primitive::bool::then_some(
          outputs == OutputChange::Changed
            || event == console_compositor::events::CompositorEvent::ConfigurationReloaded,
          (),
        )
      } {
        ::core::option::Option::Some(()) => Worth::Querying,
        ::core::option::Option::None => Worth::Ignoring,
      },
    )
  })
}

fn wallpaper_page(title: &str, looking: &ActorAddress) -> Result<Page, Never> {
  let drawing = looking.clone();
  let waiting = looking.clone();
  let backing = looking.clone();
  let Ok(asked) = tab(move || wallpaper_tab(&drawing, WallpaperReading::Live));
  let Ok(page) = Page::new(title, asked);

  let Ok(page_2) = page.meanwhile(move || {
    let Ok(rows) = wallpaper_tab(&waiting, WallpaperReading::Cached);

    rows
  });
  page_2.on_back(move |showing| {
    let Ok(destination) = looking_at(&backing);

    match destination {
      Destination::Wallpaper(_) => {
        let Ok(()) = went_up(&backing, showing);
        ActionOutcome::Stay
      }
      Destination::Settings
      | Destination::Search
      | Destination::Dictation
      | Destination::Kind(_)
      | Destination::QuickWorkspace
      | Destination::Meeting(_)
      | Destination::Languages
      | Destination::Language(_)
      | Destination::Alphabets(_)
      | Destination::Zones
      | Destination::Zone(_)
      | Destination::Clock => ActionOutcome::Close,
    }
  })
}

fn backed(page: Page, looking: &ActorAddress) -> Result<Page, Never> {
  let backing = looking.clone();

  page.on_back(move |showing| {
    let Ok(was) = looking_at(&backing);

    match matches!(was, Destination::Wallpaper(_)).then_some(()) {
      Some(()) => {
        return ActionOutcome::Close;
      }
      None => {}
    }

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
  use console_events::again::Worth;

  #[test]
  fn the_sound_tab_refreshes_when_an_application_starts_playing() -> Result<(), &'static str> {
    let Ok(page) = sound_page("Sound");
    let subscription = page.listens.ok_or("the Sound tab has no subscription")?;

    assert_eq!(
      subscription.event_group,
      console_program_contract::EventGroup::Sound
    );
    assert_eq!(
      (subscription.worth)("Event 'new' on sink-input #74"),
      Ok(Worth::Querying)
    );

    Ok(())
  }

  #[test]
  fn refreshed_outputs_use_the_supplied_reading() -> Result<(), Never> {
    let outputs = refreshed_outputs(|| {
      console_compositor::read(
        Outputs,
        r#"[{
      "id": 0, "name": "eDP-1", "width": 1920, "height": 1200,
      "scale": 1.171875, "refreshRate": 60.0, "transform": 1, "x": 0, "y": 0
    }]"#,
      )
    })?;
    let Ok(screen) = primary_screen(&outputs);

    assert_eq!(
      screen.and_then(|screen_2| {
        let Ok(size) = size::for_screen(&screen_2);
        size
      }),
      Some(size::Size::Normal)
    );
    assert_eq!(
      screen_orientation(screen.as_ref()),
      Ok((Some(turning::Turn::Left), console_screen::Shape::Taller))
    );

    Ok(())
  }

  #[test]
  fn an_unavailable_compositor_leaves_live_size_and_turn_unknown() -> Result<(), Never> {
    let outputs = refreshed_outputs(|| {
      Err(HyprctlError::Socket(
        Query::EveryMonitor,
        socket::SocketError::Unplaced(socket::Unplaced::Compositorless),
      ))
    })?;

    let Ok(screen) = primary_screen(&outputs);
    assert_eq!(screen, None);
    assert_eq!(
      screen_orientation(screen.as_ref()),
      Ok((None, console_screen::Shape::Wider))
    );

    Ok(())
  }
}
