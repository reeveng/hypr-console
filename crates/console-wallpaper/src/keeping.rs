//! What the wallpaper daemon does about what it found, and when it wakes next.
//!
//! Three of the four things this program holds between one look and the next
//! are here, and the fourth -- what the sky outside is doing -- only ever
//! arrives from somewhere else. What is decided is the settling: a wallpaper
//! that has just been covered is not put away at once, because the thing in
//! front of it may be a menu someone is about to close, and swapping a moving
//! picture for a still one and back again is worse than leaving it moving for
//! a few seconds.
//!
//! The still picture is put up before the moving one rather than after,
//! whenever the moving one is going up for the first time. A frame is what the
//! wallpaper daemon holds while anything is in front of it, so the first frame
//! of an animation someone has not seen yet is a picture rather than whatever
//! the decoder happened to hand over.
//!
//! A weather that did not arrive is not a weather. Asking is over a network on
//! a handheld that is carried out of range of one, and a curl that timed out
//! says nothing about the sky -- so the last answer stays where it is and the
//! picture goes on being the one for the rain it was raining, rather than
//! falling back to the no-weather picture every time the wifi drops and
//! climbing back out of it a minute later.
//!
//! Waking is an effect here rather than a wait in the loop. Between the three
//! reasons it wakes this program is asleep, and that is the whole of what it
//! costs a handheld -- so how long to sleep for is a decision like any other,
//! and one a test can hold against the clock rather than against a stopwatch.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::units::UnixSeconds;
use console_core_arguments::{Command, Flag, Operands, Presence, Takes};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service};
use console_location::Coordinates;
use console_program_contract::{
  Answer, Change, Effect, Elapsed, Event, EventGroup, Exit, Fork, JobId, JobOwner, Subscription,
  Task, Timer,
};
use console_weather::conditions::{self, Weather};

use crate::choose::{self, Outside, Picture, Selection, Turn, Wanted};
use crate::covered::{self, Covered, Worth};
use crate::rules::{self, Rules};

pub const NOW: Flag = Flag {
  spelling: "--now",
  takes: Takes::None,
  about: "put one picture up and stop, rather than keeping it up to date",
};

pub const RUN_SCENE: Flag = Flag {
  spelling: "--run-scene",
  takes: Takes::None,
  about: "run the selected live wallpaper for the session's wallpaper service",
};

pub const COMMAND: Command = Command {
  name: "console-wallpaper",
  about: "the picture behind the desktop, kept up to date with the weather and the time of day",
  flags: &[NOW, RUN_SCENE],
  operands: Operands::None,
};

pub const LOOK_AGAIN: Duration = Duration::from_secs(300);

pub const TRY_AGAIN: Duration = Duration::from_secs(2);

pub const SETTLE: Duration = Duration::from_secs(15);

pub const SOONEST: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Going {
  Once,
  KeepGoing,
}

impl Going {
  pub fn of(now: Presence) -> Result<Self, Never> {
    Ok(match now {
      Presence::Present => Going::Once,
      Presence::Absent => Going::KeepGoing,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Away {
  PutIt,
  NotYet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rendered {
  Yes,
  No,
}

const ASK_AGAIN: Duration = Duration::from_secs(1200);
const ASK_SOONER: Duration = Duration::from_secs(60);
const LOOK_TIMER: &str = "wallpaper look";
const WEATHER_TIMER: &str = "wallpaper weather";
const COMPOSITOR: Subscription = Subscription::EventGroup(EventGroup::Compositor);
const WEATHER_OWNER: JobOwner = JobOwner("wallpaper weather");

pub struct Input<'a> {
  pub going: Going,
  pub pictures: &'a [Picture],
  pub rule_directory: Option<&'a Path>,
}

#[derive(Debug, Clone)]
pub struct Sky<'a> {
  pub showing: Option<PathBuf>,
  pub covered_since: Option<Elapsed>,
  pub weather: Option<Weather>,
  going: Going,
  pictures: &'a [Picture],
  rule_directory: Option<&'a Path>,
  look_timer: Option<Timer>,
  weather_timer: Option<Timer>,
  weather_delay: Duration,
  weather_attempt: u64,
  weather_job: Option<Task>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
  pub seconds: UnixSeconds,
  pub elapsed: Elapsed,
  pub at: Coordinates,
  pub wanted: Wanted,
  pub pinned_file: bool,
  pub rules: Rules,
  pub covered: Covered,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WallpaperEvent {
  Reconnected,
  Observed(Observation),
  WeatherLocation(Coordinates),
  Weather(Option<Weather>),
  Looked {
    elapsed: Elapsed,
    covered: Covered,
    chosen: Option<Chosen>,
  },
  Rendered {
    at: PathBuf,
    went: Rendered,
  },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
  pub moving: PathBuf,
  pub still: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallpaperEffect {
  Observe,
  Resolve {
    name: String,
    elapsed: Elapsed,
    covered: Covered,
  },
  WeatherLocation,
  ReadWeather(Answer),
  Paint(PathBuf),
  Freeze(PathBuf),
}

impl EffectDependencies for WallpaperEffect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Observe => &[
        Service::Clock,
        Service::Compositor,
        Service::Configuration,
        Service::FileSystem,
      ],
      Self::Resolve { .. } => &[Service::FileSystem],
      Self::WeatherLocation => &[Service::Clock, Service::Configuration],
      Self::ReadWeather(_) => &[],
      Self::Paint(_) | Self::Freeze(_) => &[Service::FileSystem, Service::Display],
    })
  }
}

pub struct Sun<'a>(PhantomData<&'a [Picture]>);

impl<'a> Machine for Sun<'a> {
  type Input = Input<'a>;
  type State = Sky<'a>;
  type Request = Event<WallpaperEvent>;
  type Effect = Effect<WallpaperEffect>;

  fn initialize(
    input: &Input<'a>,
    _previous: Option<Sky<'a>>,
    _effects: &mut Effects,
  ) -> Result<Sky<'a>, Never> {
    Ok(Sky {
      showing: None,
      covered_since: None,
      weather: None,
      going: input.going,
      pictures: input.pictures,
      rule_directory: input.rule_directory,
      look_timer: None,
      weather_timer: None,
      weather_delay: ASK_SOONER,
      weather_attempt: 0,
      weather_job: None,
    })
  }

  fn handle(
    mut state: Sky<'a>,
    event: Event<WallpaperEvent>,
    effects: &mut Effects,
  ) -> Result<Sky<'a>, Never> {
    match event {
      Event::Opened => {
        let Ok(()) = opened(&state, effects);
      }
      Event::Modified(Change {
        event_group: EventGroup::Compositor,
        text,
      }) => {
        let Ok(worth) = covered::worth_waking_for(&text);

        match worth {
          Worth::Waking => {
            let Ok(()) = effects.offer(Effect::Custom(WallpaperEffect::Observe));
          }
          Worth::Ignoring => {}
        }
      }
      Event::Modified(Change {
        event_group: EventGroup::Path(folder),
        text,
      }) => {
        let Ok(change) = configuration_changed(state.rule_directory, &folder, Path::new(&text));

        match change {
          Worth::Waking => {
            let Ok(()) = effects.offer(Effect::Custom(WallpaperEffect::Observe));
          }
          Worth::Ignoring => {}
        }
      }
      Event::Custom(WallpaperEvent::Reconnected) => {
        let Ok(()) = effects.offer(Effect::Custom(WallpaperEffect::Observe));
      }
      Event::Tick(timer, _) => {
        let look = state.look_timer.filter(|scheduled| *scheduled == timer);
        let weather = state.weather_timer.filter(|scheduled| *scheduled == timer);

        match (look, weather) {
          (Some(_), _) => {
            let Ok(()) = effects.offer(Effect::Custom(WallpaperEffect::Observe));
          }
          (None, Some(_)) => {
            state.weather_timer = None;
            let Ok(()) = effects.offer(Effect::Custom(WallpaperEffect::WeatherLocation));
          }
          (None, None) => {}
        }
      }
      Event::Custom(WallpaperEvent::Observed(observed)) => {
        let Ok(()) = observe(&mut state, observed, effects);
      }
      Event::Custom(WallpaperEvent::WeatherLocation(at)) => {
        let Ok(()) = ask_weather(&mut state, &at, effects);
      }
      Event::Replied(answer) => match answer.job.filter(|job| Some(*job) == state.weather_job) {
        Some(_) => {
          state.weather_job = None;
          let Ok(()) = effects.offer(Effect::Custom(WallpaperEffect::ReadWeather(answer)));
        }
        None => {}
      },
      Event::Custom(WallpaperEvent::Weather(weather)) => {
        let Ok(()) = weather_arrived(&mut state, weather, effects);
      }
      Event::Custom(WallpaperEvent::Rendered { at, went }) => {
        let Ok(()) = rendered(&mut state, at, went, effects);
      }
      Event::Custom(WallpaperEvent::Looked {
        elapsed,
        covered,
        chosen,
      }) => {
        let Ok(()) = looked(&mut state, elapsed, covered, chosen.as_ref(), effects);
      }
      Event::Stopping => {
        let Ok(()) = stop(&mut state, effects);
      }
      Event::Modified(_) | Event::Written(_) | Event::Chosen(_) => {}
    }

    Ok(state)
  }
}

type Effects = Queue<Effect<WallpaperEffect>>;

fn opened(state: &Sky<'_>, effects: &mut Effects) -> Result<(), Never> {
  match state.going {
    Going::KeepGoing => {
      let Ok(()) = effects.offer(Effect::Subscribe(COMPOSITOR));
      let Ok(watch) = configuration_watch(state.rule_directory);

      match watch {
        Some(subscription) => {
          let Ok(()) = effects.offer(Effect::Subscribe(subscription));
        }
        None => {}
      }

      let Ok(()) = effects.offer(Effect::Custom(WallpaperEffect::WeatherLocation));
    }
    Going::Once => {}
  }

  effects.offer(Effect::Custom(WallpaperEffect::Observe))
}

fn rendered(
  state: &mut Sky<'_>,
  at: PathBuf,
  went: Rendered,
  effects: &mut Effects,
) -> Result<(), Never> {
  match (went, state.going) {
    (Rendered::Yes, going) => {
      state.showing = Some(at);

      match going {
        Going::Once => effects.offer(Effect::Stop(Exit::Success)),
        Going::KeepGoing => Ok(()),
      }
    }
    (Rendered::No, Going::Once) => effects.offer(Effect::Stop(Exit::Failure(
      "the wallpaper could not be painted".to_string(),
    ))),
    (Rendered::No, Going::KeepGoing) => {
      let Ok(retry) = look_timer(TRY_AGAIN);
      change_timer(
        &mut state.look_timer,
        Some(retry),
        Subscription::Timer,
        effects,
      )
    }
  }
}

fn configuration_watch(directory: Option<&Path>) -> Result<Option<Subscription>, Never> {
  Ok(
    directory
      .and_then(Path::parent)
      .map(|folder| Subscription::EventGroup(EventGroup::Path(folder.to_path_buf()))),
  )
}

fn configuration_changed(
  directory: Option<&Path>,
  folder: &Path,
  changed: &Path,
) -> Result<Worth, Never> {
  let rules = match directory {
    Some(rules) => rules,
    None => return Ok(Worth::Ignoring),
  };
  let scenes = folder.join(crate::scene::DIRECTORY);
  let affects_wallpaper = rules.parent() == Some(folder)
    && (changed == folder
      || changed == rules
      || changed == folder.join(crate::place::SELECTION)
      || changed == scenes
      || changed.parent() == Some(scenes.as_path())
      || (changed.parent() == Some(rules)
        && changed.extension().is_some_and(|kind| kind == "toml")));

  Ok(match bool::then_some(affects_wallpaper, ()) {
    Some(()) => Worth::Waking,
    None => Worth::Ignoring,
  })
}

fn observe(
  state: &mut Sky<'_>,
  reading: Observation,
  effects: &mut Effects,
) -> Result<(), Never> {
  let pinned = match bool::then_some(
    reading.pinned_file && !matches!(reading.wanted.selection, Selection::Following),
    (),
  ) {
    ::core::option::Option::Some(()) => Some(reading.wanted.picture.as_str()),
    ::core::option::Option::None => {
      let Ok(pinned) = choose::pinned(state.pictures, &reading.wanted);
      pinned.map(|picture| picture.name.as_str())
    }
  };

  match ::core::primitive::bool::then_some(
    state.going == Going::Once && state.weather_attempt == 0 && pinned.is_none(),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return ask_weather(state, &reading.at, effects);
    }
    ::core::option::Option::None => {}
  }

  let Ok(outside) = Outside::at(&reading.at, reading.seconds, state.weather);
  let Ok(turn) = Turn::at(reading.seconds);
  let wanted = match pinned {
    Some(name) => Some(name),
    None => {
      let Ok(picked) = rules::pick(&reading.rules, state.pictures, &outside, turn);
      picked
    }
  };

  match wanted {
    Some(name) => effects.offer(Effect::Custom(WallpaperEffect::Resolve {
      name: name.to_string(),
      elapsed: reading.elapsed,
      covered: reading.covered,
    })),
    None => looked(state, reading.elapsed, reading.covered, None, effects),
  }
}

fn ask_weather(
  state: &mut Sky<'_>,
  at: &Coordinates,
  effects: &mut Effects,
) -> Result<(), Never> {
  match state.weather_job.is_some().then_some(()) {
    ::core::option::Option::Some(()) => {
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  state.weather_attempt = state.weather_attempt.saturating_add(1);
  let job = Task {
    owner: WEATHER_OWNER,
    id: JobId(state.weather_attempt),
  };
  state.weather_job = Some(job);
  let Ok(url) = conditions::request_url(at);
  let Ok(arguments) = conditions::request_arguments(&url);
  let Ok(command) = console_program_contract::Command::external(Program::Curl, &arguments);

  effects.offer(Effect::Fork(Fork { job, command }))
}

fn weather_arrived(
  state: &mut Sky<'_>,
  weather: Option<Weather>,
  effects: &mut Effects,
) -> Result<(), Never> {
  state.weather = weather.or(state.weather);

  match (state.going == Going::KeepGoing).then_some(()) {
    ::core::option::Option::Some(()) => {
      let interval = match weather {
        Some(_) => {
          state.weather_delay = ASK_SOONER;
          ASK_AGAIN
        }
        None => {
          let delay = state.weather_delay;
          state.weather_delay = delay.saturating_mul(2).min(ASK_AGAIN);
          delay
        }
      };
      let timer = Timer {
        name: WEATHER_TIMER,
        interval,
      };
      let Ok(()) = change_timer(
        &mut state.weather_timer,
        Some(timer),
        Subscription::Once,
        effects,
      );
    }
    ::core::option::Option::None => {}
  }

  effects.offer(Effect::Custom(WallpaperEffect::Observe))
}

fn stop(state: &mut Sky<'_>, effects: &mut Effects) -> Result<(), Never> {
  match (state.going == Going::KeepGoing).then_some(()) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = effects.offer(Effect::Unsubscribe(COMPOSITOR));

      let Ok(watch) = configuration_watch(state.rule_directory);

      match watch {
        Some(subscription) => {
          let Ok(()) = effects.offer(Effect::Unsubscribe(subscription));
        }
        None => {}
      }
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = change_timer(&mut state.look_timer, None, Subscription::Timer, effects);
  let Ok(()) = change_timer(&mut state.weather_timer, None, Subscription::Once, effects);

  state.weather_job = None;

  Ok(())
}

fn change_timer(
  current: &mut Option<Timer>,
  next: Option<Timer>,
  subscription: fn(Timer) -> Subscription,
  effects: &mut Effects,
) -> Result<(), Never> {
  match std::mem::replace(current, next) {
    Some(timer) => {
      let Ok(()) = effects.offer(Effect::Unsubscribe(subscription(timer)));
    }
    ::core::option::Option::None => {}
  }

  match next {
    Some(timer) => {
      let Ok(()) = effects.offer(Effect::Subscribe(subscription(timer)));
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

#[must_use]
fn look_timer(interval: Duration) -> Result<Timer, Never> {
  Ok(Timer {
    name: LOOK_TIMER,
    interval,
  })
}

fn looked(
  state: &mut Sky<'_>,
  elapsed: Elapsed,
  covered: Covered,
  chosen: Option<&Chosen>,
  effects: &mut Effects,
) -> Result<(), Never> {
  state.covered_since = match covered {
    Covered::Yes => state.covered_since.or(Some(elapsed)),
    Covered::No => None,
  };
  let settling = state
    .covered_since
    .map(|since| SETTLE.saturating_sub(elapsed.saturating_sub(since)));
  let away = match ::core::primitive::bool::then_some(settling == Some(Duration::ZERO), ()) {
    ::core::option::Option::Some(()) => Away::PutIt,
    ::core::option::Option::None => Away::NotYet,
  };

  match chosen {
    Some(chosen_2) => {
      let Ok(put) = putting(state, chosen_2, away);
      let Ok(()) = effects.offer_all(put);
    }
    ::core::option::Option::None => {}
  }

  match state.going {
    Going::Once => match chosen {
      Some(_) => Ok(()),
      None => effects.offer(Effect::Stop(Exit::Success)),
    },
    Going::KeepGoing => {
      let waiting = match settling.filter(|_| away == Away::NotYet) {
        Some(left) => LOOK_AGAIN.min(left.max(SOONEST)),
        None => LOOK_AGAIN,
      };

      let Ok(next) = look_timer(waiting);
      change_timer(
        &mut state.look_timer,
        Some(next),
        Subscription::Timer,
        effects,
      )
    }
  }
}

fn putting(
  state: &Sky<'_>,
  chosen: &Chosen,
  away: Away,
) -> Result<Vec<Effect<WallpaperEffect>>, Never> {
  let resting = match (away, &chosen.still) {
    (Away::PutIt, Some(_)) => Resting::Yes,
    (Away::PutIt, None) | (Away::NotYet, _) => Resting::No,
  };

  let put_up = match (resting, &chosen.still) {
    (Resting::Yes, Some(still)) => still.clone(),
    (Resting::Yes, None) | (Resting::No, _) => chosen.moving.clone(),
  };

  let first = away == Away::NotYet
    && state.showing.as_deref() != Some(put_up.as_path())
    && state.showing.as_deref() != chosen.still.as_deref();
  let mut effects = Vec::new();

  match (first, &chosen.still, state.going) {
    (true, Some(still), Going::KeepGoing) => {
      effects.push(Effect::Custom(WallpaperEffect::Paint(still.clone())))
    }
    (_, _, _) => {}
  }

  let effect = match (away, resting) {
    (Away::PutIt, Resting::No) => WallpaperEffect::Freeze(put_up),
    (_, _) => WallpaperEffect::Paint(put_up),
  };
  effects.push(Effect::Custom(effect));

  Ok(effects)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resting {
  Yes,
  No,
}

#[cfg(test)]
mod tests {
  use console_core_state_machine::{Trace, Transition, run_from};

  use super::*;

  #[test]
  fn selecting_or_removing_a_scene_wakes_the_keeper() {
    let folder = Path::new("/config/console");
    let rules = folder.join("wallpaper-rules");
    let scenes = folder.join(crate::scene::DIRECTORY);

    for changed in [
      folder.join(crate::place::SELECTION),
      scenes.join("seascape"),
      scenes,
    ] {
      assert!(matches!(
        configuration_changed(Some(&rules), folder, &changed),
        Ok(Worth::Waking)
      ));
    }

    assert!(matches!(
      configuration_changed(Some(&rules), folder, &folder.join("status-bar.json")),
      Ok(Worth::Ignoring)
    ));
  }

  fn scheduled_look(effects: &[Effect<WallpaperEffect>]) -> Result<Option<Duration>, Never> {
    Ok(effects.iter().find_map(|effect| match effect {
      Effect::Subscribe(Subscription::Timer(Timer {
        name: LOOK_TIMER,
        interval,
      })) => Some(*interval),
      Effect::Subscribe(_)
      | Effect::Unsubscribe(_)
      | Effect::Write(_)
      | Effect::Run(_)
      | Effect::Stream(_)
      | Effect::Fork(_)
      | Effect::Interrupt(_)
      | Effect::Prompt(_)
      | Effect::Spawn(_)
      | Effect::Notify(_)
      | Effect::Print(_)
      | Effect::Stop(_)
      | Effect::Custom(_) => None,
    }))
  }

  enum Step {
    Looked(f64, Covered),
    Rendered(&'static str),
    Event(WallpaperEvent),
  }

  fn sky() -> Result<Sky<'static>, Never> {
    let Ok(Transition { state, effects: _ }) = Sun::initial_transition(
      &Input {
        going: Going::KeepGoing,
        pictures: &[],
        rule_directory: None,
      },
      None,
    );

    Ok(state)
  }

  fn said(
    from: &Sky<'static>,
    steps: Vec<Step>,
  ) -> Result<Trace<Sky<'static>, Event<WallpaperEvent>, Effect<WallpaperEffect>>, Never> {
    let events: Vec<Event<WallpaperEvent>> = steps
      .into_iter()
      .map(|step| match step {
        Step::Looked(seconds, covered) => WallpaperEvent::Looked {
          elapsed: Duration::from_secs_f64(seconds),
          covered,
          chosen: Some(Chosen {
            moving: PathBuf::from("/pictures/rain.gif"),
            still: Some(PathBuf::from("/pictures/rain.png")),
          }),
        },
        Step::Rendered(at) => WallpaperEvent::Rendered {
          at: PathBuf::from(at),
          went: Rendered::Yes,
        },
        Step::Event(event) => event,
      })
      .map(Event::Custom)
      .collect();

    run_from::<Sun>(from.clone(), &events)
  }

  #[test]
  fn the_still_one_goes_up_before_the_moving_one_the_first_time() {
    let Ok(sky) = sky();

    let Ok(after) = said(&sky, vec![Step::Looked(0.0, Covered::No)]);

    let Ok(effects) = after.effects();

    assert_eq!(
      effects.first(),
      Some(&Effect::Custom(WallpaperEffect::Paint(PathBuf::from(
        "/pictures/rain.png"
      ))))
    );
    assert_eq!(
      effects.get(1),
      Some(&Effect::Custom(WallpaperEffect::Paint(PathBuf::from(
        "/pictures/rain.gif"
      ))))
    );
  }

  #[test]
  fn a_picture_already_up_is_reconciled_without_replacing_it_with_a_still() {
    let Ok(sky) = sky();

    let Ok(up) = said(
      &sky,
      vec![
        Step::Looked(0.0, Covered::No),
        Step::Rendered("/pictures/rain.png"),
        Step::Rendered("/pictures/rain.gif"),
      ],
    );
    let Ok(again) = said(&up.state, vec![Step::Looked(1.0, Covered::No)]);

    let Ok(timer) = look_timer(LOOK_AGAIN);
    assert_eq!(
      again.effects(),
      Ok(vec![
        Effect::Custom(WallpaperEffect::Paint(PathBuf::from("/pictures/rain.gif"))),
        Effect::Unsubscribe(Subscription::Timer(timer)),
        Effect::Subscribe(Subscription::Timer(timer))
      ])
    );
  }

  #[test]
  fn something_in_front_of_it_does_not_put_it_away_at_once() {
    let Ok(sky) = sky();

    let Ok(up) = said(
      &sky,
      vec![
        Step::Looked(0.0, Covered::No),
        Step::Rendered("/pictures/rain.png"),
        Step::Rendered("/pictures/rain.gif"),
      ],
    );
    let Ok(covered) = said(&up.state, vec![Step::Looked(1.0, Covered::Yes)]);

    let Ok(effects) = covered.effects();

    assert!(
      !effects
        .iter()
        .any(|effect| matches!(effect, Effect::Custom(WallpaperEffect::Freeze(_)))),
      "it was put away the moment something covered it"
    );
    assert_eq!(covered.state.covered_since, Some(Duration::from_secs(1)));
  }

  #[test]
  fn it_wakes_for_the_end_of_the_settling_rather_than_for_the_sun() {
    let Ok(sky) = sky();

    let Ok(up) = said(
      &sky,
      vec![
        Step::Looked(0.0, Covered::No),
        Step::Rendered("/pictures/rain.gif"),
      ],
    );
    let Ok(covered) = said(&up.state, vec![Step::Looked(1.0, Covered::Yes)]);

    let Ok(effects) = covered.effects();

    assert_eq!(scheduled_look(&effects), Ok(Some(SETTLE)));

    let Ok(later) = said(&covered.state, vec![Step::Looked(10.0, Covered::Yes)]);
    let Ok(after) = later.effects();

    assert_eq!(
      scheduled_look(&after),
      Ok(Some(Duration::from_secs_f64(6.0)))
    );
  }

  #[test]
  fn once_it_has_settled_the_still_one_goes_up_and_it_sleeps_for_the_sun() {
    let Ok(sky) = sky();

    let Ok(up) = said(
      &sky,
      vec![
        Step::Looked(0.0, Covered::No),
        Step::Rendered("/pictures/rain.png"),
        Step::Rendered("/pictures/rain.gif"),
      ],
    );
    let Ok(covered) = said(&up.state, vec![Step::Looked(1.0, Covered::Yes)]);
    let Ok(away) = said(&covered.state, vec![Step::Looked(17.0, Covered::Yes)]);

    let Ok(effects) = away.effects();

    assert!(
      effects.contains(&Effect::Custom(WallpaperEffect::Paint(PathBuf::from(
        "/pictures/rain.png"
      ))))
    );
    assert_eq!(scheduled_look(&effects), Ok(Some(LOOK_AGAIN)));
  }

  #[test]
  fn uncovering_it_starts_the_settling_over() {
    let Ok(sky) = sky();

    let Ok(covered) = said(
      &sky,
      vec![
        Step::Looked(0.0, Covered::Yes),
        Step::Looked(5.0, Covered::Yes),
      ],
    );

    assert_eq!(covered.state.covered_since, Some(Duration::from_secs(0)));

    let Ok(seen) = said(&covered.state, vec![Step::Looked(6.0, Covered::No)]);

    assert_eq!(seen.state.covered_since, None);

    let Ok(again) = said(&seen.state, vec![Step::Looked(7.0, Covered::Yes)]);

    assert_eq!(again.state.covered_since, Some(Duration::from_secs(7)));
  }

  #[test]
  fn a_wallpaper_that_would_not_take_is_tried_again_sooner() {
    let Ok(sky) = sky();

    let Ok(after) = said(
      &sky,
      vec![Step::Event(WallpaperEvent::Rendered {
        at: PathBuf::from("/pictures/rain.gif"),
        went: Rendered::No,
      })],
    );

    let Ok(timer) = look_timer(TRY_AGAIN);
    assert_eq!(
      after.effects(),
      Ok(vec![Effect::Subscribe(Subscription::Timer(timer))])
    );
    assert_eq!(after.state.showing, None);
  }

  #[test]
  fn now_puts_one_up_and_stops() {
    let Ok(Transition {
      state: once,
      effects: _,
    }) = Sun::initial_transition(
      &Input {
        going: Going::Once,
        pictures: &[],
        rule_directory: None,
      },
      None,
    );

    let Ok(after) = said(&once, vec![Step::Looked(0.0, Covered::No)]);
    let Ok(effects) = after.effects();

    assert_eq!(
      effects.as_slice(),
      &[Effect::Custom(WallpaperEffect::Paint(PathBuf::from(
        "/pictures/rain.gif"
      )))]
    );
    let Ok(painted) = said(&after.state, vec![Step::Rendered("/pictures/rain.gif")]);
    assert_eq!(painted.effects(), Ok(vec![Effect::Stop(Exit::Success)]));
    assert_eq!(scheduled_look(&effects), Ok(None));
  }

  #[test]
  fn nothing_chosen_is_nothing_done_and_it_waits_for_the_sun() {
    let Ok(sky) = sky();

    let Ok(after) = said(
      &sky,
      vec![Step::Event(WallpaperEvent::Looked {
        elapsed: Duration::ZERO,
        covered: Covered::No,
        chosen: None,
      })],
    );

    let Ok(timer) = look_timer(LOOK_AGAIN);
    assert_eq!(
      after.effects(),
      Ok(vec![Effect::Subscribe(Subscription::Timer(timer))])
    );
  }

  #[test]
  fn what_the_sky_is_doing_only_ever_arrives_from_somewhere_else() {
    let Ok(sky) = sky();

    let Ok(after) = said(
      &sky,
      vec![Step::Event(WallpaperEvent::Weather(Some(Weather::Rain)))],
    );
    let Ok(effects) = after.effects();

    assert_eq!(after.state.weather, Some(Weather::Rain));
    assert!(effects.contains(&Effect::Custom(WallpaperEffect::Observe)));
  }

  #[test]
  fn a_weather_nobody_could_ask_for_leaves_the_last_one_where_it_was() {
    let Ok(sky) = sky();

    let Ok(after) = said(
      &sky,
      vec![
        Step::Event(WallpaperEvent::Weather(Some(Weather::Rain))),
        Step::Event(WallpaperEvent::Weather(None)),
      ],
    );

    assert_eq!(after.state.weather, Some(Weather::Rain));

    let Ok(after_2) = said(
      &after.state,
      vec![Step::Event(WallpaperEvent::Weather(Some(Weather::Snow)))],
    );

    assert_eq!(after_2.state.weather, Some(Weather::Snow));
  }
}
