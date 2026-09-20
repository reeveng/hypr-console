//! Two readings, and what they come to.
//!
//! The reading itself needs a machine and the device's own check presses it
//! there. What is asked here is the half that decides anything: a rate out of
//! two counters, with the three things a counter does that a subtraction is
//! wrong about -- a program restarting, the machine rebooting, and a night
//! spent asleep counting as time the desktop was awake for.

use console_resource_usage::{Moment, between, of, written};

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
    }
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
    let one = moment(1_000, 1_000.0, 12.5, 40.25, &[("wireplumber", 100.5)]);
    let said = written(&one).unwrap();
    let back = of(&said).unwrap().expect("a line this crate wrote");

    assert_eq!(back, one);
}
