//! Someone moves around the desktop, and is never told the wrong thing about
//! where they are.
//!
//! The second flow. Moving around is the one thing a person does before they
//! do anything else, and it is the one thing that has to be safe to try: the
//! shoulders are places and never effects, so a press that turns out to be the
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
//! what it was asked, which is the device's; and whether a second picker
//! replaces the first on the screen, which is a lock between two processes and
//! is pressed as such in `console-panel/tests/the_lock.rs`. What is answered
//! here is the daemon's half -- that the door it asks through is the one that
//! keeps.

use std::error::Error;

use console_core_never::Never;
use console_input_controller::actions::{Task, Applicability, Table, Action, Context, job};
use console_input_controller::mode::Mode;
use console_test_flows::screens;
use console_button_guide::guide::{DOABLE, MENUS, Line, Section, opens_on, button_label, sections};
use console_input_bindings::bound::{Input, Played};
use console_test_stages::device::Ready;
use console_test_stages::here::{Here, TURNS};
use console_input_event_devices::{EventType, KeyCode};

type Failure = Box<dyn Error>;

const NEXT: &str = "hl.dsp.focus({workspace = \"+1\"})";
const PREVIOUS: &str = "hl.dsp.focus({workspace = \"-1\"})";
const CARRIED_TO_THE_NEXT: &str = "hl.dsp.window.move({workspace = \"+1\"})";

fn stage() -> Result<Here, Failure> {
    let mut here = Here::new()?;

    here.set_layers(screens::NOTHING_UP)?;

    Ok(here)
}

fn as_read(guide: &[Section], mode: Mode) -> Result<Vec<Line>, Never> {
    let Ok(first) = opens_on(Input::Pad, mode);
    let Ok(mut lines) = under(guide, first);

    match first == DOABLE {
        true => {},
        false => {
            let Ok(doable) = under(guide, DOABLE);

            lines.extend(doable);
        },
    }

    Ok(lines)
}

fn under(guide: &[Section], title: &str) -> Result<Vec<Line>, Never> {
    Ok(guide
        .iter()
        .filter(|section| section.title == title)
        .flat_map(|section| section.lines.clone())
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Yes {
    It,
    Not,
}

fn names(lines: &[Line], (button, does): (&str, &str)) -> Result<Yes, Never> {
    let Ok(said) = button_label(button);
    let found = lines
        .iter()
        .filter(|line| line.does == does)
        .any(|line| line.button.split(" / ").any(|named| named == said));

    match found {
        true => Ok(Yes::It),
        false => Ok(Yes::Not),
    }
}

fn bare(table: &Table, mode: Mode) -> Result<Vec<(&'static Task, String)>, Never> {
    let Ok(every) = table.every();

    Ok(every
        .filter(|(job, _)| {
            let Ok(applicability) = job.context.applicability(mode);

            applicability == Applicability::InFront
        })
        .flat_map(|(job, bound)| {
            bound
                .iter()
                .filter(|one| {
                    let Ok(played) = one.played();

                    played == Played::ByAPress
                        && one.on == Input::Pad
                        && one.held.is_empty()
                })
                .map(move |one| (job, one.pressed.clone()))
                .collect::<Vec<(&'static Task, String)>>()
        })
        .collect())
}

fn anything(here: &Here) -> Result<Yes, Never> {
    let Ok(told) = here.pad_inputs();
    let Ok(commands) = here.commands();

    match commands.is_empty() && here.written.is_empty() && told.is_empty() {
        true => Ok(Yes::Not),
        false => Ok(Yes::It),
    }
}

#[test]
fn the_shoulders_carry_you_between_places_and_carry_nothing_else() -> Result<(), Failure> {
    let mut here = stage()?;

    here.press("r1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(dispatches) = here.dispatches();

    assert_eq!(dispatches, [NEXT], "R1 on the desktop is the place after this one");
    assert!(here.written.is_empty(), "a shoulder is a place, so it sends nothing to the pointer");

    here.press("r1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(dispatches) = here.dispatches();

    assert_eq!(dispatches, [NEXT, NEXT], "pressed again, it is one further on");

    here.press("l1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(dispatches) = here.dispatches();

    assert_eq!(
        dispatches,
        [NEXT, NEXT, PREVIOUS],
        "L1 comes back one, so the walk is two forward and one back"
    );

    let Ok(dispatches) = here.dispatches();

    assert_eq!(dispatches.len(), 3, "three presses asked for three moves and no more");
    let Ok(started) = here.names();

    assert!(started.iter().all(|name| name == "hyprctl"), "a shoulder starts nothing else");

    Ok(())
}

#[test]
fn both_triggers_held_carry_the_window_and_the_bare_shoulder_stays_out_of_it() -> Result<(), Failure> {
    let mut here = stage()?;

    here.trigger("l2", 1.0)?;
    here.trigger("r2", 1.0)?;
    here.press("r1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(dispatches) = here.dispatches();

    assert_eq!(
        dispatches,
        [CARRIED_TO_THE_NEXT],
        "both triggers held, the shoulder takes the window along"
    );
    here.trigger("r2", 0.0)?;
    let Ok(()) = here.fresh();

    here.press("r1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(dispatches) = here.dispatches();

    assert_eq!(dispatches, [NEXT], "L2 alone is the desktops, and the window stays where it was");
    here.trigger("l2", 0.0)?;
    let Ok(()) = here.fresh();

    here.press("r1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(dispatches) = here.dispatches();

    assert_eq!(dispatches, [NEXT], "the trigger let go, the shoulder is a place again");

    Ok(())
}

#[test]
fn a_picker_takes_the_shoulders_and_hands_them_back() -> Result<(), Failure> {
    let mut here = stage()?;
    here.set_layers(screens::A_PICKER)?;
    let Ok(mode) = here.mode();

    assert_eq!(mode, Mode::Tabs, "a panel over the desktop is a picker");

    here.press("r1")?;
    here.press("l1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::KEY_PAGEDOWN.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "with a picker up, R1 is the tab after this one"
    );
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::KEY_PAGEUP.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "and L1 is the tab before it"
    );
    let Ok(dispatches) = here.dispatches();

    assert!(dispatches.is_empty(), "neither of them moved you off the menu you are reading");
    let Ok(()) = here.fresh();

    here.trigger("l2", 1.0)?;
    here.press("r1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(dispatches) = here.dispatches();

    assert!(
        dispatches.is_empty(),
        "with a picker up, no shoulder is a workspace, held or not"
    );
    here.trigger("l2", 0.0)?;
    let Ok(()) = here.fresh();

    here.press("legion-left")?;
    here.press("view")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert!(started.is_empty(), "with a picker up, the desktop's own buttons start nothing");
    let Ok(()) = here.fresh();

    here.set_layers(screens::NOTHING_UP)?;
    let Ok(mode) = here.mode();

    assert_eq!(mode, Mode::Desktop, "the picker is gone");
    here.press("r1")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(dispatches) = here.dispatches();

    assert_eq!(dispatches, [NEXT], "the picker gone, R1 is a workspace in one press");
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::KEY_PAGEDOWN.0, 1);

    assert_eq!(
        sent,
        Ready::NotYet,
        "and it is not also a tab, so nothing was kept from the picker"
    );

    Ok(())
}

#[test]
fn the_guide_is_raised_from_either_place_and_reads_the_table_the_daemon_obeys() -> Result<(), Failure> {
    let mut here = stage()?;
    let Ok(table) = Table::ours();
    let Ok(guide) = sections(&table);
    let Ok(anywhere) = under(&guide, DOABLE);
    let Ok(menus) = under(&guide, MENUS);

    here.press("r1")?;
    here.press("menu")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert!(
        started.contains(&"mapping-panel".to_string()),
        "the menu button raises the guide"
    );
    let Ok(dispatches) = here.dispatches();

    assert_eq!(dispatches, [NEXT], "and raising it did not undo the move before it");
    let Ok(()) = here.fresh();
    let Ok(workspace) = Action::Workspace(1).says();
    let Ok(named) = names(&anywhere, ("r1", workspace));

    assert_eq!(
        named,
        Yes::It,
        "the guide names R1 as the place after this one, which is what it just was"
    );

    let Ok(tab) = Action::Tab(1).says();
    let Ok(named) = names(&menus, ("r1", tab));

    assert_eq!(
        named,
        Yes::It,
        "and with a picker up it is the tab, which is what it just was there"
    );

    here.set_layers(screens::A_PICKER)?;
    here.press("menu")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert!(
        started.contains(&"mapping-panel".to_string()),
        "the guide is raised from inside a picker too"
    );
    let Ok(mode) = here.mode();

    assert_eq!(
        opens_on(Input::Pad, mode),
        Ok(MENUS),
        "raised over a picker it opened on the tab where A is a click and R1 is a workspace"
    );

    Ok(())
}

#[test]
fn everything_the_guide_says_about_a_place_is_true_when_you_stand_in_it() -> Result<(), Failure> {
    let Ok(table) = Table::ours();
    let Ok(guide) = sections(&table);
    let Ok(desktop) = as_read(&guide, Mode::Desktop);
    let Ok(picker) = as_read(&guide, Mode::Tabs);

    for (mode, screen, named) in [
        (Mode::Desktop, screens::NOTHING_UP, &desktop),
        (Mode::Tabs, screens::A_PICKER, &picker),
    ] {
        let Ok(bare) = bare(&table, mode);

        for (job, button) in bare {
            let mut here = Here::new()?;

            here.set_layers(screen)?;
            here.press(&button)?;
            let Ok(()) = here.settle(TURNS);
            let Ok(anything) = anything(&here);

            assert_eq!(
                anything,
                Yes::It,
                "{mode:?}: {button} is bound to {} and the daemon did nothing about it",
                job.slug
            );

            let Ok(says) = job.action.says();
            let Ok(named) = names(named, (&button, says));

            assert_eq!(named, Yes::It, "{mode:?}: the guide does not say {button} is {says}");
        }
    }

    Ok(())
}

#[test]
fn the_right_paddle_leaves_from_wherever_it_is_pressed() -> Result<(), Failure> {
    let mut here = stage()?;
    here.set_layers(screens::A_PICKER)?;

    here.press("r1")?;
    here.press("r1")?;
    here.press("dpad-down")?;
    here.press("dpad-down")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(()) = here.fresh();

    here.press("right-paddle-top")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert_eq!(started, ["console-put-away"], "deep in a panel, the paddle puts away what is up");
    let Ok(dispatches) = here.dispatches();

    assert!(
        dispatches.is_empty(),
        "and it does not close the window behind the panel on the way"
    );
    let Ok(()) = here.fresh();

    here.press("b")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(sent) = here.check_sent(EventType::KEY, KeyCode::KEY_ESC.0, 1);

    assert_eq!(
        sent,
        Ready::Yes,
        "b in a picker is one step back"
    );
    let Ok(started) = here.names();

    assert!(started.is_empty(), "one step back starts nothing");
    let Ok(()) = here.fresh();

    here.set_layers(screens::NOTHING_UP)?;
    here.press("right-paddle-top")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(started) = here.names();

    assert_eq!(started, ["console-put-away"], "on the desktop it is the same one job");

    Ok(())
}

#[test]
fn a_menu_asked_for_while_one_is_up_is_asked_for_through_the_door_that_keeps() -> Result<(), Failure> {
    let mut here = stage()?;

    here.press("left-paddle-top")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(commands) = here.commands();

    assert_eq!(commands, [["launcher", "--keep"]], "the paddle opens the menu");
    let Ok(()) = here.fresh();

    here.set_layers(screens::A_PICKER)?;
    here.press("left-paddle-top")?;
    let Ok(()) = here.settle(TURNS);
    let Ok(commands) = here.commands();

    assert_eq!(
        commands,
        [["launcher", "--keep"]],
        "with a menu already up, the paddle asks through the same door"
    );
    let Ok(started) = here.names();

    assert_eq!(started.len(), 1, "and asks once, so there is one to replace the one up");

    Ok(())
}

#[test]
fn the_walk_is_about_the_buttons_it_names() -> Result<(), Failure> {
    let Ok(table) = Table::ours();

    for (slug, button, when) in [
        ("workspace-next", "r1", Context::OnTheDesktop),
        ("workspace-previous", "l1", Context::OnTheDesktop),
        ("tab-right", "r1", Context::WithAPanelOrApp),
        ("tab-left", "l1", Context::WithAPanelOrApp),
        ("put-away", "right-paddle-top", Context::Anywhere),
        ("guide", "menu", Context::Anywhere),
        ("menu", "left-paddle-top", Context::Anywhere),
    ] {
        let Ok(job) = job(slug);

        assert_eq!(job.map(|job| job.context), Some(when), "{slug} applies somewhere else now");

        let Ok(bindings) = table.bindings(slug);

        assert!(
            bindings
                .iter()
                .any(|one| one.pressed == button && one.held.is_empty() && one.on == Input::Pad),
            "{slug} is not on {button} any more, and this flow is walking the old machine"
        );
    }

    Ok(())
}
