//! What the daemon that reads the pad does when a button is pressed.
//!
//! Each of these is the whole path: a button on the front of the machine,
//! through the profile that says what it means, onto the devices InputPlumber
//! publishes, into the daemon, and out as the command it runs or the wheel it
//! turns. Nothing between the two ends is stood in for, so a test that passes
//! here is a statement about the profile as much as about the daemon.

mod harness;

use console_core_geometry::Point;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_input_event_devices::{EventType, KeyCode, RelativeAxisCode};
use harness::{Daemon, Failure, Go, Script, go};
use console_input_controller::effect::Effect;
use console_core_directory_listing::Descend;
use console_core_never::Never;
use console_core_number_conversion::{toward_zero_i32, toward_zero_u32};
use console_input_gamepad::world::Device;
use console_input_controller::touch::GAIN;
use console_input_controller::reading::POLL;
use console_input_controller::returning::{HELD_SECONDS, Returning};
use console_input_controller::turning::SETTLING_SECONDS;

const WHEEL: (EventType, u16) = (EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0);
const LEFT: (EventType, u16) = (EventType::KEY, KeyCode::BTN_LEFT.0);
const ONE_NOTCH_DOWN: i32 = -1;
const ACROSS: (EventType, u16) = (EventType::RELATIVE, RelativeAxisCode::REL_X.0);

fn desktop() -> Result<(Go, Daemon), Failure> {
    let go = go(console_input_gamepad::router::NAME)?;

    Ok((go, Daemon::default()))
}

#[test]
fn the_top_right_paddle_closes_what_is_up() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.press("right-paddle-top")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(names) = daemon.did.names();
    assert_eq!(names, ["console-put-away"]);

    Ok(())
}

#[test]
fn the_top_left_paddle_opens_the_menu() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.press("left-paddle-top")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(names) = daemon.did.names();
    assert_eq!(names, ["launcher"]);

    Ok(())
}

#[test]
fn l2_and_the_bottom_right_paddle_take_a_screenshot() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.trigger("l2", 1.0)?;
    go.press("right-paddle-bottom")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(names) = daemon.did.names();
    assert_eq!(names, ["console-screenshot"]);

    Ok(())
}

#[test]
fn the_bottom_right_paddle_alone_takes_no_picture() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.press("right-paddle-bottom")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(names) = daemon.did.names();

    assert!(daemon.did.commands.is_empty(), "it ran {names:?}");

    Ok(())
}

#[test]
fn the_bottom_right_paddle_scrolls_the_page_down() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.press("right-paddle-bottom")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(notches) = daemon.did.of_kind(WHEEL);

    assert_eq!(notches, [ONE_NOTCH_DOWN], "one press is one notch, downwards");

    Ok(())
}

#[test]
fn the_paddle_held_goes_on_scrolling() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.button_down("right-paddle-bottom")?;
    let Ok(()) = daemon.run(&mut go, 60);
    let Ok(notches) = daemon.did.of_kind(WHEEL);

    assert!(notches.len() > 1, "held, it turned the wheel {} time(s)", notches.len());
    assert!(notches.iter().all(|notch| *notch == ONE_NOTCH_DOWN), "every one of them downwards");

    Ok(())
}

#[test]
fn the_paddle_let_go_of_stops_scrolling() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.button_down("right-paddle-bottom")?;
    let Ok(()) = daemon.run(&mut go, 60);
    go.up("right-paddle-bottom")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(so_far) = daemon.did.of_kind(WHEEL);
    let Ok(()) = daemon.run(&mut go, 60);
    let Ok(since) = daemon.did.of_kind(WHEEL);

    assert_eq!(since, so_far, "it went on scrolling with nothing on it");

    Ok(())
}

#[test]
fn legion_right_opens_the_settings() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.press("legion-right")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(names) = daemon.did.names();
    assert_eq!(names, ["settings-panel"]);

    Ok(())
}

#[test]
fn the_menu_button_opens_the_guide() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.press("menu")?;
    let Ok(()) = daemon.run(&mut go, 2);
    assert_eq!(daemon.did.commands, [["/usr/local/bin/mapping-panel"]]);

    Ok(())
}

#[test]
fn legion_left_leaves_for_session_game() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.press("legion-left")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(names) = daemon.did.names();
    assert_eq!(names, ["session-game"]);

    Ok(())
}

fn pad(go: &mut Go) -> Result<&mut Device, Failure> {
    let pad = go.devices.sink.devices.get_mut("pad").ok_or("a pad")?;

    Ok(pad)
}

fn read_the_pad(go: &mut Go, returning: &mut Returning, now: f64) -> Result<(), Failure> {
    let pad = pad(go)?;
    let Ok(arrived) = pad.drain();

    for event in arrived {
        let Ok(()) = returning.saw(event.kind, event.code, event.value, now);
    }

    Ok(())
}

fn keys_of(go: &mut Go) -> Result<Vec<u16>, Failure> {
    let pad = pad(go)?;
    let Ok(arrived) = pad.drain();

    Ok(arrived
        .iter()
        .filter(|event| event.kind == EventType::KEY)
        .map(|event| event.code)
        .collect())
}

fn way_back() -> Result<Option<Effect>, Never> {
    let Ok(run) = Effect::run(&["/usr/local/bin/session-desktop"]);

    Ok(Some(run))
}

#[test]
fn legion_left_held_comes_back_from_session_game() -> Result<(), Failure> {
    let mut go = go("game")?;
    let mut returning = Returning::default();
    go.button_down("legion-left")?;
    read_the_pad(&mut go, &mut returning, 1000.0)?;
    assert_eq!(returning.turn(1000.0 + HELD_SECONDS), way_back());

    Ok(())
}

#[test]
fn a_press_of_it_is_steams_own_menu_and_nothing_of_ours() -> Result<(), Failure> {
    let mut go = go("game")?;
    let mut returning = Returning::default();
    go.press("legion-left")?;
    read_the_pad(&mut go, &mut returning, 1000.0)?;
    assert_eq!(returning.turn(1000.0 + HELD_SECONDS), Ok(None));

    Ok(())
}

#[test]
fn held_with_another_button_it_is_a_chord_of_steams() -> Result<(), Failure> {
    let mut go = go("game")?;
    let mut returning = Returning::default();
    go.button_down("legion-left")?;
    go.button_down("b")?;
    read_the_pad(&mut go, &mut returning, 1000.0)?;
    assert_eq!(returning.turn(1000.0 + HELD_SECONDS), Ok(None));

    Ok(())
}

#[test]
fn steam_is_handed_the_button_whatever_is_made_of_it_here() -> Result<(), Failure> {
    let mut go = go("game")?;
    go.press("legion-left")?;
    let keys = keys_of(&mut go)?;

    assert_eq!(keys, [KeyCode::BTN_MODE.0, KeyCode::BTN_MODE.0]);

    Ok(())
}

#[test]
fn everything_else_reaches_the_pad_as_itself() -> Result<(), Failure> {
    let mut go = go("game")?;
    go.press("a")?;
    let keys = keys_of(&mut go)?;

    assert_eq!(keys, [KeyCode::BTN_SOUTH.0, KeyCode::BTN_SOUTH.0]);

    Ok(())
}

#[test]
fn what_was_pressed_while_the_machine_was_asleep_is_not_acted_on() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let Ok(()) = daemon.run(&mut go, 1);
    let Ok(()) = daemon.asleep_for(2.0);
    go.press("legion-left")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(names) = daemon.did.names();

    assert!(daemon.did.commands.is_empty(), "it ran {names:?}");

    Ok(())
}

#[test]
fn what_arrives_in_the_moment_after_it_comes_back_is_thrown_away_too() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let Ok(()) = daemon.run(&mut go, 1);
    let Ok(()) = daemon.asleep_for(2.0);
    let Ok(()) = daemon.run(&mut go, 1);
    go.press("legion-left")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(names) = daemon.did.names();

    assert!(daemon.did.commands.is_empty(), "it ran {names:?}");

    Ok(())
}

#[test]
fn a_button_pressed_after_the_daemon_has_settled_is_acted_on() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let Ok(()) = daemon.run(&mut go, 1);
    let Ok(()) = daemon.asleep_for(2.0);
    let Ok(settling) = toward_zero_u32(SETTLING_SECONDS / POLL);
    let Ok(()) = daemon.run(&mut go, settling.saturating_add(2));
    go.press("legion-left")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(names) = daemon.did.names();
    assert_eq!(names, ["session-game"]);

    Ok(())
}

#[test]
fn a_button_pressed_after_a_minute_of_nobody_touching_it_is_acted_on() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let Ok(()) = daemon.run(&mut go, 1);
    let Ok(()) = daemon.idle_for(60.0);
    go.press("legion-left")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(names) = daemon.did.names();

    assert_eq!(names, ["session-game"], "waiting for a press is not being asleep");

    Ok(())
}

#[test]
fn a_stick_pushed_after_a_minute_of_nothing_scrolls_as_far_as_a_stick_pushed_at_once() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let Ok(()) = daemon.run(&mut go, 1);
    let Ok(()) = daemon.idle_for(60.0);
    go.stick("right-stick", Point { x: 0.0, y: -1.0 })?;
    let Ok(()) = daemon.run(&mut go, 11);
    let Ok(scrolled) = daemon.did.total(WHEEL);

    assert_eq!(scrolled, 2, "the minute before it was pushed is not scrolling owed");

    Ok(())
}

#[test]
fn a_held_brightness_after_a_minute_of_nothing_steps_once_before_it_repeats() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.trigger("l2", 1.0)?;
    let Ok(()) = daemon.run(&mut go, 1);
    let Ok(()) = daemon.idle_for(60.0);
    go.hold("dpad-right")?;
    let Ok(()) = daemon.run(&mut go, 2);

    assert_eq!(
        daemon.did.commands,
        [["/usr/local/bin/console-brightness", "up"]],
        "the minute before it was held is not a repeat already due"
    );

    Ok(())
}

#[test]
fn view_opens_the_browser() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.press("view")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(names) = daemon.did.names();
    assert_eq!(names, ["console-browser"]);

    Ok(())
}

#[test]
fn the_shoulders_move_between_workspaces() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.press("r1")?;
    go.press("l1")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(dispatched) = daemon.did.dispatched();

    assert_eq!(
        dispatched,
        ["hl.dsp.focus({workspace = \"+1\"})", "hl.dsp.focus({workspace = \"-1\"})"]
    );

    Ok(())
}

#[test]
fn holding_both_triggers_carries_the_window_with_you() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.trigger("l2", 1.0)?;
    go.trigger("r2", 1.0)?;
    go.press("r1")?;
    let Ok(()) = daemon.run(&mut go, 3);
    let Ok(dispatched) = daemon.did.dispatched();
    assert_eq!(dispatched, ["hl.dsp.window.move({workspace = \"+1\"})"]);

    Ok(())
}

#[test]
fn holding_l2_goes_to_the_next_desktop_and_leaves_the_window() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.trigger("l2", 1.0)?;
    go.press("r1")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(dispatched) = daemon.did.dispatched();
    assert_eq!(dispatched, ["hl.dsp.focus({workspace = \"+1\"})"]);

    Ok(())
}

#[test]
fn a_trigger_short_of_held_does_not_carry() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.trigger("l2", 1.0)?;
    go.trigger("r2", 0.4)?;
    go.press("r1")?;
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(dispatched) = daemon.did.dispatched();
    assert_eq!(dispatched, ["hl.dsp.focus({workspace = \"+1\"})"]);

    Ok(())
}

#[test]
fn l2_and_the_dpad_are_the_brightness() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.trigger("l2", 1.0)?;
    go.press("dpad-right")?;
    go.press("dpad-left")?;
    let Ok(()) = daemon.run(&mut go, 2);
    assert_eq!(
        daemon.did.commands,
        [
            ["/usr/local/bin/console-brightness", "up"],
            ["/usr/local/bin/console-brightness", "down"],
        ]
    );

    Ok(())
}

#[test]
fn l2_and_the_dpad_are_the_volume() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.trigger("l2", 1.0)?;
    go.press("dpad-up")?;
    go.press("dpad-down")?;
    let Ok(()) = daemon.run(&mut go, 2);
    assert_eq!(
        daemon.did.commands,
        [
            ["/usr/local/bin/console-volume", "up"],
            ["/usr/local/bin/console-volume", "down"],
        ]
    );

    Ok(())
}

#[test]
fn the_dpad_alone_is_not_the_brightness() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.press("dpad-right")?;
    let Ok(()) = daemon.run(&mut go, 2);
    assert!(daemon.did.commands.is_empty());

    Ok(())
}

#[test]
fn the_right_stick_turns_the_wheel() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.stick("right-stick", Point { x: 0.0, y: -1.0 })?;
    let Ok(()) = daemon.run(&mut go, 11);

    let Ok(turned) = daemon.did.total(WHEEL);

    assert_eq!(turned, 2, "a full push turns the wheel by a known amount");

    Ok(())
}

#[test]
fn a_half_pushed_stick_scrolls_less_than_a_quarter_as_fast() -> Result<(), Failure> {
    let (mut full, mut turning_full) = desktop()?;
    full.stick("right-stick", Point { x: 0.0, y: -1.0 })?;
    let Ok(()) = turning_full.run(&mut full, 44);

    let (mut half, mut turning_half) = desktop()?;
    half.stick("right-stick", Point { x: 0.0, y: -0.6 })?;
    let Ok(()) = turning_half.run(&mut half, 44);

    let Ok(half) = turning_half.did.total(WHEEL);
    let Ok(full) = turning_full.did.total(WHEEL);

    assert!(
        half.saturating_mul(4) <= full,
        "a push of six tenths is a quarter of the travel once it is squared"
    );

    Ok(())
}

#[test]
fn inside_the_deadzone_the_page_stays_where_it_is() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    go.stick("right-stick", Point { x: 0.0, y: -0.15 })?;
    let Ok(()) = daemon.run(&mut go, 20);
    let Ok(notches) = daemon.did.of_kind(WHEEL);

    assert!(notches.is_empty());

    Ok(())
}

#[test]
fn pushing_up_scrolls_up_and_pushing_down_scrolls_down() -> Result<(), Failure> {
    let (mut up, mut reading_up) = desktop()?;
    up.stick("right-stick", Point { x: 0.0, y: -1.0 })?;
    let Ok(()) = reading_up.run(&mut up, 11);
    let (mut down, mut reading_down) = desktop()?;
    down.stick("right-stick", Point { x: 0.0, y: 1.0 })?;
    let Ok(()) = reading_down.run(&mut down, 11);
    let Ok(upwards) = reading_up.did.total(WHEEL);
    let Ok(downwards) = reading_down.did.total(WHEEL);

    assert!(upwards > 0);
    assert!(downwards < 0);

    Ok(())
}

#[test]
fn a_finger_on_the_pad_moves_the_pointer() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let Ok(()) = go.drag(Point { x: 200, y: 200 }, Point { x: 400, y: 200 }, 4, 0.0);
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(across) = daemon.did.total(ACROSS);
    let Ok(down) = daemon.did.total((EventType::RELATIVE, RelativeAxisCode::REL_Y.0));
    let Ok(travelled) = toward_zero_i32(200.0 * GAIN);

    assert_eq!(down, 0);
    assert_eq!(across, travelled, "screen pixels for each unit the finger travelled");

    Ok(())
}

#[test]
fn the_pointer_does_not_jump_to_where_the_finger_landed() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let Ok(()) = go.touch_down(Point { x: 900, y: 900 });
    let Ok(()) = go.touch_up();
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(moved) = daemon.did.of_kind(ACROSS);

    assert!(moved.is_empty());

    Ok(())
}

#[test]
fn a_quick_touch_is_a_click() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let Ok(()) = go.tap(Point { x: 500, y: 500 });
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(clicks) = daemon.did.of_kind(LEFT);

    assert_eq!(clicks, [1, 0]);

    Ok(())
}

#[test]
fn a_drag_across_the_pad_is_not_a_click() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let Ok(()) = go.drag(Point { x: 100, y: 100 }, Point { x: 900, y: 900 }, 8, 0.0);
    let Ok(()) = daemon.run(&mut go, 2);
    let Ok(clicks) = daemon.did.of_kind(LEFT);

    assert!(clicks.is_empty());

    Ok(())
}

#[test]
fn pressing_the_pad_in_holds_the_button_down() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let Ok(()) = go.touch_click(1);
    let mut script: Script = BTreeMap::new();

    script.insert(
        2,
        Box::new(|go: &mut Go| {
            let Ok(()) = go.touch_click(0);

            Ok(())
        }),
    );
    daemon.between(&mut go, 4, &mut script)?;
    let Ok(clicks) = daemon.did.of_kind(LEFT);

    assert_eq!(clicks, [1, 0]);

    Ok(())
}

#[test]
fn the_pad_going_away_does_not_take_the_daemon_with_it() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let mut script: Script = BTreeMap::new();
    script.insert(1, Box::new(|go: &mut Go| {
        let pad = pad(go)?;
        let Ok(()) = pad.unplug();

        Ok(())
    }));
    script.insert(2, Box::new(|go: &mut Go| go.press("left-paddle-top").map_err(Failure::from)));
    daemon.between(&mut go, 6, &mut script)?;
    let Ok(names) = daemon.did.names();
    assert_eq!(names, ["launcher"], "the keyboard side kept working");

    Ok(())
}

#[test]
fn the_pad_is_picked_up_again_when_it_comes_back() -> Result<(), Failure> {
    let (mut go, mut daemon) = desktop()?;
    let mut script: Script = BTreeMap::new();
    script.insert(1, Box::new(|go: &mut Go| {
        let pad = pad(go)?;
        let Ok(()) = pad.unplug();

        Ok(())
    }));
    script.insert(60, Box::new(|go: &mut Go| {
        let pad = pad(go)?;
        let Ok(()) = pad.plug();

        Ok(())
    }));
    script.insert(90, Box::new(|go: &mut Go| go.press("r1").map_err(Failure::from)));
    daemon.between(&mut go, 200, &mut script)?;
    let Ok(dispatched) = daemon.did.dispatched();
    assert_eq!(dispatched, ["hl.dsp.focus({workspace = \"+1\"})"]);

    Ok(())
}

fn every_file(under: &Path, (skipping, only): (&str, &str)) -> Result<Vec<PathBuf>, Never> {
    let Ok(listing) = console_core_directory_listing::recursive(under, |at| {
        match at.file_name().is_some_and(|name| name == skipping) {
            true => Descend::Past,
            false => Descend::Into,
        }
    });

    Ok(listing
        .filter_map(|entry| match entry {
            Ok(path) => match (path.is_dir(), path.to_string_lossy().contains(only)) {
                (false, true) => Some(path),
                (true, _) | (false, false) => None,
            },
            Err(_unreadable_folder) => None,
        })
        .collect())
}

fn lines_saying(under: &Path, path: &Path, (comment, said): (&str, &[&str])) -> Result<Vec<String>, Never> {
    let held = match std::fs::read_to_string(path) {
        Ok(held) => held,
        Err(_unreadable_file) => return Ok(Vec::new()),
    };
    let here = match path.strip_prefix(under) {
        Ok(inside) => inside.display().to_string(),
        Err(_outside_the_tree) => path.display().to_string(),
    };

    Ok(held
        .lines()
        .filter(|line| !line.trim_start().starts_with(comment))
        .filter(|line| said.iter().all(|word| line.contains(word)))
        .map(|line| format!("{here}: {}", line.trim()))
        .collect())
}

#[test]
fn nothing_signals_the_daemon_or_remembers_a_profile_for_it() {
    let Ok(root) = harness::root();
    let files = root.join("files");
    let Ok(every) = every_file(&files, ("", ""));
    let mut signals = Vec::new();
    let mut remembers = Vec::new();

    for path in every {
        let Ok(killing) = lines_saying(&files, &path, ("#", &["systemctl", "kill"]));
        let Ok(remembering) = lines_saying(&files, &path, ("#", &["console-profile-before-keyboard"]));

        signals.extend(killing);
        remembers.extend(remembering);
    }

    assert!(signals.is_empty(), "something signals a unit again: {signals:?}");
    assert!(
        remembers.is_empty(),
        "something remembers the profile from before the keyboard again: {remembers:?}"
    );
}

#[test]
fn no_program_here_stops_a_unit_with_a_signal() {
    let Ok(root) = harness::root();
    let crates = root.join("crates");
    let Ok(every) = every_file(&crates, ("target", "/src/"));
    let mut signals = Vec::new();

    for path in every.iter().filter(|path| path.extension().is_some_and(|kind| kind == "rs")) {
        let Ok(quoted) = lines_saying(&crates, path, ("//", &["\"STOP\""]));
        let Ok(named) = lines_saying(&crates, path, ("//", &["signal=STOP"]));

        signals.extend(quoted);
        signals.extend(named);
    }

    assert!(signals.is_empty(), "a program stops a unit again: {signals:?}");
}

#[test]
fn the_keyboard_runs_no_hook_when_it_appears_or_goes() -> Result<(), Failure> {
    let Ok(root) = harness::root();
    let held = std::fs::read_to_string(root.join("files/etc/systemd/user/console-input-keyboard.service"))?;
    let hooks: Vec<&str> = held
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter(|line| line.contains("WVKBD_ON_") || line.contains("osk-hook"))
        .collect();

    assert!(hooks.is_empty(), "the keyboard hands the pad over again: {hooks:?}");

    Ok(())
}
