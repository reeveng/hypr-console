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

use console_input_event_devices::{EventType, KeyCode, RelativeAxisCode};

use console_core_geometry::Point;
use console_input_controller::mode::Mode;
use console_onscreen::PadInput;
use console_test_flows::screens;
use console_test_stages::device::Ready;
use console_test_stages::here::{Here, TURNS};

fn asleep() -> Here {
    let mut here = Here::new().expect("a stage");
    here.showing(screens::THE_HOME_SCREEN).expect("the home screen");
    here
}

fn standing() -> Here {
    let mut here = asleep();
    here.press("dpad-right").expect("the d-pad");
    here.settle(TURNS);
    here.fresh();
    here
}

fn mode(here: &Here) -> Mode {
    let Ok(mode) = here.mode();

    mode
}

fn told(here: &Here) -> Vec<PadInput> {
    let Ok(told) = here.told();

    told.to_vec()
}

fn sent(here: &Here, kind: EventType, code: u16, value: i32) -> Ready {
    let Ok(seen) = here.sent(kind, code, value);

    seen
}

fn wrote(here: &Here, kind: EventType, code: u16) -> i32 {
    let Ok(wrote) = here.wrote(kind, code);

    wrote
}

fn dispatches(here: &Here) -> Vec<String> {
    let Ok(dispatches) = here.dispatches();

    dispatches
}

fn settle(here: &mut Here) {
    let Ok(()) = here.settle(TURNS);
}

fn fresh(here: &mut Here) {
    let Ok(()) = here.fresh();
}

#[test]
fn asleep_it_owns_nothing_at_all() {
    let mut here = asleep();

    assert_eq!(mode(&here), Mode::HomeScreen, "it is drawn, and drawn is not in front");

    here.drag(Point { x: 200, y: 200 }, Point { x: 400, y: 200 })
        .expect("a finger on the touchpad");
    settle(&mut here);
    assert!(
        wrote(&here, EventType::RELATIVE, RelativeAxisCode::REL_X.0) > 0,
        "the touchpad still moves the pointer over a home screen that is asleep"
    );
    fresh(&mut here);

    here.press("a").expect("a");
    settle(&mut here);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::BTN_LEFT.0, 1),
        Ready::Yes,
        "asleep, a is the pointer's button"
    );
    assert!(told(&here).is_empty(), "and the home screen was not told a thing about it");
    fresh(&mut here);

    here.press("y").expect("y");
    settle(&mut here);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::BTN_RIGHT.0, 1),
        Ready::Yes,
        "asleep, y is the pointer's other button rather than the square's card"
    );
    assert!(told(&here).is_empty(), "a square nobody is standing on has nothing to offer");
    fresh(&mut here);

    here.press("r1").expect("a shoulder");
    settle(&mut here);
    assert_eq!(
        dispatches(&here),
        ["hl.dsp.focus({workspace = \"+1\"})"],
        "the shoulders are places whether or not the home screen is drawn"
    );
}

#[test]
fn the_first_press_wakes_it_and_nothing_behind_it_hears_that_press() {
    let mut here = asleep();

    here.press("dpad-right").expect("the d-pad");
    settle(&mut here);

    assert_eq!(told(&here), [PadInput::Right], "the first press is a word to the home screen");
    assert_eq!(mode(&here), Mode::Standing, "and the word is what woke it");
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::KEY_RIGHT.0, 1),
        Ready::NotYet,
        "the press that woke it was not also an arrow key to whatever is behind"
    );
    assert_eq!(
        wrote(&here, EventType::RELATIVE, RelativeAxisCode::REL_X.0),
        0,
        "nor a nudge of the pointer"
    );
}

#[test]
fn standing_on_a_square_the_buttons_are_the_squares_and_the_places_are_still_places() {
    let mut here = standing();

    here.press("a").expect("a");
    settle(&mut here);
    assert_eq!(told(&here), [PadInput::Pressed], "standing on a square, a is the square's");
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::BTN_LEFT.0, 1),
        Ready::NotYet,
        "and it is not also a click on whatever is under the highlight"
    );
    fresh(&mut here);

    here.press("y").expect("y");
    settle(&mut here);
    assert_eq!(told(&here), [PadInput::More], "y is what else can be done with this one");
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::BTN_RIGHT.0, 1),
        Ready::NotYet,
        "and it is not also the pointer's other button"
    );
    fresh(&mut here);

    here.press("r1").expect("a shoulder");
    settle(&mut here);
    assert_eq!(
        dispatches(&here),
        ["hl.dsp.focus({workspace = \"+1\"})"],
        "a highlight on a square does not make a shoulder mean something else"
    );
    assert!(told(&here).is_empty(), "and the shoulder is not the home screen's to hear");
}

#[test]
fn putting_the_highlight_away_hands_every_button_back() {
    let mut here = standing();

    here.press("b").expect("b");
    settle(&mut here);
    assert_eq!(told(&here), [PadInput::Back], "b puts the highlight away");
    assert_eq!(mode(&here), Mode::HomeScreen, "which is the home screen asleep again");
    fresh(&mut here);

    here.press("a").expect("a");
    settle(&mut here);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::BTN_LEFT.0, 1),
        Ready::Yes,
        "a is the pointer's button again the moment the highlight is gone"
    );
    assert!(
        told(&here).is_empty(),
        "a home screen that was told b is a home screen nothing is being sent to"
    );
}

#[test]
fn a_picker_over_it_takes_the_buttons_while_it_is_up() {
    let mut here = Here::new().expect("a stage");
    here.showing(screens::A_PICKER_OVER_THE_HOME_SCREEN).expect("a picker over the home screen");

    assert_eq!(mode(&here), Mode::Tabs, "what is in front is the picker and not the wallpaper");

    here.press("a").expect("a");
    settle(&mut here);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::KEY_ENTER.0, 1),
        Ready::Yes,
        "with a picker up, a takes the row it is standing on"
    );
    assert!(
        told(&here).is_empty(),
        "and the home screen under it is told nothing about a press that was never its own"
    );
    fresh(&mut here);

    here.press("r1").expect("a shoulder");
    settle(&mut here);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::KEY_PAGEDOWN.0, 1),
        Ready::Yes,
        "with a picker up the shoulder is the tab beside this one"
    );
    assert!(dispatches(&here).is_empty(), "and it is not also a workspace");
}
