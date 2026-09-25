//! The keyboard half of the table, pressed on a real keyboard.
//!
//! Everything a key does on this machine is a row in the controller daemon's
//! table, rendered into a compositor bind and handed over -- which means there
//! are two ways for a key to stop working that no unit test can see. The
//! rendering can be right and the hand-over can never have happened, because
//! the daemon was not up, or the compositor was reloaded after it was, or it
//! refused every one of them and exited zero -- which is what it did for as
//! long as this desktop's config has been lua; and the bind can be there and
//! be on the wrong key, because a code is the kernel's number plus X11's eight
//! and getting that wrong is silent.
//!
//! What it is holding is asked by description, and that is not a shortcut. A
//! compositor configured in lua answers `__lua` for the dispatcher of every
//! bind it holds, a handle for the argument, and nothing at all for a key
//! named by code -- so the description the daemon writes is the whole of what
//! comes back, and the first way this was written compared keycodes that are
//! always zero here.
//!
//! So one of these asks the compositor what it is holding, and one presses the
//! key and watches for what the row promises. The third is what decides which
//! hand the guide and the setup screen open for, which is a file no one would
//! notification going stale.
//!
//! The one that presses took three goes and each fault hid the next, so all
//! three are written down. It asked for a toplevel: `opening` runs a command
//! and waits for a window in `hyprctl clients`, which is the wrong half of
//! both -- it started a settings panel of its own, so what came up was never
//! the key's doing, and nothing the panel host draws is a toplevel, so the
//! wait ran out beside a panel that had been on screen since the first second,
//! and `close_window` was then handed a namespace where it wanted an address
//! and left it standing. It quoted the key as one word, so wtype read
//! `-M logo -P i -p i -m logo` as a modifier by that whole name and refused
//! it. And then, quoted properly, wtype pressed nothing a bind could answer:
//! a Wayland client typing at the compositor is not a keyboard, Hyprland does
//! not match a bind against one, and wtype exits zero either way. So the key
//! is sent as a chord of `Keyboard:` capabilities through the same daemon the
//! pad goes through, which is a key event from a device and is the only thing
//! on this machine a bind will answer.
//!
//! What it presses is the row rather than a spelling of it. The table says
//! which keys open the settings and this asks it, so a row that moves moves
//! the press with it and a row nothing can press says so instead of going red
//! somewhere further down.
//!
//! The third asks less than it looks like it does, and the reason is worth
//! knowing before someone makes it ask more. Which hand was last used only
//! moves when a press arrives from the other one, and the only thing that
//! counts as the other one is a keyboard on someone's desk. Every button on
//! the pad arrives as the pad however it is routed, and the keyboard this
//! presses through is the daemon's own. So the flip is `reading.rs`'s own
//! tests, and what is left for a device is the half a unit test cannot answer:
//! that the daemon is writing the file at all.

use console_core_never::Never;
use console_test_stages::checking::{Body, Check, CheckResult, failed, happened};
use console_test_stages::device::{Device, Ready, Outcome};

use console_input_bindings::bound::{Binding, Input};
use console_input_controller::binds::{self, KeyBinding, wanted};
use console_input_controller::actions::Table;

const PATIENCE: f64 = 4.0;

const THE_SETTINGS: &str = "settings";

pub const HANDED: Check = Check {
    name: "400-the-keys-the-compositor-was-handed",
    about: "Every job on a key is a bind the compositor is holding, and is holding once.",
    feature: "typing",
    since: "2026-09-08",
    bodies: &[Body::Device(handed)],
};

pub const A_KEY: Check = Check {
    name: "410-a-key-opens-what-the-button-opens",
    about: "Super and I opens the settings, the same card the settings button opens.",
    feature: "typing",
    since: "2026-09-08",
    bodies: &[Body::Device(a_key)],
};

pub const LAST_PRESS: Check = Check {
    name: "420-the-hand-that-was-last-used",
    about: "A press on the pad leaves the machine saying the pad.",
    feature: "typing",
    since: "2026-09-08",
    bodies: &[Body::Device(last_press)],
};

fn ours() -> Result<Vec<KeyBinding>, Never> {
    let Ok(table) = Table::ours();

    wanted(&table)
}

fn handed(stage: &mut Device) -> CheckResult {
    let Ok(said) = stage.hypr("binds -j");

    let holding = match console_compositor::read(console_compositor::Query::Binds, &said) {
        Ok(console_compositor::Answer::Binds(holding)) => holding,
        Ok(_not_what_was_asked) => return failed("hyprctl answered something other than binds".to_string()),
        Err(fault) => return failed(fault.to_string()),
    };
    let Ok(wanted) = ours();
    let Ok(standing) = binds::standing(&wanted, &binds::Holding::These(holding.clone()));

    match standing.missing {
        0 => {},
        many => {
            return failed(format!(
                "the table says {} keys and the compositor is holding {}, {many} of ours among \
                 them reaching it from nowhere. Is the daemon up, and did the compositor take \
                 what it was sent?",
                wanted.len(),
                holding.len()
            ));
        }
    }

    match standing.over {
        0 => Ok(()),
        many => failed(format!(
            "{many} of the {} keys are bound more than once, out of {} the compositor holds \
             altogether: every copy fires on one press, so the rocker steps twice and the card \
             comes up twice. The daemon pushed the table on top of a compositor already holding \
             it -- which is what a restart under a live session used to do.",
            wanted.len(),
            holding.len()
        )),
    }
}

fn spelled(row: &Binding) -> Result<String, Never> {
    Ok(row.held.iter().map(String::as_str).chain([row.pressed.as_str()]).collect::<Vec<_>>().join(" and "))
}

fn standing(seen: &mut Device) -> Result<Ready, Never> {
    let Ok(where_) = seen.layer(console_settings::WHO);

    Ok(match where_ {
        Some(_) => Ready::Yes,
        None => Ready::NotYet,
    })
}

fn away(seen: &mut Device) -> Result<Ready, Never> {
    let Ok(up) = standing(seen);

    up.flipped()
}

fn console_put_away(stage: &mut Device) -> Result<Outcome, Never> {
    let Ok(()) = stage.press("b");

    stage.until(away, PATIENCE)
}

fn a_key(stage: &mut Device) -> CheckResult {
    let Ok(already) = standing(stage);

    match already {
        Ready::Yes => {
            let Ok(_) = console_put_away(stage);
        },
        Ready::NotYet => {},
    }

    let Ok(table) = Table::ours();
    let Ok(bound) = table.bindings(THE_SETTINGS);

    let row = match bound.iter().find(|binding| binding.on == Input::Keyboard) {
        Some(row) => row,
        None => {
            return failed(format!(
                "the table puts no key on {THE_SETTINGS}, so there is nothing here to press"
            ));
        },
    };

    let held: Vec<&str> = row.held.iter().map(String::as_str).collect();

    stage.keyed(&held, &row.pressed)?;

    let Ok(came) = stage.until(standing, PATIENCE);

    match came {
        Outcome::Happened => {},
        Outcome::RanOut => {
            let Ok(said) = spelled(row);

            return failed(format!(
                "{said} opened nothing. The bind is the table's, so either it was never handed \
                 over or what it runs is not what the row says"
            ));
        },
    }

    let Ok(gone) = console_put_away(stage);

    happened(gone, || "the settings would not close again".to_string())
}

fn last_press(stage: &mut Device) -> CheckResult {
    let Ok(home) = stage.home();
    let Ok(at) = console_input_bindings::active::path_in(std::path::Path::new(&home));
    let at = at.display().to_string();
    let reading = format!("cat {at} 2>/dev/null");

    let Ok(was) = stage.user(&reading);
    let was = was.trim().to_string();

    let Ok(word) = Input::Pad.word();
    let Ok(()) = stage.press("b");

    let asking = reading.clone();
    let Ok(said) = stage.until(
        move |stage: &mut Device| -> Result<Ready, Never> {
            let Ok(now) = stage.user(&asking);

            Ok(match now.trim() == word {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        },
        PATIENCE,
    );

    let Ok(pad) = stage.user(&reading);

    let done = happened(said, || {
        format!("a button was pressed and the machine says the hand is {pad:?}")
    });

    match was.is_empty() {
        true => {},
        false => {
            let _ = stage.user(&format!("printf %s {was} > {at}"));
        }
    }

    done
}
