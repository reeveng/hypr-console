//! Night Light preferences and the display temperature curve.

use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_core_number_conversion::whole_u32;
use console_core_words::Words;
use std::path::{Path, PathBuf};

pub mod schedule;

pub const DAYLIGHT: u32 = 6500;

pub const WARM: u32 = 3000;

pub const STEP: u32 = 3;

pub const DUSK: (u32, u32) = (19 * 60 + 30, 21 * 60 + 30);

pub const DAY: u32 = 7 * 60;

pub const DAWN: u32 = 30;

pub const NAMED: &str = "warm";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Temperature {
  Kelvin(u32),
  Neutral,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Step {
  pub at: u32,
  pub temperature: Temperature,
}

pub fn curve() -> Result<Vec<Step>, Never> {
  let mut steps = Vec::new();

  let falling = DUSK.1.saturating_sub(DUSK.0).saturating_div(STEP);

  for part in 0..=falling {
    let Ok(warmth) = interpolate(
      Kelvin {
        from: DAYLIGHT,
        to: WARM,
      },
      Along {
        part,
        whole: falling,
      },
    );

    steps.push(Step {
      at: DUSK.0.saturating_add(part.saturating_mul(STEP)),
      temperature: Temperature::Kelvin(warmth),
    });
  }

  let climbing = DAWN.saturating_div(STEP);

  for part in 1..climbing {
    let Ok(warmth) = interpolate(
      Kelvin {
        from: WARM,
        to: DAYLIGHT,
      },
      Along {
        part,
        whole: climbing,
      },
    );

    steps.push(Step {
      at: DAY
        .saturating_sub(DAWN)
        .saturating_add(part.saturating_mul(STEP)),
      temperature: Temperature::Kelvin(warmth),
    });
  }

  steps.push(Step {
    at: DAY,
    temperature: Temperature::Neutral,
  });

  Ok(steps)
}

pub fn configuration() -> Result<String, Never> {
  let mut said = String::from(HEAD);
  let Ok(curve) = curve();

  for step in curve {
    let Ok(clock) = clock(step.at);
    let setting = match step.temperature {
      Temperature::Kelvin(kelvin) => format!("temperature = {kelvin}"),
      Temperature::Neutral => "identity = true".to_string(),
    };
    said.push_str(&format!(
      "\nprofile {{\n    time = {clock}\n    {setting}\n}}\n"
    ));
  }

  Ok(said)
}

const HEAD: &str = "\
# The fallback Night Light schedule in UTC when Console has no location.
# `console-warm` applies the same temperatures around the local sunset and
# sunrise when a location is available.
";

fn clock(minutes: u32) -> Result<String, Never> {
  Ok(format!(
    "{:02}:{:02}",
    minutes.saturating_div(60),
    minutes.wrapping_rem(60)
  ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Along {
  part: u32,
  whole: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Kelvin {
  from: u32,
  to: u32,
}

fn interpolate(kelvin: Kelvin, along: Along) -> Result<u32, Never> {
  let fraction = f64::from(along.part) / f64::from(along.whole);
  let Ok(from) = mired(kelvin.from);
  let Ok(to) = mired(kelvin.to);
  let mireds = from + (to - from) * fraction;
  let Ok(tens) = whole_u32((1_000_000.0 / mireds) / 10.0);

  Ok(tens.saturating_mul(10))
}

fn mired(kelvin: u32) -> Result<f64, Never> {
  Ok(1_000_000.0 / f64::from(kelvin))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Words)]
pub enum NightShift {
  #[words(written = "clock\n")]
  Scheduled,
  #[words(written = "always\n")]
  On,
  #[words(written = "ordinary\n")]
  Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switched {
  On,
  Off,
}

impl NightShift {
  pub fn read(held: &str) -> Result<Self, Never> {
    Ok(match held.trim() {
      "ordinary" => NightShift::Off,
      "always" => NightShift::On,
      _ => NightShift::Scheduled,
    })
  }

  pub fn other(self) -> Result<Self, Never> {
    Ok(match self {
      NightShift::Scheduled | NightShift::On => NightShift::Off,
      NightShift::Off => NightShift::Scheduled,
    })
  }

  pub fn switched(self) -> Result<Switched, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self != NightShift::Off, ()) {
        ::core::option::Option::Some(()) => Switched::On,
        ::core::option::Option::None => Switched::Off,
      },
    )
  }
}

pub fn at(home: &Path) -> Result<PathBuf, Never> {
  let Ok(ours) = console_core_places::Base::Configuration.application_under(home);

  Ok(ours.join(NAMED))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
  Loaded(NightShift),
  Invalid(String),
}

pub fn load(home: &Path) -> Result<Standing, Never> {
  let Ok(at) = at(home);
  let Ok(held) = console_core_atomic_writes::read(&at);

  Ok(match held {
    Stored::Text(said) => {
      let Ok(warmth) = NightShift::read(&said);

      Standing::Loaded(warmth)
    }
    Stored::Absent => Standing::Loaded(NightShift::Scheduled),
    Stored::Failed(fault) => Standing::Invalid(fault),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  type Failure = Box<dyn std::error::Error>;

  const A_CURVE: &str = "a curve";

  fn pairs(steps: &[Step]) -> Result<Vec<(&Step, &Step)>, Never> {
    Ok(steps.iter().zip(steps.iter().skip(1)).collect())
  }

  fn warmths(steps: &[Step], (from, to): (u32, u32)) -> Result<Vec<u32>, Never> {
    Ok(
      steps
        .iter()
        .filter(|step| step.at >= from && step.at <= to)
        .filter_map(|step| match step.temperature {
          Temperature::Kelvin(kelvin) => Some(kelvin),
          Temperature::Neutral => None,
        })
        .collect(),
    )
  }

  #[test]
  fn a_device_that_was_never_asked_follows_the_clock() {
    let Ok(unasked) = NightShift::read("");

    assert_eq!(unasked, NightShift::Scheduled);
    assert_eq!(NightShift::read("what?\n"), Ok(NightShift::Scheduled));
    assert_eq!(unasked.switched(), Ok(Switched::On));
  }

  #[test]
  fn only_the_refusal_is_remembered() {
    assert_eq!(NightShift::read("ordinary\n"), Ok(NightShift::Off));
    assert_eq!(NightShift::Off.switched(), Ok(Switched::Off));
  }

  #[test]
  fn what_was_written_is_what_is_read_back() {
    for way in [NightShift::Scheduled, NightShift::On, NightShift::Off] {
      let Ok(written) = way.written();

      assert_eq!(NightShift::read(written), Ok(way));
    }
  }

  #[test]
  fn the_switch_has_two_sides_and_they_are_each_other() {
    assert_eq!(NightShift::Scheduled.other(), Ok(NightShift::Off));
    assert_eq!(NightShift::Off.other(), Ok(NightShift::Scheduled));
  }

  #[test]
  fn the_curve_leaves_daylight_at_dusk_and_comes_back_at_seven() -> Result<(), &'static str> {
    let Ok(steps) = curve();
    let first = steps.first().ok_or(A_CURVE)?;
    let bottom = steps
      .iter()
      .find(|step| step.at == DUSK.1)
      .ok_or("the end of dusk")?;
    let last = steps.last().ok_or(A_CURVE)?;
    let morning = DAY.saturating_sub(DAWN);

    assert_eq!(first.at, DUSK.0);
    assert_eq!(first.temperature, Temperature::Kelvin(DAYLIGHT));
    assert_eq!(bottom.temperature, Temperature::Kelvin(WARM));
    assert!(
      !steps
        .iter()
        .any(|step| step.at > DUSK.1 && step.at < morning),
      "the night is what happens when nothing is said, so nothing is said for it"
    );
    assert_eq!(last.at, DAY);
    assert_eq!(last.temperature, Temperature::Neutral);

    Ok(())
  }

  #[test]
  fn dusk_falls_and_dawn_climbs() {
    let Ok(steps) = curve();
    let Ok(pairs) = pairs(&steps);
    let midnights: Vec<&(&Step, &Step)> = pairs
      .iter()
      .filter(|(before, after)| before.at >= after.at)
      .collect();

    assert_eq!(
      midnights.len(),
      1,
      "the curve crosses midnight at {midnights:?}"
    );

    let Ok(dusk) = warmths(&steps, DUSK);

    assert!(
      dusk
        .iter()
        .zip(dusk.iter().skip(1))
        .all(|(before, after)| before > after),
      "dusk does not fall: {dusk:?}"
    );

    let Ok(dawn) = warmths(&steps, (DAY.saturating_sub(DAWN), DAY));

    assert!(
      dawn
        .iter()
        .zip(dawn.iter().skip(1))
        .all(|(before, after)| before < after),
      "dawn does not climb: {dawn:?}"
    );
    assert!(
      dawn.first().is_some_and(|first| *first > WARM),
      "the climb starts above the night it is leaving"
    );
    assert!(
      dawn.last().is_some_and(|last| *last < DAYLIGHT),
      "the climb stops one short, because daylight is written as identity"
    );
  }

  #[test]
  fn no_step_is_big_enough_to_notification() {
    let Ok(steps) = curve();
    let Ok(pairs) = pairs(&steps);
    let biggest = pairs
      .iter()
      .filter_map(
        |(before, after)| match (before.temperature, after.temperature) {
          (Temperature::Kelvin(before_2), Temperature::Kelvin(after_2)) => {
            let Ok(after_3) = mired(after_2);
            let Ok(before_3) = mired(before_2);

            Some((after_3 - before_3).abs())
          }
          (Temperature::Neutral, Temperature::Kelvin(_) | Temperature::Neutral)
          | (Temperature::Kelvin(_), Temperature::Neutral) => None,
        },
      )
      .fold(0.0_f64, f64::max);

    assert!(
      biggest < 20.0,
      "one step moves {biggest} mireds, which is a jump"
    );
  }

  #[test]
  fn the_fallback_curve_can_be_printed_as_profiles() {
    let Ok(said) = configuration();
    let Ok(steps) = curve();

    assert!(said.starts_with('#'), "the file says what wrote it");
    assert!(said.contains("profile {\n    time = 19:30\n    temperature = 6500\n}\n"));
    assert!(said.contains("profile {\n    time = 21:30\n    temperature = 3000\n}\n"));
    assert!(said.contains("profile {\n    time = 07:00\n    identity = true\n}\n"));
    assert_eq!(said.matches("profile {").count(), steps.len());
  }

  #[test]
  fn warm_is_a_lamp_rather_than_daylight_or_a_fire() {
    const { assert!(WARM < 4500, "warm is not far enough from daylight to see") };

    const { assert!(WARM > 2000, "that is orange rather than warm") };
  }

  #[test]
  fn the_answer_is_kept_under_the_home_it_belongs_to() {
    let Ok(at) = at(Path::new("/home/someone"));

    assert_eq!(at, PathBuf::from("/home/someone/.config/console/warm"));
  }

  #[test]
  fn a_home_with_nothing_written_in_it_follows_the_clock() -> Result<(), Failure> {
    let home = console_core_temporary_directories::fresh("warm-never-asked")?;

    let Ok(standing) = load(&home);

    assert_eq!(standing, Standing::Loaded(NightShift::Scheduled));

    Ok(())
  }

  #[test]
  fn an_answer_that_will_not_be_read_is_not_an_answer() -> Result<(), Failure> {
    let home = console_core_temporary_directories::fresh("warm-unreadable")?;
    let Ok(at) = at(&home);

    std::fs::create_dir_all(&at)?;

    let Ok(standing) = load(&home);

    assert!(
      matches!(standing, Standing::Invalid(_)),
      "a setting that would not be read came back as {standing:?}"
    );

    std::fs::remove_dir_all(&home)?;

    Ok(())
  }

  #[test]
  fn a_refusal_that_was_written_down_is_read_back() -> Result<(), Failure> {
    let home = console_core_temporary_directories::fresh("warm-ordinary")?;
    let Ok(at) = at(&home);

    console_core_atomic_writes::whole_with_folders(&at, b"ordinary\n")?;

    let Ok(standing) = load(&home);

    assert_eq!(standing, Standing::Loaded(NightShift::Off));

    std::fs::remove_dir_all(&home)?;

    Ok(())
  }
}
