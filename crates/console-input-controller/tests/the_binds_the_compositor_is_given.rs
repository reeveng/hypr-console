//! The other end of a keyboard binding.
//!
//! `what_reaches_the_desktop` asks whether the table can find a job again. This
//! asks the question the compositor would ask: of everything in the table that
//! is bound on a keyboard, what does it get handed, and would the hand it is
//! given do the thing.
//!
//! These used to be lines in `hyprland.lua` and this used to be a test that
//! read that file, counting where a bind sat in it. The file binds nothing now,
//! so what was worth keeping out of that test is here instead: not where a line
//! is written, but whether the doing it carries is the one the job promises.
//! `docs/button-contract.md` is the argument for the move and what it costs.

use console_input_bindings::bound::Input;
use console_input_bindings::keys;
use console_input_bindings::moved::Jobs;
use console_input_controller::binds::{Bind, wanted};
use console_input_controller::means::{JOBS, Locked, Repeats, Table};

fn ok<T>(answer: Result<T, console_core_never::Never>) -> T {
    let Ok(value) = answer;

    value
}

fn every() -> Vec<Bind> {
    ok(wanted(&ok(Table::of(&ok(Jobs::none())))))
}

fn keyed(held: &[&str], pressed: &str) -> String {
    let held: Vec<String> = held.iter().map(|word| (*word).to_string()).collect();

    ok(keys::bind(&held, pressed)).unwrap_or_else(|| panic!("no key called {pressed}"))
}

fn on<'a>(every: &'a [Bind], held: &[&str], pressed: &str) -> &'a Bind {
    let keys = keyed(held, pressed);

    every
        .iter()
        .find(|bind| bind.keys == keys)
        .unwrap_or_else(|| panic!("nothing is bound to {keys}"))
}

#[test]
fn the_power_key_puts_the_panel_back_and_answers_with_the_screen_locked() {
    let every = every();
    let power = on(&every, &[], "power");

    assert!(
        power.runs.contains("console-brightness") && power.runs.contains("undim"),
        "the power key runs {:?}, which is not the program that puts the screen back",
        power.runs
    );
    assert_eq!(
        power.locked,
        Locked::EvenThen,
        "the power key would do nothing with the screen off, which is the whole state it \
         exists for"
    );
    assert_eq!(
        power.repeats,
        Repeats::Once,
        "holding the power key would run the undim over and over"
    );
}

#[test]
fn what_a_keyboard_labels_for_itself_is_bound_to_what_the_label_says() {
    let every = every();

    for (pressed, runs) in [
        ("volume-up", "up"),
        ("volume-down", "down"),
        ("mute", "mute"),
        ("brightness-up", "up"),
        ("brightness-down", "down"),
    ] {
        let bind = on(&every, &[], pressed);

        assert!(
            bind.runs.ends_with(runs),
            "the key marked {pressed} runs {:?}",
            bind.runs
        );
        assert_eq!(
            bind.locked,
            Locked::EvenThen,
            "{pressed} is not answered in the dark, and a level somebody reaches for is \
             reached for in the dark"
        );
    }
}

#[test]
fn a_level_walks_while_it_is_held_and_a_door_opens_once() {
    let every = every();

    assert_eq!(on(&every, &[], "volume-up").repeats, Repeats::WhileHeld);
    assert_eq!(on(&every, &[], "brightness-down").repeats, Repeats::WhileHeld);
    assert_eq!(
        on(&every, &["super"], "b").repeats,
        Repeats::Once,
        "holding Super and B would open a browser for as long as the finger is down"
    );
}

#[test]
fn no_two_jobs_are_handed_the_same_keys() {
    let every = every();

    for (which, bind) in every.iter().enumerate() {
        let twice = every
            .iter()
            .enumerate()
            .find(|(other, one)| *other != which && one.keys == bind.keys && one.runs != bind.runs);

        assert!(
            twice.is_none(),
            "{} is handed over twice, and the compositor keeps whichever arrived last",
            bind.keys
        );
    }
}

#[test]
fn every_key_in_the_table_is_one_the_compositor_can_be_told_about() {
    let mut asked = 0;

    for job in JOBS.iter() {
        for (on, held, pressed) in job.bound.iter().filter(|(on, _, _)| *on == Input::Keyboard) {
            let _ = on;
            let held: Vec<String> = held.iter().map(|word| (*word).to_string()).collect();

            assert!(
                ok(keys::bind(&held, pressed)).is_some(),
                "{} is bound to {pressed}, which is not a key this desktop has a word for",
                job.slug
            );

            asked += 1;
        }
    }

    assert!(asked > 1, "no job is on a keyboard, so this test asked nothing");
}
