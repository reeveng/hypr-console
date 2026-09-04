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
use console_flows::screens;
use console_guide::guide::{DOABLE, MENUS, Line, Section, said, sections};
use console_pad::jobs::{ALONE, Played};
use console_stage::device::Seen;
use console_stage::here::{Here, TURNS};
use evdev::{EventType, KeyCode};

/// The compositor's own words for a move, which is what the daemon hands to
/// `hyprctl dispatch` and the whole of what it decided.
fn to(where_: &str) -> String {
    format!("hl.dsp.focus({{workspace = \"{where_}\"}})")
}

/// The same, with the window coming along.
fn carrying(where_: &str) -> String {
    format!("hl.dsp.window.move({{workspace = \"{where_}\"}})")
}

/// A stage on the bare desktop, out of the box.
fn stage() -> Here {
    let mut here = Here::new().expect("a stage");
    here.showing(screens::NOTHING_UP).expect("the desktop");
    here
}

/// The guide, as the program that prints it reads it: this desktop's table,
/// and no compositor declaration, because a bind on a keyboard nobody has
/// plugged in is not a button anybody is walking with.
fn guide(table: &Table) -> Vec<Section> {
    sections(table, "")
}

/// The lines under one heading.
fn under(guide: &[Section], title: &str) -> Vec<Line> {
    guide
        .iter()
        .filter(|section| section.title == title)
        .flat_map(|section| section.lines.clone())
        .collect()
}

/// Whether the guide has a line saying it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Yes {
    It,
    Not,
}

/// Whether the guide, under these headings, names that button as doing exactly
/// that.
///
/// A line may name two buttons doing one job -- X and the button with a
/// keyboard drawn on it are not two things to learn -- so the line is asked
/// whether this button is one of the ones it names. And a button may be named
/// by more than one line under one heading, because a few of the ways of
/// driving a chooser are written beside the table's rows rather than read out
/// of them, so what is asked is whether one of the lines says this.
fn names(lines: &[Line], button: &str, does: &str) -> Yes {
    let found = lines
        .iter()
        .filter(|line| line.does == does)
        .any(|line| line.button.split(" / ").any(|named| named == said(button)));

    match found {
        true => Yes::It,
        false => Yes::Not,
    }
}

/// Every job that applies where you are standing and has a bare button on it.
///
/// A chord is left out on purpose: this walks what a thumb does on its own,
/// and the layers are the second thing a button does.
fn bare(table: &Table, mode: Mode) -> Vec<(&'static Job, String)> {
    table
        .every()
        .filter(|(job, _)| job.when.suits(mode) == Suits::InFront)
        .flat_map(|(job, bound)| {
            bound
                .iter()
                .filter(|one| one.played() == Played::ByAButton && one.layer == ALONE)
                .map(move |one| (job, one.button.clone()))
        })
        .collect()
}

/// Whether the daemon did anything at all about a press.
fn anything(here: &Here) -> bool {
    !here.commands().is_empty() || !here.written.is_empty() || !here.told().is_empty()
}

/// The shoulders carry you between places, and carry nothing else.
///
/// Two forward and one back, and what the compositor is asked for is the walk
/// in order. Asserted step by step rather than at the end: a daemon that sent
/// the right three in the wrong order is a machine that puts you somewhere you
/// did not ask for, and a list compared once cannot say which press was the
/// one that lied.
#[test]
fn the_shoulders_carry_you_between_places_and_carry_nothing_else() {
    let mut here = stage();

    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(here.dispatches(), [to("+1")], "R1 on the desktop is the place after this one");
    assert!(here.written.is_empty(), "a shoulder is a place, so it sends nothing to the pointer");

    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(here.dispatches(), [to("+1"), to("+1")], "pressed again, it is one further on");

    here.press("l1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(
        here.dispatches(),
        [to("+1"), to("+1"), to("-1")],
        "L1 comes back one, so the walk is two forward and one back"
    );

    // Three presses, three asks. A shoulder that also moved on the way back up
    // would be a machine that goes two places for one press, and the count on
    // the bar would be right only while nobody was looking.
    assert_eq!(here.dispatches().len(), 3, "three presses asked for three moves and no more");
    assert!(here.names().iter().all(|name| name == "hyprctl"), "a shoulder starts nothing else");
}

/// A trigger held turns the same shoulder into carrying the window, and the
/// bare shoulder stays out of it.
///
/// The only way to move a window somewhere else without a keyboard, and the
/// one place on this device where holding a trigger changes a job rather than
/// adding one. So the press has to be exactly one of the two: a chord that
/// also did what the bare button does would move you and then move the window
/// after you, which is the same two workspaces apart in the end and a window
/// left behind on the way.
#[test]
fn a_trigger_held_carries_the_window_and_the_bare_shoulder_stays_out_of_it() {
    let mut here = stage();

    here.trigger("l2", 1.0).expect("a trigger");
    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(
        here.dispatches(),
        [carrying("+1")],
        "L2 held, the shoulder takes the window along"
    );
    here.trigger("l2", 0.0).expect("a trigger let go");
    here.fresh();

    // Let go of the trigger and the same button is a place again, which is
    // what makes the chord safe to reach for: nothing was loaded, nothing was
    // remembered, and the meaning went back the moment the finger came off.
    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(here.dispatches(), [to("+1")], "the trigger let go, the shoulder is a place again");
}

/// A chooser takes the shoulders and hands them back.
///
/// With something up, the shoulders are the panel's tabs -- moving workspaces
/// from inside a menu would carry you off the menu you are reading -- and the
/// desktop's own buttons are nobody's for as long as it is up. Then the
/// chooser goes and every one of them is the desktop's again in the same
/// press, because what a button means is read off the screen and never
/// remembered.
#[test]
fn a_chooser_takes_the_shoulders_and_hands_them_back() {
    let mut here = stage();
    here.showing(screens::A_CHOOSER).expect("a chooser");
    assert_eq!(here.mode(), Mode::Tabs, "a panel over the desktop is a chooser");

    here.press("r1").expect("a shoulder");
    here.press("l1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::KEY_PAGEDOWN.0, 1),
        Seen::Yes,
        "with a chooser up, R1 is the tab after this one"
    );
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::KEY_PAGEUP.0, 1),
        Seen::Yes,
        "and L1 is the tab before it"
    );
    assert!(here.dispatches().is_empty(), "neither of them moved you off the menu you are reading");
    here.fresh();

    // Held with the trigger they are not workspaces either. Carrying a window
    // is the desktop's job, and the desktop is not what is in front.
    here.trigger("l2", 1.0).expect("a trigger");
    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert!(
        here.dispatches().is_empty(),
        "with a chooser up, no shoulder is a workspace, held or not"
    );
    here.trigger("l2", 0.0).expect("a trigger let go");
    here.fresh();

    // The two desktop jobs with nothing for a chooser to make of them are
    // nobody's while it is up, rather than firing behind it.
    here.press("legion-left").expect("the left Legion button");
    here.press("view").expect("the button with the two squares");
    here.settle(TURNS);
    assert!(here.names().is_empty(), "with a chooser up, the desktop's own buttons start nothing");
    here.fresh();

    // The chooser goes, and the same press is a place again.
    here.showing(screens::NOTHING_UP).expect("the desktop");
    assert_eq!(here.mode(), Mode::Desktop, "the chooser is gone");
    here.press("r1").expect("a shoulder");
    here.settle(TURNS);
    assert_eq!(here.dispatches(), [to("+1")], "the chooser gone, R1 is a workspace in one press");
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::KEY_PAGEDOWN.0, 1),
        Seen::NotYet,
        "and it is not also a tab, so nothing was kept from the chooser"
    );
}

/// The guide can be raised from either place, and says what a button does in
/// the words of the one table the daemon obeys.
///
/// The guide is the only program on this device whose whole job is to be
/// believed. What keeps it honest is that it is not written down beside the
/// daemon's table but read out of it, so the words below are asked of the
/// guide and of the press in the same test: if they ever part, one of the two
/// assertions goes red and names which.
#[test]
fn the_guide_is_raised_from_either_place_and_reads_the_table_the_daemon_obeys() {
    let mut here = stage();
    let guide = guide(&Table::ours());
    let anywhere = under(&guide, DOABLE);
    let menus = under(&guide, MENUS);

    // Raised in the middle of getting around.
    here.press("r1").expect("a shoulder");
    here.press("menu").expect("the button with the lines on it");
    here.settle(TURNS);
    assert!(
        here.names().contains(&"console-buttons".to_string()),
        "the menu button raises the guide"
    );
    assert_eq!(here.dispatches(), [to("+1")], "and raising it did not undo the move before it");
    here.fresh();

    // What it says about the shoulder that was just pressed is what the
    // shoulder just did.
    assert_eq!(
        names(&anywhere, "r1", What::Workspace(1).says()),
        Yes::It,
        "the guide names R1 as the place after this one, which is what it just was"
    );
    assert_eq!(
        names(&menus, "r1", What::Tab(1).says()),
        Yes::It,
        "and with a chooser up it is the tab, which is what it just was there"
    );

    // The guide is reachable from inside a chooser, because a person who has
    // forgotten a button has usually forgotten it while looking at something.
    here.showing(screens::A_CHOOSER).expect("a chooser");
    here.press("menu").expect("the button with the lines on it");
    here.settle(TURNS);
    assert!(
        here.names().contains(&"console-buttons".to_string()),
        "the guide is raised from inside a chooser too"
    );
}

/// Everything the guide says about a place is true when you stand in it.
///
/// The sweep under the flow: every job that applies where you are and has a
/// bare button on it is pressed there, and the guide is asked about the same
/// button in the same breath. Two ways of being lied to are shut at once -- a
/// line naming a button that does nothing, and a button doing something the
/// guide never mentions -- and neither can be shut by reading the table twice,
/// because one half of each assertion is a press.
#[test]
fn everything_the_guide_says_about_a_place_is_true_when_you_stand_in_it() {
    let table = Table::ours();
    let guide = guide(&table);
    // Where a person standing in each place reads. On the desktop that is the
    // one heading; with a chooser up it is the chooser's heading and the ones
    // it does not take over, which are under the first.
    //
    // Which is as far as this can go, and the gap is worth naming: the guide
    // has no idea where you are standing. Raised with a chooser up it still
    // opens on Anywhere, where A is a click and R1 is a workspace -- neither
    // of which is true of the screen it was raised over. `todos.md` carries
    // what would settle that; until it is settled, this asks the guide the
    // question a person would have to know to ask it.
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
                names(named, &button, job.what.says()),
                Yes::It,
                "{mode:?}: the guide does not say {button} is {}",
                job.what.says()
            );
        }
    }
}

/// The right paddle leaves whatever is up, from wherever it is pressed.
///
/// B unwinds one step at a time and the right paddle is the same promise in
/// one press, so it is the button somebody reaches for when they are deep
/// enough in a panel not to want to count the steps back. It is one job in
/// both places -- what closing means is decided by the program it starts,
/// which reads the screen for itself -- and that is what makes it the same
/// button everywhere rather than a button whose meaning arrives a beat after
/// the screen changes.
#[test]
fn the_right_paddle_leaves_from_wherever_it_is_pressed() {
    let mut here = stage();
    here.showing(screens::A_CHOOSER).expect("a chooser");

    // Deep in a panel: two tabs along and a few rows down, which is as much of
    // "deep" as this stage can be given and is exactly the presses that got
    // there.
    here.press("r1").expect("a shoulder");
    here.press("r1").expect("a shoulder");
    here.press("dpad-down").expect("the d-pad");
    here.press("dpad-down").expect("the d-pad");
    here.settle(TURNS);
    here.fresh();

    here.press("right-paddle-top").expect("the paddle that closes");
    here.settle(TURNS);
    assert_eq!(here.names(), ["put-away"], "deep in a panel, the paddle puts away what is up");
    assert!(
        here.dispatches().is_empty(),
        "and it does not close the window behind the panel on the way"
    );
    here.fresh();

    // B is the same promise counted out: one step, and a key the panel reads
    // as one step rather than a program that takes the whole thing down.
    here.press("b").expect("b");
    here.settle(TURNS);
    assert_eq!(
        here.sent(EventType::KEY, KeyCode::KEY_ESC.0, 1),
        Seen::Yes,
        "b in a chooser is one step back"
    );
    assert!(here.names().is_empty(), "one step back starts nothing");
    here.fresh();

    // On the bare desktop it is the same job on the same button, because what
    // closing means is the program's question and not the pad's.
    here.showing(screens::NOTHING_UP).expect("the desktop");
    here.press("right-paddle-top").expect("the paddle that closes");
    here.settle(TURNS);
    assert_eq!(here.names(), ["put-away"], "on the desktop it is the same one job");
}

/// A menu asked for while a menu is up is asked for through the door that
/// keeps.
///
/// One chooser at a time. The daemon's half of that is which door it asks
/// through: the paddles and buttons it reads only ever open, so the one on
/// screen goes and the new one takes its place -- `--keep` -- while the bar,
/// which is a finger's only way of putting a panel away, asks through the door
/// that closes. A daemon that asked through the closing door would make the
/// menu button a toggle, and a toggle on a button whose press arrives a beat
/// after the screen changed is a menu that opens or does not depending on how
/// fast the thumb was.
///
/// The other half -- that the one on screen actually goes, and that a second
/// process cannot draw over the first -- is a lock between processes, and
/// `console-panel/tests/the_lock.rs` presses it as one.
#[test]
fn a_menu_asked_for_while_one_is_up_is_asked_for_through_the_door_that_keeps() {
    let mut here = stage();

    here.press("left-paddle-top").expect("the paddle with the menu on it");
    here.settle(TURNS);
    assert_eq!(here.commands(), [["launcher", "--keep"]], "the paddle opens the menu");
    here.fresh();

    // Again, with one already up, and it is the same ask rather than a
    // different one.
    here.showing(screens::A_CHOOSER).expect("a chooser");
    here.press("left-paddle-top").expect("the paddle with the menu on it");
    here.settle(TURNS);
    assert_eq!(
        here.commands(),
        [["launcher", "--keep"]],
        "with a menu already up, the paddle asks through the same door"
    );
    assert_eq!(here.names().len(), 1, "and asks once, so there is one to replace the one up");
}

/// The jobs this flow walks are the ones the table says they are.
///
/// Not a step of the flow: a guard on the walk itself. Every assertion above
/// names a button in the words on the machine, and a button that quietly
/// stopped being bound to what this flow thinks it is would leave the walk
/// still green and no longer about anything.
#[test]
fn the_walk_is_about_the_buttons_it_names() {
    let table = Table::ours();

    for (slug, button, when) in [
        ("workspace-next", "r1", When::OnTheDesktop),
        ("workspace-previous", "l1", When::OnTheDesktop),
        ("tab-right", "r1", When::WithAChooserUp),
        ("tab-left", "l1", When::WithAChooserUp),
        ("put-away", "right-paddle-top", When::Anywhere),
        ("guide", "menu", When::Anywhere),
        ("menu", "left-paddle-top", When::Anywhere),
    ] {
        let job = job(slug).expect("a job this desktop does");
        assert_eq!(job.when, when, "{slug} applies somewhere else now");
        assert!(
            table.bindings(slug).iter().any(|one| one.button == button && one.layer == ALONE),
            "{slug} is not on {button} any more, and this flow is walking the old machine"
        );
    }
}
