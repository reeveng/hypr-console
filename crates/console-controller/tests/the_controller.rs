//! What the daemon that reads the pad does when a button is pressed.
//!
//! Each of these is the whole path: a button on the front of the machine,
//! through the profile that says what it means, onto the devices InputPlumber
//! publishes, into the daemon, and out as the command it runs or the wheel it
//! turns. Nothing between the two ends is stood in for, so a test that passes
//! here is a statement about the profile as much as about the daemon.

mod harness;

use std::collections::BTreeMap;

use evdev::{EventType, KeyCode, RelativeAxisCode};
use harness::{Daemon, Go, Script, go};
use console_controller::touch::GAIN;
use console_controller::reading::POLL;
use console_controller::turning::SETTLING_SECONDS;

const WHEEL: (EventType, u16) = (EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0);
const LEFT: (EventType, u16) = (EventType::KEY, KeyCode::BTN_LEFT.0);

/// A machine and the daemon reading it, on the profile the desktop runs.
fn desktop() -> (Go, Daemon) {
    (go("desktop"), Daemon::default())
}

fn total(daemon: &Daemon, (kind, code): (EventType, u16)) -> i32 {
    daemon.did.total(kind, code)
}

fn of_kind(daemon: &Daemon, (kind, code): (EventType, u16)) -> Vec<i32> {
    daemon.did.of_kind(kind, code)
}

/// The paddle asks for the same program whether a chooser is up or not, and
/// that program is where a chooser and a window are told apart. Written into
/// the profiles instead, the paddle meant one thing while a chooser was up and
/// another while it was not, and it changed meaning a beat after the screen
/// did.
#[test]
fn the_top_right_paddle_closes_what_is_up() {
    let (mut go, mut daemon) = desktop();
    go.press("right-paddle-top").expect("a paddle");
    daemon.run(&mut go, 2);
    assert_eq!(daemon.did.names(), ["put-away"]);
}

#[test]
fn the_top_left_paddle_opens_the_menu() {
    let (mut go, mut daemon) = desktop();
    go.press("left-paddle-top").expect("a paddle");
    daemon.run(&mut go, 2);
    assert_eq!(daemon.did.names(), ["launcher"]);
}

#[test]
fn the_bottom_right_paddle_takes_a_screenshot() {
    let (mut go, mut daemon) = desktop();
    go.press("right-paddle-bottom").expect("a paddle");
    daemon.run(&mut go, 2);
    assert_eq!(daemon.did.names(), ["console-screenshot"]);
}

/// The settings sit beside the face buttons, where a thumb already is. The
/// guide is read once and the settings are opened every day.
#[test]
fn legion_right_opens_the_settings() {
    let (mut go, mut daemon) = desktop();
    go.press("legion-right").expect("a button");
    daemon.run(&mut go, 2);
    assert_eq!(daemon.did.names(), ["settings-panel"]);
}

#[test]
fn the_menu_button_opens_the_guide() {
    let (mut go, mut daemon) = desktop();
    go.press("menu").expect("a button");
    daemon.run(&mut go, 2);
    assert_eq!(daemon.did.commands, [["/usr/local/bin/console-buttons", "--menu"]]);
}

/// The one button that goes somewhere this desktop is not.
#[test]
fn legion_left_leaves_for_game_mode() {
    let (mut go, mut daemon) = desktop();
    go.press("legion-left").expect("a button");
    daemon.run(&mut go, 2);
    assert_eq!(daemon.did.names(), ["game-mode"]);
}

/// The daemon is stopped outright while the on-screen keyboard is up, so that
/// a press does not both type a letter and do whatever the desktop makes of
/// it. Stopped is not deaf: the devices stay open and the kernel goes on
/// queueing on them. Acted on when it started again, every button pressed
/// while typing happened at once, against a desktop that had moved on, and one
/// of them takes the machine to Game Mode.
#[test]
fn what_was_pressed_while_the_daemon_was_stopped_is_not_acted_on() {
    let (mut go, mut daemon) = desktop();
    daemon.run(&mut go, 1);
    daemon.stopped_for(2.0);
    go.press("legion-left").expect("a button");
    daemon.run(&mut go, 2);
    assert!(daemon.did.commands.is_empty(), "it ran {:?}", daemon.did.names());
}

/// A backlog does not arrive in one read, so the turn it comes back on is not
/// the end of it: what a read leaves behind, and the devices a profile switch
/// took away and gives back a turn later, are all the same stale press. Five
/// of one button reached the desktop through that gap.
#[test]
fn what_arrives_in_the_moment_after_it_comes_back_is_thrown_away_too() {
    let (mut go, mut daemon) = desktop();
    daemon.run(&mut go, 1);
    daemon.stopped_for(2.0);
    daemon.run(&mut go, 1);
    go.press("legion-left").expect("a button");
    daemon.run(&mut go, 2);
    assert!(daemon.did.commands.is_empty(), "it ran {:?}", daemon.did.names());
}

/// Only for that moment. After it, the pad is the pad again.
#[test]
fn a_button_pressed_after_the_daemon_has_settled_is_acted_on() {
    let (mut go, mut daemon) = desktop();
    daemon.run(&mut go, 1);
    daemon.stopped_for(2.0);
    // Ordinary turns, because a jump would be another daemon that was away.
    daemon.run(&mut go, 2 + (SETTLING_SECONDS / POLL) as usize);
    go.press("legion-left").expect("a button");
    daemon.run(&mut go, 2);
    assert_eq!(daemon.did.names(), ["game-mode"]);
}

/// The browser moved here off the bottom left paddle, which is now where
/// something is said and typed. A button on the front is a fair place for a
/// thing done once: the paddles are for what a hand does while it is holding
/// the machine up.
#[test]
fn view_opens_the_browser() {
    let (mut go, mut daemon) = desktop();
    go.press("view").expect("a button");
    daemon.run(&mut go, 2);
    assert_eq!(daemon.did.names(), ["console-browser"]);
}

#[test]
fn the_shoulders_move_between_workspaces() {
    let (mut go, mut daemon) = desktop();
    go.press("r1").expect("a shoulder");
    go.press("l1").expect("a shoulder");
    daemon.run(&mut go, 2);
    assert_eq!(
        daemon.did.dispatched(),
        ["hl.dsp.focus({workspace = \"+1\"})", "hl.dsp.focus({workspace = \"-1\"})"]
    );
}

#[test]
fn holding_l2_carries_the_window_with_you() {
    let (mut go, mut daemon) = desktop();
    go.trigger("l2", 1.0).expect("a trigger");
    go.press("r1").expect("a shoulder");
    daemon.run(&mut go, 2);
    assert_eq!(daemon.did.dispatched(), ["hl.dsp.window.move({workspace = \"+1\"})"]);
}

#[test]
fn a_trigger_short_of_held_does_not_carry() {
    let (mut go, mut daemon) = desktop();
    go.trigger("l2", 0.4).expect("a trigger");
    go.press("r1").expect("a shoulder");
    daemon.run(&mut go, 2);
    assert_eq!(daemon.did.dispatched(), ["hl.dsp.focus({workspace = \"+1\"})"]);
}

#[test]
fn l2_and_the_dpad_are_the_brightness() {
    let (mut go, mut daemon) = desktop();
    go.trigger("l2", 1.0).expect("a trigger");
    go.press("dpad-right").expect("the dpad");
    go.press("dpad-left").expect("the dpad");
    daemon.run(&mut go, 2);
    assert_eq!(
        daemon.did.commands,
        [
            ["/usr/local/bin/console-brightness", "up"],
            ["/usr/local/bin/console-brightness", "down"],
        ]
    );
}

/// It is the arrow keys, which nothing here has to act on.
#[test]
fn the_dpad_alone_is_not_the_brightness() {
    let (mut go, mut daemon) = desktop();
    go.press("dpad-right").expect("the dpad");
    daemon.run(&mut go, 2);
    assert!(daemon.did.commands.is_empty());
}

#[test]
fn the_right_stick_turns_the_wheel() {
    let (mut go, mut daemon) = desktop();
    go.stick("right-stick", 0.0, -1.0).expect("a stick");
    daemon.run(&mut go, 11);
    assert_eq!(total(&daemon, WHEEL), 4, "a second of full deflection is a known number of notches");
}

/// Small pushes are squared, so precision at the top of the range costs
/// nothing at the bottom.
#[test]
fn a_half_pushed_stick_scrolls_less_than_a_quarter_as_fast() {
    let (mut go, mut daemon) = desktop();
    go.stick("right-stick", 0.0, -0.6).expect("a stick");
    daemon.run(&mut go, 11);
    assert_eq!(total(&daemon, WHEEL), 1);
}

#[test]
fn inside_the_deadzone_the_page_stays_where_it_is() {
    let (mut go, mut daemon) = desktop();
    go.stick("right-stick", 0.0, -0.15).expect("a stick");
    daemon.run(&mut go, 20);
    assert!(of_kind(&daemon, WHEEL).is_empty());
}

#[test]
fn pushing_up_scrolls_up_and_pushing_down_scrolls_down() {
    let (mut up, mut reading_up) = desktop();
    up.stick("right-stick", 0.0, -1.0).expect("a stick");
    reading_up.run(&mut up, 11);
    let (mut down, mut reading_down) = desktop();
    down.stick("right-stick", 0.0, 1.0).expect("a stick");
    reading_down.run(&mut down, 11);
    assert!(total(&reading_up, WHEEL) > 0);
    assert!(total(&reading_down, WHEEL) < 0);
}

#[test]
fn a_finger_on_the_pad_moves_the_pointer() {
    let (mut go, mut daemon) = desktop();
    go.drag((200, 200), (400, 200), 4, 0.0);
    daemon.run(&mut go, 2);
    let across = total(&daemon, (EventType::RELATIVE, RelativeAxisCode::REL_X.0));
    let down = total(&daemon, (EventType::RELATIVE, RelativeAxisCode::REL_Y.0));
    assert_eq!(down, 0);
    assert_eq!(across, (200.0 * GAIN) as i32, "screen pixels for each unit the finger travelled");
}

/// Position in, movement out. The first report of a touch is where it started,
/// and starting somewhere is not moving.
#[test]
fn the_pointer_does_not_jump_to_where_the_finger_landed() {
    let (mut go, mut daemon) = desktop();
    go.touch_down(900, 900);
    go.touch_up();
    daemon.run(&mut go, 2);
    assert!(of_kind(&daemon, (EventType::RELATIVE, RelativeAxisCode::REL_X.0)).is_empty());
}

#[test]
fn a_quick_touch_is_a_click() {
    let (mut go, mut daemon) = desktop();
    go.tap(500, 500);
    daemon.run(&mut go, 2);
    assert_eq!(of_kind(&daemon, LEFT), [1, 0]);
}

#[test]
fn a_drag_across_the_pad_is_not_a_click() {
    let (mut go, mut daemon) = desktop();
    go.drag((100, 100), (900, 900), 8, 0.0);
    daemon.run(&mut go, 2);
    assert!(of_kind(&daemon, LEFT).is_empty());
}

/// Not a tap. The button stays down for as long as the pad is pressed, so a
/// window can be dragged with it.
#[test]
fn pressing_the_pad_in_holds_the_button_down() {
    let (mut go, mut daemon) = desktop();
    go.touch_click(1);
    let mut script: Script = BTreeMap::new();
    script.insert(2, Box::new(|go: &mut Go| go.touch_click(0)));
    daemon.between(&mut go, 4, &mut script);
    assert_eq!(of_kind(&daemon, LEFT), [1, 0]);
}

/// A profile switch destroys the virtual pad and builds another. Reading from
/// what was left used to end this process, and the workspace buttons went with
/// it.
#[test]
fn the_pad_going_away_does_not_take_the_daemon_with_it() {
    let (mut go, mut daemon) = desktop();
    let mut script: Script = BTreeMap::new();
    script.insert(1, Box::new(|go: &mut Go| go.devices.sink.devices.get_mut("pad").expect("a pad").unplug()));
    script.insert(2, Box::new(|go: &mut Go| go.press("left-paddle-top").expect("a paddle")));
    daemon.between(&mut go, 6, &mut script);
    assert_eq!(daemon.did.names(), ["launcher"], "the keyboard side kept working");
}

/// Which is what happens every time a menu opens and closes.
#[test]
fn the_pad_is_picked_up_again_when_it_comes_back() {
    let (mut go, mut daemon) = desktop();
    let mut script: Script = BTreeMap::new();
    script.insert(1, Box::new(|go: &mut Go| go.devices.sink.devices.get_mut("pad").expect("a pad").unplug()));
    script.insert(60, Box::new(|go: &mut Go| go.devices.sink.devices.get_mut("pad").expect("a pad").plug()));
    script.insert(90, Box::new(|go: &mut Go| go.press("r1").expect("a shoulder")));
    daemon.between(&mut go, 200, &mut script);
    assert_eq!(daemon.did.dispatched(), ["hl.dsp.focus({workspace = \"+1\"})"]);
}

/// The on-screen keyboard takes the pad by signalling this daemon's unit. A
/// signal to a unit reaches every process in its control group unless it is
/// told otherwise, and the menu, the panel and anything opened from the menu
/// are all in that group: a control group is inherited by every child and
/// nothing a program can do to itself leaves one. Named wrongly, raising the
/// keyboard over a panel stopped the panel.
#[test]
fn the_keyboard_stops_the_daemon_and_nothing_the_daemon_started() {
    let hook = harness::root().join("files/usr/local/bin/osk-hook");
    let read = std::fs::read_to_string(&hook).expect("osk-hook");
    let signals: Vec<&str> = read
        .lines()
        .filter(|line| line.contains("systemctl") && line.contains("kill"))
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();
    assert!(!signals.is_empty(), "osk-hook no longer signals the daemon at all");
    for line in signals {
        assert!(
            line.contains("--kill-whom=main"),
            "osk-hook signals the whole control group: {}",
            line.trim()
        );
    }
}

