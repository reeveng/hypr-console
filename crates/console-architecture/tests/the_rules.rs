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

use console_architecture::Architecture;
use console_architecture::facts::{self, Kind};
use console_core_never::Never;

type Failure = Box<dyn std::error::Error>;

fn architecture() -> Result<Architecture, Failure> {
  let root = console_repository::root()?;
  let architecture = Architecture::read(&root)?;

  Ok(architecture)
}

#[test]
fn portable_packages_are_checked_for_direct_and_linked_host_access() -> Result<(), Failure> {
  let collected = facts::read(concat!(
    "{\"package\":\"decoder\",\"target\":\"lib\",\"kind\":\"host-access\",\"what\":\"clock std::time::Instant::now\"}\n",
    "{\"package\":\"decoder\",\"target\":\"lib\",\"kind\":\"links\",\"what\":\"helper\"}\n",
    "{\"package\":\"decoder\",\"target\":\"lib\",\"kind\":\"links\",\"what\":\"pixels\"}\n",
    "{\"package\":\"decoder\",\"target\":\"lib\",\"kind\":\"links\",\"what\":\"console_operating_system_linux\"}\n",
    "{\"package\":\"console-operating-system-linux\",\"target\":\"lib\",\"kind\":\"builds\",\"what\":\"console_operating_system_linux\"}\n",
    "{\"package\":\"helper\",\"target\":\"lib\",\"kind\":\"builds\",\"what\":\"helper\"}\n",
    "{\"package\":\"helper\",\"target\":\"lib\",\"kind\":\"host-access\",\"what\":\"filesystem std::fs::read\"}\n",
    "{\"package\":\"pixels\",\"target\":\"lib\",\"kind\":\"builds\",\"what\":\"pixels\"}\n",
  ))?;
  let portable = BTreeSet::from([String::from("decoder"), String::from("pixels")]);
  let Ok(found) = facts::portable_violations(&collected, &portable);

  assert_eq!(
    found,
    BTreeSet::from([
      String::from("decoder (lib) reaches clock std::time::Instant::now"),
      String::from("decoder (lib) links to helper, which supplies or uses host services"),
      String::from(
        "decoder (lib) links to console-operating-system-linux, which supplies or uses host services"
      ),
    ]),
  );

  Ok(())
}

fn ratchet(rule: &str, found: BTreeSet<String>, excused: &[(&str, &str)]) -> Result<(), Never> {
  let excused_2: BTreeSet<String> = excused
    .iter()
    .map(|(named, _why)| String::from(*named))
    .collect();
  let breaking: Vec<&String> = found.difference(&excused_2).collect();
  let mended: Vec<&String> = excused_2.difference(&found).collect();

  assert!(breaking.is_empty(), "{rule}: {breaking:?}");
  assert!(
    mended.is_empty(),
    "{rule} holds now for {mended:?}; take them out of the list of exceptions so it cannot come back"
  );

  Ok(())
}

const COMPOSITOR_EVENTS_EXCUSED: [(&str, &str); 2] = [
  (
    "console-events",
    "the pool: the one connection to the compositor's events, told to everyone else",
  ),
  (
    "console-onscreen",
    "`console_onscreen::events` is where the pool asks the socket's place; it spells a path and opens nothing",
  ),
];

#[test]
fn nothing_but_the_pool_opens_the_compositors_events() -> Result<(), Failure> {
  let architecture = architecture()?;
  let Ok(found) = facts::whose(&architecture.facts, Kind::Socket, "Events");

  let Ok(()) = ratchet(
    "these reach for the compositor's event socket themselves; subscribe to EventGroup::Compositor through \
         console_events instead",
    found,
    &COMPOSITOR_EVENTS_EXCUSED,
  );

  Ok(())
}

const COMPOSITOR: &str = "console-compositor";

const TEST_FAMILY: &str = "console-test-";

#[test]
fn nothing_but_the_compositor_crate_runs_hyprctl() -> Result<(), Failure> {
  let architecture = architecture()?;
  let Ok(found) = facts::whose(&architecture.facts, Kind::Runs, "Hyprctl");
  let found_2 = found
    .into_iter()
    .filter(|package| package != COMPOSITOR && !package.starts_with(TEST_FAMILY))
    .collect();

  let Ok(()) = ratchet(
    "these run hyprctl themselves; ask through console_compositor, which keeps hyprctl's words and \
         answers in the desktop's",
    found_2,
    &[],
  );

  Ok(())
}

const SOURCELESS: [(&str, &str); 1] = [(
  "Units",
  "systemd says a unit changed only to a connection holding `Subscribe` open, and a monitor never asks; \
     what it wants is a program that subscribes and stays, which is not a line in sources.rs \
     (console-events/src/sources.rs says the rest)",
)];

#[test]
fn every_event_group_the_pool_serves_has_a_source() -> Result<(), Failure> {
  let architecture = architecture()?;
  let Ok(handled) = facts::of_kind(&architecture.facts, Kind::Handles);
  let Ok(sourced) = facts::of_kind(&architecture.facts, Kind::Sources);

  assert!(
    !handled.is_empty(),
    "nothing in console_events::sources::hold handles an event group, so this rule asks nothing; \
         the lint has lost the function"
  );

  let Ok(()) = ratchet(
    "a program can subscribe to these event groups and nothing in console_events::sources::hold says anything on them",
    handled.difference(&sourced).cloned().collect(),
    &SOURCELESS,
  );

  Ok(())
}

const TICKING: [(&str, &str); 6] = [
  (
    "console-music",
    "`music-bar` draws its line again every ten seconds underneath its Player and Compositor \
         subscriptions; a tick, the same shape as the bar's",
  ),
  (
    "console-wallpaper",
    "uses shared runtime timers to refresh the solar picture and schedule weather retries",
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
    "console-operating-system-linux",
    "the loop that carries out the timers and subscriptions other programs ask for; the clock is theirs",
  ),
  (
    "console-input-controller",
    "the profile loader asks the bus again every second until it answers, which is waiting for a thing to \
         come up rather than reading an event group again",
  ),
];

#[test]
fn nothing_both_subscribes_and_keeps_a_clock() -> Result<(), Failure> {
  let architecture = architecture()?;
  let Ok(subscribing) = facts::packages(&architecture.facts, Kind::Subscribes);
  let Ok(timing) = facts::packages(&architecture.facts, Kind::Timer);
  let found = subscribing
    .keys()
    .filter(|package| timing.contains_key(*package))
    .cloned()
    .collect();

  let Ok(()) = ratchet(
    "these subscribe to an event group and also keep a clock of their own; a reading the pool tells about needs no \
         tick behind it",
    found,
    &TICKING,
  );

  Ok(())
}
