//! The settings panel: it opens, it draws, and it lets go.
//!
//! Putting a panel away at once waits for its request to be accepted and its
//! frame to be presented. Those milestones belong to this opening alone; a
//! timing line or an old frame cannot satisfy them. Closing follows directly
//! after Drawn, and Gone confirms the host finished releasing the panel.

use console_core_geometry::Point;
use std::collections::BTreeSet;
use std::time::Duration;

use console_response_times::FELT;

use console_test_stages::checking::{
  Body, Check, CheckResult, empty, expect_ready, happened, more_than, not_empty, same,
};
use console_test_stages::desktop::Desktop;
use console_test_stages::device::{Device, PATIENCE, Ready};
use console_test_stages::here::{Here, InputHandling, TURNS};
use console_test_stages::palette::palette;

use crate::picker::{Expected, opens};

pub const PANEL: Check = Check {
  name: "080-the-panel",
  about: "Legion right opens the settings panel.",
  feature: "panel",
  since: "2026-08-28",
  bodies: &[Body::Here(here), Body::Device(there)],
};

pub const DRAWS: Check = Check {
  name: "160-the-panel-draws",
  about: "The settings panel opens, and draws itself.",
  feature: "panel",
  since: "2026-08-28",
  bodies: &[Body::Desktop(draws)],
};

pub const WITH_THE_KEYBOARD: Check = Check {
  name: "180-out-of-a-panel-with-the-keyboard-up",
  about: "B closes a panel with the keyboard over it, and leaves the pad usable.",
  feature: "panel",
  since: "2026-08-28",
  bodies: &[
    Body::Here(without_a_screen),
    Body::Device(with_the_keyboard),
  ],
};

pub const PUT_AWAY_AT_ONCE: Check = Check {
  name: "185-a-panel-is-put-away-at-once",
  about: "A panel the host is holding is gone within a frame of being put away, and every opening is written down.",
  feature: "panel",
  since: "2026-09-24",
  bodies: &[Body::Desktop(put_away_at_once)],
};

const OPENED_AND_PUT_AWAY: u32 = 6;

const HOSTED: &str = "settings-panel";

const OVER_A_PANEL: &str = r#"{"eDP-1":{"levels":{
    "0":[{"namespace":"console-paper","h":1600}],
    "2":[{"namespace":"console-bar","h":40}],
    "3":[{"namespace":"settings-panel","h":1562},{"namespace":"console-keyboard","h":520}]}}}"#;

const THE_PANEL_ALONE: &str = r#"{"eDP-1":{"levels":{
    "0":[{"namespace":"console-paper","h":1600}],
    "2":[{"namespace":"console-bar","h":40}],
    "3":[{"namespace":"settings-panel","h":1562}]}}}"#;

const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
    "0":[{"namespace":"console-paper","h":1600}],
    "2":[{"namespace":"console-bar","h":40}]}}}"#;

fn without_a_screen(stage: &mut Here) -> CheckResult {
  stage.set_layers(THE_PANEL_ALONE)?;

  let Ok(alone) = stage.input_handling();

  same(&alone, &InputHandling::Enabled, || {
    "a panel up and the daemon has stood down, so no button on it does anything".to_string()
  })?;

  stage.set_layers(OVER_A_PANEL)?;
  stage.press("legion-right")?;
  stage.press("b")?;

  let Ok(()) = stage.settle(TURNS);
  let Ok(under) = stage.commands();

  empty(under, || {
    format!("the daemon acted under the keyboard: {under:?}")
  })?;

  stage.set_layers(NOTHING_UP)?;

  let Ok(after) = stage.input_handling();

  same(&after, &InputHandling::Enabled, || {
    "the keyboard went and the daemon is still standing down".to_string()
  })?;

  let Ok(commands) = stage.commands();
  let Ok(before) = console_core_number_conversion::fitted::<_, u32>(commands.len());

  stage.press("legion-right")?;

  let Ok(()) = stage.settle(TURNS);
  let Ok(again) = stage.commands();
  let Ok(after_2) = console_core_number_conversion::fitted::<_, u32>(again.len());

  more_than(after_2, before, || {
    "the keyboard went and the daemon never started acting again".to_string()
  })
}

pub const ACROSS: f64 = 200.0;
pub const DOWN: std::ops::Range<i32> = 150..520;
pub const EVERY: u32 = 6;

pub fn drew(stage: &mut Desktop) -> CheckResult {
  let Ok(wanted) = palette();
  let mut down: BTreeSet<String> = BTreeSet::new();

  let Ok(every) = console_core_number_conversion::index(EVERY);

  for y in DOWN.step_by(every.max(1)) {
    let said = stage.color(Point {
      x: ACROSS,
      y: f64::from(y),
    })?;

    let _ = down.insert(said);
  }

  let any_of = |names: &[&str]| {
    let any = names
      .iter()
      .any(|name| wanted.get(*name).is_some_and(|color| down.contains(color)));

    match ::core::primitive::bool::then_some(any, ()) {
      ::core::option::Option::Some(()) => Ready::Yes,
      ::core::option::Option::None => Ready::NotYet,
    }
  };

  expect_ready(any_of(&["panel", "ground"]), || {
    format!("nothing of the panel is on the screen where it should be: {down:?}")
  })?;

  expect_ready(any_of(&["pink"]), || {
    format!("the panel drew but nothing on it is highlighted: {down:?}")
  })
}

fn here(stage: &mut Here) -> CheckResult {
  stage.press("legion-right")?;
  let Ok(()) = stage.settle(TURNS);
  let Ok(ran) = stage.names();

  same(&ran, &["settings-panel"], || format!("it ran {ran:?}"))
}

fn there(stage: &mut Device) -> CheckResult {
  opens(stage, "legion-right", Expected("panel"))
}

fn put_away_at_once(stage: &mut Desktop) -> CheckResult {
  stage.panel_cycles(OPENED_AND_PUT_AWAY, HOSTED, &["Sound"])?;
  let timings = stage.timings()?;
  let about = |what: &str| {
    timings
      .iter()
      .filter(|entry| entry.who == HOSTED && entry.what == what)
      .collect::<Vec<_>>()
  };
  let closings = about("closing");
  let Ok(many) = console_core_number_conversion::fitted::<_, u32>(closings.len());

  same(&many, &OPENED_AND_PUT_AWAY, || {
    format!("put away {OPENED_AND_PUT_AWAY} times and the host closed {many}")
  })?;

  let stopping: Vec<Duration> = closings
    .iter()
    .flat_map(|closing| closing.marks.iter())
    .filter(|(name, _)| name == "drawing")
    .map(|(_, took)| *took)
    .collect();
  let felt: Vec<&Duration> = stopping.iter().filter(|took| **took > FELT).collect();

  empty(&felt, || {
    format!(
      "the drawing went on after the panel was put away, for longer than a frame: {stopping:?}"
    )
  })?;

  let openings = about("opening");
  let Ok(written) = console_core_number_conversion::fitted::<_, u32>(openings.len());

  same(&written, &OPENED_AND_PUT_AWAY, || {
    format!("opened {OPENED_AND_PUT_AWAY} times and {written} openings were written down")
  })
}

fn draws(stage: &mut Desktop) -> CheckResult {
  stage.open("settings-panel Sound")?;
  drew(stage)
}

fn with_the_keyboard(stage: &mut Device) -> CheckResult {
  let Ok(()) = stage.press("legion-right");
  let Ok(drawn) = stage.wait_for_menu(PATIENCE);

  happened(drawn, || "the panel did not draw".to_string())?;

  let Ok(()) = stage.press("x");
  let Ok(over) = stage.until(Device::keyboard, PATIENCE);

  happened(over, || {
    "the keyboard did not come up over the panel".to_string()
  })?;

  let Ok(up) = stage.menus();

  not_empty(&up, || {
    "the keyboard came up and the panel went".to_string()
  })?;

  let Ok(()) = stage.press("b");
  let Ok(closed) = stage.wait_for_close(PATIENCE);

  happened(closed, || "B did not close the panel".to_string())?;

  let Ok(()) = stage.press("x");
  let Ok(went) = stage.until(
    |seen| {
      let Ok(up_2) = seen.keyboard();

      up_2.flipped()
    },
    PATIENCE,
  );

  happened(went, || "the keyboard would not go away".to_string())?;

  let Ok(held) = stage.profile();

  same(&held, crate::picker::WORN, || {
    format!("the keyboard went and the pad was left wearing {held}")
  })?;

  let Ok(()) = stage.press("legion-right");
  let Ok(again) = stage.wait_for_menu(PATIENCE);

  happened(again, || {
    "the settings button stopped drawing anything".to_string()
  })?;

  let Ok(()) = stage.press("b");
  let Ok(gone) = stage.wait_for_close(PATIENCE);

  happened(gone, || "the panel would not close again".to_string())
}
