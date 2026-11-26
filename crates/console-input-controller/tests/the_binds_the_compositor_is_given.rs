//! The other end of a keyboard binding.
//!
//! `what_reaches_the_desktop` asks whether the table can find a job again. This
//! asks the question the compositor would ask: of everything in the table that
//! is bound on a keyboard, what does it get handed, and would the hand it is
//! given do the thing.
//!
//! These used to be lines in `hyprland.lua` and this used to be a test that
//! read that file, counting where a bind sat in it. The file binds nothing now,
//! so what was worth keeping out of that test is here instead: not where a line
//! is written, but whether the effect it carries is the one the job promises.
//! The `button-contract.md` note in Orbit is the argument for the move and what
//! it costs.

use std::error::Error;

use console_core_never::Never;
use console_input_bindings::bound::Input;
use console_input_bindings::keys;
use console_input_bindings::moved::Tasks;
use console_input_controller::actions::{ButtonPress, JOBS, LockBehavior, RepeatMode, Table};
use console_input_controller::binds::{BindingAction, KeyBinding, desired_binds};
use console_input_controller::effect::Effect;
use console_input_controller::mode::Mode;

type Failure = Box<dyn Error>;

fn unmoved() -> Result<Vec<KeyBinding>, Never> {
  let Ok(none) = Tasks::none();
  let Ok(table) = Table::of(&none);

  desired_binds(&table)
}

fn on<'a>(
  every: &'a [KeyBinding],
  held: &[&str],
  pressed: &str,
) -> Result<&'a KeyBinding, Failure> {
  let held_2: Vec<String> = held.iter().map(|word| (*word).to_string()).collect();
  let Ok(keys) = keys::bind(&held_2, pressed);
  let keys_2 = keys.ok_or_else(|| format!("no key called {pressed}"))?;
  let bind = every
    .iter()
    .find(|bind| bind.keys == keys_2)
    .ok_or_else(|| format!("nothing is bound to {keys}", keys = keys_2))?;

  Ok(bind)
}

fn command(bind: &KeyBinding) -> Result<&str, Failure> {
  match &bind.action {
    BindingAction::Run(runs) => Ok(runs),
    BindingAction::Dispatch(_) => Err(Failure::from("expected an external command")),
  }
}

#[test]
fn a_named_special_workspace_uses_the_same_action_for_keys_and_the_controller()
-> Result<(), Failure> {
  let read = Tasks::read(
    r#"
[jobs]
special-workspace-notes = ["keyboard: super + alt + u", "pad: r2 + y"]
"special-workspace-research notes" = ["keyboard: super + alt + c"]
"#,
  );
  let tasks = read?;
  let Ok(saved) = tasks.serialize();
  let reread = Tasks::read(&saved);
  let persisted = reread?;
  assert_eq!(persisted, tasks);

  let Ok(table) = Table::of(&tasks);
  let Ok(bindings) = desired_binds(&table);
  let Ok(program) = console_core_internal_programs::InternalProgram::Compositor.path();
  let notes = on(&bindings, &["super", "alt"], "u")?;
  let chat = on(&bindings, &["super", "alt"], "c")?;
  let note_command = command(notes)?;
  let chat_command = command(chat)?;

  assert_eq!(note_command, format!("{program} special-workspace notes"));
  assert_eq!(
    chat_command,
    format!("{program} special-workspace 'research notes'")
  );
  assert_eq!(notes.repeats, RepeatMode::Once);
  assert_eq!(notes.locked, LockBehavior::Unlocked);

  let Ok(button) = table.what(Input::Pad, &["r2"], "y", Mode::Desktop);
  let job = button.ok_or("the controller shortcut should be available");
  let selected = job?;
  let Ok(action) = selected.action.does(ButtonPress::Down);

  assert_eq!(
    action,
    Some(Effect::Run(vec![
      program.to_owned(),
      "special-workspace".to_owned(),
      "notes".to_owned()
    ]))
  );
  assert_eq!(selected.action.does(ButtonPress::Up), Ok(None));
  Ok(())
}

#[test]
fn print_screen_and_super_shift_s_open_the_area_selector() -> Result<(), Failure> {
  let Ok(every) = unmoved();
  let Ok(program) = console_core_internal_programs::InternalProgram::ControlCenter.path();
  let flag = console_core_internal_programs::SELECT_SCREENSHOT_AREA.spelling;
  let print = on(&every, &[], "print")?;
  let chord = on(&every, &["super", "shift"], "s")?;

  for bind in [print, chord] {
    let installed = command(bind)?;
    assert_eq!(installed, format!("{program} {flag}"));
    assert_eq!(bind.repeats, RepeatMode::Once);
  }

  Ok(())
}

#[test]
fn the_power_key_puts_the_panel_back_and_answers_with_the_screen_locked() -> Result<(), Failure> {
  let Ok(every) = unmoved();
  let power = on(&every, &[], "power")?;
  let runs = command(power)?;

  assert!(
    runs.contains("console-brightness") && runs.contains("undim"),
    "the power key runs {runs:?}, which is not the program that puts the screen back"
  );
  assert_eq!(
    power.locked,
    LockBehavior::EvenThen,
    "the power key would do nothing with the screen off, which is the whole state it \
         exists for"
  );
  assert_eq!(
    power.repeats,
    RepeatMode::Once,
    "holding the power key would run the undim over and over"
  );

  Ok(())
}

#[test]
fn what_a_keyboard_labels_for_itself_is_bound_to_what_the_label_says() -> Result<(), Failure> {
  let Ok(every) = unmoved();
  let Ok(volume) = console_core_internal_programs::InternalProgram::Volume.path();
  let Ok(brightness) = console_core_internal_programs::InternalProgram::Brightness.path();

  for (pressed, program, argument) in [
    ("volume-up", &volume, "up"),
    ("volume-down", &volume, "down"),
    ("mute", &volume, "mute"),
    ("mic-mute", &volume, "mic-mute"),
    ("brightness-up", &brightness, "up"),
    ("brightness-down", &brightness, "down"),
  ] {
    let bind = on(&every, &[], pressed)?;
    let bound = command(bind)?;

    assert_eq!(bound, format!("{program} {argument}"), "{pressed}");
    assert_eq!(
      bind.locked,
      LockBehavior::EvenThen,
      "{pressed} is not answered in the dark, and a level someone reaches for is \
             reached for in the dark"
    );
  }

  Ok(())
}

#[test]
fn a_level_walks_while_it_is_held_and_a_door_opens_once() -> Result<(), Failure> {
  let Ok(every) = unmoved();
  let louder = on(&every, &[], "volume-up")?;
  let dimmer = on(&every, &[], "brightness-down")?;
  let microphone = on(&every, &[], "mic-mute")?;
  let browser = on(&every, &["super"], "b")?;

  assert_eq!(louder.repeats, RepeatMode::WhileHeld);
  assert_eq!(dimmer.repeats, RepeatMode::WhileHeld);
  assert_eq!(microphone.keys, "code:256");
  assert_eq!(microphone.repeats, RepeatMode::Once);
  assert_eq!(
    browser.repeats,
    RepeatMode::Once,
    "holding Super and B would open a browser for as long as the finger is down"
  );

  Ok(())
}

#[test]
fn dedicated_laptop_keys_run_their_actions_once_per_press() -> Result<(), Failure> {
  use console_core_internal_programs::InternalProgram;

  let Ok(every) = unmoved();
  let Ok(hotkey) = InternalProgram::Hotkey.path();
  let Ok(settings) = InternalProgram::SettingsPanel.name();
  let Ok(control_center) = InternalProgram::ControlCenter.path();
  let area = console_core_internal_programs::SELECT_SCREENSHOT_AREA.spelling;

  for (pressed, code, program, argument) in [
    ("display-switch", "code:235", settings, "Display"),
    ("selective-screenshot", "code:642", control_center, area),
    ("performance", "code:709", hotkey, "power-profile"),
    ("play-pause", "code:172", hotkey, "play-pause"),
    ("next-track", "code:171", hotkey, "next-track"),
    ("previous-track", "code:173", hotkey, "previous-track"),
    ("touchpad-toggle", "code:538", hotkey, "toggle-touchpad"),
  ] {
    let bind = on(&every, &[], pressed)?;
    let installed = command(bind)?;

    assert_eq!(bind.keys, code);
    assert_eq!(bind.repeats, RepeatMode::Once);
    assert_eq!(installed, format!("{program} {argument}"));
  }

  let fallback = on(&every, &["super"], "f8")?;
  let installed = command(fallback)?;
  assert_eq!(installed, format!("{hotkey} power-profile"));

  Ok(())
}

#[test]
fn tiled_windows_use_physical_keys_for_swapping_resizing_and_stacking() -> Result<(), Failure> {
  let Ok(every) = unmoved();
  let swap = on(&every, &["super", "shift"], "j")?;
  let resize = on(&every, &["super", "ctrl"], "k")?;
  let above_left = on(&every, &["super", "shift"], "u")?;
  let above_right = on(&every, &["super", "shift"], "i")?;
  let below_left = on(&every, &["super", "shift"], "n")?;
  let below_right = on(&every, &["super", "shift"], "m")?;
  let root = on(&every, &["super", "shift"], "backspace")?;
  let split = on(&every, &["super", "shift"], "enter")?;

  for (bind, dispatched, repeating) in [
    (
      swap,
      "window.swap({ direction = 'down' })",
      RepeatMode::Once,
    ),
    (
      resize,
      "window.resize({ x = 0, y = -32, relative = true })",
      RepeatMode::WhileHeld,
    ),
    (above_left, ")('left', 'u')", RepeatMode::Once),
    (above_right, ")('right', 'u')", RepeatMode::Once),
    (below_left, ")('left', 'd')", RepeatMode::Once),
    (below_right, ")('right', 'd')", RepeatMode::Once),
    (root, "layout('movetoroot')", RepeatMode::Once),
    (split, "layout('togglesplit')", RepeatMode::Once),
  ] {
    let Ok(script) = bind.bind_command();
    assert!(bind.keys.contains("code:"));
    assert!(script.to_string().contains(dispatched));
    assert_eq!(bind.repeats, repeating);
  }

  Ok(())
}

#[test]
fn quick_workspace_only_has_user_assigned_shortcuts() -> Result<(), Failure> {
  let Ok(defaults) = Table::ours();
  let Ok(default_bindings) = defaults.bindings("quick-workspace");
  assert!(default_bindings.is_empty());
  let tasks =
    Tasks::read("[jobs]\nquick-workspace = [\"keyboard: super + alt + q\", \"pad: l2 + y\"]\n")?;
  let Ok(custom) = Table::of(&tasks);
  let Ok(custom_bindings) = custom.bindings("quick-workspace");
  assert_eq!(custom_bindings.len(), 2);
  Ok(())
}

#[test]
fn no_two_jobs_are_handed_the_same_keys() {
  let Ok(every) = unmoved();
  let mut seen: std::collections::BTreeMap<&str, &BindingAction> =
    std::collections::BTreeMap::new();

  for bind in &every {
    let before = seen.insert(&bind.keys, &bind.action);

    assert!(
      before.is_none_or(|runs| runs == &bind.action),
      "{} is handed over twice, and the compositor keeps whichever arrived last",
      bind.keys
    );
  }
}

#[test]
fn every_key_in_the_table_is_one_the_compositor_can_be_told_about() {
  let mut asked: u32 = 0;

  for job in JOBS.iter() {
    for (_, held, pressed) in job.bound.iter().filter(|(on, _, _)| *on == Input::Keyboard) {
      let held_2: Vec<String> = held.iter().map(|word| (*word).to_string()).collect();

      assert_ne!(
        keys::bind(&held_2, pressed),
        Ok(None),
        "{} is bound to {pressed}, which is not a key this desktop has a word for",
        job.slug
      );

      asked = asked.saturating_add(1);
    }
  }

  assert!(
    asked > 1,
    "no job is on a keyboard, so this test asked nothing"
  );
}

#[test]
fn the_alphabets_step_both_ways_and_shift_is_the_way_back() -> Result<(), Failure> {
  let Ok(every) = unmoved();
  let on_to = on(&every, &["super", "shift"], "space")?;
  let back = on(&every, &["super", "ctrl"], "space")?;
  let forward_command = command(on_to)?;
  let backward_command = command(back)?;

  assert!(
    forward_command.contains(console_input_language::NAMED),
    "the key that steps the alphabets runs {:?}",
    forward_command
  );
  assert!(
    backward_command.contains(console_input_language::NAMED)
      && backward_command.contains(console_input_language::BACK.spelling),
    "the other half runs {:?}, which is not the same walk the other way",
    backward_command
  );
  assert_ne!(
    forward_command, backward_command,
    "both halves of the walk run the same thing, so a walk of three cannot be undone"
  );

  Ok(())
}

#[test]
fn the_windows_are_reached_on_the_arrows_and_on_hjkl() -> Result<(), Failure> {
  let Ok(every) = unmoved();

  for (arrow, letter) in [("left", "h"), ("down", "j"), ("up", "k"), ("right", "l")] {
    let by_arrow = on(&every, &["super"], arrow)?;
    let by_letter = on(&every, &["super"], letter)?;

    assert_eq!(
      by_arrow.action, by_letter.action,
      "{letter} and {arrow} part company, so one hand's way round the screen is not the \
             other's"
    );
  }

  for (arrow, letter, direction) in [("left", "h", "left"), ("right", "l", "right")] {
    let by_arrow = on(&every, &["super", "shift"], arrow)?;
    let by_letter = on(&every, &["super", "shift"], letter)?;
    let Ok(swap) = by_letter.bind_command();
    assert!(by_arrow.about.starts_with("carry-"));
    assert!(
      swap
        .to_string()
        .contains(&format!("window.swap({{ direction = '{direction}' }})"))
    );
  }

  Ok(())
}
