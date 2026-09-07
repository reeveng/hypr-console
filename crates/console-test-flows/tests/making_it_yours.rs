//! Somebody makes the buttons their own, and the desktop keeps its promises
//! after.
//!
//! The first flow, and first on purpose: every other flow walks the desktop
//! through the table of what a button means, so the table being movable is
//! the claim under all of them. What is walked here is the whole seam a
//! person's own answers travel -- the move the setup screen works out, the
//! file it writes, the file read back the way the daemon reads it, and then
//! every place on the desktop the rebound table has to go on making sense in.
//!
//! `docs/flows.md` is the strategy this belongs to. The stage is `here`: the
//! daemon in this process against the captured devices and the real profile
//! files, so a press below travels the road a thumb's press travels, and what
//! is asserted is what the daemon decided -- which program was started, which
//! key was sent, which word was said to the home screen.

use std::collections::BTreeMap;

use evdev::{EventType, KeyCode};

use console_input_controller::means::Table;
use console_input_controller::mode::Mode;
use console_onscreen::Said;
use console_test_flows::screens;
use console_input_gamepad::jobs::{Binding, Jobs, Moved, Played};
use console_test_stages::device::Seen;
use console_test_stages::here::{Here, TURNS};

fn stage() -> Here {
    let mut here = Here::new().expect("a stage");
    here.showing(screens::NOTHING_UP).expect("the desktop");
    here
}

fn every(table: &Table) -> BTreeMap<String, Vec<Binding>> {
    let Ok(every) = table.every();

    every.map(|(job, bound)| (job.slug.to_string(), bound.to_vec())).collect()
}

fn ours() -> Table {
    let Ok(table) = Table::ours();

    table
}

fn of(said: &Jobs) -> Table {
    let Ok(table) = Table::of(said);

    table
}

fn none() -> Jobs {
    let Ok(said) = Jobs::none();

    said
}

fn moving(
    said: &mut Jobs,
    every: &BTreeMap<String, Vec<Binding>>,
    job: &str,
    onto: &Binding,
) -> Moved {
    let Ok(moved) = said.moving(every, job, onto);

    moved
}

fn written(said: &Jobs) -> String {
    let Ok(written) = said.written();

    written
}

fn bindings<'a>(table: &'a Table, slug: &str) -> &'a [Binding] {
    let Ok(bindings) = table.bindings(slug);

    bindings
}

fn bound_by(here: &mut Here, table: Table) {
    let Ok(()) = here.bound_by(table);
}

fn started(here: &Here) -> Vec<String> {
    let Ok(names) = here.names();

    names
}

fn sent(here: &Here, kind: EventType, code: u16, value: i32) -> Seen {
    let Ok(seen) = here.sent(kind, code, value);

    seen
}

fn wrote(here: &Here, kind: EventType, code: u16) -> i32 {
    let Ok(wrote) = here.wrote(kind, code);

    wrote
}

fn told(here: &Here) -> Vec<Said> {
    let Ok(told) = here.told();

    told.to_vec()
}

fn mode(here: &Here) -> Mode {
    let Ok(mode) = here.mode();

    mode
}

fn dispatches(here: &Here) -> Vec<String> {
    let Ok(dispatches) = here.dispatches();

    dispatches
}

#[test]
fn moving_a_job_moves_it_and_nothing_else() {
    let mut here = stage();

    here.trigger("l2", 1.0).expect("a trigger");
    here.press("right-paddle-bottom").expect("a paddle");
    here.settle(TURNS);
    assert!(
        started(&here).contains(&"console-screenshot".to_string()),
        "out of the box, l2 + right-paddle-bottom is the screenshot"
    );
    here.trigger("l2", 0.0).expect("a trigger let go");
    here.fresh();

    let mut said = none();
    let onto = Binding::read("r2 + a").expect("a binding");
    assert_eq!(moving(&mut said, &every(&ours()), "screenshot", &onto), Moved::Onto);
    let read = Jobs::read(&written(&said)).expect("what the setup screen wrote reads back");
    bound_by(&mut here, of(&read));

    here.trigger("r2", 1.0).expect("a trigger");
    here.press("a").expect("a");
    here.settle(TURNS);
    assert!(
        started(&here).contains(&"console-screenshot".to_string()),
        "moved onto r2 + a, the screenshot is taken there"
    );
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::BTN_LEFT.0, 1),
        Seen::NotYet,
        "the chord that takes the picture does not also click"
    );
    here.trigger("r2", 0.0).expect("a trigger let go");
    here.fresh();

    here.trigger("l2", 1.0).expect("a trigger");
    here.press("right-paddle-bottom").expect("a paddle");
    here.settle(TURNS);
    assert!(
        !started(&here).contains(&"console-screenshot".to_string()),
        "the screenshot has left the paddle it was moved off"
    );
    assert!(
        wrote(&here, EventType::RELATIVE, evdev::RelativeAxisCode::REL_WHEEL.0) < 0,
        "bare of its second job, the paddle goes on scrolling the page"
    );
    here.trigger("l2", 0.0).expect("a trigger let go");
    here.fresh();

    here.press("a").expect("a");
    here.settle(TURNS);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::BTN_LEFT.0, 1),
        Seen::Yes,
        "a on its own is still a click"
    );
    assert!(started(&here).is_empty(), "a click starts nothing");
}

#[test]
fn the_move_holds_wherever_a_person_goes() {
    let mut here = stage();
    let said = Jobs::read("[jobs]\nscreenshot = \"r2 + a\"\n").expect("a table");
    bound_by(&mut here, of(&said));

    let shot = |here: &mut Here| {
        here.trigger("r2", 1.0).expect("a trigger");
        here.press("a").expect("a");
        here.settle(TURNS);
        let taken = started(here).contains(&"console-screenshot".to_string());
        here.trigger("r2", 0.0).expect("a trigger let go");
        here.fresh();
        taken
    };

    assert!(shot(&mut here), "on the desktop, the moved chord takes the picture");

    here.showing(screens::A_CHOOSER).expect("a chooser");
    assert!(shot(&mut here), "with a chooser up, the moved chord still takes the picture");
    here.press("a").expect("a");
    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::KEY_ENTER.0, 1),
        Seen::Yes,
        "bare a with a chooser up takes the row it is standing on"
    );
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::KEY_PAGEDOWN.0, 1),
        Seen::Yes,
        "a shoulder with a chooser up is the tab beside this one"
    );
    assert!(dispatches(&here).is_empty(), "with a chooser up, a shoulder is not a workspace");
    here.fresh();

    here.showing(screens::THE_HOME_SCREEN).expect("the home screen");
    assert_eq!(mode(&here), Mode::Home, "the home screen is drawn and asleep");
    here.press("dpad-right").expect("the d-pad");
    here.settle(TURNS);
    assert!(
        told(&here).contains(&Said::Right),
        "the first d-pad press is a word to the home screen"
    );
    assert_eq!(mode(&here), Mode::Standing, "the word woke it");
    here.fresh();

    here.press("a").expect("a");
    here.settle(TURNS);
    assert!(
        told(&here).contains(&Said::Pressed),
        "standing on a square, a is the square's"
    );
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::BTN_LEFT.0, 1),
        Seen::NotYet,
        "standing on a square, a is not a click"
    );
    here.fresh();

    assert!(shot(&mut here), "standing on a square, the moved chord still takes the picture");

    here.press("b").expect("b");
    here.settle(TURNS);
    assert!(told(&here).contains(&Said::Back), "b puts the highlight away");
    assert_eq!(mode(&here), Mode::Home, "the home screen is asleep again");
    here.fresh();

    here.press("a").expect("a");
    here.settle(TURNS);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::BTN_LEFT.0, 1),
        Seen::Yes,
        "asleep again, a is the pointer's button"
    );
    here.fresh();

    here.showing(screens::THE_KEYBOARD).expect("the keyboard");
    here.trigger("r2", 1.0).expect("a trigger");
    here.press("a").expect("a");
    here.settle(TURNS);
    assert!(started(&here).is_empty(), "under the keyboard, the chord starts nothing");
    assert!(told(&here).is_empty(), "under the keyboard, nothing is said to the home screen");
    assert_eq!(
        wrote(&here, EventType::KEY, KeyCode::BTN_LEFT.0),
        0,
        "under the keyboard, nothing reaches the pointer"
    );
}

#[test]
fn the_file_says_several_buttons_a_chord_or_nothing_at_all() {
    let mut here = stage();
    let said = Jobs::read(
        "[jobs]\nmenu = [\"left-paddle-top\", \"l2 + b\"]\ndictate = \"\"\nteleport = \"y\"\n",
    )
    .expect("a table");
    bound_by(&mut here, of(&said));

    here.press("left-paddle-top").expect("a paddle");
    here.settle(TURNS);
    assert_eq!(started(&here), ["launcher"], "the paddle still opens the menu");
    here.fresh();

    here.trigger("l2", 1.0).expect("a trigger");
    here.press("b").expect("b");
    here.settle(TURNS);
    assert_eq!(started(&here), ["launcher"], "and so does the chord beside it");
    here.trigger("l2", 0.0).expect("a trigger let go");
    here.fresh();

    here.press("b").expect("b");
    here.settle(TURNS);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::KEY_ESC.0, 1),
        Seen::Yes,
        "b on its own is still back"
    );
    assert!(started(&here).is_empty(), "b on its own opens nothing");
    here.fresh();

    here.press("left-paddle-bottom").expect("a paddle");
    here.settle(TURNS);
    assert!(started(&here).is_empty(), "a job with its button taken off starts nothing");
    here.fresh();

    here.press("y").expect("y");
    here.settle(TURNS);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::BTN_RIGHT.0, 1),
        Seen::Yes,
        "a job from some newer desktop does not take y from more-options"
    );
    here.fresh();

    let fault = Jobs::read("[jobs]\nmenu = \"a\"\nscreenshot = \"nose + a\"\n")
        .expect_err("nose is not a trigger");
    assert!(fault.starts_with("screenshot: "), "the fault names the line: {fault}");
    here.press("left-paddle-top").expect("a paddle");
    here.settle(TURNS);
    assert_eq!(started(&here), ["launcher"], "the table already loaded is left standing");
}

#[test]
fn one_press_still_does_one_thing() {
    let mut here = stage();

    let mut said = none();
    let onto = Binding::read("left-paddle-top").expect("a binding");
    assert_eq!(
        moving(&mut said, &every(&ours()), "guide", &onto),
        Moved::TookFrom("menu".to_string())
    );

    let read = Jobs::read(&written(&said)).expect("what the setup screen wrote reads back");
    let table = of(&read);
    assert_eq!(bindings(&table, "menu").len(), 1);
    let Some(one) = bindings(&table, "menu").first() else { panic!("the menu is bound to something") };

    assert_eq!(one.played(), Ok(Played::ByNothing));
    bound_by(&mut here, table);

    here.press("left-paddle-top").expect("a paddle");
    here.settle(TURNS);
    assert_eq!(
        started(&here),
        ["console-buttons"],
        "the paddle opens the guide, and does not also open the menu"
    );
    here.fresh();

    here.press("menu").expect("the button with the lines on it");
    here.settle(TURNS);
    assert!(started(&here).is_empty(), "the guide's old button was left playing nothing");
}
