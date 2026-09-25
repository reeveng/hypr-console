//! What the desktop's shape promises, asked of what the compiler saw.
//!
//! Each rule is a question about the whole tree that no crate can answer about
//! itself, and each is a ratchet: what breaks it today is named below with the
//! reason it is allowed to, and a name that stops breaking it is red too, so
//! the list only ever gets shorter. A crate that is fixed takes its line out
//! of the list in the same commit.
//!
//! The facts are `docs/architecture/facts.jsonl`, collected by `just map`. A
//! test crate's own code is not in them -- the lint leaves test builds out --
//! and neither is a crate's use of its own definitions, so "names" below means
//! a crate reaching across into another one.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use console_architecture::Architecture;
use console_architecture::facts::{self, Kind};

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    from.canonicalize().unwrap_or(from)
}

fn architecture() -> Architecture {
    Architecture::read(&root()).expect("the facts, the manifest and the units")
}

fn ratchet(rule: &str, found: BTreeSet<String>, excused: &[(&str, &str)]) {
    let excused: BTreeSet<String> = excused.iter().map(|(named, _why)| String::from(*named)).collect();
    let breaking: Vec<&String> = found.difference(&excused).collect();
    let mended: Vec<&String> = excused.difference(&found).collect();

    assert!(breaking.is_empty(), "{rule}: {breaking:?}");
    assert!(
        mended.is_empty(),
        "{rule} holds now for {mended:?}; take them out of the list of exceptions so it cannot come back"
    );
}

const COMPOSITOR_EVENTS_EXCUSED: [(&str, &str); 2] = [
    ("console-events", "the pool: the one connection to the compositor's events, told to everyone else"),
    (
        "console-onscreen",
        "`console_onscreen::events` is where the pool asks the socket's place; it spells a path and opens nothing",
    ),
];

#[test]
fn nothing_but_the_pool_opens_the_compositors_events() {
    let Ok(found) = facts::whose(&architecture().facts, Kind::Socket, "Events");

    ratchet(
        "these reach for the compositor's event socket themselves; subscribe to Topic::Compositor through \
         console_events instead",
        found,
        &COMPOSITOR_EVENTS_EXCUSED,
    );
}

const COMPOSITOR: &str = "console-compositor";

const TEST_FAMILY: &str = "console-test-";

const HYPRCTL_EXCUSED: [(&str, &str); 2] = [
    (
        "console-input-controller",
        "a button's dispatch is handed back as an `Effect::Run` of `hyprctl dispatch` for the loop to carry out, \
         so every decision can be asked twice without a compositor; the way out is an effect that names a \
         `console_compositor::Request` rather than a program",
    ),
    (
        "console-settings",
        "the size tab reads the screens through the card's own `asked`, which runs a program by name and keeps \
         its answer; it wants `console_compositor::query(Query::Monitors)` and has not been moved onto it",
    ),
];

#[test]
fn nothing_but_the_compositor_crate_runs_hyprctl() {
    let Ok(found) = facts::whose(&architecture().facts, Kind::Runs, "Hyprctl");
    let found = found.into_iter().filter(|package| package != COMPOSITOR && !package.starts_with(TEST_FAMILY)).collect();

    ratchet(
        "these run hyprctl themselves; ask through console_compositor, which keeps hyprctl's words and \
         answers in the desktop's",
        found,
        &HYPRCTL_EXCUSED,
    );
}

const SOURCELESS: [(&str, &str); 1] = [(
    "Units",
    "systemd says a unit changed only to a connection holding `Subscribe` open, and a monitor never asks; \
     what it wants is a program that subscribes and stays, which is not a line in sources.rs \
     (console-events/src/sources.rs says the rest)",
)];

#[test]
fn every_topic_the_pool_serves_has_a_source() {
    let architecture = architecture();
    let Ok(handled) = facts::said(&architecture.facts, Kind::Handles);
    let Ok(sourced) = facts::said(&architecture.facts, Kind::Sources);

    assert!(
        !handled.is_empty(),
        "no topic was seen handled in console_events::sources::hold, so this rule would be asking nothing; \
         the lint has lost the function"
    );

    ratchet(
        "these topics can be subscribed to and nothing in console_events::sources::hold says anything on them",
        handled.difference(&sourced).cloned().collect(),
        &SOURCELESS,
    );
}

const TICKING: [(&str, &str); 6] = [
    (
        "console-music",
        "`music-bar` draws its line again every ten seconds underneath its Player and Compositor \
         subscriptions; a tick, the same shape as the bar's",
    ),
    (
        "console-wallpaper",
        "the sky changes with the hour, and the wait is for the next change the sky itself named rather than \
         a reading of the compositor taken again",
    ),
    (
        "console-status-bar",
        "`watch::tick` keeps two clocks, each for something no source says: the battery every thirty seconds, \
         because the firmware is not obliged to raise a uevent per percent, and the network every sixty, because \
         `nmcli monitor` never says how strong the signal is. When both go this line goes with them",
    ),
    (
        "console-notifications",
        "the clock is a notification's own expiry, a deadline the notification named rather than a reading \
         taken again; the rule cannot tell a deadline from a tick",
    ),
    (
        "console-program-runtime",
        "the loop that carries out the timers and subscriptions other programs ask for; the clock is theirs",
    ),
    (
        "console-input-controller",
        "the profile loader asks the bus again every second until it answers, which is waiting for a thing to \
         come up rather than reading a topic again",
    ),
];

#[test]
fn nothing_both_subscribes_and_keeps_a_clock() {
    let architecture = architecture();
    let Ok(subscribing) = facts::packages(&architecture.facts, Kind::Subscribes);
    let Ok(timing) = facts::packages(&architecture.facts, Kind::Timer);
    let found = subscribing.keys().filter(|package| timing.contains_key(*package)).cloned().collect();

    ratchet(
        "these subscribe to a topic and also keep a clock of their own; a reading the pool tells about needs no \
         tick behind it",
        found,
        &TICKING,
    );
}
