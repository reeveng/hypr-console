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
//! is written, but whether the effect it carries is the one the job promises.
//! `docs/button-contract.md` is the argument for the move and what it costs.

use std::error::Error;

use console_core_never::Never;
use console_input_bindings::bound::Input;
use console_input_bindings::keys;
use console_input_bindings::moved::Tasks;
use console_input_controller::binds::{KeyBinding, desired_binds};
use console_input_controller::actions::{JOBS, LockBehavior, RepeatMode, Table};

type Failure = Box<dyn Error>;

fn unmoved() -> Result<Vec<KeyBinding>, Never> {
    let Ok(none) = Tasks::none();
    let Ok(table) = Table::of(&none);

    desired_binds(&table)
}

fn on<'a>(every: &'a [KeyBinding], held: &[&str], pressed: &str) -> Result<&'a KeyBinding, Failure> {
    let held: Vec<String> = held.iter().map(|word| (*word).to_string()).collect();
    let Ok(keys) = keys::bind(&held, pressed);
    let keys = keys.ok_or_else(|| format!("no key called {pressed}"))?;
    let bind = every.iter().find(|bind| bind.keys == keys).ok_or_else(|| format!("nothing is bound to {keys}"))?;

    Ok(bind)
}

#[test]
fn the_power_key_puts_the_panel_back_and_answers_with_the_screen_locked() -> Result<(), Failure> {
    let Ok(every) = unmoved();
    let power = on(&every, &[], "power")?;

    assert!(
        power.runs.contains("console-brightness") && power.runs.contains("undim"),
        "the power key runs {:?}, which is not the program that puts the screen back",
        power.runs
    );
    assert_eq!(
        power.locked,
        LockBehavior::EvenThen,
        "the power key would do nothing with the screen off, which is the whole state it \
         exists for"
    );
    assert_eq!(
        power.repeats,
        RepeatMode::Once,
        "holding the power key would run the undim over and over"
    );

    Ok(())
}

#[test]
fn what_a_keyboard_labels_for_itself_is_bound_to_what_the_label_says() -> Result<(), Failure> {
    let Ok(every) = unmoved();

    for (pressed, runs) in [
        ("volume-up", "up"),
        ("volume-down", "down"),
        ("mute", "mute"),
        ("brightness-up", "up"),
        ("brightness-down", "down"),
    ] {
        let bind = on(&every, &[], pressed)?;

        assert!(
            bind.runs.ends_with(runs),
            "the key marked {pressed} runs {:?}",
            bind.runs
        );
        assert_eq!(
            bind.locked,
            LockBehavior::EvenThen,
            "{pressed} is not answered in the dark, and a level someone reaches for is \
             reached for in the dark"
        );
    }

    Ok(())
}

#[test]
fn a_level_walks_while_it_is_held_and_a_door_opens_once() -> Result<(), Failure> {
    let Ok(every) = unmoved();
    let louder = on(&every, &[], "volume-up")?;
    let dimmer = on(&every, &[], "brightness-down")?;
    let browser = on(&every, &["super"], "b")?;

    assert_eq!(louder.repeats, RepeatMode::WhileHeld);
    assert_eq!(dimmer.repeats, RepeatMode::WhileHeld);
    assert_eq!(
        browser.repeats,
        RepeatMode::Once,
        "holding Super and B would open a browser for as long as the finger is down"
    );

    Ok(())
}

#[test]
fn no_two_jobs_are_handed_the_same_keys() {
    let Ok(every) = unmoved();
    let mut seen: std::collections::BTreeMap<&str, &str> = std::collections::BTreeMap::new();

    for bind in &every {
        let before = seen.insert(&bind.keys, &bind.runs);

        assert!(
            before.is_none_or(|runs| runs == bind.runs),
            "{} is handed over twice, and the compositor keeps whichever arrived last",
            bind.keys
        );
    }
}

#[test]
fn every_key_in_the_table_is_one_the_compositor_can_be_told_about() {
    let mut asked: u32 = 0;

    for job in JOBS.iter() {
        for (_, held, pressed) in job.bound.iter().filter(|(on, _, _)| *on == Input::Keyboard) {
            let held: Vec<String> = held.iter().map(|word| (*word).to_string()).collect();

            assert_ne!(
                keys::bind(&held, pressed),
                Ok(None),
                "{} is bound to {pressed}, which is not a key this desktop has a word for",
                job.slug
            );

            asked = asked.saturating_add(1);
        }
    }

    assert!(asked > 1, "no job is on a keyboard, so this test asked nothing");
}

#[test]
fn the_alphabets_step_both_ways_and_shift_is_the_way_back() -> Result<(), Failure> {
    let Ok(every) = unmoved();
    let on_to = on(&every, &["super", "shift"], "space")?;
    let back = on(&every, &["super", "ctrl"], "space")?;

    assert!(
        on_to.runs.contains(console_input_language::NAMED),
        "the key that steps the alphabets runs {:?}",
        on_to.runs
    );
    assert!(
        back.runs.contains(console_input_language::NAMED)
            && back.runs.contains(console_input_language::BACK.spelling),
        "the other half runs {:?}, which is not the same walk the other way",
        back.runs
    );
    assert_ne!(
        on_to.runs, back.runs,
        "both halves of the walk run the same thing, so a walk of three cannot be undone"
    );

    Ok(())
}

#[test]
fn the_windows_are_reached_on_the_arrows_and_on_hjkl() -> Result<(), Failure> {
    let Ok(every) = unmoved();

    for (arrow, letter) in [("left", "h"), ("down", "j"), ("up", "k"), ("right", "l")] {
        let by_arrow = on(&every, &["super"], arrow)?;
        let by_letter = on(&every, &["super"], letter)?;

        assert_eq!(
            by_arrow.runs,
            by_letter.runs,
            "{letter} and {arrow} part company, so one hand's way round the screen is not the \
             other's"
        );
    }

    for (arrow, letter) in [("left", "h"), ("right", "l")] {
        let by_arrow = on(&every, &["super", "shift"], arrow)?;
        let by_letter = on(&every, &["super", "shift"], letter)?;

        assert_eq!(
            by_arrow.runs,
            by_letter.runs,
            "Shift and {letter} does not carry the window where Shift and {arrow} carries it"
        );
    }

    Ok(())
}
