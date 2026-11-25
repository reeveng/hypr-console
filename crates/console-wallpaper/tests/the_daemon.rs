use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_location::{Coordinates, Latitude, Longitude};
use console_program_contract::{
  Answer, Change, Command, Effect, Event, EventGroup, Exit, ExitStatus, Fork, JobId, JobOwner,
  Subscription, Task, Timer,
};
use console_program_runtime::simulation::{After, Expected, Simulation};
use console_program_runtime::{Limit, Subscribed, Wake, execute};
use console_wallpaper::choose::{Selection, Set, Wanted};
use console_wallpaper::covered::Covered;
use console_wallpaper::keeping::{
  Chosen, Going, Input, Observation, Rendered, Sun, WallpaperEffect, WallpaperEvent,
};
use console_wallpaper::units::UnixSeconds;
use console_weather::conditions::{self, Weather};

type Failure = Box<dyn Error>;
type Script = Expected<WallpaperEvent, WallpaperEffect>;

const AT: Coordinates = Coordinates {
  latitude: Latitude(50.85),
  longitude: Longitude(4.35),
};
const SUNSET: UnixSeconds = UnixSeconds(1_781_812_200.0);
const DUSK: UnixSeconds = UnixSeconds(1_781_812_500.0);
const COMPOSITOR: Subscription = Subscription::EventGroup(EventGroup::Compositor);

#[must_use]
fn plain(effect: Effect<WallpaperEffect>) -> Result<Script, Never> {
  Ok(Script {
    effect,
    replies: Vec::new(),
  })
}

#[must_use]
fn custom(effect: WallpaperEffect, event: WallpaperEvent) -> Result<Script, Never> {
  Ok(Script {
    effect: Effect::Custom(effect),
    replies: vec![After {
      delay: Duration::ZERO,
      event: Event::Custom(event),
    }],
  })
}

#[must_use]
fn observation(seconds: UnixSeconds, elapsed: u64, wanted: Wanted) -> Result<Script, Never> {
  custom(
    WallpaperEffect::Observe,
    WallpaperEvent::Observed(Observation {
      seconds,
      elapsed: Duration::from_secs(elapsed),
      at: AT,
      wanted,
      pinned_file: false,
      rules: Default::default(),
      covered: Covered::No,
    }),
  )
}

#[must_use]
fn locate() -> Result<Script, Never> {
  custom(
    WallpaperEffect::WeatherLocation,
    WallpaperEvent::WeatherLocation(AT),
  )
}

#[must_use]
fn attempt(id: u64, delay: Duration, status: ExitStatus) -> Result<(Script, Answer), Never> {
  let Ok(url) = conditions::request_url(&AT);
  let Ok(arguments) = conditions::request_arguments(&url);
  let Ok(command) = Command::external(Program::Curl, &arguments);
  let job = Task {
    owner: JobOwner("wallpaper weather"),
    id: JobId(id),
  };
  let answer = Answer {
    job: Some(job),
    command: command.clone(),
    output: String::new(),
    error_output: String::new(),
    status,
  };
  let script = Script {
    effect: Effect::Fork(Fork { job, command }),
    replies: vec![After {
      delay,
      event: Event::Replied(answer.clone()),
    }],
  };

  Ok((script, answer))
}

#[must_use]
fn painting(name: &str, elapsed: u64) -> Result<[Result<Script, Never>; 2], Never> {
  let path = PathBuf::from(format!("/pictures/{name}.gif"));
  Ok([
    custom(
      WallpaperEffect::Resolve {
        name: name.to_string(),
        elapsed: Duration::from_secs(elapsed),
        covered: Covered::No,
      },
      WallpaperEvent::Looked {
        elapsed: Duration::from_secs(elapsed),
        covered: Covered::No,
        chosen: Some(Chosen {
          moving: path.clone(),
          still: None,
        }),
      },
    ),
    custom(
      WallpaperEffect::Paint(path.clone()),
      WallpaperEvent::Rendered {
        at: path,
        went: Rendered::Yes,
      },
    ),
  ])
}

#[test]
fn a_runtime_timer_selects_dusk_and_shutdown_discards_the_pending_forecast() -> Result<(), Failure>
{
  let table: Set = toml::from_str(
    r#"
    [[picture]]
    name = "sunset"
    says = "Sunset"
    sky = ["sunset"]

    [[picture]]
    name = "dusk"
    says = "Dusk"
    sky = ["dusk"]
  "#,
  )?;
  let Ok((weather, _)) = attempt(1, Duration::from_secs(1_000), ExitStatus::Success);
  let mut expected = vec![
    plain(Effect::Subscribe(COMPOSITOR)),
    locate(),
    observation(SUNSET, 0, Wanted::default()),
    Ok(weather),
  ];
  let Ok(sunset) = painting("sunset", 0);
  expected.extend(sunset);
  expected.push(observation(DUSK, 300, Wanted::default()));
  let Ok(dusk) = painting("dusk", 300);
  expected.extend(dusk);
  expected.push(plain(Effect::Unsubscribe(COMPOSITOR)));
  let Ok(script) = expected.into_iter().collect::<Result<Vec<_>, Never>>();
  let Ok(mut simulation) = Simulation::new(script, Subscribed::No);
  simulation.schedule(Duration::from_secs(301), Wake::Stop(Exit::Success))?;

  let input = Input {
    going: Going::KeepGoing,
    pictures: &table.pictures,
    rule_directory: None,
  };
  let finished = execute::<Sun, _>(&input, &mut simulation, Limit::Turns(40))?;

  assert_eq!(
    finished.state.showing,
    Some(PathBuf::from("/pictures/dusk.gif"))
  );
  assert_eq!(finished.exit, Exit::Success);
  assert!(simulation.events.contains(&Event::Stopping));
  assert!(
    !simulation
      .events
      .iter()
      .any(|event| matches!(event, Event::Replied(_)))
  );
  simulation.finish()?;

  Ok(())
}

#[test]
fn a_rule_saved_while_the_daemon_is_running_changes_the_wallpaper() -> Result<(), Failure> {
  let table: Set = toml::from_str("[[picture]]\nname = 'bundled'\nsays = 'Bundled'")?;
  let folder = PathBuf::from("/home/ada/.config/console");
  let directory = folder.join("wallpaper-rules");
  let group = EventGroup::Path(folder);
  let watch = Subscription::EventGroup(group.clone());
  let Ok((weather, _)) = attempt(1, Duration::from_secs(30), ExitStatus::Success);
  let mut expected = vec![
    plain(Effect::Subscribe(COMPOSITOR)),
    plain(Effect::Subscribe(watch.clone())),
    locate(),
    observation(SUNSET, 0, Wanted::default()),
    Ok(weather),
  ];
  let Ok(bundled) = painting("bundled", 0);
  expected.extend(bundled);
  let custom_rule = toml::from_str("automatic = true\nsky = ['sunset']")?;
  expected.push(custom(
    WallpaperEffect::Observe,
    WallpaperEvent::Observed(Observation {
      seconds: SUNSET,
      elapsed: Duration::from_secs(1),
      at: AT,
      wanted: Wanted::default(),
      pinned_file: false,
      rules: [("my-photo".to_string(), custom_rule)]
        .into_iter()
        .collect(),
      covered: Covered::No,
    }),
  ));
  let Ok(configured) = painting("my-photo", 1);
  expected.extend(configured);
  expected.extend([
    plain(Effect::Unsubscribe(COMPOSITOR)),
    plain(Effect::Unsubscribe(watch)),
  ]);
  let Ok(script) = expected.into_iter().collect::<Result<Vec<_>, Never>>();
  let Ok(mut simulation) = Simulation::new(script, Subscribed::No);
  simulation.schedule(
    Duration::from_secs(1),
    Wake::Event(Event::Modified(Change {
      event_group: group,
      text: directory.join("my-photo.toml").display().to_string(),
    })),
  )?;
  simulation.schedule(Duration::from_secs(2), Wake::Stop(Exit::Success))?;

  let input = Input {
    going: Going::KeepGoing,
    pictures: &table.pictures,
    rule_directory: Some(&directory),
  };
  let finished = execute::<Sun, _>(&input, &mut simulation, Limit::Turns(40))?;

  assert_eq!(
    finished.state.showing,
    Some(PathBuf::from("/pictures/my-photo.gif"))
  );
  simulation.finish()?;

  Ok(())
}

#[test]
fn offline_retries_back_off_and_a_success_restores_the_normal_interval() -> Result<(), Failure> {
  let mut expected = vec![
    plain(Effect::Subscribe(COMPOSITOR)),
    locate(),
    observation(SUNSET, 0, Wanted::default()),
  ];

  for (id, at, weather) in [(1, 1, None), (2, 62, None), (3, 183, Some(Weather::Rain))] {
    match (id > 1).then_some(()) {
      ::core::option::Option::Some(()) => {
        expected.push(locate());
      }
      ::core::option::Option::None => {}
    }

    let status = match weather {
      Some(_) => ExitStatus::Success,
      None => ExitStatus::Failure(Some(28)),
    };
    let Ok((asking, answer)) = attempt(id, Duration::from_secs(1), status);
    expected.extend([
      Ok(asking),
      custom(
        WallpaperEffect::ReadWeather(answer),
        WallpaperEvent::Weather(weather),
      ),
      observation(SUNSET, at, Wanted::default()),
    ]);
  }

  expected.push(plain(Effect::Unsubscribe(COMPOSITOR)));
  let Ok(script) = expected.into_iter().collect::<Result<Vec<_>, Never>>();
  let Ok(mut simulation) = Simulation::new(script, Subscribed::No);
  simulation.schedule(Duration::from_secs(184), Wake::Stop(Exit::Success))?;

  let input = Input {
    going: Going::KeepGoing,
    pictures: &[],
    rule_directory: None,
  };
  let finished = execute::<Sun, _>(&input, &mut simulation, Limit::Turns(50))?;
  let attempts: Vec<_> = simulation
    .events
    .iter()
    .filter_map(|event| match event {
      Event::Tick(
        Timer {
          name: "wallpaper weather",
          ..
        },
        elapsed,
      ) => Some(elapsed.as_secs()),
      Event::Tick(..)
      | Event::Opened
      | Event::Modified(_)
      | Event::Replied(_)
      | Event::Written(_)
      | Event::Chosen(_)
      | Event::Stopping
      | Event::Custom(_) => None,
    })
    .collect();
  let delays: Vec<_> = simulation
    .effects
    .iter()
    .flatten()
    .filter_map(|effect| match effect {
      Effect::Subscribe(Subscription::Once(Timer {
        name: "wallpaper weather",
        interval,
      })) => Some(interval.as_secs()),
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
    })
    .collect();

  assert_eq!(attempts, [61, 182]);
  assert_eq!(delays, [60, 120, 1200]);
  assert_eq!(finished.state.weather, Some(Weather::Rain));

  Ok(())
}

#[test]
fn a_pinned_picture_can_be_put_up_once_without_a_forecast() -> Result<(), Failure> {
  let table: Set = toml::from_str(
    r#"
    [[picture]]
    name = "pinned"
    says = "Pinned"
  "#,
  )?;
  let wanted = Wanted {
    selection: Selection::Still,
    picture: "pinned".to_string(),
  };
  let mut expected = vec![observation(SUNSET, 0, wanted)];
  let Ok(pinned) = painting("pinned", 0);
  expected.extend(pinned);
  let Ok(script) = expected.into_iter().collect::<Result<Vec<_>, Never>>();
  let Ok(mut simulation) = Simulation::new(script, Subscribed::No);
  let input = Input {
    going: Going::Once,
    pictures: &table.pictures,
    rule_directory: None,
  };

  let finished = execute::<Sun, _>(&input, &mut simulation, Limit::Turns(10))?;

  assert_eq!(finished.exit, Exit::Success);
  assert!(
    !simulation
      .effects
      .iter()
      .flatten()
      .any(|effect| matches!(effect, Effect::Subscribe(_)))
  );

  Ok(())
}

#[test]
fn an_imported_picture_can_be_pinned_without_a_catalog_entry() -> Result<(), Failure> {
  let mut expected = vec![custom(
    WallpaperEffect::Observe,
    WallpaperEvent::Observed(Observation {
      seconds: SUNSET,
      elapsed: Duration::ZERO,
      at: AT,
      wanted: Wanted {
        selection: Selection::Still,
        picture: "my-photo".to_string(),
      },
      pinned_file: true,
      rules: Default::default(),
      covered: Covered::No,
    }),
  )];
  let Ok(pinned) = painting("my-photo", 0);
  expected.extend(pinned);
  let Ok(script) = expected.into_iter().collect::<Result<Vec<_>, Never>>();
  let Ok(mut simulation) = Simulation::new(script, Subscribed::No);
  let input = Input {
    going: Going::Once,
    pictures: &[],
    rule_directory: None,
  };

  let finished = execute::<Sun, _>(&input, &mut simulation, Limit::Turns(10))?;

  assert_eq!(finished.exit, Exit::Success);
  simulation.finish()?;

  Ok(())
}
