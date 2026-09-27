//! Someone puts the device down on the home screen, picks it up again, and
//! the buttons are whoever's the screen says they are.
//!
//! The third flow, and the promise under it is the one this desktop is most
//! able to break by accident: the home screen is drawn under everything for
//! as long as the machine is on, so anything it keeps hold of, it keeps hold
//! of everywhere. A highlight that woke when nobody asked, a word still being
//! sent to it after the highlight was put away, an A that stopped being a
//! click on a desktop with nothing in front of it -- each of those is a device
//! that answers the wrong thing to the most ordinary press there is, and none
//! of them is visible in any one feature's check, because the fault is that a
//! surface which is always there is being asked whether it is in front.
//!
//! `docs/flows.md` is the strategy this belongs to, and the promise is "the
//! home screen holds nothing". The stage is `here`: the daemon in this process
//! against the captured devices and the real profile files, so a press below
//! travels the road a thumb's press travels, and what is asserted is what the
//! daemon decided -- which key it sent, which word it said to the home screen,
//! what it started.
//!
//! What this flow hands up rather than answering: everything the home screen
//! itself does with a word once it has one. Whether the first press draws a
//! highlight without moving it, whether a carried application lands where it
//! was put down, and whether the arrangement is still there after a restart
//! are all inside `console-home`, which is a compositor surface and a toolkit
//! loop; they want a screen and they are the desktop stage's. What is
//! answered here is the half nothing else can see: that the word was sent at
//! all, that it was the only thing sent, and that nothing behind the home
//! screen heard the press that made it.

use std::error::Error;

use console_input_event_devices::{EventType, KeyCode, RelativeAxisCode};

use console_core_geometry::Point;
use console_input_controller::mode::Mode;
use console_onscreen::PadInput;
use console_test_flows::screens;
use console_test_stages::device::Ready;
use console_test_stages::here::{Here, TURNS};

type Failure = Box<dyn Error>;

fn asleep() -> Result<Here, Failure> {
    let mut here = Here::new()?;

    here.set_layers(screens::THE_HOME_SCREEN)?;

    Ok(here)
}

fn standing() -> Result<Here, Failure> {
    let mut here = asleep()?;

    here.press("dpad-right")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(()) = here.fresh();

    Ok(here)
}

#[test]
fn asleep_it_owns_nothing_at_all() -> Result<(), Failure> {
    let mut here = asleep()?;

    let Ok(mode) = here.mode();

    assert_eq!(mode, Mode::HomeScreen, "it is drawn, and drawn is not in front");

    here.drag(Point { x: 200, y: 200 }, Point { x: 400, y: 200 })
        ?;
    let Ok(()) = here.settle(TURNS);
    let Ok(wrote) = here.wrote(EventType::RELATIVE, RelativeAxisCode::REL_X.0);

    assert!(
        wrote > 0,
        "the touchpad still moves the pointer over a home screen that is asleep"
    );
    let Ok(()) = here.fresh();

    here.press("a")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "asleep, a is the pointer's button"
    );
    let Ok(told) = here.pad_inputs();

    assert!(told.is_empty(), "and the home screen was not told a thing about it");
    let Ok(()) = here.fresh();

    here.press("y")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::BTN_RIGHT.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "asleep, y is the pointer's other button rather than the square's card"
    );
    let Ok(told) = here.pad_inputs();

    assert!(told.is_empty(), "a square nobody is standing on has nothing to offer");
    let Ok(()) = here.fresh();

    here.press("r1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(dispatches) = here.dispatches();

    assert_eq!(
        dispatches,
        ["hl.dsp.focus({workspace = \"+1\"})"],
        "the shoulders are places whether or not the home screen is drawn"
    );

    Ok(())
}

#[test]
fn the_first_press_wakes_it_and_nothing_behind_it_hears_that_press() -> Result<(), Failure> {
    let mut here = asleep()?;

    here.press("dpad-right")?;
    let Ok(()) = here.settle(TURNS);

    let Ok(told) = here.pad_inputs();

    assert_eq!(told, [PadInput::Right], "the first press is a word to the home screen");
    let Ok(mode) = here.mode();

    assert_eq!(mode, Mode::Standing, "and the word is what woke it");
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::KEY_RIGHT.0, 1);

    assert_eq!(
        sent,
        Ready::NotYet,
        "the press that woke it was not also an arrow key to whatever is behind"
    );
    let Ok(wrote) = here.wrote(EventType::RELATIVE, RelativeAxisCode::REL_X.0);

    assert_eq!(
        wrote,
        0,
        "nor a nudge of the pointer"
    );

    Ok(())
}

#[test]
fn standing_on_a_square_the_buttons_are_the_squares_and_the_places_are_still_places() -> Result<(), Failure> {
    let mut here = standing()?;

    here.press("a")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(told) = here.pad_inputs();

    assert_eq!(told, [PadInput::Pressed], "standing on a square, a is the square's");
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1);

    assert_eq!(
        sent,
        Ready::NotYet,
        "and it is not also a click on whatever is under the highlight"
    );
    let Ok(()) = here.fresh();

    here.press("y")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(told) = here.pad_inputs();

    assert_eq!(told, [PadInput::More], "y is what else can be done with this one");
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::BTN_RIGHT.0, 1);

    assert_eq!(
        sent,
        Ready::NotYet,
        "and it is not also the pointer's other button"
    );
    let Ok(()) = here.fresh();

    here.press("r1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(dispatches) = here.dispatches();

    assert_eq!(
        dispatches,
        ["hl.dsp.focus({workspace = \"+1\"})"],
        "a highlight on a square does not make a shoulder mean something else"
    );
    let Ok(told) = here.pad_inputs();

    assert!(told.is_empty(), "and the shoulder is not the home screen's to hear");

    Ok(())
}

#[test]
fn putting_the_highlight_away_hands_every_button_back() -> Result<(), Failure> {
    let mut here = standing()?;

    here.press("b")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(told) = here.pad_inputs();

    assert_eq!(told, [PadInput::Back], "b puts the highlight away");
    let Ok(mode) = here.mode();

    assert_eq!(mode, Mode::HomeScreen, "which is the home screen asleep again");
    let Ok(()) = here.fresh();

    here.press("a")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "a is the pointer's button again the moment the highlight is gone"
    );
    let Ok(told) = here.pad_inputs();

    assert!(
        told.is_empty(),
        "a home screen that was told b is a home screen nothing is being sent to"
    );

    Ok(())
}

#[test]
fn a_picker_over_it_takes_the_buttons_while_it_is_up() -> Result<(), Failure> {
    let mut here = Here::new()?;
    here.set_layers(screens::A_PICKER_OVER_THE_HOME_SCREEN)?;

    let Ok(mode) = here.mode();

    assert_eq!(mode, Mode::Tabs, "what is in front is the picker and not the wallpaper");

    here.press("a")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::KEY_ENTER.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "with a picker up, a takes the row it is standing on"
    );
    let Ok(told) = here.pad_inputs();

    assert!(
        told.is_empty(),
        "and the home screen under it is told nothing about a press that was never its own"
    );
    let Ok(()) = here.fresh();

    here.press("r1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::KEY_PAGEDOWN.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "with a picker up the shoulder is the tab beside this one"
    );
    let Ok(dispatches) = here.dispatches();

    assert!(dispatches.is_empty(), "and it is not also a workspace");

    Ok(())
}
