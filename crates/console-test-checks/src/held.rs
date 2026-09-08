//! The panels, held by one program: it draws each of them, and holds nothing
//! between them.
//!
//! Two things can go wrong with a resident host and neither of them is visible
//! the first time somebody opens a panel, which is why both are pressed here
//! rather than reasoned about.
//!
//! The first is a panel that was drawn once. Every opening builds its card
//! again from the same call its `main` used to make, and a host that kept a
//! window, a widget or an actor would draw the second opening out of the first
//! one's leavings. So the menu is opened, put away, another panel is opened
//! over the top of it, and the menu is opened again -- and the last of those
//! has to be the shape the first one was. Two openings would not have found
//! it; the third is the one that reads state nobody rebuilt.
//!
//! What is asked is the compositor, not a picture. A surface's namespace and
//! its size are what `hyprctl layers` reports, and a menu drawn out of the
//! settings panel's leavings is a menu of the wrong height -- which is a
//! difference the machine can state, where "the rows look wrong" is not.
//!
//! The second is a panel that was closed. What a panel holds while it is up --
//! a `busctl monitor`, a `pactl subscribe`, a thread of its own -- used to be
//! bounded by the process ending. It is not any more, and the whole argument
//! for a warm host on a handheld is that it costs nothing while nothing is on
//! the screen. So with every panel closed the host is asked what it is still
//! holding, and the answer has to be nothing.
//!
//! Both are written for the handheld and nowhere else. The nested desktop
//! takes one picture of one session and cannot put a panel away and open
//! another, so a check written for it would be a check about a sleep.

use console_core_never::Never;
use console_test_stages::checking::{Body, Check, Done, empty, failed, happened, not_empty, same};
use console_test_stages::device::{Device, PATIENCE, Seen, Waited};

pub const AGAIN: Check = Check {
    name: "330-a-panel-opened-again-is-drawn-again",
    about: "A panel opened, put away, and opened again after another one is the shape it was.",
    feature: "panel",
    since: "2026-09-06",
    bodies: &[Body::Device(again)],
};

pub const HOLDS_NOTHING: Check = Check {
    name: "340-a-closed-panel-is-holding-nothing",
    about: "With every panel closed, the program that holds them has no child left running.",
    feature: "panel",
    since: "2026-09-06",
    bodies: &[Body::Device(holds_nothing)],
};

const MENU: &str = "launcher";

const OPENS_THE_MENU: &str = "left-paddle-top";

const OPENS_THE_PANEL: &str = "legion-right";

fn opened(stage: &mut Device, button: &str, who: &str) -> Result<Option<(u32, u32)>, Never> {
    let Ok(()) = stage.press(button);
    let Ok(drawn) = stage.drawn(PATIENCE);

    match drawn {
        Waited::RanOut => return Ok(None),
        Waited::Happened => {},
    }

    let Ok(where_) = stage.layer(who);

    Ok(where_.map(|(_, _, wide, tall)| (wide, tall)))
}

fn put_away(stage: &mut Device) -> Done {
    let Ok(()) = stage.press("b");
    let Ok(gone) = stage.gone(PATIENCE);

    happened(gone, || "B did not put the panel away".to_string())
}

fn again(stage: &mut Device) -> Done {
    let Ok(first) = opened(stage, OPENS_THE_MENU, MENU);

    let first = match first {
        Some(first) => first,
        None => return failed("the menu did not draw at all, so nothing here was answered".to_string()),
    };

    put_away(stage)?;

    let Ok(between) = opened(stage, OPENS_THE_PANEL, "settings-panel");

    match between {
        Some(_) => {},
        None => {
            return failed(
                "the settings panel did not draw, so the menu was never opened over anything"
                    .to_string(),
            );
        }
    }

    put_away(stage)?;

    let Ok(after) = opened(stage, OPENS_THE_MENU, MENU);

    let after = match after {
        Some(after) => after,
        None => {
            return failed(
                "the menu drew once and would not draw again after another panel had".to_string(),
            );
        }
    };

    same(&after, &first, || {
        format!("the menu was {first:?} and opened again as {after:?}, out of the last one's leavings")
    })?;

    put_away(stage)
}

const HOST: &str = "console-panels";

fn host(seen: &mut Device) -> Result<Vec<String>, Never> {
    let Ok(said) = seen.user(&format!("pgrep -x {HOST} || true"));

    lines(&said)
}

fn held(seen: &mut Device) -> Result<Vec<String>, Never> {
    let Ok(said) =
        seen.user(&format!("pgrep -P \"$(pgrep -x {HOST} | head -1)\" -a || true"));

    lines(&said)
}

fn lines(said: &str) -> Result<Vec<String>, Never> {
    Ok(said
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect())
}

fn holds_nothing(stage: &mut Device) -> Done {
    let Ok(_) = stage.until::<Never>(
        |seen| {
            let Ok(up) = host(seen);

            Ok(match up.is_empty() {
                true => Seen::NotYet,
                false => Seen::Yes,
            })
        },
        PATIENCE,
    );

    let Ok(up) = host(stage);

    not_empty(&up, || {
        format!("{HOST} is not up, so nothing here was asked about what it holds")
    })?;

    let Ok(_) = stage.until::<Never>(
        |seen| {
            let Ok(left) = held(seen);

            Ok(match left.is_empty() {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        },
        PATIENCE,
    );

    let Ok(left) = held(stage);

    empty(&left, || format!("{HOST} is holding a child with every panel closed: {left:?}"))
}
