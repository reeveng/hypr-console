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

use console_controller::means::Table;
use console_controller::mode::Mode;
use console_door::Said;
use console_flows::screens;
use console_pad::jobs::{Binding, Jobs, Moved, Played};
use console_stage::device::Seen;
use console_stage::here::{Here, TURNS};

/// A stage on the bare desktop, out of the box.
fn stage() -> Here {
    let mut here = Here::new().expect("a stage");
    here.showing(screens::NOTHING_UP).expect("the desktop");
    here
}

/// What each job is bound to now, defaults and all, which is what the setup
/// screen works a move out against.
fn every(table: &Table) -> BTreeMap<String, Vec<Binding>> {
    table.every().map(|(job, bound)| (job.slug.to_string(), bound.to_vec())).collect()
}

/// The screenshot is moved off its paddle and onto R2 + A, and only the
/// screenshot is different afterwards.
///
/// The move is made the way the setup screen makes it -- worked out against
/// what everything is bound to, written to the file, and the file read back --
/// so the road from a row on the setup screen to a thumb on the trigger is
/// walked whole rather than joined in the middle.
#[test]
fn moving_a_job_moves_it_and_nothing_else() {
    let mut here = stage();

    // Out of the box, L2 with the bottom-right paddle takes a screenshot.
    here.trigger("l2", 1.0).expect("a trigger");
    here.press("right-paddle-bottom").expect("a paddle");
    here.settle(TURNS);
    assert!(
        here.names().contains(&"console-screenshot".to_string()),
        "out of the box, l2 + right-paddle-bottom is the screenshot"
    );
    here.trigger("l2", 0.0).expect("a trigger let go");
    here.fresh();

    // Somebody moves it onto R2 + A, and what the daemon is handed is what
    // the setup screen wrote, read back.
    let mut said = Jobs::none();
    let onto = Binding::read("r2 + a").expect("a binding");
    assert_eq!(said.moving(&every(&Table::ours()), "screenshot", &onto), Moved::Onto);
    let read = Jobs::read(&said.written()).expect("what the setup screen wrote reads back");
    here.bound_by(Table::of(&read));

    // The new chord takes the picture, and takes nothing else with it: A is
    // in that chord, and a press that also clicked would be one press doing
    // two things.
    here.trigger("r2", 1.0).expect("a trigger");
    here.press("a").expect("a");
    here.settle(TURNS);
    assert!(
        here.names().contains(&"console-screenshot".to_string()),
        "moved onto r2 + a, the screenshot is taken there"
    );
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1),
        Seen::NotYet,
        "the chord that takes the picture does not also click"
    );
    here.trigger("r2", 0.0).expect("a trigger let go");
    here.fresh();

    // The old chord no longer takes one. What the paddle does bare -- turn
    // the wheel a notch -- is what it goes on doing under L2, because a
    // button with no second job keeps doing its first one.
    here.trigger("l2", 1.0).expect("a trigger");
    here.press("right-paddle-bottom").expect("a paddle");
    here.settle(TURNS);
    assert!(
        !here.names().contains(&"console-screenshot".to_string()),
        "the screenshot has left the paddle it was moved off"
    );
    assert!(
        here.wrote(EventType::RELATIVE, evdev::RelativeAxisCode::REL_WHEEL.0) < 0,
        "bare of its second job, the paddle goes on scrolling the page"
    );
    here.trigger("l2", 0.0).expect("a trigger let go");
    here.fresh();

    // And A on its own is still the pointer's button.
    here.press("a").expect("a");
    here.settle(TURNS);
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1),
        Seen::Yes,
        "a on its own is still a click"
    );
    assert!(here.names().is_empty(), "a click starts nothing");
}

/// The moved chord means the same thing everywhere a person can stand.
///
/// A job bound `Anywhere` promises to mean one thing wherever you are, and a
/// move must not cost it that. So the desktop is walked after the move: a
/// chooser is put up, the home screen is stood on, the keyboard is raised --
/// and at every stop the chord still takes the picture, the place's own
/// buttons still mean what the place says, and under the keyboard nothing is
/// acted on at all.
#[test]
fn the_move_holds_wherever_a_person_goes() {
    let mut here = stage();
    let said = Jobs::read("[jobs]\nscreenshot = \"r2 + a\"\n").expect("a table");
    here.bound_by(Table::of(&said));

    let shot = |here: &mut Here| {
        here.trigger("r2", 1.0).expect("a trigger");
        here.press("a").expect("a");
        here.settle(TURNS);
        let taken = here.names().contains(&"console-screenshot".to_string());
        here.trigger("r2", 0.0).expect("a trigger let go");
        here.fresh();
        taken
    };

    // On the bare desktop.
    assert!(shot(&mut here), "on the desktop, the moved chord takes the picture");

    // With a chooser up, where A bare takes the row and the shoulders turn
    // tabs rather than moving workspaces.
    here.showing(screens::A_CHOOSER).expect("a chooser");
    assert!(shot(&mut here), "with a chooser up, the moved chord still takes the picture");
    here.press("a").expect("a");
    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::KEY_ENTER.0, 1),
        Seen::Yes,
        "bare a with a chooser up takes the row it is standing on"
    );
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::KEY_PAGEDOWN.0, 1),
        Seen::Yes,
        "a shoulder with a chooser up is the tab beside this one"
    );
    assert!(here.dispatches().is_empty(), "with a chooser up, a shoulder is not a workspace");
    here.fresh();

    // On the home screen, asleep and then standing on a square. The first
    // d-pad press wakes it; A is the square's while the highlight is up; the
    // moved chord still cuts through and takes the picture; B puts the
    // highlight away and A is the pointer's again.
    here.showing(screens::THE_HOME_SCREEN).expect("the home screen");
    assert_eq!(here.mode(), Mode::Home, "the home screen is drawn and asleep");
    here.press("dpad-right").expect("the d-pad");
    here.settle(TURNS);
    assert!(
        here.told().contains(&Said::Right),
        "the first d-pad press is a word to the home screen"
    );
    assert_eq!(here.mode(), Mode::Standing, "the word woke it");
    here.fresh();

    here.press("a").expect("a");
    here.settle(TURNS);
    assert!(
        here.told().contains(&Said::Pressing) && here.told().contains(&Said::Released),
        "standing on a square, both halves of a are the square's"
    );
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1),
        Seen::NotYet,
        "standing on a square, a is not a click"
    );
    here.fresh();

    assert!(shot(&mut here), "standing on a square, the moved chord still takes the picture");

    here.press("b").expect("b");
    here.settle(TURNS);
    assert!(here.told().contains(&Said::Back), "b puts the highlight away");
    assert_eq!(here.mode(), Mode::Home, "the home screen is asleep again");
    here.fresh();

    here.press("a").expect("a");
    here.settle(TURNS);
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1),
        Seen::Yes,
        "asleep again, a is the pointer's button"
    );
    here.fresh();

    // Under the keyboard nothing is acted on, moved or not: somebody else is
    // reading the pad.
    here.showing(screens::THE_KEYBOARD).expect("the keyboard");
    here.trigger("r2", 1.0).expect("a trigger");
    here.press("a").expect("a");
    here.settle(TURNS);
    assert!(here.names().is_empty(), "under the keyboard, the chord starts nothing");
    assert!(here.told().is_empty(), "under the keyboard, nothing is said to the home screen");
    assert_eq!(
        here.wrote(EventType::KEY, KeyCode::BTN_LEFT.0),
        0,
        "under the keyboard, nothing reaches the pointer"
    );
}

/// Everything the file can say, said, and every saying lands.
///
/// One job on several buttons, a job on a chord that leaves the bare button
/// alone, a job with its button taken off, a job this desktop has never heard
/// of, and a file with a fault in it -- each of them is a line somebody could
/// write, and each is answered the way the file's own rules promise.
#[test]
fn the_file_says_several_buttons_a_chord_or_nothing_at_all() {
    let mut here = stage();
    let said = Jobs::read(
        "[jobs]\nmenu = [\"left-paddle-top\", \"l2 + b\"]\ndictate = \"\"\nteleport = \"y\"\n",
    )
    .expect("a table");
    here.bound_by(Table::of(&said));

    // One job, two ways to play it: the paddle it was always on, and a chord.
    here.press("left-paddle-top").expect("a paddle");
    here.settle(TURNS);
    assert_eq!(here.names(), ["launcher"], "the paddle still opens the menu");
    here.fresh();

    here.trigger("l2", 1.0).expect("a trigger");
    here.press("b").expect("b");
    here.settle(TURNS);
    assert_eq!(here.names(), ["launcher"], "and so does the chord beside it");
    here.trigger("l2", 0.0).expect("a trigger let go");
    here.fresh();

    // The chord did not take the bare button: B on its own is still the way
    // back out of things.
    here.press("b").expect("b");
    here.settle(TURNS);
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::KEY_ESC.0, 1),
        Seen::Yes,
        "b on its own is still back"
    );
    assert!(here.names().is_empty(), "b on its own opens nothing");
    here.fresh();

    // A job somebody took the button off plays nothing, and is not back where
    // it started.
    here.press("left-paddle-bottom").expect("a paddle");
    here.settle(TURNS);
    assert!(here.names().is_empty(), "a job with its button taken off starts nothing");
    here.fresh();

    // A job this desktop has never heard of is left alone rather than argued
    // with, and the button it named goes on doing its own work.
    here.press("y").expect("y");
    here.settle(TURNS);
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::BTN_RIGHT.0, 1),
        Seen::Yes,
        "a job from some newer desktop does not take y from more-options"
    );
    here.fresh();

    // A file with one bad line does not come back as a file with the rest of
    // its lines, and the fault says which line. The daemon's rule is the
    // other half -- a table that will not read leaves the one already loaded
    // standing -- and here that is the table above, still answering.
    let fault = Jobs::read("[jobs]\nmenu = \"a\"\nscreenshot = \"nose + a\"\n")
        .expect_err("nose is not a trigger");
    assert!(fault.starts_with("screenshot: "), "the fault names the line: {fault}");
    here.press("left-paddle-top").expect("a paddle");
    here.settle(TURNS);
    assert_eq!(here.names(), ["launcher"], "the table already loaded is left standing");
}

/// One press still does one thing: a button moved onto is a button taken.
///
/// On a machine where every button worth pressing already does something,
/// refusing a move onto a taken button would be a setup screen that cannot be
/// used. So the button goes to the job being moved, the job that had it is
/// left saying so, and the press afterwards has exactly one answer.
#[test]
fn one_press_still_does_one_thing() {
    let mut here = stage();

    // The guide is moved onto the menu's paddle, and the move says whose
    // button it took.
    let mut said = Jobs::none();
    let onto = Binding::read("left-paddle-top").expect("a binding");
    assert_eq!(
        said.moving(&every(&Table::ours()), "guide", &onto),
        Moved::TookFrom("menu".to_string())
    );

    // What was written reads back whole, and the job that lost its button is
    // written down as playing nothing rather than left out.
    let read = Jobs::read(&said.written()).expect("what the setup screen wrote reads back");
    let table = Table::of(&read);
    assert_eq!(table.bindings("menu").len(), 1);
    assert_eq!(table.bindings("menu")[0].played(), Played::ByNothing);
    here.bound_by(table);

    // The paddle has one answer now: the guide, and only the guide.
    here.press("left-paddle-top").expect("a paddle");
    here.settle(TURNS);
    assert_eq!(
        here.names(),
        ["console-buttons"],
        "the paddle opens the guide, and does not also open the menu"
    );
    here.fresh();

    // And the button the guide came off plays nothing, because the guide was
    // moved and not copied.
    here.press("menu").expect("the button with the lines on it");
    here.settle(TURNS);
    assert!(here.names().is_empty(), "the guide's old button was left playing nothing");
}
