//! The panels, held by one program: it draws each of them, and holds nothing
//! between them.
//!
//! Two things can go wrong with a resident host and neither of them is visible
//! the first time someone opens a panel, which is why both are pressed here
//! rather than reasoned about.
//!
//! The first is a panel that was drawn once. Every opening builds its card
//! again from the same call its `main` used to make, and a host that kept a
//! window, a widget or an actor would draw the second opening out of the first
//! one's leavings. So the menu is opened, put away, another panel is opened,
//! and the menu is opened again over the top of it -- and the last of those
//! has to be the shape the first one was. Two openings would not have found
//! it; the third is the one that reads state no one rebuilt.
//!
//! The third is also the one that swaps. A panel asked for over another is
//! drawn before the one under it goes, so the screen is never empty between
//! them, and the lock the one going held has to end up with the one that came.
//! So the menu has to be the only thing left up, and B has to put it away: a
//! lock still naming the settings panel is a B that closes nothing.
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
//! And a panel the bar opened has to be let go of by the bar once it ends. The
//! bar used to hold only the last program a tap started, so every tap before it
//! was a dead entry in the process table for as long as the session lasted --
//! a few hundred after an evening of tapping, found on the device by counting.
//! So the clock is tapped open and shut twice, and once the calendar is gone
//! the bar has to have no child left that has ended and not been reaped.
//!
//! All three are written for the handheld and nowhere else. The nested desktop
//! takes one picture of one session and cannot put a panel away and open
//! another, so a check written for it would be a check about a sleep.

use console_core_never::Never;
use console_test_stages::checking::{Body, Check, CheckResult, empty, failed, happened, not_empty, same};
use console_test_stages::device::{Device, PATIENCE, Ready, Outcome};

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

pub const LEFT_NOTHING: Check = Check {
    name: "341-a-tapped-bar-leaves-nothing-behind",
    about: "Panels tapped open and shut from the bar leave no ended program under it.",
    feature: "panel",
    since: "2026-09-24",
    bodies: &[Body::Device(left_nothing)],
};

const MENU: &str = "launcher";

const OPENS_THE_MENU: &str = "left-paddle-top";

const OPENS_THE_PANEL: &str = "legion-right";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Layer<'a>(&'a str);

fn opened(stage: &mut Device, button: &str, who: Layer<'_>) -> Result<Option<(u32, u32)>, Never> {
    let Ok(()) = stage.press(button);
    let Ok(drawn) = stage.drawn(PATIENCE);

    match drawn {
        Outcome::RanOut => return Ok(None),
        Outcome::Happened => {},
    }

    let Ok(where_) = stage.layer(who.0);

    Ok(where_.map(|(_, _, wide, tall)| (wide, tall)))
}

fn alone(seen: &mut Device, who: Layer<'_>) -> Result<Ready, Never> {
    let Ok(up) = seen.menus();

    Ok(match up.as_slice() == [who.0] {
        true => Ready::Yes,
        false => Ready::NotYet,
    })
}

fn console_put_away(stage: &mut Device) -> CheckResult {
    let Ok(()) = stage.press("b");
    let Ok(gone) = stage.closed(PATIENCE);

    happened(gone, || "B did not put the panel away".to_string())
}

fn again(stage: &mut Device) -> CheckResult {
    let Ok(first) = opened(stage, OPENS_THE_MENU, Layer(MENU));

    let first = match first {
        Some(first) => first,
        None => return failed("the menu did not draw at all, so nothing here was answered".to_string()),
    };

    console_put_away(stage)?;

    let Ok(between) = opened(stage, OPENS_THE_PANEL, Layer("settings-panel"));

    match between {
        Some(_) => {},
        None => {
            return failed(
                "the settings panel did not draw, so the menu was never opened over anything"
                    .to_string(),
            );
        }
    }

    let Ok(()) = stage.press(OPENS_THE_MENU);
    let Ok(replaced) = stage.until::<Never>(|seen| alone(seen, Layer(MENU)), PATIENCE);
    let Ok(up) = stage.menus();

    happened(replaced, || {
        format!("the menu was opened over the settings panel and the screen was left holding {up:?}")
    })?;

    let Ok(after) = stage.layer(MENU);

    let after = match after.map(|(_, _, wide, tall)| (wide, tall)) {
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

    console_put_away(stage)
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

fn holds_nothing(stage: &mut Device) -> CheckResult {
    let Ok(_) = stage.until::<Never>(
        |seen| {
            let Ok(up) = host(seen);

            Ok(match up.is_empty() {
                true => Ready::NotYet,
                false => Ready::Yes,
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
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        },
        PATIENCE,
    );

    let Ok(left) = held(stage);

    empty(&left, || format!("{HOST} is holding a child with every panel closed: {left:?}"))
}

const BAR: &str = "console-bar";

const TAPS: u32 = 4;

fn ended_under_the_bar(seen: &mut Device) -> Result<Vec<String>, Never> {
    let Ok(said) = seen.user(&format!(
        "ps -o pid=,stat=,comm= --ppid \"$(pgrep -x {BAR} | head -1)\" | awk '$2 ~ /^Z/' || true"
    ));

    lines(&said)
}

fn left_nothing(stage: &mut Device) -> CheckResult {
    let Ok(bar) = stage.layer(console_onscreen::BAR);

    let clock = match bar {
        Some((across, down, wide, tall)) => (
            across.saturating_add(wide.saturating_div(2)),
            down.saturating_add(tall.saturating_div(2)),
        ),
        None => return failed(format!("{BAR} is not on the screen, so nothing was tapped")),
    };

    for _tap in 0..TAPS {
        stage.touch(clock)?;
    }

    let Ok(gone) = stage.until::<Never>(
        |seen| {
            let Ok(up) = seen.menus();

            Ok(match up.is_empty() {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        },
        PATIENCE,
    );
    let Ok(up) = stage.menus();

    happened(gone, || format!("the clock was tapped {TAPS} times and {up:?} is still up"))?;

    let Ok(_) = stage.until::<Never>(
        |seen| {
            let Ok(left) = ended_under_the_bar(seen);

            Ok(match left.is_empty() {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        },
        PATIENCE,
    );

    let Ok(left) = ended_under_the_bar(stage);

    empty(&left, || {
        let first: Vec<&str> = left.iter().take(3).map(|one| one.trim()).collect();

        format!("{BAR} started {} programs that ended and were never reaped, first {first:?}", left.len())
    })
}
