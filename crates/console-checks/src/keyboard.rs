//! The on-screen keyboard: X brings it up, X puts it away, and it has depth.

use std::collections::BTreeSet;

use console_stage::checking::{Body, Check, Done, ought};
use console_stage::desktop::Desktop;
use console_stage::device::Device;
use console_stage::palette::palette;

pub const KEYBOARD: Check = Check {
    name: "110-the-keyboard",
    about: "X shows the keyboard, and X puts it away.",
    feature: "keyboard",
    since: "2026-08-25",
    bodies: &[Body::Device(there)],
};

/// The keyboard is the piece this desktop has broken most often, and the last
/// way it broke was not that it failed to appear: the slab behind the keys and a
/// key that is not a letter had been given the same colour, so Esc, Tab, the
/// arrows and Enter had nothing underneath them. They read as letters lying on
/// the desktop, and the whole keyboard read as something you could see through.
///
/// So this asks for three colours and not one. Two of them being the same is the
/// fault, and a check that only asked whether the keyboard was there would have
/// had nothing to say about it.
pub const DRAWS: Check = Check {
    name: "170-the-keyboard-draws",
    about: "The on-screen keyboard comes up, and every key has a key under it.",
    feature: "keyboard",
    since: "2026-08-28",
    bodies: &[Body::Desktop(draws)],
};

/// The keyboard is a slab along the bottom, in the coordinates the compositor
/// lays out in. Swept rather than sampled: which key sits where is the layout's
/// business and none of this check's.
const ACROSS: (i32, i32, usize) = (20, 1010, 14);
const DOWN: (i32, i32, usize) = (390, 636, 12);

/// The three the keyboard is made of. A letter key, the slab behind them, and a
/// key that is not a letter.
const SHADES: [&str; 3] = ["ground", "night", "panel"];

fn there(stage: &mut Device) -> Done {
    if stage.keyboard() {
        stage.press("x");
        stage.settle(1.5);
    }
    stage.press("x");
    stage.settle(1.5);
    ought(stage.keyboard(), || "the keyboard did not come up".to_string())?;
    stage.press("x");
    stage.settle(1.5);
    ought(!stage.keyboard(), || "the keyboard would not go away".to_string())
}

fn draws(stage: &mut Desktop) -> Done {
    let wanted = palette();
    stage.open("osk")?;

    let mut there = BTreeSet::new();
    for across in (ACROSS.0..ACROSS.1).step_by(ACROSS.2) {
        for down in (DOWN.0..DOWN.1).step_by(DOWN.2) {
            there.insert(stage.colour(f64::from(across), f64::from(down))?);
        }
    }

    let missing: Vec<&str> =
        SHADES.into_iter().filter(|name| !there.contains(&wanted[*name])).collect();
    ought(missing.is_empty(), || {
        format!(
            "the keyboard is not three shades; nothing is {}. The slab, a letter key and a key \
             that is not a letter have to differ or some of the keys have nothing under them.",
            missing.join(" or ")
        )
    })
}

/// The keyboard after the desktop has been brought up again, over and over.
///
/// X reaching the keyboard is not this desktop's own doing. Every profile
/// passes X through untouched -- it arrives on the pad as North, which
/// `console-pad/tests/the_button_contract.rs` asks for on purpose -- and wvkbd
/// finds the pad in /dev/input and reads it itself. So two programs go looking
/// for one pad and neither of them owns it, and the controller's own
/// ExecStartPost destroys it on the way past: a profile switch takes the pad
/// away and builds a new one, which is `console_controller::turning::Gone`.
///
/// Started with nothing said about the order, wvkbd could open the pad that
/// switch was about to take. What that looked like was X doing nothing until
/// the next reboot, and working again on the one after that.
///
/// Which is why this restarts rather than presses. `110-the-keyboard` presses X
/// once, on a desktop that has been up for a while, and it passed all the way
/// through: a fault that appears on one restart in three is invisible to a
/// check that arrives once. Nothing else here can see a flake, and this is the
/// shape of the one that can -- the same press, after the same beginning, as
/// many times as it takes for a coin to have to come up heads every time.
pub const EVERY_TIME: Check = Check {
    name: "240-the-keyboard-comes-back-with-the-desktop",
    about: "X raises the keyboard on every one of twenty restarts, not most of them.",
    feature: "keyboard",
    since: "2026-08-31",
    bodies: &[Body::Device(every_time)],
};

/// How many times the desktop is brought up again.
///
/// Twenty because of what it is looking for. The fault was met twice by hand,
/// once failing and once not, so a coin is the honest guess at its odds; twenty
/// rounds of a coin that lands wrong a third of the time comes up clean about
/// three times in ten thousand. At one in ten it is nearer one in eight, which
/// is the number to remember before reading a pass here as proof.
const RESTARTS: usize = 20;

/// How long the desktop is given to come back.
///
/// Generous, because the last thing to happen on the way up is the controller
/// loading the router, and `controller-profile` waits on InputPlumber reaching
/// the bus, which at login it has not yet done.
const UP: f64 = 90.0;

/// How long a press is given to be answered.
const ANSWERS: f64 = 4.0;

fn every_time(stage: &mut Device) -> Done {
    for round in 1..=RESTARTS {
        stage.user("systemctl --user restart console.target");

        // The profile is what says the way up has finished, because loading it
        // is the last thing that happens and it is the thing that takes the pad
        // away. Pressing before it lands would be this check racing the same
        // race rather than watching it.
        ought(stage.until(|seen| seen.profile() == "Router", UP), || {
            format!(
                "round {round} of {RESTARTS}: the controller never loaded the router, so nothing \
                 here has been asked yet. journalctl --user -u console-controller says why."
            )
        })?;

        // wvkbd is started --hidden and stays for the session, but a restart
        // leaves whatever the last round did on the screen.
        if stage.keyboard() {
            stage.press("x");
            stage.settle(1.5);
        }

        stage.press("x");
        ought(stage.until(Device::keyboard, ANSWERS), || {
            format!(
                "round {round} of {RESTARTS}: X did not raise the keyboard. The pad wvkbd opened \
                 is not the one X arrives on: console-keyboard.service is ordered After= the \
                 controller so the profile switch has already happened, and something has undone \
                 that or the fork has stopped looking again."
            )
        })?;

        stage.press("x");
        ought(stage.until(|seen| !seen.keyboard(), ANSWERS), || {
            format!(
                "round {round} of {RESTARTS}: X raised the keyboard and would not put it away, \
                 which is one pad read twice rather than none read at all."
            )
        })?;
    }
    Ok(())
}

/// The keyboard over a page, and the keys actually arriving in it.
///
/// Every other keyboard check here asks whether the slab is on the screen. That
/// is not the same question as whether it types, and the browser is the window
/// where the difference could bite: it is the one thing on this device that
/// draws its own text fields rather than using the toolkit everything else
/// does, and it is the window a password is typed into.
///
/// It cannot in fact tell the difference, and the reason is worth writing down.
/// wvkbd never types into a window: it makes a virtual keyboard at the
/// compositor, and the compositor hands the keys to whoever holds the focus. So
/// a browser sees the real keyboard. This asks anyway, because that is a
/// sentence about how it ought to work, and the page saying the letters back is
/// the only thing that knows whether it does.
pub const IN_A_PAGE: Check = Check {
    name: "250-the-keyboard-types-into-a-page",
    about: "X raises the keyboard over the browser, and what it types reaches the page.",
    feature: "keyboard",
    since: "2026-09-01",
    bodies: &[Body::Device(in_a_page)],
};

/// A page that says what it was given, in the one place a check can read from:
/// `hyprctl` repeats a window's title, and the field sets it.
///
/// No apostrophes anywhere in it. It is written to the device through a shell,
/// and a quote in here would end the string it travels inside.
const PAGE: &str = r#"<!doctype html><title>osk-check</title>
<input id=i autofocus style="font-size:32px;width:90%">
<script>
var i = document.getElementById("i");
i.focus();
i.addEventListener("input", function () { document.title = "GOT[" + i.value + "]"; });
</script>"#;

/// Short, and letters only. What is being asked is whether a key arrives at
/// all; a layout is `170-the-keyboard-draws` and not this.
const TYPED: &str = "hello";

/// How long the browser is given to be on the screen. It is the heaviest thing
/// this desktop starts, and on a handheld from cold it is not quick.
const OPENS: f64 = 45.0;

/// How long the letters are given to arrive once they have been sent.
const ARRIVES: f64 = 6.0;

fn in_a_page(stage: &mut Device) -> Done {
    let home = stage.home();
    let at = format!("{home}/.cache/console-osk-check.html");
    stage.user(&format!("mkdir -p {home}/.cache && printf %s '{PAGE}' > {at}"));

    let up = stage.open(&format!("librewolf --new-window file://{at}"), OPENS);
    ought(up, || {
        "the browser never came up, so nothing here has been asked yet".to_string()
    })?;
    ought(stage.until(|seen| seen.titles().iter().any(|title| title.contains("osk-check")), OPENS), || {
        "the browser came up on something other than the page this check wrote".to_string()
    })?;

    // Whatever the last thing to touch it left behind.
    if stage.keyboard() {
        stage.press("x");
        stage.settle(1.5);
    }
    stage.press("x");
    ought(stage.until(Device::keyboard, ARRIVES), || {
        "X did not raise the keyboard over the browser, though it does over everything else.          The browser is a layer surface away from the pad, not a profile away from it."
            .to_string()
    })?;

    stage.types(TYPED);
    let arrived = stage.until(
        |seen| seen.titles().iter().any(|title| title.contains(&format!("GOT[{TYPED}]"))),
        ARRIVES,
    );

    // Put the screen back the way it was found, pass or fail.
    stage.press("x");
    stage.settle(1.0);
    stage.user("pkill -x librewolf");
    stage.user(&format!("rm -f {at}"));

    ought(arrived, || {
        format!(
            "the keyboard came up over the browser and {TYPED:?} did not reach the field. The              keys go to whoever holds the focus, so either the page never had it or the browser              is not taking a virtual keyboard -- MOZ_ENABLE_WAYLAND is what decides the second."
        )
    })
}
