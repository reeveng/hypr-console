//! Two readings, and what they come to.
//!
//! The reading itself needs a machine and the device's own check presses it
//! there. What is asked here is the half that decides anything: a rate out of
//! two counters, with the three things a counter does that a subtraction is
//! wrong about -- a program restarting, the machine rebooting, and a night
//! spent asleep counting as time the desktop was awake for. The last of those
//! is two questions: the night is not awake time, and the battery it fell by
//! is not the desktop's rate either, so the window it fell across is no part of
//! the watts.

use std::error::Error;

use console_core_never::Never;
use console_resource_usage::{
  Hours, Moment, Seconds, Sleep, WattHours, dump_program, of, restarts_in, serialize,
  usage_between, watts,
};

struct Reading {
  at: u64,
  up: Seconds,
  slept: Seconds,
  energy: WattHours,
}

fn moment(reading: Reading, busy: &[(&str, f64)]) -> Result<Moment, Never> {
  Ok(Moment {
    at: reading.at,
    up: Some(reading.up),
    slept: reading.slept,
    energy: Some(reading.energy),
    draw: Some(9.0),
    gpu: Some(3.0),
    percent: Some(80),
    busy: busy
      .iter()
      .map(|(named, seconds)| ((*named).to_string(), *seconds))
      .collect(),
    held: Vec::new(),
    restarts: Vec::new(),
    crashes: Vec::new(),
  })
}

fn moments(readings: Vec<(Reading, &[(&str, f64)])>) -> Result<Vec<Moment>, Never> {
  Ok(
    readings
      .into_iter()
      .map(|(reading, busy)| {
        let Ok(moment) = moment(reading, busy);

        moment
      })
      .collect(),
  )
}

fn owned(counted: &[(&str, f64)]) -> Result<Vec<(String, f64)>, Never> {
  Ok(
    counted
      .iter()
      .map(|(named, times)| ((*named).to_string(), *times))
      .collect(),
  )
}

fn with_counts(
  moment: Moment,
  restarts: &[(&str, f64)],
  crashes: &[(&str, f64)],
) -> Result<Moment, Never> {
  let Ok(restarts_2) = owned(restarts);
  let Ok(crashes_2) = owned(crashes);

  Ok(Moment {
    restarts: restarts_2,
    crashes: crashes_2,
    ..moment
  })
}

fn with_memory(moment: Moment, held: &[(&str, f64)]) -> Result<Moment, Never> {
  let Ok(held_2) = owned(held);

  Ok(Moment {
    held: held_2,
    ..moment
  })
}

fn quiet((at, up, energy): (u64, f64, f64)) -> Result<Moment, Never> {
  moment(
    Reading {
      at,
      up: Seconds(up),
      slept: Seconds(0.0),
      energy: WattHours(energy),
    },
    &[],
  )
}

#[test]
fn a_unit_that_restarted_twice_and_a_program_that_crashed_for_the_first_time_are_both_counted()
-> Result<(), Box<dyn Error>> {
  let Ok(first) = quiet((1_000, 1_000.0, 40.0));
  let Ok(last) = quiet((4_600, 4_600.0, 39.0));
  let Ok(first_2) = with_counts(
    first,
    &[("console-panels.service", 1.0)],
    &[("alacritty", 3.0)],
  );
  let Ok(last_2) = with_counts(
    last,
    &[("console-panels.service", 3.0)],
    &[("alacritty", 3.0), ("console-bar", 1.0)],
  );
  let Ok(measured) = usage_between(&[first_2, last_2]);
  let used = measured.ok_or("two readings are a measurement")?;

  assert_eq!(
    used.restarted,
    vec![("console-panels.service".to_string(), 2.0)]
  );
  assert_eq!(used.crashed, vec![("console-bar".to_string(), 1.0)]);

  Ok(())
}

#[test]
fn a_dump_vacuumed_away_is_not_a_crash() -> Result<(), Box<dyn Error>> {
  let Ok(first) = quiet((1_000, 1_000.0, 40.0));
  let Ok(last) = quiet((4_600, 4_600.0, 39.0));
  let Ok(first_2) = with_counts(first, &[], &[("alacritty", 9.0)]);
  let Ok(last_2) = with_counts(last, &[], &[("alacritty", 2.0)]);
  let Ok(measured) = usage_between(&[first_2, last_2]);
  let used = measured.ok_or("two readings are a measurement")?;

  assert_eq!(used.crashed, Vec::new());

  Ok(())
}

#[test]
fn a_dump_is_named_for_the_program_even_with_a_dot_in_its_name() {
  let dump =
    "core.beam\\x2esmp.1000.0f4b9cfa000e476db9e4e1bd6226413b.739458.1789841676000000.zst";

  assert_eq!(dump_program(dump), Ok(Some("beam.smp".to_string())));
  assert_eq!(
    dump_program("core.kew.1000.0f4b.12.1789841676000000"),
    Ok(Some("kew".to_string()))
  );
  assert_eq!(
    dump_program("core.1000.0f4b.12.1789841676000000.zst"),
    Ok(None)
  );
  assert_eq!(dump_program("notes.txt"), Ok(None));
}

#[test]
fn only_the_units_that_restarted_are_counted() {
  let said = "Id=seascape.service\nNRestarts=0\n\nId=default.target\n\nId=console-panels.service\nNRestarts=4\n";

  assert_eq!(
    restarts_in(said),
    Ok(vec![("console-panels.service".to_string(), 4.0)])
  );
}

#[test]
fn what_grew_is_the_last_reading_less_the_first_for_the_names_at_both()
-> Result<(), Box<dyn Error>> {
  let Ok(first) = quiet((1_000, 1_000.0, 40.0));
  let Ok(middle) = quiet((4_600, 4_600.0, 39.0));
  let Ok(last) = quiet((8_200, 8_200.0, 38.0));
  let Ok(first_2) = with_memory(first, &[("console-bar", 40.0), ("kew", 30.0)]);
  let Ok(middle_2) = with_memory(middle, &[("console-bar", 55.0)]);
  let Ok(last_2) = with_memory(
    last,
    &[("console-bar", 90.0), ("kew", 20.0), ("foot", 12.0)],
  );
  let Ok(measured) = usage_between(&[first_2, middle_2, last_2]);
  let used = measured.ok_or("three readings are a measurement")?;

  assert_eq!(used.grew, vec![("console-bar".to_string(), 50.0)]);

  Ok(())
}

#[test]
fn a_reading_from_before_memory_was_kept_is_not_where_growth_starts() -> Result<(), Box<dyn Error>>
{
  let Ok(first) = quiet((1_000, 1_000.0, 40.0));
  let Ok(middle) = quiet((4_600, 4_600.0, 39.0));
  let Ok(last) = quiet((8_200, 8_200.0, 38.0));
  let Ok(middle_2) = with_memory(middle, &[("console-bar", 40.0)]);
  let Ok(last_2) = with_memory(last, &[("console-bar", 44.0)]);
  let Ok(measured) = usage_between(&[first, middle_2, last_2]);
  let used = measured.ok_or("three readings are a measurement")?;

  assert_eq!(used.grew, vec![("console-bar".to_string(), 4.0)]);

  Ok(())
}

#[test]
fn an_hour_of_nine_watts_is_nine_watts() -> Result<(), Box<dyn Error>> {
  let Ok(moments) = moments(vec![
    (
      Reading {
        at: 1_000,
        up: Seconds(1_000.0),
        slept: Seconds(0.0),
        energy: WattHours(40.0),
      },
      &[("wireplumber", 100.0)],
    ),
    (
      Reading {
        at: 4_600,
        up: Seconds(4_600.0),
        slept: Seconds(0.0),
        energy: WattHours(31.0),
      },
      &[("wireplumber", 640.0)],
    ),
  ]);
  let Ok(measured) = usage_between(&moments);
  let used = measured.ok_or("two readings are a measurement")?;

  assert_eq!(used.awake, Seconds(3_600.0));
  assert_eq!(used.watthours, WattHours(9.0));
  assert_eq!(used.flat, Hours(1.0));
  assert_eq!(used.busy, vec![("wireplumber".to_string(), 540.0)]);

  Ok(())
}

#[test]
fn a_night_asleep_is_not_time_the_desktop_was_awake() -> Result<(), Box<dyn Error>> {
  let Ok(moments) = moments(vec![
    (
      Reading {
        at: 1_000,
        up: Seconds(1_000.0),
        slept: Seconds(0.0),
        energy: WattHours(40.0),
      },
      &[],
    ),
    (
      Reading {
        at: 37_000,
        up: Seconds(37_000.0),
        slept: Seconds(32_400.0),
        energy: WattHours(39.0),
      },
      &[],
    ),
  ]);
  let Ok(measured) = usage_between(&moments);
  let used = measured.ok_or("two readings are a measurement")?;

  assert_eq!(used.asleep, Seconds(32_400.0));
  assert_eq!(used.awake, Seconds(3_600.0));
  assert_eq!(used.watthours, WattHours(0.0));
  assert_eq!(used.flat, Hours(0.0));

  Ok(())
}

#[test]
fn the_watts_are_the_readings_that_held_no_sleep() -> Result<(), Box<dyn Error>> {
  let Ok(moments) = moments(vec![
    (
      Reading {
        at: 1_000,
        up: Seconds(1_000.0),
        slept: Seconds(0.0),
        energy: WattHours(40.0),
      },
      &[],
    ),
    (
      Reading {
        at: 4_600,
        up: Seconds(4_600.0),
        slept: Seconds(0.0),
        energy: WattHours(34.0),
      },
      &[],
    ),
    (
      Reading {
        at: 40_600,
        up: Seconds(40_600.0),
        slept: Seconds(32_400.0),
        energy: WattHours(33.0),
      },
      &[],
    ),
    (
      Reading {
        at: 44_200,
        up: Seconds(44_200.0),
        slept: Seconds(32_400.0),
        energy: WattHours(27.0),
      },
      &[],
    ),
  ]);
  let Ok(measured) = usage_between(&moments);
  let used = measured.ok_or("four readings are three measurements")?;

  assert_eq!(used.awake, Seconds(10_800.0));
  assert_eq!(used.asleep, Seconds(32_400.0));
  assert_eq!(used.watthours, WattHours(12.0));
  assert_eq!(used.flat, Hours(2.0));
  assert_eq!(watts(&used), Ok(Some(6.0)));

  Ok(())
}

#[test]
fn a_program_that_restarted_did_not_spend_less_than_nothing() -> Result<(), Box<dyn Error>> {
  let Ok(moments) = moments(vec![
    (
      Reading {
        at: 1_000,
        up: Seconds(1_000.0),
        slept: Seconds(0.0),
        energy: WattHours(40.0),
      },
      &[("kew", 900.0), ("pipewire", 100.0)],
    ),
    (
      Reading {
        at: 4_600,
        up: Seconds(4_600.0),
        slept: Seconds(0.0),
        energy: WattHours(39.0),
      },
      &[("kew", 12.0), ("pipewire", 460.0)],
    ),
  ]);
  let Ok(measured) = usage_between(&moments);
  let used = measured.ok_or("two readings are a measurement")?;

  assert_eq!(used.busy, vec![("pipewire".to_string(), 360.0)]);

  Ok(())
}

#[test]
fn a_reboot_is_not_a_window() {
  let Ok(moments) = moments(vec![
    (
      Reading {
        at: 1_000,
        up: Seconds(90_000.0),
        slept: Seconds(0.0),
        energy: WattHours(40.0),
      },
      &[("kew", 900.0)],
    ),
    (
      Reading {
        at: 4_600,
        up: Seconds(30.0),
        slept: Seconds(0.0),
        energy: WattHours(39.0),
      },
      &[("kew", 2.0)],
    ),
  ]);

  assert_eq!(usage_between(&moments), Ok(None));
}

#[test]
fn a_line_read_back_is_the_reading_that_was_written() -> Result<(), Box<dyn Error>> {
  let Ok(one) = moment(
    Reading {
      at: 1_000,
      up: Seconds(1_000.0),
      slept: Seconds(12.5),
      energy: WattHours(40.25),
    },
    &[("wireplumber", 100.5)],
  );
  let Ok(one_2) = with_memory(one, &[("kew", 41.5)]);
  let Ok(one_3) = with_counts(one_2, &[("kew.service", 2.0)], &[("alacritty", 3.0)]);
  let Ok(said) = serialize(&one_3);
  assert_eq!(
    said,
    r#"{"at":1000,"up":1000.0,"slept":12.5,"energy":40.250,"draw":9.00,"gpu":3.00,"percent":80,"busy":{"wireplumber":100.5},"held":{"kew":41.5},"restarts":{"kew.service":2},"crashes":{"alacritty":3}}"#
  );
  let Ok(read) = of(&said);
  let back = read.ok_or("a line this crate wrote")?;

  assert_eq!(back, one_3);

  Ok(())
}

#[test]
fn the_worst_sleep_is_the_one_that_drew_the_most_for_each_hour_asleep()
-> Result<(), Box<dyn Error>> {
  let Ok(moments) = moments(vec![
    (
      Reading {
        at: 1_000,
        up: Seconds(1_000.0),
        slept: Seconds(0.0),
        energy: WattHours(40.0),
      },
      &[],
    ),
    (
      Reading {
        at: 30_100,
        up: Seconds(30_100.0),
        slept: Seconds(28_800.0),
        energy: WattHours(39.0),
      },
      &[],
    ),
    (
      Reading {
        at: 33_700,
        up: Seconds(33_700.0),
        slept: Seconds(28_800.0),
        energy: WattHours(36.0),
      },
      &[],
    ),
    (
      Reading {
        at: 40_900,
        up: Seconds(40_900.0),
        slept: Seconds(36_000.0),
        energy: WattHours(34.0),
      },
      &[],
    ),
  ]);
  let Ok(measured) = usage_between(&moments);
  let used = measured.ok_or("four readings are a measurement")?;
  let worst = used.worst_sleep.ok_or("a window that slept")?;

  assert_eq!(
    worst,
    Sleep {
      woke: 40_900,
      asleep: Seconds(7_200.0),
      watthours: WattHours(2.0)
    }
  );
  assert_eq!(worst.at_most(), Ok(1.0));

  Ok(())
}

#[test]
fn a_window_mostly_awake_or_on_the_cable_is_not_a_sleep() -> Result<(), Box<dyn Error>> {
  let Ok(moments) = moments(vec![
    (
      Reading {
        at: 1_000,
        up: Seconds(1_000.0),
        slept: Seconds(0.0),
        energy: WattHours(40.0),
      },
      &[],
    ),
    (
      Reading {
        at: 4_600,
        up: Seconds(4_600.0),
        slept: Seconds(600.0),
        energy: WattHours(38.0),
      },
      &[],
    ),
    (
      Reading {
        at: 40_600,
        up: Seconds(40_600.0),
        slept: Seconds(32_400.0),
        energy: WattHours(45.0),
      },
      &[],
    ),
  ]);
  let Ok(measured) = usage_between(&moments);
  let used = measured.ok_or("three readings are a measurement")?;

  assert_eq!(used.worst_sleep, None);

  Ok(())
}
