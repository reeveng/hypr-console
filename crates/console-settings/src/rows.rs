//! What each tab holds, as a function of what the machine said.
//!
//! Reading the machine is one thing and knowing what to draw from it is
//! another. Everything here is the second, so the shape of every tab can be
//! asked without a machine to ask.

use std::collections::BTreeSet;
use std::sync::Arc;

use console_core_external_programs::Program;
use console_core_internal_programs::InternalProgram;
use console_core_localization::text;
use console_core_never::Never;
use console_default_applications::engines;
use console_home_screen::shape::Shape;
use console_lock_screen::idle::{self, Delay};
use console_lock_screen::{LockScreen, Method};
use console_login_window::stored_pattern::StoredPattern;

use crate::words::Word;
use console_application_books::appearance::{
  self, Appearance, ColorRole, Paint, TYPEFACES, Typeface,
};
use console_bluetooth::adapter::{Discovery, Power};
use console_bluetooth::agent::{LONGEST_PIN, PinCode};
use console_bluetooth::device::{Connection, Device, Pairing};
use console_bluetooth_agent::agent::{self, PinEntry};
use console_bluetooth_agent::cards;
use console_bluetooth_agent::command::{self, Action, Once};
use console_core_controls::Switch;
use console_input_dictation::languages;
use console_notifications::reading::DoNotDisturb;
use console_panel::page::{
  ActionOutcome, Active, Answer, Aside, ButtonPress, Choices, Handler, Initial, Level, NOW, Row,
  Showing, Subject, YET,
};
use console_panel::tasks::Task;
use console_program_contract::Command;
use console_response_times::measuring::{self, Measuring};

use console_default_applications::clock::{self, Clock};

use crate::hours::{self, Place};
use crate::languages::{Language, Locale, Made, Names, generation_state};
use crate::learned::Following;
use crate::level::{CELLS, Muted, bar, slider};
use crate::machine::MachineSetting;
use crate::named::{self, Allowed};
use crate::power::{self, Request};
use crate::power_profile::Profile;
use crate::size::{EVERY, Size};
use crate::turning::{self, Turn};
use crate::warm::NightShift;
use crate::{bluetooth, sound, wifi};
use console_sound_effects::SoundEffects;

const NOWHERE_SAID: &str = "";

pub const ON: &str = "On";

pub const OFF: &str = "Off";

pub fn switch(says: &str, state: Aside<'_>, command: Command) -> Result<Row, Never> {
  let Ok(task) = Task::of(command);

  task_row(says, state, task)
}

fn task_row(says: &str, state: Aside<'_>, task: Task) -> Result<Row, Never> {
  let Ok(runs) = Handler::and_stay(move |showing| showing.run(task.clone()));

  Row::new(says, state, runs)
}

pub fn turned_by(says: &str, now: Switch, command: Command) -> Result<Row, Never> {
  let Ok(row) = switch(says, Aside(""), command);

  row.switched(now)
}

fn turned(says: &str, now: Switch, turning: Handler) -> Result<Row, Never> {
  let Ok(row) = Row::new(says, Aside(""), turning);

  row.switched(now)
}

pub(crate) fn choices(
  said: Vec<String>,
  chosen: u32,
  choose: impl Fn(&dyn Showing, u32) + Send + Sync + 'static,
) -> Result<Choices, Never> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the panel's choice control clones rows between drawing and input handling"
    )
  )]
  let shared = Arc::new(choose);

  Ok(Choices {
    said,
    chosen,
    choose: shared,
  })
}

pub(crate) fn adjustment(adjust: impl Fn(i32) + Send + Sync + 'static) -> Result<Level, Never> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the panel's slider control clones rows between drawing and input handling"
    )
  )]
  let level = Arc::new(adjust);

  Ok(level)
}

pub(crate) fn answer(
  respond: impl Fn(&dyn Showing, &str) + Send + Sync + 'static,
) -> Result<Answer, Never> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the panel keeps and clones its text response while handling the active prompt"
    )
  )]
  let response = Arc::new(respond);

  Ok(response)
}

pub struct Offered {
  pub said: String,
  pub runs: Command,
}

fn picking_among(
  says: &str,
  offered: Vec<Offered>,
  chosen: u32,
  task: impl Fn(Command) -> Result<Task, Never>,
) -> Result<Row, Never> {
  let (said, tasks): (Vec<String>, Vec<Task>) = offered
    .into_iter()
    .map(|offer| {
      let Ok(task_2) = task(offer.runs);

      (offer.said, task_2)
    })
    .unzip();

  let Ok(control) = choices(said, chosen, move |showing, which| {
    let Ok(at) = console_core_number_conversion::index(which);

    match tasks.get(at) {
      Some(task_3) => showing.run(task_3.clone()),
      None => {}
    }
  });

  Row::picking(says, control)
}

pub fn one_of<T: Copy + PartialEq>(
  named: &str,
  choices: (&[T], Option<T>),
  offer: impl Fn(T) -> Result<Offered, Never>,
) -> Result<Vec<Row>, Never> {
  one_of_with(named, choices, offer, Task::of)
}

fn one_of_with<T: Copy + PartialEq>(
  named: &str,
  (every, now): (&[T], Option<T>),
  offer: impl Fn(T) -> Result<Offered, Never>,
  task: impl Fn(Command) -> Result<Task, Never>,
) -> Result<Vec<Row>, Never> {
  let offered: Vec<Offered> = every
    .iter()
    .map(|it| {
      let Ok(offered) = offer(*it);

      offered
    })
    .collect();

  match now.and_then(|now_2| every.iter().position(|it| *it == now_2)) {
    Some(at) => {
      let Ok(at_2) = console_core_number_conversion::fitted::<_, u32>(at);
      let Ok(row) = picking_among(named, offered, at_2, task);

      Ok(vec![row])
    }
    None => {
      let Ok(naming) = Row::naming(named, Aside(""));
      let mut rows = vec![naming];

      rows.extend(offered.into_iter().map(|offer_2| {
        let Ok(task_2) = task(offer_2.runs);
        let Ok(row) = task_row(&offer_2.said, Aside(""), task_2);

        row
      }));

      Ok(rows)
    }
  }
}

pub(crate) struct SoundControls {
  pub select: fn(&str),
  pub mute: fn(sound::VolumeTarget) -> Result<Handler, Never>,
  pub volume: fn(sound::VolumeTarget) -> Result<Level, Never>,
  pub reset: fn(Option<&sound::App>),
  pub bypassed: Option<BTreeSet<String>>,
  pub leveling: fn(&str, Switch) -> Result<(), Never>,
}

pub(crate) fn sound_rows(
  sinks: &[sound::Thing],
  playing: &[sound::Thing],
  default: &str,
  controls: SoundControls,
) -> Result<Vec<Row>, Never> {
  let mut rows = Vec::new();
  let Ok(available) = sound::outputs(sinks);
  let outputs: Vec<_> = available.collect();
  let Ok(selected) = sound::selected(&outputs, default);
  let Ok(position) = console_core_number_conversion::index(selected);

  match outputs.get(position) {
    Some(output) => {
      let label = match outputs.as_slice() {
        [_only] => {
          let Ok(name) = output.output_name();
          name
        }
        _several => {
          let Ok(picker) = output_picker(&outputs, selected, controls.select);
          rows.push(picker);
          "Volume".to_string()
        }
      };

      let Ok(row) = volume_row(
        &label,
        &[*output],
        sound::VolumeTarget::Output(output.index),
        &controls,
      );
      rows.push(row);
    }
    None => {
      let Ok(row) = Row::placeholder("No audio output connected");
      rows.push(row);
    }
  }

  let Ok(apps) = sound::applications(playing);
  let mut mix_changed = false;

  for (app, streams) in &apps {
    let first = {
      let optional_first = streams.first();

      match optional_first {
        Some(matched_first) => matched_first,
        ::core::option::Option::None => continue,
      }
    };

    let Ok(said) = first.display_name();
    let target = sound::VolumeTarget::App(app.clone());
    let Ok(mut row) = volume_row(&said, streams, target, &controls);
    let adjusted = streams
      .iter()
      .any(|stream| sound::adjusted(stream) == Ok(sound::StreamVolume::Adjusted));
    mix_changed |= adjusted;
    let leveling = controls.bypassed.as_ref().and_then(|bypassed| {
      let key = ({
        let Ok(explicit_value) = app.leveling_key();
        explicit_value
      })?;
      let enabled = match bypassed.contains(&key).then_some(()) {
        Some(()) => Switch::Off,
        None => Switch::On,
      };
      Some((key, enabled))
    });

    match { ::core::primitive::bool::then_some(adjusted || leveling.is_some(), ()) } {
      ::core::option::Option::Some(()) => {
        let app_2 = app.clone();
        let reset = controls.reset;
        let change = controls.leveling;
        let Ok(menu) = row.offering(move |showing| {
          let app_3 = app_2.clone();
          let leveling_2 = leveling.clone();
          let mut actions = Vec::new();

          match bool::then_some(adjusted, ()) {
            Some(()) => actions.push("Reset to 100%"),
            None => {}
          }

          match &leveling_2 {
            Some((_, enabled)) => {
              actions.push(match enabled {
                Switch::On => "Bypass Leveling",
                Switch::Off => "Use Global Leveling",
              });
            }
            ::core::option::Option::None => {}
          }

          showing.sure(
            &said,
            Subject(""),
            &actions,
            Box::new(move |showing_2, selected_2| {
              match bool::then_some(adjusted && selected_2 == 0, ()) {
                Some(()) => reset(Some(&app_3)),
                None => match &leveling_2 {
                  Some((key, enabled)) => {
                    let next = match enabled {
                      Switch::On => Switch::Off,
                      Switch::Off => Switch::On,
                    };
                    let Ok(()) = change(key, next);
                  }
                  None => {}
                },
              }

              showing_2.refresh();
            }),
          );
          ActionOutcome::Stay
        });
        row = menu;
      }
      ::core::option::Option::None => {}
    }

    rows.push(row);
  }

  match ::core::primitive::bool::then_some(mix_changed, ()) {
    ::core::option::Option::Some(()) => {
      let reset = controls.reset;
      let Ok(choose) = Handler::and_stay(move |showing| {
        reset(None);
        showing.refresh();
      });
      let Ok(row) = Row::new("Reset App Volumes", Aside("100%"), choose);
      rows.push(row);
    }
    ::core::option::Option::None => {
      match { ::core::primitive::bool::then_some(apps.is_empty() && !outputs.is_empty(), ()) } {
        ::core::option::Option::Some(()) => {
          let Ok(row) = Row::placeholder("Nothing else is playing");
          rows.push(row);
        }
        ::core::option::Option::None => {}
      }
    }
  }

  Ok(rows)
}

fn output_picker(
  outputs: &[&sound::Thing],
  selected: u32,
  select: fn(&str),
) -> Result<Row, Never> {
  let labels = outputs
    .iter()
    .map(|output| {
      let Ok(label) = output.output_name();
      label
    })
    .collect();
  let names: Vec<_> = outputs.iter().map(|output| output.name.clone()).collect();
  let Ok(control) = choices(labels, selected, move |showing, which| {
    let Ok(at) = console_core_number_conversion::index(which);

    {
      match names.get(at) {
        Some(name) => {
          select(name);
          showing.refresh();
        }
        ::core::option::Option::None => {}
      };
    }
  });

  Row::picking("Output", control)
}

fn volume_row(
  label: &str,
  streams: &[&sound::Thing],
  target: sound::VolumeTarget,
  controls: &SoundControls,
) -> Result<Row, Never> {
  let muted = match ::core::primitive::bool::then_some(
    streams.iter().all(|stream| stream.mute == Muted::Yes),
    (),
  ) {
    ::core::option::Option::Some(()) => Muted::Yes,
    ::core::option::Option::None => Muted::No,
  };
  let Ok(at) = sound::loudest(streams);
  let Ok(level) = crate::level::said(at, muted);
  let Ok(standing) = slider(at);
  let Ok(silence) = (controls.mute)(target.clone());
  let Ok(turning) = (controls.volume)(target);
  let Ok(row) = Row::new(label, Aside(&level), silence);
  let Ok(leveled) = row.with_level(turning);

  leveled.sliding(standing)
}

pub(crate) fn playback_switch(
  label: &str,
  now: Switch,
  change: fn(Switch) -> Result<(), Never>,
) -> Result<Row, Never> {
  let next = match now {
    Switch::On => Switch::Off,
    Switch::Off => Switch::On,
  };
  let Ok(choose) = Handler::and_stay(move |showing| {
    let Ok(()) = change(next);
    showing.refresh();
  });
  turned(label, now, choose)
}

fn chosen_among<T>(
  says: &str,
  it: T,
  now: T,
  choose: fn(T) -> Result<(), Never>,
) -> Result<Row, Never>
where
  T: Copy + PartialEq + Send + Sync + 'static,
{
  let Ok(choosing) = Handler::and_stay(move |showing| {
    let Ok(()) = choose(it);

    showing.refresh();
  });

  Row::new(
    says,
    Aside(match ::core::primitive::bool::then_some(it == now, ()) {
      ::core::option::Option::Some(()) => NOW,
      ::core::option::Option::None => "",
    }),
    choosing,
  )
}

fn swatches(painted: ColorRole, now: Paint) -> Result<Vec<Row>, Never> {
  let Ok(default) = Handler::and_stay(move |showing| {
    let Ok(()) = painted.choose(Paint::Desktop);

    showing.refresh();
  });
  let Ok(default_2) = Row::new(
    "Default",
    Aside(match now {
      Paint::Desktop => NOW,
      Paint::Chosen(_) => "",
    }),
    default,
  );
  let Ok(grid) = appearance::grid();
  let mut rows = vec![default_2];

  for colors in grid {
    let mut presses = Vec::new();
    let mut at = 0;

    for (column, color) in (0_u32..).zip(colors) {
      let chosen = Paint::Chosen(color);
      let in_effect = match ::core::primitive::bool::then_some(chosen == now, ()) {
        ::core::option::Option::Some(()) => {
          at = column;

          Active::Yes
        }
        ::core::option::Option::None => Active::No,
      };
      let Ok(press) = ButtonPress::swatch(color, in_effect, move |showing| {
        let Ok(()) = painted.choose(chosen);

        showing.refresh();
      });

      presses.push(press);
    }

    let Ok(row) = Row::pressing(presses, at);

    rows.push(row);
  }

  Ok(rows)
}

pub fn books_rows(now: Appearance) -> Result<Vec<Row>, Never> {
  let Ok(background) = Row::placeholder("Background");
  let Ok(pages) = swatches(ColorRole::Background, now.background);
  let Ok(text) = Row::placeholder("Text");
  let Ok(inks) = swatches(ColorRole::Text, now.text);
  let Ok(font) = Row::placeholder("Font");
  let mut rows = vec![background];

  rows.extend(pages);
  rows.push(text);
  rows.extend(inks);
  rows.push(font);

  for typeface in TYPEFACES {
    let Ok(says) = typeface.says();
    let Ok(row) = chosen_among(says, *typeface, now.typeface, Typeface::choose);

    rows.push(row);
  }

  Ok(rows)
}

pub fn sound_effects(effects: SoundEffects) -> Result<Row, Never> {
  let state = match effects {
    SoundEffects::On => Switch::On,
    SoundEffects::Off => Switch::Off,
  };
  let Ok(turning) = Handler::and_stay(move |showing| {
    let Ok(turned) = effects.flipped();
    let Ok(()) = turned.choose();

    showing.refresh();
  });

  turned("Sound Effects", state, turning)
}

pub fn following(following: Following) -> Result<Row, Never> {
  let Ok(follow_the_room) = text(&Word::AutoBrightness);

  let state = match following {
    Following::Yes => Switch::On,
    Following::No => Switch::Off,
  };

  let Ok(following_2) = Command::internal(InternalProgram::Brightness, &["follow-or-not"]);

  turned_by(&follow_the_room, state, following_2)
}

pub fn warmth(warm: NightShift) -> Result<Row, Never> {
  let Ok(night_colors) = text(&Word::NightShift);

  let chosen = match warm {
    NightShift::Off => 0,
    NightShift::Scheduled => 1,
    NightShift::On => 2,
  };
  let Ok(scheduled) = text(&Word::Scheduled);
  let offered = [(OFF, "off"), (&scheduled, "scheduled"), (ON, "on")]
    .into_iter()
    .map(|(said, word)| {
      let Ok(runs) = Command::internal(InternalProgram::NightShift, &[word]);
      Offered {
        said: said.to_owned(),
        runs,
      }
    })
    .collect();

  picking_among(
    &night_colors,
    offered,
    chosen,
    crate::display::rows::display_task,
  )
}

pub fn threshold(level: i32) -> Result<String, Never> {
  Ok(match level {
    console_battery::NEVER => {
      let Ok(never) = text(&Word::Never);

      never
    }
    level_2 => format!("{level}%", level = level_2),
  })
}

pub fn dwindling(
  levels: console_battery::Levels,
  guard: impl Fn(console_battery::Step) -> Result<Level, Never>,
) -> Result<Vec<Row>, Never> {
  let Ok(when_the_battery_gets_low) = text(&Word::LowBattery);
  let Ok(naming) = Row::naming(&when_the_battery_gets_low, Aside(""));
  let mut rows = vec![naming];

  rows.extend(console_battery::EVERY.into_iter().map(|step| {
    let Ok(word) = word_of(step);
    let Ok(level) = levels.at(step);
    let Ok(at) = threshold(level);
    let Ok(words) = text(&word);
    let Ok(row) = Row::text(&words, Aside(&at));
    let Ok(guarding) = guard(step);
    let Ok(leveled) = row.with_level(guarding);

    leveled
  }));

  let Ok(emergency) = text(&Word::EmergencyProtectionAt);
  let Ok(cutoff) = threshold(console_battery::EMERGENCY_PERCENT);
  let Ok(row) = Row::text(&emergency, Aside(&cutoff));

  rows.push(row);

  Ok(rows)
}

fn word_of(step: console_battery::Step) -> Result<Word, Never> {
  Ok(match step {
    console_battery::Step::Low => Word::AlertAt,
    console_battery::Step::Lower => Word::AlertAgainAt,
    console_battery::Step::Protect => Word::ShutDownAt,
  })
}

pub fn home_rows(
  shape: Shape,
  across: Level,
  down: Level,
  sized: Level,
) -> Result<Vec<Row>, Never> {
  let Ok(the_home_screen) = text(&Word::HomeScreen);
  let Ok(naming) = Row::naming(&the_home_screen, Aside(""));
  let Ok(applications_across) = text(&Word::Columns);
  let Ok(columns) = Row::text(&applications_across, Aside(&shape.columns.to_string()));
  let Ok(columns_2) = columns.with_level(across);
  let Ok(applications_down) = text(&Word::Rows);
  let Ok(down_row) = Row::text(&applications_down, Aside(&shape.rows.to_string()));
  let Ok(down_row_2) = down_row.with_level(down);
  let Ok(how_big_they_are) = text(&Word::IconSize);
  let Ok(big) = icon_sizes(&how_big_they_are, shape.size, sized);

  Ok(vec![naming, columns_2, down_row_2, big])
}

fn icon_sizes(
  says: &str,
  now: console_home_screen::shape::Size,
  sized: Level,
) -> Result<Row, Never> {
  let every = console_home_screen::shape::EVERY;
  let said: Vec<String> = every
    .iter()
    .map(|size| {
      let Ok(word) = word_of_home_size(*size);
      let Ok(said) = text(&word);

      said
    })
    .collect();

  match every.iter().position(|size| *size == now) {
    Some(at) => {
      let Ok(chosen) = console_core_number_conversion::fitted::<_, u32>(at);
      let Ok(from) = console_core_number_conversion::fitted::<u32, i32>(chosen);
      let Ok(control) = choices(said, chosen, move |_showing, which| {
        let Ok(to) = console_core_number_conversion::fitted::<u32, i32>(which);

        sized(to.saturating_sub(from));
      });

      Row::picking(says, control)
    }
    None => {
      let Ok(word) = word_of_home_size(now);
      let Ok(words) = text(&word);
      let Ok(row) = Row::text(says, Aside(&words));

      row.with_level(sized)
    }
  }
}

fn word_of_home_size(size: console_home_screen::shape::Size) -> Result<Word, Never> {
  Ok(match size {
    console_home_screen::shape::Size::Tiny => Word::SizeSmallest,
    console_home_screen::shape::Size::Smaller => Word::SizeSmaller,
    console_home_screen::shape::Size::Normal => Word::SizeDefault,
    console_home_screen::shape::Size::Bigger => Word::SizeLarger,
    console_home_screen::shape::Size::Huge => Word::SizeLargest,
  })
}

pub fn screen_rows(
  brightness: Option<i32>,
  dim: Level,
  room: Option<Following>,
  warm: NightShift,
  scale: Option<ScreenScale<'_>>,
  home: Vec<Row>,
) -> Result<Vec<Row>, Never> {
  let level = match brightness {
    Some(level) => crate::level::said(level, Muted::No)?,
    None => YET.to_string(),
  };
  let Ok(screen_brightness) = text(&Word::Brightness);
  let Ok(bright) = Row::text(&screen_brightness, Aside(&level));
  let Ok(bright_2) = bright.with_level(dim);

  let Ok(bright_3) = match brightness {
    Some(level_2) => {
      let Ok(standing_2) = slider(level_2);

      bright_2.sliding(standing_2)
    }
    None => Ok(bright_2),
  };
  let Ok(warmth) = warmth(warm);
  let mut rows = vec![bright_3];

  match room {
    Some(room_2) => {
      let Ok(row) = following(room_2);

      rows.push(row);
    }
    None => {}
  }

  rows.push(warmth);

  match scale {
    Some(scale_2) => {
      let Ok(controls) = screen_scale_rows(scale_2);
      rows.extend(controls);
    }
    ::core::option::Option::None => {}
  }

  rows.extend(home);

  Ok(rows)
}

pub struct ScreenScale<'a> {
  pub connector: Option<&'a str>,
  pub zoom: Option<Size>,
  pub turn: Option<Turn>,
}

fn screen_scale_rows(scale: ScreenScale<'_>) -> Result<Vec<Row>, Never> {
  let command = |written| match scale.connector {
    Some(output) => Command::internal(InternalProgram::Scale, &["--output", output, written]),
    None => Command::internal(InternalProgram::Scale, &[written]),
  };
  let Ok(zoom) = text(&Word::DisplayZoom);
  let task = crate::display::rows::display_task;
  let Ok(mut rows) = one_of_with(
    &zoom,
    (&EVERY, scale.zoom),
    |size| {
      let Ok(word) = word_of_size(size);
      let Ok(said) = text(&word);
      let Ok(written) = size.written();
      let Ok(runs) = command(written);

      Ok(Offered { said, runs })
    },
    task,
  );

  let Ok(which_way_up) = text(&Word::Rotation);
  let Ok(ways) = one_of_with(
    &which_way_up,
    (&turning::EVERY, scale.turn),
    |turn| {
      let Ok(word) = word_of_turn(turn);
      let Ok(said) = text(&word);
      let Ok(written) = turn.written();
      let Ok(runs) = command(written);

      Ok(Offered { said, runs })
    },
    task,
  );

  rows.extend(ways);

  Ok(rows)
}

fn word_of_turn(turn: Turn) -> Result<Word, Never> {
  Ok(match turn {
    Turn::Left => Word::RotatedLeft,
    Turn::Upright => Word::Standard,
    Turn::Right => Word::RotatedRight,
    Turn::Over => Word::UpsideDown,
  })
}

fn word_of_size(size: Size) -> Result<Word, Never> {
  Ok(match size {
    Size::Tiny => Word::SizeSmallest,
    Size::Smaller => Word::SizeSmaller,
    Size::Normal => Word::SizeDefault,
    Size::Bigger => Word::SizeLarger,
    Size::Huge => Word::SizeLargest,
  })
}

pub fn battery_rows(
  running: Option<&str>,
  levels: console_battery::Levels,
  guard: impl Fn(console_battery::Step) -> Result<Level, Never>,
) -> Result<Vec<Row>, Never> {
  let Ok(how_fast_the_machine_runs) = text(&Word::PowerMode);
  let running_3 = running.and_then(|running_2| {
    Profile::ALL
      .iter()
      .copied()
      .find(|profile| profile.written() == Ok(running_2))
  });
  let Ok(speeds) = one_of(
    &how_fast_the_machine_runs,
    (&Profile::ALL, running_3),
    |profile| {
      let Ok(said) = profile.label();
      let Ok(written) = profile.written();
      let Ok(runs) = Command::external(Program::Powerprofilesctl, &["set", written]);

      Ok(Offered { said, runs })
    },
  );
  let Ok(mut rows) = power_rows();

  rows.extend(speeds);

  let Ok(dwindling) = dwindling(levels, guard);

  rows.extend(dwindling);

  Ok(rows)
}

pub(crate) fn charge_limit_rows() -> Result<Vec<Row>, Never> {
  let Ok(limits) = console_battery::charge_limit::available();
  let multiple = limits.len() > 1;
  Ok(
    limits
      .into_iter()
      .map(|limit| {
        let label = match ::core::primitive::bool::then_some(multiple, ()) {
          ::core::option::Option::Some(()) => {
            format!("Maximum Charge ({})", limit.battery)
          }
          ::core::option::Option::None => "Maximum Charge".to_owned(),
        };
        let Ok(row) = charge_limit_row(&label, limit);
        row
      })
      .collect(),
  )
}

fn charge_limit_row(
  label: &str,
  limit: console_battery::charge_limit::Limit,
) -> Result<Row, Never> {
  let Ok(edit) = Handler::and_stay(move |showing| {
    let battery = limit.battery.clone();
    let Ok(respond) = answer(move |showing_2, written| {
      let Ok(parsed) = console_battery::charge_limit::percent(written);
      let percent = match parsed {
        Some(percent) => percent,
        None => {
          let Ok(()) = console_notifications::notify(
            console_notifications::Application("Settings"),
            "Enter a charge limit from 1 to 100 percent",
            None,
          );
          return;
        }
      };

      let Ok(command) = Command::internal(
        InternalProgram::Battery,
        &[
          console_battery::charge_limit::COMMAND,
          &battery,
          &percent.to_string(),
        ],
      );
      let Ok(task) = Task::of(command);
      let Ok(attributed) = task.from_application(console_notifications::Application("Settings"));
      let Ok(task_2) = attributed.then(|answer| {
        let details = answer.error_output.trim();
        Ok(
          matches!(
            answer.status,
            console_program_contract::ExitStatus::Failure(_)
          )
          .then(|| console_program_contract::Notification {
            summary: "Could not change maximum charge".to_owned(),
            body: (!details.is_empty()).then(|| details.to_owned()),
          }),
        )
      });
      showing_2.run(task_2);
    });
    showing.ask_aloud(
      "Maximum charge (%)",
      Initial(&limit.percent.to_string()),
      respond,
    );
  });
  Row::new(label, Aside(&format!("{}%", limit.percent)), edit)
}

pub fn strength(signal: i32) -> Result<String, Never> {
  bar(signal, Muted::No, CELLS.saturating_div(2))
}

pub fn wifi_rows<Share: Chosen>(
  on: wifi::Radio,
  networks: Vec<wifi::Network>,
  known: &[String],
  join: impl Fn(wifi::Network, wifi::Known) -> Result<Handler, Never>,
  share: impl Fn(wifi::Network) -> Result<Share, Never>,
) -> Result<Vec<Row>, Never> {
  match on {
    wifi::Radio::Off => {
      let Ok(turning_on) = Command::external(Program::Nmcli, &["radio", "wifi", "on"]);
      let Ok(row) = turned_by(WIFI, Switch::Off, turning_on.clone());
      let Ok(row_4) = row.while_running(&turning_on, TURNING_ON.0);

      return Ok(vec![row_4]);
    }
    wifi::Radio::On => {}
  }

  let Ok(turning_off) = Command::external(Program::Nmcli, &["radio", "wifi", "off"]);
  let Ok(off) = turned_by(WIFI, Switch::On, turning_off.clone());
  let Ok(off_2) = off.while_running(&turning_off, TURNING_OFF.0);
  let mut rows = vec![off_2];
  let known_2: BTreeSet<&str> = known.iter().map(String::as_str).collect();

  for network in networks {
    match ::core::primitive::bool::then_some(network.here, ()) {
      ::core::option::Option::Some(()) => {
        let name = network.name.clone();
        let Ok(shares) = share(network);
        let Ok(row) = Row::text(&name, Aside(NOW));
        let Ok(row_2) = row.offering(move |showing| {
          let shares_2 = shares.clone();

          showing.sure(
            &name,
            Subject(""),
            &[SHOW_CODE],
            Box::new(move |showing_2, _only_one| shares_2(showing_2)),
          );

          ActionOutcome::Stay
        });

        rows.push(row_2);
        continue;
      }
      ::core::option::Option::None => {}
    }

    let Ok(aside) = strength(network.signal);
    let says = network.name.clone();
    let already =
      match ::core::primitive::bool::then_some(known_2.contains(network.name.as_str()), ()) {
        ::core::option::Option::Some(()) => wifi::Known::Yes,
        ::core::option::Option::None => wifi::Known::No,
      };
    let Ok(plain) = wifi::joining(&network.name, already);
    let Ok(joins) = join(network, already);
    let Ok(row) = Row::new(&says, Aside(&aside), joins);
    let Ok(row_3) = row.while_running(&plain, &format!("{} {says}", JOINING.0));

    rows.push(row_3);
  }

  Ok(rows)
}

pub trait Opens: Fn(&Device, u32, &dyn Showing) + Clone + Send + Sync + 'static {}

impl<F> Opens for F where F: Fn(&Device, u32, &dyn Showing) + Clone + Send + Sync + 'static {}

const RADIO: u32 = 1;

const WIFI: &str = "Wi-Fi";

pub const SHOW_CODE: &str = "Show Network QR Code";

const BLUETOOTH: &str = "Bluetooth";

const PAIR: &str = "Pair";

const FORGET: &str = "Forget This Device";

const FORGET_SURE: &str = "Forget This Device?";

const FORGET_YES: &str = "Forget";

const JOIN: &str = "Connect";

const LEAVE: &str = "Disconnect";

pub const LOOK: &str = "Search for Devices";

pub const LOOKING: &str = "Searching for Devices";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Underway<'a>(&'a str);

const PAIRING: Underway<'static> = Underway("Pairing");

const CONNECTING: Underway<'static> = Underway("Connecting");

const DISCONNECTING: Underway<'static> = Underway("Disconnecting");

const TURNING_ON: Underway<'static> = Underway("Turning On");

const JOINING: Underway<'static> = Underway("Joining");

const TURNING_OFF: Underway<'static> = Underway("Turning Off");

fn bluetooth_task(action: &Action) -> Result<Task, Never> {
  let Ok(command) = command::of(action);
  let Ok(task) = Task::of(command);
  let Ok(attributed) = task.from_application(console_notifications::Application("Settings"));
  let Ok(answered) = attributed.then(cards::told);

  match action {
    Action::Introduce { .. } => answered.out_loud(),
    Action::Search | Action::Once(_) => Ok(answered),
  }
}

fn bluetooth_switch(
  says: &str,
  state: Aside<'_>,
  action: &Action,
  underway: Underway<'_>,
) -> Result<Row, Never> {
  let Ok(command) = command::of(action);
  let Ok(task) = bluetooth_task(action);
  let Ok(runs) = Handler::and_stay(move |showing| showing.run(task.clone()));
  let Ok(row) = Row::new(says, state, runs);

  row.while_running(&command, underway.0)
}

struct ConnectionChange {
  action: Action,
  offered: &'static str,
  underway: Underway<'static>,
  listed: String,
}

fn connection_change(device: &Device) -> Result<ConnectionChange, Never> {
  Ok(match device.connection {
    Connection::Connected => ConnectionChange {
      action: Action::Once(Once::Disconnect(device.address)),
      offered: LEAVE,
      underway: DISCONNECTING,
      listed: format!("{} {}", DISCONNECTING.0, device.alias),
    },
    Connection::NotConnected => ConnectionChange {
      action: Action::Once(Once::Connect(device.address)),
      offered: JOIN,
      underway: CONNECTING,
      listed: format!("{} to {}", CONNECTING.0, device.alias),
    },
  })
}

pub fn bluetooth_rows(
  power: Power,
  discovery: Discovery,
  shown: Vec<Device>,
  open: impl Opens,
) -> Result<Vec<Row>, Never> {
  match power {
    Power::Off => {
      let Ok(row) = bluetooth_switch(
        BLUETOOTH,
        Aside(""),
        &Action::Once(Once::Power(Power::On)),
        TURNING_ON,
      );
      let Ok(row_2) = row.switched(Switch::Off);

      return Ok(vec![row_2]);
    }
    Power::On => {}
  }

  let Ok(off) = bluetooth_switch(
    BLUETOOTH,
    Aside(""),
    &Action::Once(Once::Power(Power::Off)),
    TURNING_OFF,
  );
  let Ok(off_2) = off.switched(Switch::On);
  let mut rows = vec![off_2];
  let Ok(ordered) = bluetooth::in_order(shown);

  for (from, device) in ordered.into_iter().enumerate() {
    let Ok(from_2) = console_core_number_conversion::fitted::<_, u32>(from);
    let at = from_2.saturating_add(RADIO);
    let Ok(row) = device_row(device, at, &open);

    rows.push(row);
  }

  let Ok(last) = match discovery {
    Discovery::Searching => Row::text(LOOKING, Aside(YET)),
    Discovery::NotSearching => {
      bluetooth_switch(LOOK, Aside(""), &Action::Search, Underway(LOOKING))
    }
  };

  rows.push(last);

  Ok(rows)
}

fn heard_aside(device: &Device) -> Result<String, Never> {
  let heard = match device.signal {
    Some(heard) => heard,
    None => return Ok(YET.to_string()),
  };
  let Ok(share) = bluetooth::share(heard);

  strength(share)
}

fn device_row(device: Device, at: u32, open: &impl Opens) -> Result<Row, Never> {
  match device.pairing {
    Pairing::NotPaired => {
      let opening = open.clone();
      let opened = device.clone();
      let Ok(opens) = Handler::and_stay(move |showing| opening(&opened, at, showing));
      let Ok(aside) = heard_aside(&device);

      Row::new(&device.alias, Aside(&aside), opens)
    }
    Pairing::Paired => {
      let aside = match device.connection {
        Connection::Connected => NOW,
        Connection::NotConnected => "",
      };
      let Ok(change) = connection_change(&device);
      let Ok(mut row) = bluetooth_switch(
        &device.alias,
        Aside(""),
        &change.action,
        Underway(&change.listed),
      );

      row.aside = aside.to_string();

      let opening = open.clone();
      let opened = device.clone();

      row.offering(move |showing| {
        opening(&opened, at, showing);

        ActionOutcome::Stay
      })
    }
  }
}

pub fn meeting_rows(device: &Device, back: impl Chosen) -> Result<Vec<Row>, Never> {
  let leaving = back.clone();
  let Ok(way_back) = Row::back(&device.alias, move |showing| leaving(showing));
  let mut rows = vec![way_back];

  match device.pairing {
    Pairing::NotPaired => {
      let Ok(row) = pair_row(device);

      rows.push(row);
    }
    Pairing::Paired => {
      let Ok(change) = connection_change(device);
      let Ok(row) = bluetooth_switch(change.offered, Aside(""), &change.action, change.underway);

      rows.push(row);

      let Ok(forget) = forget_row(device, back);

      rows.push(forget);
    }
  }

  Ok(rows)
}

const PAIRING_PROMPT: &str = "PIN";

fn not_a_pin() -> Result<String, Never> {
  Ok(format!("A PIN is 1 to {LONGEST_PIN} characters"))
}

fn pair_row(device: &Device) -> Result<Row, Never> {
  let Ok(entry) = agent::pin_entry(device);
  let address = device.address;
  let introducing = Action::Introduce { address, pin: None };

  match entry {
    PinEntry::NotTyped => bluetooth_switch(PAIR, Aside(""), &introducing, PAIRING),
    PinEntry::Typed => {
      let Ok(plain) = command::of(&introducing);
      let Ok(asks) = Handler::and_stay(move |showing| {
        let Ok(respond) = answer(move |answered, written| {
          let Ok(read) = PinCode::read(written);

          match read {
            Some(pin) => {
              let Ok(task) = bluetooth_task(&Action::Introduce {
                address,
                pin: Some(pin),
              });

              answered.run(task);
            }
            None => {
              let Ok(refused) = not_a_pin();

              let Ok(()) = console_notifications::notify(
                console_notifications::Application("Settings"),
                &refused,
                None,
              );
            }
          }
        });
        showing.ask(PAIRING_PROMPT, respond);
      });

      let Ok(row) = Row::new(PAIR, Aside(""), asks);

      row.while_running(&plain, PAIRING.0)
    }
  }
}

fn forget_row(device: &Device, back: impl Chosen) -> Result<Row, Never> {
  let Ok(task) = bluetooth_task(&Action::Once(Once::Forget(device.address)));
  let name = device.alias.clone();

  let Ok(forgets) = Handler::and_stay(move |showing| {
    let removing = task.clone();
    let going_back = back.clone();

    showing.sure(
      FORGET_SURE,
      Subject(&name),
      &[FORGET_YES],
      Box::new(move |sure, _| {
        sure.run(removing);
        going_back(sure);
      }),
    );
  });

  Row::new(FORGET, Aside(""), forgets)
}

pub fn search_rows(engine: &str, back: impl Chosen) -> Result<Vec<Row>, Never> {
  let leaving = back.clone();
  let Ok(configuration) = configuration();
  let Ok(way_back) = Row::back(&configuration, move |showing| leaving(showing));
  let Ok(naming) = Row::naming("Search Engine", Aside(""));
  let mut rows = vec![way_back, naming];

  for offered in &engines::EVERY {
    let mark = match ::core::primitive::bool::then_some(offered.key == engine, ()) {
      ::core::option::Option::Some(()) => NOW,
      ::core::option::Option::None => "",
    };
    let key = offered.key;
    let back_2 = back.clone();
    let Ok(telling) = set_engine_command(key);
    let Ok(tells) = Task::of(telling);
    let Ok(chooses) = Handler::and_stay(move |showing| {
      let Ok(()) = engines::choose(key);

      showing.run(tells.clone());
      back_2(showing);
    });
    let Ok(row) = Row::new(offered.says, Aside(mark), chooses);

    rows.push(row);
  }

  Ok(rows)
}

pub fn engine_says(engine: &str) -> Result<String, Never> {
  let known = engines::one(engine)?;

  Ok(match known.map(|found| found.says.to_string()) {
    Some(says) => says,
    None => String::new(),
  })
}

pub fn dictation_rows(language: &str, back: impl Chosen) -> Result<Vec<Row>, Never> {
  let leaving = back.clone();
  let Ok(tab) = the_language();
  let Ok(way_back) = Row::back(&tab, move |showing| leaving(showing));
  let Ok(listens) = text(&Word::DictationLanguage);
  let Ok(naming) = Row::naming(&listens, Aside(""));
  let mut rows = vec![way_back, naming];

  for offered in &languages::EVERY {
    let mark = match ::core::primitive::bool::then_some(offered.key == language, ()) {
      ::core::option::Option::Some(()) => NOW,
      ::core::option::Option::None => "",
    };
    let key = offered.key;
    let back_2 = back.clone();
    let Ok(chooses) = Handler::and_stay(move |showing| {
      let Ok(()) = languages::choose(key);

      back_2(showing);
    });
    let Ok(row) = Row::new(offered.says, Aside(mark), chooses);

    rows.push(row);
  }

  Ok(rows)
}

pub fn dictation_says(language: &str) -> Result<String, Never> {
  let known = languages::one(language)?;

  Ok(match known.map(|found| found.says.to_string()) {
    Some(says) => says,
    None => String::new(),
  })
}

pub fn set_engine_command(engine: &str) -> Result<Command, Never> {
  Command::external(Program::Sudo, &["-n", "console-engine", engine])
}

pub fn power_rows() -> Result<Vec<Row>, Never> {
  let Ok(sleep) = power::row(Request::Sleep);
  let Ok(restart) = power::row(Request::Restart);
  let Ok(shut_down) = power::row(Request::ShutDown);
  let Ok(sign_out) = power::row(Request::LogOut);
  let mut rows = vec![sleep, restart, shut_down, sign_out];
  let Ok(locks) = locking();

  match locks {
    Locks::Yes => {
      let Ok(lock) = power::row(Request::Lock);

      rows.push(lock);
    }
    Locks::No => {}
  }

  Ok(rows)
}

const LOCK_SCREEN: &str = "Lock Screen";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Locks {
  Yes,
  No,
}

fn locking() -> Result<Locks, Never> {
  let Ok(home) = console_core_places::home();
  let stored = home
    .as_deref()
    .map(console_login_window::stored_pattern::stored);
  let Ok(chosen) = console_lock_screen::current();

  Ok(match stored {
    Some(Ok(stored_2)) => {
      let Ok(wanted) = console_lock_screen::desired(stored_2, chosen);

      match wanted {
        console_lock_screen::Wanted::Lock(_) => Locks::Yes,
        console_lock_screen::Wanted::Absent => Locks::No,
      }
    }
    Some(Err(_unreadable)) => Locks::No,
    None => Locks::No,
  })
}

pub fn lock_screen(chosen: LockScreen) -> Result<Row, Never> {
  lock_switch(LOCK_SCREEN, chosen, console_lock_screen::choose)
}

fn lock_switch(
  label: &str,
  chosen: LockScreen,
  save: fn(LockScreen) -> Result<(), Never>,
) -> Result<Row, Never> {
  let state = match chosen {
    LockScreen::On => Switch::On,
    LockScreen::Off => Switch::Off,
  };
  let Ok(turning) = Handler::and_stay(move |showing| {
    let Ok(turned) = chosen.flipped();
    let Ok(()) = save(turned);

    let Ok(()) = apply_lock_timing(showing);
  });

  turned(label, state, turning)
}

fn apply_lock_timing(
  showing: &dyn Showing,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let Ok(command) = Program::Systemctl.arguments(&["--user", "try-restart", idle::SERVICE]);
    let Ok(()) = console_panel::running::on_exit(command, |result| match result {
      console_panel::running::Exit::Success => {}
      console_panel::running::Exit::Failure { said } => {
        console_core_logging::warn!("applying screen and lock preferences: {said}");
        let Ok(()) = console_notifications::saying::report(
          console_notifications::Application("Settings"),
          "idle-settings",
          console_notifications::saying::Content {
            summary: "Could not apply inactivity settings",
            body: &said,
          },
        );
      }
    });
    showing.refresh();
  };

  Ok(())
}

#[derive(Clone, Copy)]
struct DelayTarget {
  action: idle::Action,
  profile: idle::Profile,
}

fn idle_delay_rows(label: &str, target: DelayTarget, delay: Delay) -> Result<Vec<Row>, Never> {
  let preset = Delay::PRESETS
    .iter()
    .zip(0_u32..)
    .find_map(|((option, _), position)| (*option == delay).then_some(position));
  let Ok(number_of_presets) =
    console_core_number_conversion::fitted::<_, u32>(Delay::PRESETS.len());
  let chosen = match preset {
    Some(position) => position,
    None => number_of_presets,
  };
  let mut labels: Vec<_> = Delay::PRESETS
    .iter()
    .map(|(_, label_2)| (*label_2).to_owned())
    .collect();
  let Ok(timeout) = delay.seconds();
  let custom = timeout.filter(|_| preset.is_none());

  match custom {
    Some(seconds) => {
      labels.push(format!("{seconds} s"));
    }
    ::core::option::Option::None => {}
  }

  let Ok(control) = choices(labels, chosen, move |showing, which| {
    let Ok(index) = console_core_number_conversion::index(which);

    {
      match Delay::PRESETS.get(index) {
        Some((delay_2, _)) => {
          let Ok(()) = save_idle_delay(showing, target, *delay_2);
        }
        ::core::option::Option::None => {}
      };
    }
  });
  let Ok(picker) = Row::picking(label, control);
  let Ok(edit) = Handler::and_stay(move |showing| {
    let Ok(explicit_value) = ask_idle_delay(showing, target, delay);
    explicit_value
  });
  let custom_label = match target.action {
    idle::Action::Dim => "Custom Dim Delay…",
    idle::Action::Blank => "Custom Display Off Delay…",
    idle::Action::Lock => "Custom Lock Delay…",
    idle::Action::Suspend => "Custom Suspend Delay…",
  };
  let Ok(custom_2) = Row::new(custom_label, Aside(""), edit);
  Ok(vec![picker, custom_2])
}

fn ask_idle_delay(
  showing: &dyn Showing,
  target: DelayTarget,
  delay: Delay,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let Ok(timeout) = delay.seconds();
    let initial = timeout.map_or_else(String::new, |seconds| seconds.to_string());
    let Ok(respond) = answer(move |showing_2, written| match Delay::custom(written) {
      Ok(parsed) => {
        let Ok(explicit_value) = save_idle_delay(showing_2, target, parsed);
        explicit_value
      }
      Err(idle::InvalidDelay) => {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Settings"),
          &format!("Enter a delay from 1 to {} seconds", Delay::MAX_SECONDS),
          None,
        );
      }
    });
    let prompt = match target.action {
      idle::Action::Dim => "Dim delay (e.g. 90s, 15m, 2h)",
      idle::Action::Blank => "Display off delay (e.g. 90s, 15m, 2h)",
      idle::Action::Lock => "Lock delay (e.g. 90s, 15m, 2h)",
      idle::Action::Suspend => "Suspend delay (e.g. 90s, 15m, 2h)",
    };
    showing.ask_aloud(prompt, Initial(&initial), respond);
  };

  Ok(())
}

fn save_idle_delay(
  showing: &dyn Showing,
  target: DelayTarget,
  delay: Delay,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let Ok(before) = target.action.delay(target.profile);
    let Ok(()) = target.action.choose(target.profile, delay);

    match (delay != before).then_some(()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = apply_lock_timing(showing);
      }
      ::core::option::Option::None => {
        showing.refresh();
      }
    }
  };

  Ok(())
}

pub(crate) fn inactivity_rows(power: console_battery::PowerSource) -> Result<Vec<Row>, Never> {
  let Ok(home) = console_core_places::home();
  let lock = match home
    .as_deref()
    .map(console_login_window::stored_pattern::stored)
  {
    Some(Ok(StoredPattern::Hash(_))) => Locks::Yes,
    Some(Ok(StoredPattern::Absent)) | None => Locks::No,
    Some(Err(fault)) => {
      console_core_logging::warn!("Reading login pattern for inactivity settings: {fault}");
      Locks::No
    }
  };
  let profiles: &[(idle::Profile, &str)] = match power {
    console_battery::PowerSource::WithoutBattery => &[(idle::Profile::General, "Inactivity")],
    console_battery::PowerSource::OnBattery | console_battery::PowerSource::PluggedIn => &[
      (idle::Profile::OnBattery, "On Battery"),
      (idle::Profile::PluggedIn, "Plugged In"),
    ],
  };
  let mut rows = Vec::new();

  match lock {
    Locks::Yes => {
      let Ok(on_suspend) = idle::on_suspend();
      let Ok(suspend_lock) =
        lock_switch("Lock Before Suspend", on_suspend, idle::choose_on_suspend);
      rows.push(suspend_lock);
    }
    Locks::No => {}
  }

  for &(profile, title) in profiles {
    let Ok(heading) = Row::naming(title, Aside(""));
    let Ok(controls) = inactivity_profile_rows(profile, lock);
    rows.push(heading);
    rows.extend(controls);
  }

  Ok(rows)
}

fn inactivity_profile_rows(profile: idle::Profile, lock: Locks) -> Result<Vec<Row>, Never> {
  let mut rows = Vec::new();

  for action in idle::Action::ALL {
    let label = match (action, lock) {
      (idle::Action::Dim, _) => "Dim Displays After Inactivity",
      (idle::Action::Blank, _) => "Turn Displays Off After Inactivity",
      (idle::Action::Lock, Locks::Yes) => "Lock After Inactivity",
      (idle::Action::Lock, Locks::No) => continue,
      (idle::Action::Suspend, _) => "Suspend After Inactivity",
    };
    let Ok(delay) = action.delay(profile);
    let Ok(controls) = idle_delay_rows(label, DelayTarget { action, profile }, delay);
    rows.extend(controls);
  }

  Ok(rows)
}

fn unlock_method() -> Result<Row, Never> {
  let Ok(preferred) = console_lock_screen::preferred();
  let Ok(chosen) = console_core_number_conversion::fitted::<_, u32>(
    match Method::ALL.iter().position(|method| *method == preferred) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    },
  );
  let labels = Method::ALL
    .iter()
    .map(|method| {
      let Ok(label) = method.label();
      label.to_owned()
    })
    .collect();
  let Ok(control) = choices(labels, chosen, |showing, which| {
    let Ok(index) = console_core_number_conversion::index(which);

    {
      match Method::ALL.get(index) {
        Some(method) => {
          let Ok(()) = console_lock_screen::prefer(*method);
          showing.refresh();
        }
        ::core::option::Option::None => {}
      };
    }
  });
  Row::picking("Default Unlock Method", control)
}

pub fn game_row() -> Result<Row, Never> {
  let Ok(session_game) = InternalProgram::SessionGame.path();
  let Ok(game) = Handler::run(&[session_game]);

  Row::new("Game Mode", Aside(""), game)
}

const LOGIN_PATTERN: &str = "Login Pattern";

const TURN_OFF_LOGIN_PATTERN: &str = "Turn Off Login Pattern";

pub fn login_rows() -> Result<Vec<Row>, Never> {
  let Ok(home) = console_core_places::home();
  let stored = home
    .as_deref()
    .map(console_login_window::stored_pattern::stored);
  let Ok(setter) = InternalProgram::SettingsLoginPattern.name();
  let Ok(chooses) = Handler::run(&[setter]);
  let Ok(forgets) = Handler::run(&[setter, "off"]);

  match stored {
    Some(Ok(StoredPattern::Hash(_))) => {
      let Ok(change) = Row::new(LOGIN_PATTERN, Aside(ON), chooses);
      let Ok(chosen) = console_lock_screen::current();
      let Ok(lock) = lock_screen(chosen);
      let Ok(method) = unlock_method();
      let Ok(off) = Row::new(TURN_OFF_LOGIN_PATTERN, Aside(""), forgets);

      Ok(vec![change, lock, method, off])
    }
    Some(Ok(StoredPattern::Absent)) => {
      let Ok(set) = Row::new(LOGIN_PATTERN, Aside(OFF), chooses);
      let Ok(lock) = Row::text(LOCK_SCREEN, Aside("Set a login pattern to enable"));

      Ok(vec![set, lock])
    }
    Some(Err(why)) => {
      let Ok(unread) = Row::new(LOGIN_PATTERN, Aside(&why.to_string()), chooses);

      Ok(vec![unread])
    }
    None => Ok(Vec::new()),
  }
}

const MEASUREMENTS: &str = "Measurements";

pub fn measurements(measuring: Measuring) -> Result<Row, Never> {
  let state = match measuring {
    Measuring::On => Switch::On,
    Measuring::Off => Switch::Off,
  };
  let Ok(turning) = Handler::and_stay(move |showing| {
    let Ok(turned) = measuring.flipped();
    let Ok(()) = measuring::choose(turned);

    showing.refresh();
  });

  turned(MEASUREMENTS, state, turning)
}

pub fn security_rows() -> Result<Vec<Row>, Never> {
  let Ok(mut rows) = login_rows();
  let Ok(measuring) = measuring::current();
  let Ok(switch) = measurements(measuring);

  rows.push(switch);

  Ok(rows)
}

const NOTIFICATIONS: &str = "Notifications";

const BELL: &str = "Notification Center";

pub fn notifications_rows(do_not_disturb: DoNotDisturb) -> Result<Vec<Row>, Never> {
  let says = NOTIFICATIONS;
  let showing = match do_not_disturb {
    DoNotDisturb::On => Switch::Off,
    DoNotDisturb::Off => Switch::On,
  };
  let Ok(quietening) = console_notifications::serving::quieten();

  let Ok(row) = turned_by(says, showing, quietening);

  let Ok(bell) = Row::text(BELL, Aside("All, even silenced"));

  Ok(vec![row, bell])
}

pub fn tabs() -> Result<[String; 13], Never> {
  Ok(console_status_bar::settings::SettingsTab::EVERY.map(|tab| {
    let Ok(title) = tab.title();

    title
  }))
}

pub fn the_language() -> Result<String, Never> {
  text(&Word::Language)
}

pub fn where_you_are() -> Result<String, Never> {
  text(&Word::TimeZone)
}

pub fn the_clock() -> Result<String, Never> {
  text(&Word::TimeFormat)
}

pub fn configuration() -> Result<String, Never> {
  text(&Word::General)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Languages<'a> {
  pub says: &'a str,
  pub types: &'a str,
  pub listens: &'a str,
}

pub fn language_rows(languages: Languages<'_>, into: [Handler; 3]) -> Result<Vec<Row>, Never> {
  let [words, keyboard, dictation] = into;

  let Ok(what_says) = text(&Word::PreferredLanguage);
  let Ok(what_types) = text(&Word::Keyboards);
  let Ok(what_listens) = text(&Word::DictationLanguage);

  let Ok(row) = Row::new(&what_says, Aside(languages.says), words);
  let Ok(said) = row.opening();
  let Ok(row_2) = Row::new(&what_types, Aside(languages.types), keyboard);
  let Ok(typed) = row_2.opening();
  let Ok(row_3) = Row::new(&what_listens, Aside(languages.listens), dictation);
  let Ok(heard) = row_3.opening();

  let Ok(english) = text(&Word::EnglishOnly);
  let Ok(standing) = Row::placeholder(&english);

  Ok(vec![said, typed, heard, standing])
}

fn leading(says: &str, back: impl Chosen) -> Result<Vec<Row>, Never> {
  let Ok(tab) = the_language();
  let Ok(way_back) = Row::back(&tab, move |showing| back(showing));
  let Ok(naming) = Row::naming(says, Aside(""));

  Ok(vec![way_back, naming])
}

fn running(command: Command, note: Option<String>, back: impl Chosen) -> Result<Handler, Never> {
  let Ok(task) = Task::of(command);

  Handler::and_stay(move |showing| {
    match &note {
      Some(said) => {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Settings"),
          said,
          None,
        );
      }
      None => {}
    }

    showing.run(task.clone());

    back(showing);
  })
}

pub fn language_picker_rows(
  languages: &[Language],
  standing: Option<&Locale>,
  back: impl Chosen,
  opening: impl Fn(u32, &str) -> Result<Handler, Never>,
) -> Result<Vec<Row>, Never> {
  let Ok(what_says) = text(&Word::PreferredLanguage);
  let Ok(mut rows) = leading(&what_says, back);

  let Ok(first) = console_core_number_conversion::fitted::<_, u32>(rows.len());
  let here = match standing.map(|locale| locale.language.clone()) {
    Some(here) => here,
    None => String::new(),
  };

  for (at, language) in languages.iter().enumerate() {
    let mark = match ::core::primitive::bool::then_some(language.language == here, ()) {
      ::core::option::Option::Some(()) => NOW,
      ::core::option::Option::None => "",
    };
    let Ok(at_2) = console_core_number_conversion::fitted::<_, u32>(at);
    let Ok(opens) = opening(at_2.saturating_add(first), &language.language);
    let Ok(row) = Row::new(&language.says, Aside(mark), opens);
    let Ok(row_2) = row.opening();

    rows.push(row_2);
  }

  Ok(rows)
}

pub fn place_rows(
  language: &Language,
  names: &Names,
  generated: &[String],
  standing: Option<&Locale>,
  back: impl Chosen,
) -> Result<Vec<Row>, Never> {
  let leaving = back.clone();
  let Ok(where_) = text(&Word::Region);
  let Ok(mut rows) = leading(&where_, leaving);

  let here = match standing.map(|locale| locale.name.clone()) {
    Some(here) => here,
    None => String::new(),
  };

  for locale in &language.locales {
    let mark = match ::core::primitive::bool::then_some(locale.name == here, ()) {
      ::core::option::Option::Some(()) => NOW.to_string(),
      ::core::option::Option::None => String::new(),
    };
    let Ok(region) = names.where_(locale);
    let says = match bool::then_some(region.is_empty(), ()) {
      Some(()) => language.says.clone(),
      None => region,
    };

    let Ok(made) = generation_state(generated, locale);

    let note = match made {
      Made::Yes => None,
      Made::No => {
        let Ok(making) = text(&Word::PreparingLanguage);

        Some(making)
      }
    };

    let Ok(language_2) = MachineSetting::Language.word();
    let Ok(choosing) = Command::external(
      Program::Sudo,
      &[
        "-n",
        "console-machine",
        language_2,
        &locale.name,
        &locale.charset,
      ],
    );
    let Ok(chooses) = running(choosing, note, back.clone());
    let Ok(row) = Row::new(&says, Aside(&mark), chooses);

    rows.push(row);
  }

  Ok(rows)
}

pub fn region_rows(
  regions: &[String],
  standing: Option<&str>,
  back: impl Chosen,
  opening: impl Fn(u32, &str) -> Result<Handler, Never>,
) -> Result<Vec<Row>, Never> {
  let Ok(where_) = text(&Word::TimeZone);
  let Ok(mut rows) = leading(&where_, back);

  let Ok(first) = console_core_number_conversion::fitted::<_, u32>(rows.len());
  let told = match standing {
    Some(told) => told,
    None => NOWHERE_SAID,
  };

  let Ok(here) = hours::part_of(told);

  for (at, region) in regions.iter().enumerate() {
    let mark = match ::core::primitive::bool::then_some(*region == here, ()) {
      ::core::option::Option::Some(()) => NOW,
      ::core::option::Option::None => "",
    };
    let Ok(at_2) = console_core_number_conversion::fitted::<_, u32>(at);
    let Ok(opens) = opening(at_2.saturating_add(first), region);
    let Ok(row) = Row::new(region, Aside(mark), opens);
    let Ok(row_2) = row.opening();

    rows.push(row_2);
  }

  Ok(rows)
}

pub fn zone_rows(
  region: &str,
  places: &[Place],
  standing: Option<&str>,
  back: impl Chosen,
) -> Result<Vec<Row>, Never> {
  let leaving = back.clone();
  let Ok(mut rows) = leading(region, leaving);

  let here = match standing {
    Some(here) => here,
    None => NOWHERE_SAID,
  };

  for place in places {
    let mark = match ::core::primitive::bool::then_some(place.zone == here, ()) {
      ::core::option::Option::Some(()) => NOW,
      ::core::option::Option::None => "",
    };
    let Ok(hour) = MachineSetting::Hour.word();
    let Ok(choosing) =
      Command::external(Program::Sudo, &["-n", "console-machine", hour, &place.zone]);
    let Ok(chooses) = running(choosing, None, back.clone());
    let Ok(row) = Row::new(&place.says, Aside(mark), chooses);

    rows.push(row);
  }

  Ok(rows)
}

pub fn clock_rows(now: Clock, back: impl Chosen) -> Result<Vec<Row>, Never> {
  let leaving = back.clone();
  let Ok(the_clock) = text(&Word::TimeFormat);
  let Ok(mut rows) = leading(&the_clock, leaving);

  for reading in clock::EVERY {
    let mark = match ::core::primitive::bool::then_some(reading == now, ()) {
      ::core::option::Option::Some(()) => NOW,
      ::core::option::Option::None => "",
    };
    let Ok(says) = reading.says();
    let leaving_2 = back.clone();
    let Ok(chooses) = Handler::and_stay(move |showing| {
      let Ok(()) = clock::choose(reading);

      leaving_2(showing);
    });
    let Ok(row) = Row::new(says, Aside(mark), chooses);

    rows.push(row);
  }

  Ok(rows)
}

pub fn called_row(name: &str) -> Result<Row, Never> {
  let Ok(says) = text(&Word::Name);
  let asking = says.clone();
  let Ok(asks) = Handler::and_stay(move |showing| {
    let Ok(respond) = answer(move |showing_2, word| {
      let Ok(allowed) = named::allowed(word);

      match allowed {
        Allowed::Yes => {
          let Ok(name_2) = MachineSetting::Name.word();
          let Ok(naming) =
            Command::external(Program::Sudo, &["-n", "console-machine", name_2, word]);
          let Ok(names) = Task::of(naming);

          showing_2.run(names);
        }
        Allowed::No => {
          let Ok(()) = console_notifications::notify(
            console_notifications::Application("Settings"),
            NOT_A_NAME,
            None,
          );
        }
      }
    });
    showing.ask(&asking, respond);
  });

  Row::new(&says, Aside(name), asks)
}

const NOT_A_NAME: &str = "Use only letters, numbers and hyphens";

pub trait Chosen: Fn(&dyn Showing) + Clone + Send + Sync + 'static {}

impl<F> Chosen for F where F: Fn(&dyn Showing) + Clone + Send + Sync + 'static {}

#[cfg(test)]
mod tests {
  use std::collections::BTreeSet;

  use super::*;
  use crate::bluetooth::room::{FRIEND, JOINED, STRANGER, device, named, quiet};
  use console_bluetooth::device::PairingMethod;
  use console_core_localization::Localized;
  use console_core_number_conversion::{fitted, index};
  use console_panel::page::{Action, Active, Control, Heading, Nowhere};
  use console_panel::tasks::{self, Running, Speaks, Started};

  type Failure = Box<dyn std::error::Error>;

  type Recorded = std::sync::mpsc::Receiver<(u32, String)>;

  trait Opening: Fn(u32, &str) -> Result<Handler, Never> {}

  impl<F: Fn(u32, &str) -> Result<Handler, Never>> Opening for F {}

  fn nothing() -> Result<Level, Never> {
    adjustment(|_| ())
  }

  fn nowhere() -> Result<impl Chosen, Never> {
    Ok(|_: &dyn Showing| ())
  }

  fn grid() -> Result<Vec<Row>, Never> {
    let Ok(rows) = nothing();
    let Ok(columns) = nothing();
    let Ok(size) = nothing();

    home_rows(Shape::USUAL, rows, columns, size)
  }

  fn silence(_: i64, _: &'static str) -> Result<Handler, Never> {
    Handler::and_stay(|_| ())
  }

  fn opening(_: u32, _: &str) -> Result<Handler, Never> {
    Handler::and_stay(|_| ())
  }

  fn says(rows: &[Row]) -> Result<Vec<&str>, Never> {
    Ok(rows.iter().map(|row| row.says.as_str()).collect())
  }

  fn marked(rows: &[Row]) -> Result<Vec<&str>, Never> {
    Ok(
      rows
        .iter()
        .filter(|row| row.now() == Ok(Active::Yes))
        .map(|row| row.says.as_str())
        .collect(),
    )
  }

  fn offered<'a>(
    rows: &'a [Row],
    said: &str,
  ) -> Result<(Vec<&'a str>, Option<&'a str>), Failure> {
    let found = row(rows, said)?;
    let choices = found
      .choices
      .as_ref()
      .ok_or(format!("{said:?} carries no choices"))?;
    let Ok(at) = index(choices.chosen);
    let every: Vec<&str> = choices.said.iter().map(String::as_str).collect();
    let chosen = every.get(at).copied();

    Ok((every, chosen))
  }

  fn row<'a>(rows: &'a [Row], said: &str) -> Result<&'a Row, Failure> {
    let found = rows
      .iter()
      .find(|row| row.says == said)
      .ok_or(format!("no row says {said:?}"))?;

    Ok(found)
  }

  fn at(rows: &[Row], place: u32) -> Result<&Row, Failure> {
    let Ok(place_at) = index(place);
    let found = rows
      .get(place_at)
      .ok_or(format!("no row {place} among {}", rows.len()))?;

    Ok(found)
  }

  fn part(rows: &[Row], (from, to): (u32, u32)) -> Result<&[Row], Failure> {
    let Ok(start) = index(from);
    let Ok(end) = index(to);
    let part = rows
      .get(start..end)
      .ok_or(format!("no rows {from} to {to} among {}", rows.len()))?;

    Ok(part)
  }

  fn after(rows: &[Row], skipped: u32) -> Result<&[Row], Failure> {
    let Ok(start) = index(skipped);
    let part = rows
      .get(start..)
      .ok_or(format!("no rows after {skipped} among {}", rows.len()))?;

    Ok(part)
  }

  fn from<'a>(rows: &'a [Row], said: &str) -> Result<&'a [Row], Never> {
    Ok(match rows.iter().position(|row| row.says == said) {
      Some(start) => match rows.get(start..) {
        Some(rest) => rest,
        None => &[],
      },
      None => &[],
    })
  }

  fn sound(sinks: &[sound::Thing], default: &str) -> Result<Vec<Row>, Never> {
    sound_rows(sinks, &[], default, {
      let Ok(explicit_value) = test_sound_controls();
      explicit_value
    })
  }

  #[must_use]
  fn test_sound_controls() -> core::result::Result<SoundControls, Never> {
    Ok(SoundControls {
      select: |_| {},
      mute: |_| silence(0, ""),
      volume: |_| nothing(),
      reset: |_| {},
      bypassed: None,
      leveling: |_, _| Ok(()),
    })
  }

  fn wifi_of(on: wifi::Radio, networks: Vec<wifi::Network>) -> Result<Vec<Row>, Never> {
    wifi_rows(on, networks, &[], |_, _| silence(0, ""), |_| nowhere())
  }

  fn bluetooth_of(on: Power, shown: Vec<Device>) -> Result<Vec<Row>, Never> {
    bluetooth_rows(
      on,
      Discovery::NotSearching,
      shown,
      |_: &Device, _, _: &dyn Showing| (),
    )
  }

  fn while_looking(shown: Vec<Device>) -> Result<Vec<Row>, Never> {
    bluetooth_rows(
      Power::On,
      Discovery::Searching,
      shown,
      |_: &Device, _, _: &dyn Showing| (),
    )
  }

  fn heard(said: &[&str], loudness: &[i16]) -> Result<Vec<Device>, Never> {
    Ok(
      said
        .iter()
        .zip(loudness)
        .zip(0_u8..)
        .map(|((alias, heard), last)| {
          let Ok(one) = device(
            last,
            alias,
            (Pairing::NotPaired, Connection::NotConnected, Some(*heard)),
          );

          one
        })
        .collect(),
    )
  }

  fn meeting(device: &Device) -> Result<Vec<String>, Never> {
    let Ok(nowhere) = nowhere();
    let Ok(rows) = meeting_rows(device, nowhere);

    Ok(rows.into_iter().map(|row| row.says).collect())
  }

  struct Pressed {
    typed: &'static str,
    asked: std::cell::Cell<Vec<String>>,
    ran: std::cell::Cell<Vec<console_program_contract::Command>>,
    speaking: std::cell::Cell<Vec<Speaks>>,
  }

  fn written<T>(into: &std::cell::Cell<Vec<T>>, one: T) -> Result<(), Never> {
    let mut records = into.take();
    records.push(one);
    into.set(records);

    Ok(())
  }

  fn read_back<T>(from: &std::cell::Cell<Vec<T>>) -> Result<Vec<T>, Never> {
    Ok(from.take())
  }

  impl Showing for Pressed {
    fn refresh(&self) {}

    fn replace(&self, _standing_on: u32) {}

    fn forget_typing(&self) {}

    fn ask(&self, question: &str, then: console_panel::page::Answer) {
      let Ok(()) = written(&self.asked, question.to_string());

      then(self, self.typed);
    }

    fn sure(
      &self,
      question: &str,
      _about: Subject<'_>,
      _does: &[&str],
      _then: console_panel::page::Confirmation,
    ) {
      let Ok(()) = written(&self.asked, question.to_string());
    }

    fn ask_aloud(
      &self,
      _question: &str,
      _initial: console_panel::page::Initial<'_>,
      _then: console_panel::page::Answer,
    ) {
    }

    fn open_out(&self) {}

    fn toggle_full_screen(&self) {}

    fn zoom_by(&self, _factor: f64) {}

    fn select(&self, _keys: Vec<String>) {}

    fn turn_to(&self, _tab: u32) {}

    fn run(&self, task: Task) {
      let Ok(()) = written(&self.speaking, task.speaks);
      let Ok(()) = written(&self.ran, task.command);
    }

    fn interrupt(&self, _command: &console_program_contract::Command) {}

    fn leave_running(&self, _arguments: Vec<String>) {}
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  struct Typing(&'static str);

  fn pressed(rows: &[Row], says: &str, typing: Typing) -> Result<Pressed, Failure> {
    let found = row(rows, says)?;
    let pressing = Pressed {
      typed: typing.0,
      asked: std::cell::Cell::new(Vec::new()),
      ran: std::cell::Cell::new(Vec::new()),
      speaking: std::cell::Cell::new(Vec::new()),
    };

    match &found.does {
      Some(Handler::Call(act)) => {
        let _stays = act(&pressing);
      }
      Some(Handler::Run(_)) | None => {
        return Err(Failure::from(format!("{says:?} has nothing to press")));
      }
    }

    Ok(pressing)
  }

  fn line(action: &command::Action) -> Result<console_program_contract::Command, Never> {
    command::of(action)
  }

  fn back_to(says: &str) -> Result<String, Never> {
    let Ok(row) = Row::back(says, |_| ());

    Ok(row.says)
  }

  fn searching(engine: &str) -> Result<Vec<Row>, Never> {
    let Ok(nowhere) = nowhere();

    search_rows(engine, nowhere)
  }

  fn listening_for(language: &str) -> Result<Vec<Row>, Never> {
    let Ok(nowhere) = nowhere();

    dictation_rows(language, nowhere)
  }

  fn sized(standing: Option<Size>) -> Result<Vec<Row>, Never> {
    let Ok(level) = nothing();
    let Ok(grid) = grid();

    screen_rows(
      Some(50),
      level,
      None,
      NightShift::Off,
      Some(ScreenScale {
        connector: None,
        zoom: standing,
        turn: None,
      }),
      grid,
    )
  }

  fn battery() -> Result<Vec<Row>, Never> {
    battery_rows(Some("balanced"), console_battery::Levels::default(), |_| {
      nothing()
    })
  }

  fn sample_languages() -> Result<Vec<Language>, Never> {
    let Ok(names) = Names::none();
    let Ok(supported) = crate::languages::supported(
      "en_GB.UTF-8 UTF-8\nen_US.UTF-8 UTF-8\nnl_NL.UTF-8 UTF-8\nth_TH.UTF-8 UTF-8",
    );

    crate::languages::languages(&supported, &names)
  }

  fn recorder() -> Result<(Recorded, impl Opening), Never> {
    let (telling, seen) = std::sync::mpsc::sync_channel(128);

    Ok((seen, move |at: u32, name: &str| {
      assert_eq!(telling.try_send((at, name.to_string())), Ok(()));
      Handler::and_stay(|_| ())
    }))
  }

  fn opened(seen: &Recorded) -> Result<Vec<(u32, String)>, Never> {
    Ok(seen.try_iter().collect())
  }

  fn opened_from(rows: &[Row], seen: &[(u32, String)]) -> Result<(), Never> {
    for (place, says) in seen {
      let Ok(place_at) = index(*place);
      let standing = rows.get(place_at).map(|row| row.says.as_str());

      assert_eq!(
        standing,
        Some(says.as_str()),
        "opened from row {place}, and that row is {standing:?}"
      );
    }

    Ok(())
  }

  fn language() -> Result<Vec<Row>, Never> {
    let Ok(nothing) = Handler::and_stay(|_| ());
    let Ok(also) = Handler::and_stay(|_| ());
    let Ok(third) = Handler::and_stay(|_| ());

    language_rows(
      Languages {
        says: "English (United Kingdom)",
        types: "Latin, Thai",
        listens: "Dutch",
      },
      [nothing, also, third],
    )
  }

  #[test]
  fn measurements_say_whether_they_are_on() {
    let Ok(off) = measurements(Measuring::Off);
    let Ok(on) = measurements(Measuring::On);

    assert_eq!(
      (off.says.as_str(), off.control),
      ("Measurements", Some(Control::Switch(Switch::Off)))
    );
    assert_eq!(on.control, Some(Control::Switch(Switch::On)));
    assert!(off.does.is_some(), "the row is a switch someone can press");
  }

  #[test]
  fn sound_effects_say_whether_they_are_on() {
    let Ok(off) = sound_effects(SoundEffects::Off);
    let Ok(on) = sound_effects(SoundEffects::On);

    assert_eq!(
      (off.says.as_str(), off.control),
      ("Sound Effects", Some(Control::Switch(Switch::Off)))
    );
    assert_eq!(on.control, Some(Control::Switch(Switch::On)));
    assert!(off.does.is_some(), "the row is a switch someone can press");
  }

  #[test]
  fn the_two_things_that_are_held_at_a_level_are_on_a_panel() -> Result<(), Failure> {
    let Ok(screen) = sized(Some(Size::Normal));
    let Ok(sinks) = sound::read(r#"[{"index": 1, "name": "a", "volume": {}}]"#);
    let Ok(speakers) = sound(&sinks, "a");

    let screen_first = at(&screen, 0)?;
    assert!(screen_first.level.is_some(), "the screen is not a level");
    let speakers_first = at(&speakers, 0)?;
    assert!(
      speakers_first.level.is_some(),
      "the speakers are not a level"
    );

    Ok(())
  }

  #[test]
  fn choosing_an_output_shows_its_own_volume() -> Result<(), Failure> {
    let Ok(sinks) = sound::read(
      r#"[
      {"name":"speakers","description":"Speakers","volume":{"mono":{"value_percent":"60%"}}},
      {"name":"headphones","description":"Headphones","volume":{"mono":{"value_percent":"25%"}}}
    ]"#,
    );
    let Ok(speakers) = sound(&sinks, "speakers");
    let Ok(headphones) = sound(&sinks, "headphones");

    let (_, speaker_output) = offered(&speakers, "Output")?;
    let (_, headphone_output) = offered(&headphones, "Output")?;
    let speaker_volume = row(&speakers, "Volume")?;
    let headphone_volume = row(&headphones, "Volume")?;
    assert_eq!(speaker_output, Some("Speakers"));
    assert_eq!(headphone_output, Some("Headphones"));
    assert_eq!(speaker_volume.aside, "60%");
    assert_eq!(headphone_volume.aside, "25%");
    Ok(())
  }

  #[test]
  fn an_app_has_one_slider_and_its_changed_volumes_can_be_reset() -> Result<(), Failure> {
    let Ok(playing) = sound::read(
      r#"[
        {"index":1,"properties":{"application.id":"browser","application.name":"Browser","media.name":"Video"},"volume":{"mono":{"value_percent":"40%"}}},
        {"index":2,"properties":{"application.id":"browser","application.name":"Browser","media.name":"Music"},"volume":{"mono":{"value_percent":"55%"}}},
        {"index":3,"properties":{"application.id":"game","application.name":"Game"},"volume":{"mono":{"value_percent":"100%"}}},
        {"index":4,"properties":{"console.balance.output":"9","application.name":"Filter"}}
      ]"#,
    );
    let Ok(rows) = sound_rows(&[], &playing, "", {
      let Ok(explicit_value) = test_sound_controls();
      explicit_value
    });
    let browser = row(&rows, "Browser")?;
    assert_eq!(browser.aside, "55%");
    assert!(browser.more.is_some());
    let game = row(&rows, "Game")?;
    assert!(game.more.is_none());
    let labels = says(&rows)?;
    assert_eq!(
      labels,
      [
        "No audio output connected",
        "Browser",
        "Game",
        "Reset App Volumes"
      ]
    );
    Ok(())
  }

  #[test]
  fn the_profile_in_use_is_the_one_chosen() -> Result<(), Failure> {
    let Ok(rows) = battery_rows(
      Some("performance"),
      console_battery::Levels::default(),
      |_| nothing(),
    );
    let high = Word::HighPower.english();
    let (_speeds, chosen) = offered(&rows, &Word::PowerMode.english())?;

    assert_eq!(chosen, Some(high.as_str()));
    assert_eq!(
      marked(&rows),
      Ok(Vec::new()),
      "a speed is still written as a row of its own"
    );

    Ok(())
  }

  #[test]
  fn the_battery_tab_opens_on_stopping_the_machine() -> Result<(), Failure> {
    let Ok(rows) = battery();
    let rows_part = part(&rows, (0, 4))?;
    let Ok(said) = says(rows_part);

    assert_eq!(said, ["Sleep", "Restart", "Shut Down", "Log Out"]);

    Ok(())
  }

  #[test]
  fn the_three_speeds_are_one_row_with_the_least_of_them_first() -> Result<(), Failure> {
    let Ok(rows) = battery();
    let named = at(&rows, 4)?;
    let (speeds, chosen) = offered(&rows, &Word::PowerMode.english())?;
    let automatic = Word::Automatic.english();

    assert_eq!(
      named.says,
      Word::PowerMode.english(),
      "the speeds are not under stopping the machine"
    );
    assert_eq!(
      speeds,
      [
        Word::LowPower.english(),
        Word::Automatic.english(),
        Word::HighPower.english()
      ]
    );
    assert_eq!(chosen, Some(automatic.as_str()));

    Ok(())
  }

  #[test]
  fn the_books_tab_marks_the_page_the_ink_and_the_face_that_were_chosen() -> Result<(), Failure> {
    let Ok(grid) = appearance::grid();
    let page = grid
      .get(2)
      .and_then(|row| row.get(3))
      .ok_or("the grid has a third row of four")?;
    let now = Appearance {
      background: Paint::Chosen(*page),
      text: Paint::Desktop,
      typeface: Typeface::Monospaced,
      ..Appearance::default()
    };
    let Ok(rows) = books_rows(now);
    let marked: Vec<&str> = rows
      .iter()
      .filter(|row| row.aside == NOW)
      .map(|row| row.says.as_str())
      .collect();

    assert_eq!(
      marked,
      ["Default", "Monospaced"],
      "the ink is the desktop's and the face was chosen"
    );

    let lit: Vec<(u32, console_panel::page::Face)> = rows
      .iter()
      .filter_map(|row| row.buttons.as_ref())
      .flat_map(|across| {
        across
          .presses
          .iter()
          .filter(|press| press.now == Active::Yes)
          .map(move |press| (across.at, press.face))
      })
      .collect();

    assert_eq!(
      lit,
      [(3, console_panel::page::Face::Swatch(*page))],
      "the page chosen is the one swatch lit, and it is where the row starts"
    );
    assert_eq!(
      rows.len(),
      grid
        .len()
        .saturating_add(1)
        .saturating_mul(2)
        .saturating_add(3)
        .saturating_add(TYPEFACES.len())
    );

    Ok(())
  }

  #[test]
  fn each_power_source_has_its_own_display_and_sleep_controls() {
    let Ok(rows) = inactivity_rows(console_battery::PowerSource::OnBattery);
    let find = |name| rows.iter().filter(|row| row.says == name).count();
    assert_eq!(find("On Battery"), 1);
    assert_eq!(find("Plugged In"), 1);
    assert_eq!(find("Turn Displays Off After Inactivity"), 2);
    assert_eq!(find("Suspend After Inactivity"), 2);
    let Ok(display) = sized(None);
    assert!(
      !display
        .iter()
        .any(|row| row.says.contains("After Inactivity"))
    );
  }

  #[test]
  fn the_screen_and_how_hard_the_machine_works_are_two_tabs() {
    let Ok(rows) = battery();
    let Ok(said) = says(&rows);
    let battery = said.join("\n");

    for screen in [Word::Brightness.english(), Word::DisplayZoom.english()] {
      assert!(
        !battery.contains(&screen),
        "{screen:?} is still on the Battery tab"
      );
    }

    assert!(
      !battery.contains("night colors"),
      "the evening is still on the Battery tab"
    );
  }

  #[test]
  fn the_sizes_are_one_row_with_the_smallest_of_them_first() -> Result<(), Failure> {
    let Ok(rows) = sized(Some(Size::Normal));
    let (ladder, chosen) = offered(&rows, &Word::DisplayZoom.english())?;
    let usual = Word::SizeDefault.english();

    assert_eq!(
      ladder,
      [
        Word::SizeSmallest.english(),
        Word::SizeSmaller.english(),
        Word::SizeDefault.english(),
        Word::SizeLarger.english(),
        Word::SizeLargest.english(),
      ]
    );
    assert_eq!(chosen, Some(usual.as_str()));
    assert!(
      rows
        .iter()
        .take_while(|row| row.says != Word::DisplayZoom.english())
        .any(|row| row.level.is_some()),
      "the brightness is above the sizes, not under them"
    );

    Ok(())
  }

  #[test]
  fn the_home_screens_own_shape_is_under_the_size_of_everything_else() -> Result<(), Failure> {
    let Ok(rows) = sized(Some(Size::Normal));
    let Ok(named) = from(&rows, &Word::HomeScreen.english());
    let Ok(ladder) = from(&rows, &Word::DisplayZoom.english());
    let named_after = after(named, 1)?;
    let Ok(shape) = says(named_after);

    assert!(!ladder.is_empty(), "the sizes are named");
    assert!(
      ladder.len() > named.len(),
      "the home screen is under the ladder, not over it"
    );
    let named_first = at(named, 0)?;
    assert!(
      named_first.naming,
      "the name is a row the highlight can land on"
    );
    assert_eq!(
      shape,
      [
        Word::Columns.english(),
        Word::Rows.english(),
        Word::IconSize.english()
      ]
    );

    Ok(())
  }

  #[test]
  fn the_home_screens_rows_say_what_they_are_at_and_can_all_be_moved() -> Result<(), Failure> {
    let Ok(wide) = Shape::USUAL.with_columns(7);

    let Ok(deep) = wide.with_rows(4);

    let Ok(shape) = deep.sized(console_home_screen::shape::Size::Bigger);

    let Ok(across) = nothing();
    let Ok(down) = nothing();
    let Ok(size) = nothing();
    let Ok(rows) = home_rows(shape, across, down, size);
    let moved: Vec<&str> = rows
      .iter()
      .filter(|row| row.level.is_some())
      .map(|row| row.aside.as_str())
      .collect();
    let (_sizes, chosen) = offered(&rows, &Word::IconSize.english())?;
    let larger = Word::SizeLarger.english();

    assert_eq!(
      moved,
      ["7", "4"],
      "one of them cannot be moved or says nothing"
    );
    assert_eq!(
      chosen,
      Some(larger.as_str()),
      "the icons are not at the size chosen"
    );

    Ok(())
  }

  #[test]
  fn the_way_up_the_screen_stands_is_the_one_chosen_and_every_way_is_offered()
  -> Result<(), Failure> {
    let Ok(level) = nothing();
    let Ok(grid) = grid();
    let Ok(rows) = screen_rows(
      Some(50),
      level,
      None,
      NightShift::Off,
      Some(ScreenScale {
        connector: None,
        zoom: None,
        turn: Some(Turn::Left),
      }),
      grid,
    );
    let (ways, chosen) = offered(&rows, &Word::Rotation.english())?;
    let left = Word::RotatedLeft.english();

    assert_eq!(
      ways,
      [
        Word::RotatedLeft.english(),
        Word::Standard.english(),
        Word::RotatedRight.english(),
        Word::UpsideDown.english()
      ]
    );
    assert_eq!(chosen, Some(left.as_str()));

    Ok(())
  }

  #[test]
  fn the_size_the_screen_is_at_is_the_one_chosen() -> Result<(), Failure> {
    let Ok(rows) = sized(Some(Size::Bigger));
    let larger = Word::SizeLarger.english();
    let (_ladder, chosen) = offered(&rows, &Word::DisplayZoom.english())?;

    assert_eq!(chosen, Some(larger.as_str()));

    Ok(())
  }

  #[test]
  fn a_screen_at_a_size_of_its_own_marks_none_of_the_rungs() {
    let Ok(rows) = sized(None);

    assert_eq!(marked(&rows), Ok(Vec::new()), "something is marked");
    assert_eq!(
      rows.iter().filter(|row| row.does.is_some()).count(),
      EVERY.len().saturating_add(turning::EVERY.len()),
      "a rung or a way up went missing"
    );
  }

  #[test]
  fn three_battery_thresholds_are_adjustable_and_the_final_cutoff_is_fixed() -> Result<(), Failure>
  {
    let Ok(rows) = dwindling(console_battery::Levels::default(), |_| nothing());
    let Ok(said) = says(&rows);

    assert_eq!(
      said,
      [
        Word::LowBattery.english(),
        Word::AlertAt.english(),
        Word::AlertAgainAt.english(),
        Word::ShutDownAt.english(),
        Word::EmergencyProtectionAt.english(),
      ]
    );
    let rows_after = after(&rows, 1)?;
    let (emergency, configurable) = rows_after.split_last().ok_or("an emergency threshold")?;
    assert!(
      configurable.iter().all(|row| row.level.is_some()),
      "a threshold that cannot be moved"
    );
    assert!(emergency.level.is_none());
    assert_eq!(emergency.aside, "1%");
    let second = at(&rows, 1)?;
    assert_eq!(second.aside, "25%");

    Ok(())
  }

  #[test]
  fn a_threshold_turned_off_says_so_in_a_word() {
    assert_eq!(threshold(console_battery::NEVER), Ok(Word::Never.english()));
    assert_eq!(threshold(5), Ok("5%".to_string()));
  }

  #[test]
  fn a_radio_that_is_looking_says_so_where_the_press_that_starts_it_was() -> Result<(), Failure> {
    let Ok(looking) = while_looking(Vec::new());
    let last = looking.last().ok_or("a last row")?;

    assert_eq!(last.says, LOOKING);
    assert!(
      last.does.is_none(),
      "a row saying it is looking is not a row to press"
    );

    let Ok(idle) = bluetooth_of(Power::On, Vec::new());
    let last_2 = idle.last().ok_or("a last row")?;

    assert_eq!(last_2.says, LOOK);

    Ok(())
  }

  #[test]
  fn the_loudest_stranger_is_the_nearest_row_to_the_thumb() {
    let Ok(heard) = heard(&["Far", "Near", "Middling"], &[-95, -40, -70]);
    let Ok(rows) = while_looking(heard);

    assert_eq!(
      says(&rows),
      Ok(vec![BLUETOOTH, "Near", "Middling", "Far", LOOKING])
    );
  }

  #[test]
  fn a_device_that_only_said_its_address_is_drawn_under_the_ones_that_said_a_name() {
    let Ok(silent) = quiet(
      0xFF,
      (Pairing::NotPaired, Connection::NotConnected, Some(-40)),
    );
    let Ok(keys) = device(
      0x66,
      "Blue Keys",
      (Pairing::NotPaired, Connection::NotConnected, Some(-95)),
    );
    let Ok(rows) = while_looking(vec![silent, keys]);

    assert_eq!(
      says(&rows),
      Ok(vec![BLUETOOTH, "Blue Keys", "E0-C1-02-00-B0-FF", LOOKING])
    );
  }

  #[test]
  fn what_a_stranger_says_beside_it_is_how_loud_it_is() -> Result<(), Failure> {
    let Ok(near) = device(
      0,
      "Near",
      (Pairing::NotPaired, Connection::NotConnected, Some(-50)),
    );
    let Ok(silent) = device(1, "Quiet", STRANGER);
    let Ok(rows) = while_looking(vec![near, silent]);
    let Ok(loud) = strength(100);

    let near_2 = row(&rows, "Near")?;
    assert_eq!(near_2.aside, loud);
    let quiet = row(&rows, "Quiet")?;
    assert_eq!(
      quiet.aside, YET,
      "a device the radio has heard nothing from says nothing"
    );

    Ok(())
  }

  #[test]
  fn a_radio_that_is_off_says_so_beside_its_own_name_rather_than_in_it() {
    let Ok(off) = wifi_of(wifi::Radio::Off, Vec::new());
    let Ok(on) = wifi_of(wifi::Radio::On, Vec::new());
    let Ok(on_says) = says(&on);

    assert_eq!(says(&off), Ok(vec![WIFI]));
    assert_eq!(on_says.first().copied(), Some(WIFI));
    assert_eq!(
      off.first().and_then(|row| row.control),
      Some(Control::Switch(Switch::Off))
    );
    assert_eq!(
      on.first().and_then(|row| row.control),
      Some(Control::Switch(Switch::On))
    );

    let Ok(dark) = bluetooth_of(Power::Off, Vec::new());

    assert_eq!(says(&dark), Ok(vec![BLUETOOTH]));
    assert_eq!(
      dark.first().and_then(|row| row.control),
      Some(Control::Switch(Switch::Off))
    );
    assert!(
      dark.first().is_some_and(|row| row.does.is_some()),
      "a row that says Off is still the row that turns it on"
    );
  }

  #[test]
  fn the_one_we_are_on_is_marked_rather_than_offered() -> Result<(), Failure> {
    let Ok(networks) = wifi::networks("yes:Home:71:2437 MHz:WPA2\nno:Cafe:50:2437 MHz:");
    let Ok(rows) = wifi_of(wifi::Radio::On, networks);
    let home = row(&rows, "Home")?;
    let cafe = row(&rows, "Cafe")?;

    assert_eq!(home.now(), Ok(Active::Yes));
    assert!(
      home.does.is_none(),
      "there is nothing to do about being where you are"
    );
    assert!(cafe.does.is_some());

    Ok(())
  }

  fn joined_by_the_card(said: &str, saved: &[String]) -> Result<Vec<Row>, Never> {
    let Ok(networks) = wifi::networks(said);

    wifi_rows(wifi::Radio::On, networks, saved, crate::card::join, |_| {
      nowhere()
    })
  }

  const HOME_CAFE_AND_LOCKED: &str =
    "no:Home:71:2437 MHz:WPA2\nno:Cafe:50:2437 MHz:\nno:Locked:40:2437 MHz:WPA2";

  #[test]
  fn a_network_is_joined_by_a_press_and_a_locked_one_with_the_password_typed_for_it()
  -> Result<(), Failure> {
    let saved = vec!["Home".to_string()];
    let Ok(rows) = joined_by_the_card(HOME_CAFE_AND_LOCKED, &saved);
    let home = pressed(&rows, "Home", Typing(""))?;
    let cafe = pressed(&rows, "Cafe", Typing(""))?;
    let locked = pressed(&rows, "Locked", Typing("hunter2"))?;
    let Ok(bringing_up) = wifi::joining("Home", wifi::Known::Yes);
    let Ok(connecting) = wifi::joining("Cafe", wifi::Known::No);
    let Ok(with_its_password) = wifi::joining_with("Locked", wifi::Password("hunter2"));

    assert_eq!(
      read_back(&home.ran),
      Ok(vec![bringing_up]),
      "a saved network is brought up without a password"
    );
    assert_eq!(read_back(&cafe.ran), Ok(vec![connecting]));
    assert_eq!(
      read_back(&locked.asked),
      Ok(vec!["Password for Locked".to_string()])
    );
    assert_eq!(read_back(&locked.ran), Ok(vec![with_its_password]));

    Ok(())
  }

  #[test]
  fn a_network_being_joined_says_so_and_a_second_press_starts_nothing() -> Result<(), Failure> {
    let saved = vec!["Home".to_string()];
    let Ok(rows) = joined_by_the_card(HOME_CAFE_AND_LOCKED, &saved);
    let Ok(bringing_up) = wifi::joining("Home", wifi::Known::Yes);
    let Ok(asked_for) = wifi::joining("Locked", wifi::Known::No);
    let Ok(typed) = wifi::joining_with("Locked", wifi::Password("hunter2"));

    let Ok(home) = while_running(rows.clone(), &bringing_up, &bringing_up);
    let Ok(locked) = while_running(rows, &typed, &asked_for);
    let joining_home = row(&home, "Joining Home")?;
    let joining_locked = row(&locked, "Joining Locked")?;
    let cafe = row(&home, "Cafe")?;

    assert_eq!(joining_home.aside, YET);
    assert!(
      joining_home.does.is_none(),
      "a second press of a network being joined starts nothing"
    );
    assert!(
      joining_locked.does.is_none(),
      "the network a password was typed for is the one being joined"
    );
    assert!(
      cafe.does.is_some(),
      "joining one network leaves the others to be pressed"
    );

    Ok(())
  }

  #[test]
  fn the_wifi_radio_is_turned_by_a_press_and_says_which_way_while_it_turns() -> Result<(), Failure>
  {
    let Ok(dark) = wifi_of(wifi::Radio::Off, Vec::new());
    let Ok(lit) = wifi_of(wifi::Radio::On, Vec::new());
    let Ok(on) = Command::external(Program::Nmcli, &["radio", "wifi", "on"]);
    let Ok(off) = Command::external(Program::Nmcli, &["radio", "wifi", "off"]);
    let switched_on = pressed(&dark, WIFI, Typing(""))?;
    let switched_off = pressed(&lit, WIFI, Typing(""))?;

    assert_eq!(read_back(&switched_on.ran), Ok(vec![on.clone()]));
    assert_eq!(read_back(&switched_off.ran), Ok(vec![off.clone()]));

    let Ok(turning_on) = while_running(dark, &on, &on);
    let Ok(turning_off) = while_running(lit, &off, &off);
    let turning_on_row = row(&turning_on, TURNING_ON.0)?;
    let turning_off_row = row(&turning_off, TURNING_OFF.0)?;

    assert!(turning_on_row.does.is_none());
    assert!(turning_off_row.does.is_none());

    Ok(())
  }

  #[test]
  fn the_network_we_are_on_offers_its_code_on_y_and_no_other_does() {
    let Ok(networks) = wifi::networks("yes:Home:71:2437 MHz:WPA2\nno:Cafe:50:2437 MHz:WPA2");
    let Ok(rows) = wifi_of(wifi::Radio::On, networks);
    let offered: Vec<&str> = rows
      .iter()
      .filter(|row| row.more.is_some())
      .map(|row| row.says.as_str())
      .collect();

    assert_eq!(offered, ["Home"]);
    assert_eq!(
      says(&rows),
      Ok(vec![WIFI, "Home", "Cafe"]),
      "the code is a choice on Y rather than a row of its own"
    );
  }

  #[test]
  fn a_joined_device_is_marked_and_offers_the_way_out_of_it() -> Result<(), Failure> {
    let Ok(pads) = device(0, "Pads", JOINED);
    let Ok(speaker) = device(1, "Speaker", FRIEND);
    let Ok(rows) = bluetooth_of(Power::On, vec![pads, speaker]);

    let pads_2 = row(&rows, "Pads")?;
    assert_eq!(pads_2.now(), Ok(Active::Yes));
    let speaker_2 = row(&rows, "Speaker")?;
    assert_eq!(speaker_2.now(), Ok(Active::No));

    Ok(())
  }

  #[test]
  fn a_device_no_one_has_been_introduced_to_is_not_offered_a_word_bluez_would_refuse()
  -> Result<(), Failure> {
    let Ok(devices) = named(&["Blue Keys"], STRANGER);
    let Ok(rows) = bluetooth_of(Power::On, devices);
    let stranger = row(&rows, "Blue Keys")?;

    assert!(
      stranger.does.is_some(),
      "a stranger has to be pressable to become known"
    );
    assert_eq!(
      stranger.aside, YET,
      "and has to read as somewhere still to go"
    );

    Ok(())
  }

  #[test]
  fn a_stranger_is_offered_the_pairing_and_a_friend_the_road_and_the_way_out() {
    let Ok(back) = back_to("Blue Keys");
    let Ok(stranger) = device(0, "Blue Keys", STRANGER);
    let Ok(friend) = device(0, "Blue Keys", FRIEND);
    let Ok(joined) = device(0, "Blue Keys", JOINED);

    let Ok(stranger_rows) = meeting(&stranger);
    assert_eq!(stranger_rows, [back.as_str(), PAIR]);
    let Ok(friend_rows) = meeting(&friend);
    assert_eq!(friend_rows, [back.as_str(), JOIN, FORGET]);
    let Ok(joined_rows) = meeting(&joined);
    assert_eq!(joined_rows, [back.as_str(), LEAVE, FORGET]);
  }

  #[test]
  fn a_row_that_acts_on_the_device_the_page_names_does_not_name_it_again() {
    for standing in [STRANGER, FRIEND] {
      let Ok(keyboard) = device(0, "Blue Keys", standing);
      let Ok(said) = meeting(&keyboard);

      for row in said.iter().skip(1) {
        assert!(
          !row.contains("Blue Keys"),
          "{row:?} says the name the page it is on is already titled with"
        );
      }
    }
  }

  #[test]
  fn the_word_for_going_ahead_is_short_and_stands_for_nothing() {
    for word in [FORGET_YES, JOIN, LEAVE, PAIR] {
      let words: BTreeSet<String> = word.split_whitespace().map(str::to_lowercase).collect();

      assert!(
        words.len() <= 2,
        "{word:?} is a sentence where a verb was wanted"
      );

      for standing in ["it", "them", "this", "that", "these", "those"] {
        assert!(
          !words.contains(standing),
          "{word:?} says {standing:?} where the question already said the thing"
        );
      }
    }
  }

  #[test]
  fn every_device_row_is_the_row_its_own_page_comes_back_to() {
    let Ok(devices) = named(&["One", "Two", "Three"], STRANGER);
    let (telling, seen) = std::sync::mpsc::sync_channel(3);
    let Ok(rows) = bluetooth_rows(
      Power::On,
      Discovery::NotSearching,
      devices.clone(),
      move |_: &Device, at, _: &dyn Showing| {
        assert_eq!(telling.try_send(at), Ok(()));
      },
    );

    for row in &rows {
      match &row.does {
        Some(Handler::Call(act)) => {
          let _ = act(&Nowhere);
        }
        Some(Handler::Run(_)) | None => {}
      }
    }

    let recorded: Vec<_> = seen.try_iter().collect();
    let names: BTreeSet<&str> = devices.iter().map(|device| device.alias.as_str()).collect();
    let standing: Vec<u32> = rows
      .iter()
      .enumerate()
      .filter(|(_, row)| names.contains(row.says.as_str()))
      .map(|(place, _)| {
        let Ok(place_2) = fitted(place);

        place_2
      })
      .collect();

    assert_eq!(recorded, standing);
  }

  #[test]
  fn every_bluetooth_row_tells_console_bluetooth_about_the_device_it_is_on() -> Result<(), Failure>
  {
    let Ok(pads) = device(0, "Pads", JOINED);
    let Ok(speaker) = device(1, "Speaker", FRIEND);
    let Ok(rows) = bluetooth_of(Power::On, vec![pads.clone(), speaker.clone()]);
    let Ok(dark) = bluetooth_of(Power::Off, Vec::new());

    let Ok(off) = line(&command::Action::Once(Once::Power(Power::Off)));
    let Ok(on) = line(&command::Action::Once(Once::Power(Power::On)));
    let Ok(leaving) = line(&command::Action::Once(Once::Disconnect(pads.address)));
    let Ok(joining) = line(&command::Action::Once(Once::Connect(speaker.address)));
    let Ok(searching) = line(&command::Action::Search);

    let switched = pressed(&rows, BLUETOOTH, Typing(""))?;
    let lit = pressed(&dark, BLUETOOTH, Typing(""))?;
    let left = pressed(&rows, "Pads", Typing(""))?;
    let joined = pressed(&rows, "Speaker", Typing(""))?;
    let looked = pressed(&rows, LOOK, Typing(""))?;

    assert_eq!(read_back(&switched.ran), Ok(vec![off]));
    assert_eq!(read_back(&lit.ran), Ok(vec![on]));
    assert_eq!(read_back(&left.ran), Ok(vec![leaving]));
    assert_eq!(read_back(&joined.ran), Ok(vec![joining]));
    assert_eq!(read_back(&looked.ran), Ok(vec![searching]));

    Ok(())
  }

  #[test]
  fn a_device_that_pairs_without_a_typed_pin_pairs_at_the_press() -> Result<(), Failure> {
    let Ok(keyboard) = device(0xC3, "Blue Keys", STRANGER);
    let Ok(nowhere) = nowhere();
    let Ok(rows) = meeting_rows(&keyboard, nowhere);
    let pairing = pressed(&rows, PAIR, Typing(""))?;

    let Ok(introducing) = line(&command::Action::Introduce {
      address: keyboard.address,
      pin: None,
    });

    assert_eq!(
      read_back(&pairing.asked),
      Ok(Vec::new()),
      "nothing is asked of somebody bluez will not need a PIN from"
    );
    assert_eq!(read_back(&pairing.ran), Ok(vec![introducing]));

    Ok(())
  }

  #[test]
  fn a_device_that_pairs_only_by_a_pin_from_its_box_asks_for_it_before_pairing_starts()
  -> Result<(), Failure> {
    let Ok(plain) = device(0xC3, "Old Thing", STRANGER);
    let old = Device {
      pairs_with: PairingMethod::Pin,
      ..plain
    };
    let Ok(nowhere) = nowhere();
    let Ok(rows) = meeting_rows(&old, nowhere);
    let pairing = pressed(&rows, PAIR, Typing("1234"))?;

    let Ok(pin) = PinCode::read("1234");
    let Ok(introducing) = line(&command::Action::Introduce {
      address: old.address,
      pin,
    });

    assert_eq!(read_back(&pairing.asked), Ok(vec!["PIN".to_string()]));
    assert_eq!(
      read_back(&pairing.ran),
      Ok(vec![introducing]),
      "the PIN rides on the pairing it was asked for"
    );

    Ok(())
  }

  #[test]
  fn a_pin_bluez_would_not_take_is_said_rather_than_sent() -> Result<(), Failure> {
    let Ok(plain) = device(0xC3, "Old Thing", STRANGER);
    let old = Device {
      pairs_with: PairingMethod::Pin,
      ..plain
    };
    let Ok(nowhere) = nowhere();
    let Ok(rows) = meeting_rows(&old, nowhere);
    let pairing = pressed(&rows, PAIR, Typing("0123456789abcdefg"))?;

    assert_eq!(read_back(&pairing.ran), Ok(Vec::new()));

    Ok(())
  }

  #[test]
  fn a_pairing_says_its_code_while_it_runs_and_the_rest_only_when_they_end() -> Result<(), Failure>
  {
    let Ok(keyboard) = device(0xC3, "Blue Keys", STRANGER);
    let Ok(speaker) = device(1, "Speaker", FRIEND);
    let Ok(nowhere) = nowhere();
    let Ok(stranger_page) = meeting_rows(&keyboard, nowhere);
    let Ok(list) = bluetooth_of(Power::On, vec![speaker]);

    let pairing = pressed(&stranger_page, PAIR, Typing(""))?;
    let joining = pressed(&list, "Speaker", Typing(""))?;

    assert_eq!(
      read_back(&pairing.speaking),
      Ok(vec![Speaks::OutLoud]),
      "a keyboard's code is said while the pairing waits on it"
    );
    assert_eq!(read_back(&joining.speaking), Ok(vec![Speaks::AtTheEnd]));

    Ok(())
  }

  fn while_running(
    rows: Vec<Row>,
    command: &console_program_contract::Command,
    row: &console_program_contract::Command,
  ) -> Result<Vec<Row>, Never> {
    let Ok(started) = Started::of(command.clone(), row.clone());
    let running = Running {
      started: vec![started],
      ..Running::default()
    };

    tasks::shown(rows, &running)
  }

  #[test]
  fn a_pairing_that_is_running_says_so_and_a_second_press_starts_nothing() -> Result<(), Failure>
  {
    let Ok(keyboard) = device(0xC3, "Blue Keys", STRANGER);
    let Ok(nowhere) = nowhere();
    let Ok(rows) = meeting_rows(&keyboard, nowhere);
    let Ok(introducing) = line(&command::Action::Introduce {
      address: keyboard.address,
      pin: None,
    });

    let Ok(drawn) = while_running(rows, &introducing, &introducing);
    let pairing = row(&drawn, PAIRING.0)?;

    assert_eq!(pairing.aside, YET);
    assert!(
      pairing.does.is_none(),
      "a second press of a pairing that is running starts nothing"
    );
    assert!(
      drawn.iter().all(|offered| offered.says != PAIR),
      "nothing on the page offers to pair again"
    );

    Ok(())
  }

  #[test]
  fn a_pairing_a_pin_was_typed_for_is_the_pairing_its_row_is_waiting_on() -> Result<(), Failure> {
    let Ok(plain) = device(0xC3, "Old Thing", STRANGER);
    let old = Device {
      pairs_with: PairingMethod::Pin,
      ..plain
    };
    let Ok(nowhere) = nowhere();
    let Ok(rows) = meeting_rows(&old, nowhere);
    let Ok(pin) = PinCode::read("1234");
    let Ok(asked_for) = line(&command::Action::Introduce {
      address: old.address,
      pin: None,
    });
    let Ok(typed) = line(&command::Action::Introduce {
      address: old.address,
      pin,
    });

    let Ok(drawn) = while_running(rows, &typed, &asked_for);
    let pairing = row(&drawn, PAIRING.0)?;

    assert!(pairing.does.is_none());

    Ok(())
  }

  #[test]
  fn a_device_on_the_list_that_is_connecting_says_which_one_and_offers_only_to_stop()
  -> Result<(), Failure> {
    let Ok(speaker) = device(1, "Speaker", FRIEND);
    let Ok(rows) = bluetooth_of(Power::On, vec![speaker.clone()]);
    let Ok(joining) = line(&command::Action::Once(Once::Connect(speaker.address)));

    let Ok(drawn) = while_running(rows, &joining, &joining);
    let connecting = row(&drawn, "Connecting to Speaker")?;
    let stopping = Pressed {
      typed: "",
      asked: std::cell::Cell::new(Vec::new()),
      ran: std::cell::Cell::new(Vec::new()),
      speaking: std::cell::Cell::new(Vec::new()),
    };

    match &connecting.more {
      Some(more) => {
        let _stays = more(&stopping);
      }
      None => {
        return Err(Failure::from(
          "a connection in progress offers nothing to do with it".to_string(),
        ));
      }
    }

    assert!(connecting.does.is_none());
    assert_eq!(
      read_back(&stopping.asked),
      Ok(vec![format!(
        "{} Connecting to Speaker?",
        console_panel::tasks::STOP
      )]),
      "what a connection in progress offers is to stop it, and never its page"
    );

    Ok(())
  }

  #[test]
  fn the_radio_says_which_way_it_is_turning() -> Result<(), Failure> {
    let Ok(dark) = bluetooth_of(Power::Off, Vec::new());
    let Ok(lit) = bluetooth_of(Power::On, Vec::new());
    let Ok(on) = line(&command::Action::Once(Once::Power(Power::On)));
    let Ok(off) = line(&command::Action::Once(Once::Power(Power::Off)));

    let Ok(turning_on) = while_running(dark, &on, &on);
    let Ok(turning_off) = while_running(lit, &off, &off);

    let turning_on_row = row(&turning_on, TURNING_ON.0)?;
    let turning_off_row = row(&turning_off, TURNING_OFF.0)?;

    assert!(turning_on_row.does.is_none());
    assert!(turning_off_row.does.is_none());

    Ok(())
  }

  #[test]
  fn an_unavailable_output_is_shown_without_a_volume_control() {
    let Ok(rows) = sound(&[], "");

    assert_eq!(says(&rows), Ok(vec!["No audio output connected"]));
  }

  #[test]
  fn the_engine_in_use_is_the_one_marked() {
    let Ok(rows) = searching("startpage");

    assert_eq!(marked(&rows), Ok(vec!["Startpage"]));
  }

  #[test]
  fn the_browsers_are_told_without_stopping_to_ask_for_a_password() {
    let Ok(telling) = set_engine_command("startpage");

    assert_eq!(
      telling.program,
      console_program_contract::Executable::External(Program::Sudo)
    );
    assert_eq!(telling.arguments, ["-n", "console-engine", "startpage"]);
  }

  #[test]
  fn the_list_is_the_way_back_and_then_a_row_that_only_reads() -> Result<(), Failure> {
    let Ok(rows) = searching("duckduckgo");
    let Ok(configured) = configuration();
    let back = at(&rows, 0)?;
    let named = at(&rows, 1)?;

    assert!(
      back.says.ends_with(&configured),
      "{:?} is not the way back",
      back.says
    );
    assert_eq!(named.says, "Search Engine");
    assert_eq!(named.heading(), Ok(Heading::Yes));

    Ok(())
  }

  #[test]
  fn the_engine_is_named_the_way_it_is_named_on_its_own_row() {
    assert_eq!(engine_says("startpage"), Ok("Startpage".to_string()));
    assert_eq!(engine_says("telepathy"), Ok(String::new()));
  }

  #[test]
  fn the_language_being_listened_for_is_the_one_marked() {
    let Ok(rows) = listening_for("nl");

    assert_eq!(marked(&rows), Ok(vec!["Dutch"]));
  }

  #[test]
  fn chinese_is_not_on_the_list() -> Result<(), Failure> {
    let Ok(rows) = listening_for("auto");
    let Ok(said) = says(&rows);
    let rows_after = after(&rows, 2)?;
    let Ok(offered) = says(rows_after);

    assert!(!said.contains(&"Chinese"));
    assert_eq!(offered, ["Automatic", "English", "Dutch", "Thai"]);

    Ok(())
  }

  #[test]
  fn the_languages_are_a_list_under_the_tab_like_the_engines() -> Result<(), Failure> {
    let Ok(rows) = listening_for("auto");
    let Ok(tab) = the_language();
    let Ok(listens) = text(&Word::DictationLanguage);
    let back = at(&rows, 0)?;
    let named = at(&rows, 1)?;

    assert!(
      back.says.ends_with(&tab),
      "{:?} is not the way back",
      back.says
    );
    assert_eq!(named.says, listens);
    assert_eq!(named.heading(), Ok(Heading::Yes));

    Ok(())
  }

  #[test]
  fn the_language_is_named_the_way_it_is_named_on_its_own_row() {
    assert_eq!(dictation_says("th"), Ok("Thai".to_string()));
    assert_eq!(dictation_says("auto"), Ok("Automatic".to_string()));
    assert_eq!(dictation_says("zh"), Ok(String::new()));
  }

  #[test]
  fn the_language_tab_is_three_rows_that_open_and_one_that_says_what_it_does_not_do()
  -> Result<(), Failure> {
    let Ok(rows) = language();
    let Ok(words) = text(&Word::PreferredLanguage);
    let Ok(types) = text(&Word::Keyboards);
    let Ok(listens) = text(&Word::DictationLanguage);
    let Ok(english) = text(&Word::EnglishOnly);
    let three = part(&rows, (0, 3))?;
    let Ok(said) = says(three);

    assert_eq!(said, [words.as_str(), types.as_str(), listens.as_str()]);

    for row in three {
      assert_eq!(row.acts(), Ok(Action::Yes), "{:?}", row.says);
    }

    assert_eq!(rows.last().map(|row| row.says.clone()), Some(english));

    Ok(())
  }

  #[test]
  fn each_of_the_three_says_what_it_is_now_beside_it() {
    let Ok(rows) = language();
    let asides: Vec<&str> = rows.iter().take(3).map(|row| row.aside.as_str()).collect();

    assert_eq!(asides, ["English (United Kingdom)", "Latin, Thai", "Dutch"]);
  }

  #[test]
  fn the_languages_are_the_way_back_a_heading_and_then_every_language() -> Result<(), Failure> {
    let Ok(spoken) = sample_languages();
    let Ok(nowhere) = nowhere();
    let Ok(rows) = language_picker_rows(&spoken, None, nowhere, opening);
    let Ok(tab) = the_language();
    let back = at(&rows, 0)?;
    let rows_after = after(&rows, 2)?;
    let Ok(languages) = says(rows_after);

    assert!(
      back.says.ends_with(&tab),
      "{:?} is not the way back",
      back.says
    );
    let second = at(&rows, 1)?;
    assert_eq!(second.heading(), Ok(Heading::Yes));
    assert_eq!(languages, ["en", "nl", "th"]);

    Ok(())
  }

  #[test]
  fn every_language_carries_the_row_it_was_opened_from_so_b_lands_back_on_it() {
    let Ok((seen, opening)) = recorder();
    let Ok(spoken) = sample_languages();
    let Ok(nowhere) = nowhere();
    let Ok(rows) = language_picker_rows(&spoken, None, nowhere, opening);
    let Ok(seen_2) = opened(&seen);
    let Ok(()) = opened_from(&rows, &seen_2);
  }

  #[test]
  fn every_part_of_the_world_carries_the_row_it_was_opened_from() {
    let Ok((seen, opening)) = recorder();
    let Ok(zones) = hours::zones("Europe/Amsterdam\nAsia/Bangkok\nUTC");
    let Ok(regions) = hours::regions(&zones);
    let Ok(nowhere) = nowhere();
    let Ok(rows) = region_rows(&regions, None, nowhere, opening);
    let Ok(seen_2) = opened(&seen);
    let Ok(()) = opened_from(&rows, &seen_2);
  }

  #[test]
  fn the_language_the_machine_is_in_is_the_one_marked() {
    let Ok(spoken) = sample_languages();
    let Ok(standing) = crate::languages::current_locale(&spoken, Some("nl_NL.UTF-8"));
    let Ok(nowhere) = nowhere();
    let Ok(rows) = language_picker_rows(&spoken, standing.as_ref(), nowhere, opening);

    assert_eq!(marked(&rows), Ok(vec!["nl"]));
  }

  #[test]
  fn the_places_a_language_is_spoken_are_a_list_under_it() -> Result<(), Failure> {
    let Ok(spoken) = sample_languages();
    let Ok(names) = Names::none();
    let english = spoken
      .iter()
      .find(|language| language.language == "en")
      .ok_or("English")?;
    let Ok(standing) = crate::languages::current_locale(&spoken, Some("en_GB.UTF-8"));
    let Ok(nowhere) = nowhere();
    let Ok(rows) = place_rows(english, &names, &[], standing.as_ref(), nowhere);
    let rows_after = after(&rows, 2)?;
    let Ok(places) = says(rows_after);

    assert_eq!(places, ["GB", "US"]);
    assert_eq!(marked(&rows), Ok(vec!["GB"]));

    Ok(())
  }

  #[test]
  fn the_parts_of_the_world_open_onto_the_places_in_them() -> Result<(), Failure> {
    let Ok(zones) = hours::zones("Europe/Amsterdam\nEurope/London\nAsia/Bangkok");
    let Ok(regions) = hours::regions(&zones);
    let Ok(nowhere) = nowhere();
    let Ok(rows) = region_rows(&regions, Some("Europe/Amsterdam"), nowhere, opening);
    let rows_after = after(&rows, 2)?;
    let Ok(parts) = says(rows_after);

    assert_eq!(parts, ["Asia", "Europe"]);
    assert_eq!(marked(&rows), Ok(vec!["Europe"]));

    Ok(())
  }

  #[test]
  fn a_part_of_the_world_is_its_places_with_the_one_it_is_in_marked() -> Result<(), Failure> {
    let Ok(zones) = hours::zones("Europe/Amsterdam\nEurope/London\nAsia/Bangkok");
    let Ok(places) = hours::places(&zones, "Europe");
    let Ok(nowhere) = nowhere();
    let Ok(rows) = zone_rows("Europe", &places, Some("Europe/London"), nowhere);
    let rows_after = after(&rows, 2)?;
    let Ok(cities) = says(rows_after);

    assert_eq!(cities, ["Amsterdam", "London"]);
    assert_eq!(marked(&rows), Ok(vec!["London"]));

    Ok(())
  }

  #[test]
  fn the_clock_is_the_hour_written_both_ways_with_one_of_them_marked() -> Result<(), Failure> {
    let Ok(nowhere) = nowhere();
    let Ok(rows) = clock_rows(Clock::Twelve, nowhere);
    let rows_after = after(&rows, 2)?;
    let Ok(ways) = says(rows_after);

    assert_eq!(ways, ["14:30", "2:30 pm"]);
    assert_eq!(marked(&rows), Ok(vec!["2:30 pm"]));

    Ok(())
  }

  #[test]
  fn the_machines_name_is_a_row_that_asks_rather_than_one_that_opens() {
    let Ok(row) = called_row("legion");
    let Ok(asks) = text(&Word::Name);

    assert_eq!(row.says, asks);
    assert_eq!(row.aside, "legion");
    assert_eq!(row.acts(), Ok(Action::Yes));
  }

  #[test]
  fn every_tab_is_named_once() {
    let Ok(tabs) = tabs();
    let named: BTreeSet<&String> = tabs.iter().collect();

    assert_eq!(named.len(), tabs.len());
  }
}
