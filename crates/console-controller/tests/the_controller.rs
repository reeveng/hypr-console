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
use console_controller::doing::Doing;
use console_controller::touch::GAIN;
use console_controller::reading::POLL;
use console_controller::returning::{HELD_SECONDS, Returning};
use console_controller::turning::SETTLING_SECONDS;

const WHEEL: (EventType, u16) = (EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0);
const LEFT: (EventType, u16) = (EventType::KEY, KeyCode::BTN_LEFT.0);

/// A machine and the daemon reading it, on the profile the desktop runs.
fn desktop() -> (Go, Daemon) {
    (go(console_pad::router::NAME), Daemon::default())
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

/// Held with L2, like the brightness. On its own this paddle is under the
/// finger that holds the machine up, and it took ninety-six pictures in two
/// days that nobody asked for.
#[test]
fn l2_and_the_bottom_right_paddle_take_a_screenshot() {
    let (mut go, mut daemon) = desktop();
    go.trigger("l2", 1.0).expect("a trigger");
    go.press("right-paddle-bottom").expect("a paddle");
    daemon.run(&mut go, 2);
    assert_eq!(daemon.did.names(), ["console-screenshot"]);
}

/// And the half of that which is the whole point of it: bare, it takes no
/// picture. What it does instead is scroll, which runs nothing.
#[test]
fn the_bottom_right_paddle_alone_takes_no_picture() {
    let (mut go, mut daemon) = desktop();
    go.press("right-paddle-bottom").expect("a paddle");
    daemon.run(&mut go, 2);
    assert!(daemon.did.commands.is_empty(), "it ran {:?}", daemon.did.names());
}

/// The finger is already resting on it and a page is read downwards.
#[test]
fn the_bottom_right_paddle_scrolls_the_page_down() {
    let (mut go, mut daemon) = desktop();
    go.press("right-paddle-bottom").expect("a paddle");
    daemon.run(&mut go, 2);
    assert_eq!(of_kind(&daemon, WHEEL), [-1], "one press is one notch, downwards");
}

/// Held, it goes on scrolling, which is what a page longer than a press is.
/// The same repeat the volume has, and for the same reason: a thumb is already
/// on the button and asking for it again is the part nobody wants to do.
#[test]
fn the_paddle_held_goes_on_scrolling() {
    let (mut go, mut daemon) = desktop();
    go.down("right-paddle-bottom").expect("a paddle");
    daemon.run(&mut go, 60);
    let notches = of_kind(&daemon, WHEEL);
    assert!(notches.len() > 1, "held, it turned the wheel {} time(s)", notches.len());
    assert!(notches.iter().all(|notch| *notch == -1), "every one of them downwards");
}

/// And it stops when the finger comes off, rather than scrolling on because
/// nothing was there to hear the release.
#[test]
fn the_paddle_let_go_of_stops_scrolling() {
    let (mut go, mut daemon) = desktop();
    go.down("right-paddle-bottom").expect("a paddle");
    daemon.run(&mut go, 60);
    go.up("right-paddle-bottom").expect("a paddle");
    daemon.run(&mut go, 2);
    let so_far = of_kind(&daemon, WHEEL).len();
    daemon.run(&mut go, 60);
    assert_eq!(of_kind(&daemon, WHEEL).len(), so_far, "it went on scrolling with nothing on it");
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

// ------------------------------------------------------------------ and back
//
// The other side of that button, which is a second daemon: the desktop's own is
// stopped along with the rest of console.target on the way to Game Mode, so
// what reads the pad there is `game-return`. Game Mode's profile translates
// nothing, which is why the press arrives at Steam as itself and opens Steam's
// menu. These are about what happens if it is kept down.

/// Everything the pad has to say just now, read the way Game Mode reads it.
fn read_the_pad(go: &mut Go, returning: &mut Returning, now: f64) {
    for event in go.devices.sink.devices.get_mut("pad").expect("a pad").drain() {
        returning.saw(event.event_type(), event.code(), event.value(), now);
    }
}

/// Every key the pad sent, in order, which is what a game is handed.
fn keys_of(go: &mut Go) -> Vec<u16> {
    go.devices
        .sink
        .devices
        .get_mut("pad")
        .expect("a pad")
        .drain()
        .iter()
        .filter(|event| event.event_type() == EventType::KEY)
        .map(evdev::InputEvent::code)
        .collect()
}

fn way_back() -> Option<Doing> {
    Some(Doing::run(&["/usr/local/bin/desktop-mode"]))
}

/// One button for the door, whichever side of it you are on.
#[test]
fn legion_left_held_comes_back_from_game_mode() {
    let mut go = go("game");
    let mut returning = Returning::default();
    go.down("legion-left").expect("a button");
    read_the_pad(&mut go, &mut returning, 1000.0);
    assert_eq!(returning.turn(1000.0 + HELD_SECONDS), way_back());
}

/// A press is Steam's. Taken outright, Game Mode would lose the menu that the
/// library, the power and the way out of a game are on.
#[test]
fn a_press_of_it_is_steams_own_menu_and_nothing_of_ours() {
    let mut go = go("game");
    let mut returning = Returning::default();
    go.press("legion-left").expect("a button");
    read_the_pad(&mut go, &mut returning, 1000.0);
    assert_eq!(returning.turn(1000.0 + HELD_SECONDS), None);
}

/// Steam's own shortcuts are that button and another one together, and the one
/// that makes a game give up is held for longer than this is.
#[test]
fn held_with_another_button_it_is_a_chord_of_steams() {
    let mut go = go("game");
    let mut returning = Returning::default();
    go.down("legion-left").expect("a button");
    go.down("b").expect("a button");
    read_the_pad(&mut go, &mut returning, 1000.0);
    assert_eq!(returning.turn(1000.0 + HELD_SECONDS), None);
}

/// And the press reaches Steam either way, held or not: the button it acts on
/// is the pad's own, and nothing here takes it.
#[test]
fn steam_is_handed_the_button_whatever_is_made_of_it_here() {
    let mut go = go("game");
    go.press("legion-left").expect("a button");
    assert_eq!(keys_of(&mut go), [KeyCode::BTN_MODE.0, KeyCode::BTN_MODE.0]);
}

/// Every other button too, untouched, which is the whole of what Game Mode's
/// profile is for: what is on the screen there is a game, and a game expects a
/// pad rather than this desktop.
#[test]
fn everything_else_reaches_the_pad_as_itself() {
    let mut go = go("game");
    go.press("a").expect("a button");
    assert_eq!(keys_of(&mut go), [KeyCode::BTN_SOUTH.0, KeyCode::BTN_SOUTH.0]);
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

#[test]
fn l2_and_the_dpad_are_the_volume() {
    let (mut go, mut daemon) = desktop();
    go.trigger("l2", 1.0).expect("a trigger");
    go.press("dpad-up").expect("the dpad");
    go.press("dpad-down").expect("the dpad");
    daemon.run(&mut go, 2);
    assert_eq!(
        daemon.did.commands,
        [
            ["/usr/local/bin/console-volume", "up"],
            ["/usr/local/bin/console-volume", "down"],
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

    // The number tracks `scroll::MAX_HZ` and moved when the stick was slowed
    // down. What is being held here is that a full push turns the wheel by a
    // repeatable amount, not that the amount is this integer.
    assert_eq!(total(&daemon, WHEEL), 2, "a full push turns the wheel by a known amount");
}

/// Small pushes are squared, so precision at the top of the range costs
/// nothing at the bottom.
#[test]
fn a_half_pushed_stick_scrolls_less_than_a_quarter_as_fast() {
    let (mut full, mut turning_full) = desktop();
    full.stick("right-stick", 0.0, -1.0).expect("a stick");
    turning_full.run(&mut full, 44);

    let (mut half, mut turning_half) = desktop();
    half.stick("right-stick", 0.0, -0.6).expect("a stick");
    turning_half.run(&mut half, 44);

    // Asserted as the two against each other rather than as two numbers. The
    // squaring is a relationship and the speed is not: both counts move when
    // `scroll::MAX_HZ` changes and this does not, which is the difference
    // between a test about the curve and a test about how fast the stick is.
    // Written as a number it said 1, and slowing the stick made it 0 -- which
    // failed while the thing it is named for was still true.
    assert!(
        total(&turning_half, WHEEL) * 4 <= total(&turning_full, WHEEL),
        "a push of six tenths is a quarter of the travel once it is squared"
    );
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

/// Nothing hands the pad to the keyboard, because there is nothing to hand.
///
/// `osk-hook` ran at both ends of the on-screen keyboard and did two things.
/// It stopped this daemon with SIGSTOP and started it again with SIGCONT, so
/// that this and the keyboard did not both act on the right stick -- which
/// navigates and scrolls at once, and flickers. And it loaded the pad profile
/// the keyboard needs, remembering the one that was there in a file so it
/// could be put back.
///
/// Both are the daemon's now, and neither is remembered. It asks the
/// compositor what is in front of it: under the keyboard it acts on nothing,
/// and the profile the pad wants is a function of that answer rather than of
/// what somebody wrote down when the keyboard went up.
///
/// This holds all of it out at once, because each piece put back brings a
/// fault with it. A signal is the backlog -- stopped is not deaf, the kernel
/// went on queueing, and every button pressed while typing arrived in one
/// instant against a desktop that had moved on, which is how the machine once
/// left for Game Mode on its own -- and it is the control group, since a
/// signal to a unit reaches everything the menu opened unless told otherwise.
/// A remembered profile is the stale one: a panel closed while the keyboard
/// was over it had already put the desktop back, and laying the remembered
/// profile over that left the pad answering to a panel that was gone.
#[test]
fn nothing_signals_the_daemon_or_remembers_a_profile_for_it() {
    let files = harness::root().join("files");
    let mut signals = Vec::new();
    let mut remembers = Vec::new();
    let mut walk = vec![files.clone()];
    while let Some(at) = walk.pop() {
        let Ok(reading) = std::fs::read_dir(&at) else { continue };
        for child in reading.filter_map(Result::ok) {
            let path = child.path();
            if path.is_dir() {
                walk.push(path);
                continue;
            }
            let Ok(held) = std::fs::read_to_string(&path) else { continue };
            let here = path.strip_prefix(&files).unwrap_or(&path).display().to_string();
            for line in held.lines().filter(|line| !line.trim_start().starts_with('#')) {
                if line.contains("systemctl") && line.contains("kill") {
                    signals.push(format!("{here}: {}", line.trim()));
                }
                if line.contains("console-profile-before-keyboard") {
                    remembers.push(format!("{here}: {}", line.trim()));
                }
            }
        }
    }
    assert!(signals.is_empty(), "something signals a unit again: {signals:?}");
    assert!(
        remembers.is_empty(),
        "something remembers the profile from before the keyboard again: {remembers:?}"
    );
}

/// And no program stops the daemon either, which is where one still did.
///
/// The test above walks `files/`, because that is where the two programs that
/// did this lived. `console-buttons --identify` is a crate, so it was never
/// looked at, and it went on sending SIGSTOP and SIGCONT for months after the
/// shell scripts that did the same thing were deleted for it.
///
/// It was the worst of the three, and only because nobody presses it often.
/// The SIGCONT sat after a loop the program tells you to leave with Ctrl-C, so
/// on the documented way out it never ran: the daemon was not sometimes left
/// stopped, it was always left stopped, until somebody restarted the unit. And
/// with no `--kill-whom=main` the signal took the whole control group, so run
/// from the menu it stopped the menu it was opened from, and the second press
/// -- the one that would have told you what the button was -- arrived at a
/// stopped program.
///
/// What replaces it is a grab, and the reason is that a grab cannot be left
/// behind. The kernel holds it and the kernel releases it when the process
/// goes, however it goes, so there is no path on which the undoing is missed.
/// That is the property SIGSTOP never had and could not be given.
#[test]
fn no_program_here_stops_a_unit_with_a_signal() {
    let crates = harness::root().join("crates");
    let mut signals = Vec::new();
    let mut walk = vec![crates.clone()];
    while let Some(at) = walk.pop() {
        let Ok(reading) = std::fs::read_dir(&at) else { continue };
        for child in reading.filter_map(Result::ok) {
            let path = child.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                walk.push(path);
                continue;
            }
            if path.extension().is_none_or(|kind| kind != "rs") {
                continue;
            }
            // The programs, which is what ships. A test naming the signal is a
            // test about the signal being gone, and this is one of those.
            if !path.components().any(|part| part.as_os_str() == "src") {
                continue;
            }
            let Ok(held) = std::fs::read_to_string(&path) else { continue };
            let here = path.strip_prefix(&crates).unwrap_or(&path).display().to_string();
            for line in held.lines().filter(|line| !line.trim_start().starts_with("//")) {
                // The words as they are passed, so a comment recording why this
                // is gone does not read as this being back.
                if line.contains("\"STOP\"") || line.contains("signal=STOP") {
                    signals.push(format!("{here}: {}", line.trim()));
                }
            }
        }
    }
    assert!(signals.is_empty(), "a program stops a unit again: {signals:?}");
}

/// And the keyboard is not asked to run anything at either end.
///
/// The keyboard runs its own hooks on show/hide, so a hook put back there
/// would be a second opinion about the pad that this daemon never hears about.
#[test]
fn the_keyboard_runs_no_hook_when_it_appears_or_goes() {
    let unit = harness::root().join("files/etc/systemd/user/console-keyboard.service");
    let held = std::fs::read_to_string(&unit).expect("the keyboard service");
    let hooks: Vec<&str> = held
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter(|line| line.contains("WVKBD_ON_") || line.contains("osk-hook"))
        .collect();
    assert!(hooks.is_empty(), "the keyboard hands the pad over again: {hooks:?}");
}

