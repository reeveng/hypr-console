//! Someone makes the buttons their own, and the desktop keeps its promises
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
use std::error::Error;

use console_core_never::Never;

use console_input_event_devices::{EventType, KeyCode};

use console_input_controller::actions::Table;
use console_input_controller::mode::Mode;
use console_onscreen::PadInput;
use console_test_flows::screens;
use console_input_bindings::bound::{Binding, Input, Played};
use console_input_bindings::moved::{Tasks, Moved};
use console_test_stages::device::Ready;
use console_test_stages::here::{Here, TURNS};

type Failure = Box<dyn Error>;

const SCREENSHOT: &str = "console-screenshot";

fn stage() -> Result<Here, Failure> {
    let mut here = Here::new()?;

    here.set_layers(screens::NOTHING_UP)?;

    Ok(here)
}

fn every(table: &Table) -> Result<BTreeMap<String, Vec<Binding>>, Never> {
    let Ok(every) = table.every();

    Ok(every.map(|(job, bound)| (job.slug.to_string(), bound.to_vec())).collect())
}

fn bound_by(here: &mut Here, said: &Tasks) -> Result<(), Never> {
    let Ok(table) = Table::of(said);

    here.bound_by(table)
}

fn written_and_read(said: &Tasks) -> Result<Tasks, Failure> {
    let Ok(written) = said.serialize();
    let read = Tasks::read(&written)?;

    Ok(read)
}

#[test]
fn moving_a_job_moves_it_and_nothing_else() -> Result<(), Failure> {
    let mut here = stage()?;

    here.trigger("l2", 1.0)?;
    here.press("right-paddle-bottom")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert!(
        started.contains(&"console-screenshot".to_string()),
        "out of the box, l2 + right-paddle-bottom is the screenshot"
    );
    here.trigger("l2", 0.0)?;
    let Ok(()) = here.fresh();

    let Ok(mut said) = Tasks::none();
    let onto = Binding::read("r2 + a")?;
    let off = Binding::read("l2 + right-paddle-bottom")?;
    let Ok(table) = Table::ours();
    let Ok(out_of_the_box) = every(&table);

    assert_eq!(said.add(&out_of_the_box, "screenshot", &onto), Ok(Moved::Onto));

    let Ok(table) = Table::of(&said);
    let Ok(moved) = every(&table);
    let Ok(()) = said.remove(&moved, "screenshot", &off);
    let read = written_and_read(&said)?;
    let Ok(()) = bound_by(&mut here, &read);

    here.trigger("r2", 1.0)?;
    here.press("a")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert!(
        started.contains(&"console-screenshot".to_string()),
        "moved onto r2 + a, the screenshot is taken there"
    );
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1);

    assert_eq!(
        sent,
        Ready::NotYet,
        "the chord that takes the picture does not also click"
    );
    here.trigger("r2", 0.0)?;
    let Ok(()) = here.fresh();

    here.trigger("l2", 1.0)?;
    here.press("right-paddle-bottom")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert!(
        !started.contains(&"console-screenshot".to_string()),
        "the screenshot has left the paddle it was moved off"
    );
    let Ok(wrote) = here.wrote(EventType::RELATIVE, console_input_event_devices::RelativeAxisCode::REL_WHEEL.0);

    assert!(
        wrote < 0,
        "bare of its second job, the paddle goes on scrolling the page"
    );
    here.trigger("l2", 0.0)?;
    let Ok(()) = here.fresh();

    here.press("a")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "a on its own is still a click"
    );
    let Ok(started) = here.names();

    assert!(started.is_empty(), "a click starts nothing");

    Ok(())
}

fn chorded(here: &mut Here) -> Result<Vec<String>, Failure> {
    here.trigger("r2", 1.0)?;
    here.press("a")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    here.trigger("r2", 0.0)?;
    let Ok(()) = here.fresh();

    Ok(started)
}

#[test]
fn the_move_holds_wherever_a_person_goes() -> Result<(), Failure> {
    let mut here = stage()?;
    let said = Tasks::read("[jobs]\nscreenshot = \"r2 + a\"\n")?;
    let Ok(()) = bound_by(&mut here, &said);
    let started = chorded(&mut here)?;

    assert!(started.contains(&SCREENSHOT.to_string()), "on the desktop, the moved chord takes the picture");

    in_a_picker(&mut here)?;
    on_the_home_screen(&mut here)?;
    under_the_keyboard(&mut here)?;

    Ok(())
}

fn in_a_picker(here: &mut Here) -> Result<(), Failure> {
    here.set_layers(screens::A_PICKER)?;
    let started = chorded(here)?;

    assert!(started.contains(&SCREENSHOT.to_string()), "with a picker up, the moved chord still takes the picture");
    here.press("a")?;
    here.press("r1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::KEY_ENTER.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "bare a with a picker up takes the row it is standing on"
    );
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::KEY_PAGEDOWN.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "a shoulder with a picker up is the tab beside this one"
    );
    let Ok(dispatches) = here.dispatches();

    assert!(dispatches.is_empty(), "with a picker up, a shoulder is not a workspace");
    let Ok(()) = here.fresh();

    Ok(())
}

fn on_the_home_screen(here: &mut Here) -> Result<(), Failure> {
    here.set_layers(screens::THE_HOME_SCREEN)?;
    let Ok(mode) = here.mode();

    assert_eq!(mode, Mode::HomeScreen, "the home screen is drawn and asleep");
    here.press("dpad-right")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(told) = here.pad_inputs();

    assert!(
        told.contains(&PadInput::Right),
        "the first d-pad press is a word to the home screen"
    );
    let Ok(mode) = here.mode();

    assert_eq!(mode, Mode::Standing, "the word woke it");
    let Ok(()) = here.fresh();

    here.press("a")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(told) = here.pad_inputs();

    assert!(
        told.contains(&PadInput::Pressed),
        "standing on a square, a is the square's"
    );
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1);

    assert_eq!(
        sent,
        Ready::NotYet,
        "standing on a square, a is not a click"
    );
    let Ok(()) = here.fresh();

    let started = chorded(here)?;

    assert!(started.contains(&SCREENSHOT.to_string()), "standing on a square, the moved chord still takes the picture");

    here.press("b")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(told) = here.pad_inputs();

    assert!(told.contains(&PadInput::Back), "b puts the highlight away");
    let Ok(mode) = here.mode();

    assert_eq!(mode, Mode::HomeScreen, "the home screen is asleep again");
    let Ok(()) = here.fresh();

    here.press("a")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "asleep again, a is the pointer's button"
    );
    let Ok(()) = here.fresh();

    Ok(())
}

fn under_the_keyboard(here: &mut Here) -> Result<(), Failure> {
    here.set_layers(screens::THE_KEYBOARD)?;
    here.trigger("r2", 1.0)?;
    here.press("a")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert!(started.is_empty(), "under the keyboard, the chord starts nothing");
    let Ok(told) = here.pad_inputs();

    assert!(told.is_empty(), "under the keyboard, nothing is said to the home screen");
    let Ok(wrote) = here.wrote(EventType::KEY, KeyCode::BTN_LEFT.0);

    assert_eq!(
        wrote,
        0,
        "under the keyboard, nothing reaches the pointer"
    );

    Ok(())
}

#[test]
fn the_file_says_several_buttons_a_chord_or_nothing_at_all() -> Result<(), Failure> {
    let mut here = stage()?;
    let said = Tasks::read(
        "[jobs]\nmenu = [\"left-paddle-top\", \"l2 + b\"]\ndictate = \"\"\nteleport = \"y\"\n",
    )
    ?;
    let Ok(()) = bound_by(&mut here, &said);

    here.press("left-paddle-top")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert_eq!(started, ["launcher"], "the paddle still opens the menu");
    let Ok(()) = here.fresh();

    here.trigger("l2", 1.0)?;
    here.press("b")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert_eq!(started, ["launcher"], "and so does the chord beside it");
    here.trigger("l2", 0.0)?;
    let Ok(()) = here.fresh();

    here.press("b")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::KEY_ESC.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "b on its own is still back"
    );
    let Ok(started) = here.names();

    assert!(started.is_empty(), "b on its own opens nothing");
    let Ok(()) = here.fresh();

    here.press("left-paddle-bottom")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert!(started.is_empty(), "a job with its button taken off starts nothing");
    let Ok(()) = here.fresh();

    here.press("y")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::BTN_RIGHT.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "a job from some newer desktop does not take y from more-options"
    );
    let Ok(()) = here.fresh();

    let read = Tasks::read("[jobs]\nmenu = \"a\"\nscreenshot = \"nose + a\"\n");

    assert!(
        read.as_ref().is_err_and(|fault| fault.to_string().starts_with("screenshot: ")),
        "the fault names the line: {read:?}"
    );
    here.press("left-paddle-top")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert_eq!(started, ["launcher"], "the table already loaded is left standing");

    Ok(())
}

#[test]
fn one_press_still_does_one_thing() -> Result<(), Failure> {
    let mut here = stage()?;

    let Ok(mut said) = Tasks::none();
    let onto = Binding::read("left-paddle-top")?;
    let Ok(table) = Table::ours();
    let Ok(out_of_the_box) = every(&table);

    assert_eq!(said.add(&out_of_the_box, "guide", &onto), Ok(Moved::TookFrom("menu".to_string())));

    let read = written_and_read(&said)?;
    let Ok(table) = Table::of(&read);
    let Ok(left) = table.bindings("menu");

    assert!(
        !left.iter().any(|one| one.on == Input::Pad),
        "the menu has no button on the pad now"
    );
    assert!(
        left.iter().any(|one| one.on == Input::Keyboard && one.played() == Ok(Played::ByAPress)),
        "and taking its paddle away did not take the key someone else reaches it by"
    );
    let Ok(()) = here.bound_by(table);

    here.press("left-paddle-top")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert_eq!(
        started,
        ["mapping-panel"],
        "the paddle opens the guide, and does not also open the menu"
    );
    let Ok(()) = here.fresh();

    here.press("menu")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert_eq!(
        started,
        ["mapping-panel"],
        "a button given to the guide is added beside the one it had"
    );

    Ok(())
}
