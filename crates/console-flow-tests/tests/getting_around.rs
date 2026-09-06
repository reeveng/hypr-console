//! Somebody moves around the desktop, and is never told the wrong thing about
//! where they are.
//!
//! The second flow. Moving around is the one thing a person does before they
//! do anything else, and it is the one thing that has to be safe to try: the
//! shoulders are places and never actions, so a press that turns out to be the
//! wrong one costs a press back. What makes that a flow rather than four
//! checks is that the same four buttons mean different things in the two
//! places this walks through, the meaning is read off the compositor in the
//! moment, and the guide is a third program claiming to know all of it.
//!
//! `docs/flows.md` is the strategy this belongs to. The stage is `here`: the
//! daemon in this process against the captured devices and the real profile
//! files, so a press below travels the road a thumb's press travels, and what
//! is asserted is what the daemon decided -- which workspace it asked the
//! compositor for, which key it sent, what it started.
//!
//! What this flow hands up rather than answering: whether the compositor did
//! what it was asked, which is the device's; and whether a second chooser
//! replaces the first on the screen, which is a lock between two processes and
//! is pressed as such in `console-panel/tests/the_lock.rs`. What is answered
//! here is the daemon's half -- that the door it asks through is the one that
//! keeps.

use console_controller::means::{Job, Suits, Table, What, When, job};
use console_controller::mode::Mode;
use console_flow_tests::screens;
use console_guide::guide::{DOABLE, MENUS, Line, Section, said, sections};
use console_gamepad::jobs::{ALONE, Played};
use console_test_stages::device::Seen;
use console_test_stages::here::{Here, TURNS};
use evdev::{EventType, KeyCode};

fn to(where_: &str) -> String {
    format!("hl.dsp.focus({{workspace = \"{where_}\"}})")
}

fn carrying(where_: &str) -> String {
    format!("hl.dsp.window.move({{workspace = \"{where_}\"}})")
}

fn ours() -> Table {
    let Ok(table) = Table::ours();

    table
}

fn dispatches(here: &Here) -> Vec<String> {
    let Ok(dispatches) = here.dispatches();

    dispatches
}

fn started(here: &Here) -> Vec<String> {
    let Ok(names) = here.names();

    names
}

fn commands(here: &Here) -> Vec<Vec<String>> {
    let Ok(commands) = here.commands();

    commands.to_vec()
}

fn mode(here: &Here) -> Mode {
    let Ok(mode) = here.mode();

    mode
}

fn sent(here: &Here, kind: EventType, code: u16, value: i32) -> Seen {
    let Ok(seen) = here.sent(kind, code, value);

    seen
}

fn says(what: What) -> &'static str {
    let Ok(says) = what.says();

    says
}

fn stage() -> Here {
    let mut here = Here::new().expect("a stage");
    here.showing(screens::NOTHING_UP).expect("the desktop");
    here
}

fn guide(table: &Table) -> Vec<Section> {
    let Ok(sections) = sections(table, "");

    sections
}

fn under(guide: &[Section], title: &str) -> Vec<Line> {
    guide
        .iter()
        .filter(|section| section.title == title)
        .flat_map(|section| section.lines.clone())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Yes {
    It,
    Not,
}

fn names(lines: &[Line], button: &str, does: &str) -> Yes {
    let Ok(said) = said(button);
    let found = lines
        .iter()
        .filter(|line| line.does == does)
        .any(|line| line.button.split(" / ").any(|named| named == said));

    match found {
        true => Yes::It,
        false => Yes::Not,
    }
}

fn bare(table: &Table, mode: Mode) -> Vec<(&'static Job, String)> {
    let Ok(every) = table.every();

    every
        .filter(|(job, _)| {
            let Ok(suits) = job.when.suits(mode);

            suits == Suits::InFront
        })
        .flat_map(|(job, bound)| {
            bound
                .iter()
                .filter(|one| {
                    let Ok(played) = one.played();

                    played == Played::ByAButton && one.layer == ALONE
                })
                .map(move |one| (job, one.button.clone()))
                .collect::<Vec<(&'static Job, String)>>()
        })
        .collect()
}

fn anything(here: &Here) -> bool {
    let Ok(told) = here.told();

    !commands(here).is_empty() || !here.written.is_empty() || !told.is_empty()
}

#[test]
fn the_shoulders_carry_you_between_places_and_carry_nothing_else() {
    let mut here = stage();

    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(dispatches(&here), [to("+1")], "R1 on the desktop is the place after this one");
    assert!(here.written.is_empty(), "a shoulder is a place, so it sends nothing to the pointer");

    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(dispatches(&here), [to("+1"), to("+1")], "pressed again, it is one further on");

    here.press("l1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(
        dispatches(&here),
        [to("+1"), to("+1"), to("-1")],
        "L1 comes back one, so the walk is two forward and one back"
    );

    assert_eq!(dispatches(&here).len(), 3, "three presses asked for three moves and no more");
    assert!(started(&here).iter().all(|name| name == "hyprctl"), "a shoulder starts nothing else");
}

#[test]
fn a_trigger_held_carries_the_window_and_the_bare_shoulder_stays_out_of_it() {
    let mut here = stage();

    here.trigger("l2", 1.0).expect("a trigger");
    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(
        dispatches(&here),
        [carrying("+1")],
        "L2 held, the shoulder takes the window along"
    );
    here.trigger("l2", 0.0).expect("a trigger let go");
    here.fresh();

    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(dispatches(&here), [to("+1")], "the trigger let go, the shoulder is a place again");
}

#[test]
fn a_chooser_takes_the_shoulders_and_hands_them_back() {
    let mut here = stage();
    here.showing(screens::A_CHOOSER).expect("a chooser");
    assert_eq!(mode(&here), Mode::Tabs, "a panel over the desktop is a chooser");

    here.press("r1").expect("a shoulder");
    here.press("l1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::KEY_PAGEDOWN.0, 1),
        Seen::Yes,
        "with a chooser up, R1 is the tab after this one"
    );
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::KEY_PAGEUP.0, 1),
        Seen::Yes,
        "and L1 is the tab before it"
    );
    assert!(dispatches(&here).is_empty(), "neither of them moved you off the menu you are reading");
    here.fresh();

    here.trigger("l2", 1.0).expect("a trigger");
    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert!(
        dispatches(&here).is_empty(),
        "with a chooser up, no shoulder is a workspace, held or not"
    );
    here.trigger("l2", 0.0).expect("a trigger let go");
    here.fresh();

    here.press("legion-left").expect("the left Legion button");
    here.press("view").expect("the button with the two squares");
    here.settle(TURNS);
    assert!(started(&here).is_empty(), "with a chooser up, the desktop's own buttons start nothing");
    here.fresh();

    here.showing(screens::NOTHING_UP).expect("the desktop");
    assert_eq!(mode(&here), Mode::Desktop, "the chooser is gone");
    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(dispatches(&here), [to("+1")], "the chooser gone, R1 is a workspace in one press");
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::KEY_PAGEDOWN.0, 1),
        Seen::NotYet,
        "and it is not also a tab, so nothing was kept from the chooser"
    );
}

#[test]
fn the_guide_is_raised_from_either_place_and_reads_the_table_the_daemon_obeys() {
    let mut here = stage();
    let guide = guide(&ours());
    let anywhere = under(&guide, DOABLE);
    let menus = under(&guide, MENUS);

    here.press("r1").expect("a shoulder");
    here.press("menu").expect("the button with the lines on it");
    here.settle(TURNS);
    assert!(
        started(&here).contains(&"console-buttons".to_string()),
        "the menu button raises the guide"
    );
    assert_eq!(dispatches(&here), [to("+1")], "and raising it did not undo the move before it");
    here.fresh();

    assert_eq!(
        names(&anywhere, "r1", says(What::Workspace(1))),
        Yes::It,
        "the guide names R1 as the place after this one, which is what it just was"
    );
    assert_eq!(
        names(&menus, "r1", says(What::Tab(1))),
        Yes::It,
        "and with a chooser up it is the tab, which is what it just was there"
    );

    here.showing(screens::A_CHOOSER).expect("a chooser");
    here.press("menu").expect("the button with the lines on it");
    here.settle(TURNS);
    assert!(
        started(&here).contains(&"console-buttons".to_string()),
        "the guide is raised from inside a chooser too"
    );
}

#[test]
fn everything_the_guide_says_about_a_place_is_true_when_you_stand_in_it() {
    let table = ours();
    let guide = guide(&table);
    let desktop = under(&guide, DOABLE);
    let chooser = [under(&guide, MENUS), under(&guide, DOABLE)].concat();

    for (mode, screen, named) in [
        (Mode::Desktop, screens::NOTHING_UP, &desktop),
        (Mode::Tabs, screens::A_CHOOSER, &chooser),
    ] {
        for (job, button) in bare(&table, mode) {
            let mut here = Here::new().expect("a stage");
            here.showing(screen).expect("somewhere to stand");
            here.press(&button).expect("a button");
            here.settle(TURNS);

            assert!(
                anything(&here),
                "{mode:?}: {button} is bound to {} and the daemon did nothing about it",
                job.slug
            );
            assert_eq!(
                names(named, &button, says(job.what)),
                Yes::It,
                "{mode:?}: the guide does not say {button} is {}",
                says(job.what)
            );
        }
    }
}

#[test]
fn the_right_paddle_leaves_from_wherever_it_is_pressed() {
    let mut here = stage();
    here.showing(screens::A_CHOOSER).expect("a chooser");

    here.press("r1").expect("a shoulder");
    here.press("r1").expect("a shoulder");
    here.press("dpad-down").expect("the d-pad");
    here.press("dpad-down").expect("the d-pad");
    here.settle(TURNS);
    here.fresh();

    here.press("right-paddle-top").expect("the paddle that closes");
    here.settle(TURNS);
    assert_eq!(started(&here), ["put-away"], "deep in a panel, the paddle puts away what is up");
    assert!(
        dispatches(&here).is_empty(),
        "and it does not close the window behind the panel on the way"
    );
    here.fresh();

    here.press("b").expect("b");
    here.settle(TURNS);
    assert_eq!(
        sent(&here, EventType::KEY, KeyCode::KEY_ESC.0, 1),
        Seen::Yes,
        "b in a chooser is one step back"
    );
    assert!(started(&here).is_empty(), "one step back starts nothing");
    here.fresh();

    here.showing(screens::NOTHING_UP).expect("the desktop");
    here.press("right-paddle-top").expect("the paddle that closes");
    here.settle(TURNS);
    assert_eq!(started(&here), ["put-away"], "on the desktop it is the same one job");
}

#[test]
fn a_menu_asked_for_while_one_is_up_is_asked_for_through_the_door_that_keeps() {
    let mut here = stage();

    here.press("left-paddle-top").expect("the paddle with the menu on it");
    here.settle(TURNS);
    assert_eq!(commands(&here), [["launcher", "--keep"]], "the paddle opens the menu");
    here.fresh();

    here.showing(screens::A_CHOOSER).expect("a chooser");
    here.press("left-paddle-top").expect("the paddle with the menu on it");
    here.settle(TURNS);
    assert_eq!(
        commands(&here),
        [["launcher", "--keep"]],
        "with a menu already up, the paddle asks through the same door"
    );
    assert_eq!(started(&here).len(), 1, "and asks once, so there is one to replace the one up");
}

#[test]
fn the_walk_is_about_the_buttons_it_names() {
    let table = ours();

    for (slug, button, when) in [
        ("workspace-next", "r1", When::OnTheDesktop),
        ("workspace-previous", "l1", When::OnTheDesktop),
        ("tab-right", "r1", When::WithAChooserUp),
        ("tab-left", "l1", When::WithAChooserUp),
        ("put-away", "right-paddle-top", When::Anywhere),
        ("guide", "menu", When::Anywhere),
        ("menu", "left-paddle-top", When::Anywhere),
    ] {
        let Ok(job) = job(slug);
        let job = job.expect("a job this desktop does");
        assert_eq!(job.when, when, "{slug} applies somewhere else now");
        let Ok(bindings) = table.bindings(slug);

        assert!(
            bindings.iter().any(|one| one.button == button && one.layer == ALONE),
            "{slug} is not on {button} any more, and this flow is walking the old machine"
        );
    }
}
