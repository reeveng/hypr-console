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

use console_resource_usage::{Moment, Sleep, between, dumped, of, restarts_in, watts, written};

fn moment(at: u64, up: f64, slept: f64, energy: f64, busy: &[(&str, f64)]) -> Moment {
    Moment {
        at,
        up: Some(up),
        slept,
        energy: Some(energy),
        draw: Some(9.0),
        gpu: Some(3.0),
        percent: Some(80),
        busy: busy.iter().map(|(named, seconds)| ((*named).to_string(), *seconds)).collect(),
        held: Vec::new(),
        restarts: Vec::new(),
        crashes: Vec::new(),
    }
}

fn counting(moment: Moment, restarts: &[(&str, f64)], crashes: &[(&str, f64)]) -> Moment {
    let owned = |counted: &[(&str, f64)]| counted.iter().map(|(named, times)| ((*named).to_string(), *times)).collect();

    Moment { restarts: owned(restarts), crashes: owned(crashes), ..moment }
}

#[test]
fn a_unit_that_restarted_twice_and_a_program_that_crashed_for_the_first_time_are_both_counted() {
    let moments = [
        counting(moment(1_000, 1_000.0, 0.0, 40.0, &[]), &[("console-panels.service", 1.0)], &[("alacritty", 3.0)]),
        counting(moment(4_600, 4_600.0, 0.0, 39.0, &[]), &[("console-panels.service", 3.0)], &[("alacritty", 3.0), ("console-bar", 1.0)]),
    ];
    let used = between(&moments).unwrap().expect("two readings are a measurement");

    assert_eq!(used.restarted, vec![("console-panels.service".to_string(), 2.0)]);
    assert_eq!(used.crashed, vec![("console-bar".to_string(), 1.0)]);
}

#[test]
fn a_dump_vacuumed_away_is_not_a_crash() {
    let moments = [
        counting(moment(1_000, 1_000.0, 0.0, 40.0, &[]), &[], &[("alacritty", 9.0)]),
        counting(moment(4_600, 4_600.0, 0.0, 39.0, &[]), &[], &[("alacritty", 2.0)]),
    ];
    let used = between(&moments).unwrap().expect("two readings are a measurement");

    assert_eq!(used.crashed, Vec::new());
}

#[test]
fn a_dump_is_named_for_the_program_even_with_a_dot_in_its_name() {
    let dump = "core.beam\\x2esmp.1000.0f4b9cfa000e476db9e4e1bd6226413b.739458.1789841676000000.zst";

    assert_eq!(dumped(dump).unwrap(), Some("beam.smp".to_string()));
    assert_eq!(dumped("core.kew.1000.0f4b.12.1789841676000000").unwrap(), Some("kew".to_string()));
    assert_eq!(dumped("core.1000.0f4b.12.1789841676000000.zst").unwrap(), None);
    assert_eq!(dumped("notes.txt").unwrap(), None);
}

#[test]
fn only_the_units_that_restarted_are_counted() {
    let said = "Id=seascape.service\nNRestarts=0\n\nId=default.target\n\nId=console-panels.service\nNRestarts=4\n";

    assert_eq!(restarts_in(said).unwrap(), vec![("console-panels.service".to_string(), 4.0)]);
}

fn holding(moment: Moment, held: &[(&str, f64)]) -> Moment {
    Moment { held: held.iter().map(|(named, megabytes)| ((*named).to_string(), *megabytes)).collect(), ..moment }
}

#[test]
fn what_grew_is_the_last_reading_less_the_first_for_the_names_at_both() {
    let moments = [
        holding(moment(1_000, 1_000.0, 0.0, 40.0, &[]), &[("console-bar", 40.0), ("kew", 30.0)]),
        holding(moment(4_600, 4_600.0, 0.0, 39.0, &[]), &[("console-bar", 55.0)]),
        holding(moment(8_200, 8_200.0, 0.0, 38.0, &[]), &[("console-bar", 90.0), ("kew", 20.0), ("foot", 12.0)]),
    ];
    let used = between(&moments).unwrap().expect("three readings are a measurement");

    assert_eq!(used.grew, vec![("console-bar".to_string(), 50.0)]);
}

#[test]
fn a_reading_from_before_memory_was_kept_is_not_where_growth_starts() {
    let moments = [
        moment(1_000, 1_000.0, 0.0, 40.0, &[]),
        holding(moment(4_600, 4_600.0, 0.0, 39.0, &[]), &[("console-bar", 40.0)]),
        holding(moment(8_200, 8_200.0, 0.0, 38.0, &[]), &[("console-bar", 44.0)]),
    ];
    let used = between(&moments).unwrap().expect("three readings are a measurement");

    assert_eq!(used.grew, vec![("console-bar".to_string(), 4.0)]);
}

#[test]
fn an_hour_of_nine_watts_is_nine_watts() {
    let moments = [
        moment(1_000, 1_000.0, 0.0, 40.0, &[("wireplumber", 100.0)]),
        moment(4_600, 4_600.0, 0.0, 31.0, &[("wireplumber", 640.0)]),
    ];
    let used = between(&moments).unwrap().expect("two readings are a measurement");

    assert_eq!(used.awake, 3_600.0);
    assert_eq!(used.watthours, 9.0);
    assert_eq!(used.flat, 1.0);
    assert_eq!(used.busy, vec![("wireplumber".to_string(), 540.0)]);
}

#[test]
fn a_night_asleep_is_not_time_the_desktop_was_awake() {
    let moments = [
        moment(1_000, 1_000.0, 0.0, 40.0, &[]),
        moment(37_000, 37_000.0, 32_400.0, 39.0, &[]),
    ];
    let used = between(&moments).unwrap().expect("two readings are a measurement");

    assert_eq!(used.asleep, 32_400.0);
    assert_eq!(used.awake, 3_600.0);
    assert_eq!(used.watthours, 0.0);
    assert_eq!(used.flat, 0.0);
}

#[test]
fn the_watts_are_the_readings_that_held_no_sleep() {
    let moments = [
        moment(1_000, 1_000.0, 0.0, 40.0, &[]),
        moment(4_600, 4_600.0, 0.0, 34.0, &[]),
        moment(40_600, 40_600.0, 32_400.0, 33.0, &[]),
        moment(44_200, 44_200.0, 32_400.0, 27.0, &[]),
    ];
    let used = between(&moments).unwrap().expect("four readings are three measurements");

    assert_eq!(used.awake, 10_800.0);
    assert_eq!(used.asleep, 32_400.0);
    assert_eq!(used.watthours, 12.0);
    assert_eq!(used.flat, 2.0);
    assert_eq!(watts(&used).unwrap(), Some(6.0));
}

#[test]
fn a_program_that_restarted_did_not_spend_less_than_nothing() {
    let moments = [
        moment(1_000, 1_000.0, 0.0, 40.0, &[("kew", 900.0), ("pipewire", 100.0)]),
        moment(4_600, 4_600.0, 0.0, 39.0, &[("kew", 12.0), ("pipewire", 460.0)]),
    ];
    let used = between(&moments).unwrap().expect("two readings are a measurement");

    assert_eq!(used.busy, vec![("pipewire".to_string(), 360.0)]);
}

#[test]
fn a_reboot_is_not_a_window() {
    let moments = [
        moment(1_000, 90_000.0, 0.0, 40.0, &[("kew", 900.0)]),
        moment(4_600, 30.0, 0.0, 39.0, &[("kew", 2.0)]),
    ];

    assert_eq!(between(&moments).unwrap(), None);
}

#[test]
fn a_line_read_back_is_the_reading_that_was_written() {
    let one = counting(
        holding(moment(1_000, 1_000.0, 12.5, 40.25, &[("wireplumber", 100.5)]), &[("kew", 41.5)]),
        &[("kew.service", 2.0)],
        &[("alacritty", 3.0)],
    );
    let said = written(&one).unwrap();
    let back = of(&said).unwrap().expect("a line this crate wrote");

    assert_eq!(back, one);
}

#[test]
fn the_worst_sleep_is_the_one_that_drew_the_most_for_each_hour_asleep() {
    let moments = [
        moment(1_000, 1_000.0, 0.0, 40.0, &[]),
        moment(30_100, 30_100.0, 28_800.0, 39.0, &[]),
        moment(33_700, 33_700.0, 28_800.0, 36.0, &[]),
        moment(40_900, 40_900.0, 36_000.0, 34.0, &[]),
    ];
    let used = between(&moments).unwrap().expect("four readings are a measurement");

    assert_eq!(used.worst_sleep, Some(Sleep { woke: 40_900, asleep: 7_200.0, watthours: 2.0 }));
    assert_eq!(used.worst_sleep.map(|slept| slept.at_most().unwrap()), Some(1.0));
}

#[test]
fn a_window_mostly_awake_or_on_the_cable_is_not_a_sleep() {
    let moments = [
        moment(1_000, 1_000.0, 0.0, 40.0, &[]),
        moment(4_600, 4_600.0, 600.0, 38.0, &[]),
        moment(40_600, 40_600.0, 32_400.0, 45.0, &[]),
    ];
    let used = between(&moments).unwrap().expect("three readings are a measurement");

    assert_eq!(used.worst_sleep, None);
}
