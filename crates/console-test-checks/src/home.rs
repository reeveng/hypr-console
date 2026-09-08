//! The home screen: whose buttons are whose, and whether the bar can be
//! pressed while it is drawn.
//!
//! Both of these are one fault, found on the device after everything about it
//! had been asked from the wrong side and answered yes. The home screen asked
//! the compositor for the keyboard exclusively, which is the only way a layer
//! drawn under everything can have it -- and Hyprland answers an exclusive
//! layer by handing it every pointer and every touch on the screen, wherever
//! they land, because that is what a lock screen needs. So a finger on the
//! launcher, the keyboard, the music or the sound reached the home screen
//! instead, which opened whatever the highlight happened to be standing on.
//!
//! Nothing caught it, and the reason is worth keeping. Every check that
//! touched this asked about plumbing: the bar's surface is there, at the right
//! size, on the right layer, with the right modules, and the daemon binds the
//! buttons it says it binds. Every one of those answers was yes. None of them
//! is a finger. `PRESSABLE` is the one that presses, and it is the shape any
//! check of a surface that answers a touch should have.
//!
//! `POINTED` is the same shape with the other hand. The squares answer a
//! pointer as well as a d-pad, and until `console-point` there was nothing but
//! a person to move one, so the hover was written and never pressed by
//! anything. It runs in the emulator as well as on the device, because what
//! says the highlight moved is the colour of the screen and both of them have
//! one. It reads the same edge `ARRANGING` reads, for the same reason and so
//! that there is one answer here to where the highlight is standing: a square
//! the pointer has raised is a square wearing `#square.here`, and that rule
//! is the border.
//!
//! That colour is what the waiting is made of too. A repaint is not a thing the
//! home screen announces, so there is nothing to ask but the screen itself, and
//! the checks here used to hand that to a number of seconds instead. They read
//! it now: `settled` waits for the same reading twice running, which is a
//! screen that has finished arriving, and `newly` waits for a square that was
//! not lit to light, or for one that lit to go out. Each is the assertion after
//! it asked as the wait, which is why neither carries a message of its own.
//!
//! `ARRANGING` asks the machine and never looks at it.
//!
//! What it is checking is where an application is, and where an application is
//! is a line in the home screen's own file. Every version of this before it
//! asserted exactly that and then spent its whole patience reading the screen
//! anyway, for two things that were never the assertion: which square the
//! highlight was standing on, and whether the square was in the hand yet.
//!
//! The first does not need reading, because the grid has walls. `moved`
//! saturates at both ends of a row and both ends of a pane, so a d-pad walked
//! left and then up far enough *is* the first square, wherever it started
//! from, and there is nothing to confirm. So the icon in the top left corner
//! is carried to the bottom right corner, and then carried back, and the file
//! says both times. How far is far enough is worked out from the shape the
//! person chose rather than counted into this file: a home screen five across
//! is not one nine across, and a walk written for the first stops halfway
//! across the second.
//!
//! The walk is a press per square and not a held d-pad, and that is not a
//! preference. InputPlumber cannot hold a button down at all -- `SendEvent`
//! panics on its own runtime rather than emitting anything -- so the hold this
//! was written as was a hold nothing ever felt, and the highlight never left
//! the square it started on. Pressing also says what it means: the number in
//! the message is squares, which is what a grid is measured in, rather than a
//! number of seconds worked back from the key-repeat timing and wrong the
//! moment somebody changes it.
//!
//! The corner it is carried to is a pane further out than the file has. A
//! square in the hand adds one to what `shown` answers, so there is always an
//! empty pane past the last one to put something on, and the bottom right of
//! the home screen while you are holding something is on it. That is the
//! behaviour and this asserts it rather than working around it; the way back
//! is the same walk in reverse and the pane goes when the square leaves it.
//!
//! The second is the only thing here the file cannot say, and the answer was
//! to say it. The card sends `carry` on its way out, so the card being gone is
//! a moment before the square is in the hand, and a d-pad press in that gap
//! walks the highlight and then lifts whatever it landed on -- the wrong icon
//! moves, and the check goes red for a race. `console_onscreen::carrying` is
//! that moment written down, the way being awake already was, and asking for
//! it costs one question rather than a screenshot.
//!
//! What that took away is worth naming, because it will be proposed again. The
//! screen was read for `panel`, which is a dark neutral, against a wallpaper
//! that is an animated photograph -- so the reading was hundreds of points of
//! somebody's picture, moving, and the highlight was never in it. Reading the
//! `mint` edge instead of the plate would have worked, and it would still have
//! been a screenshot per look, on a machine somebody else is using, for a fact
//! the home screen could simply have said.

use std::collections::BTreeSet;

use console_home_screen::{Holding, Home, Spot};
use console_core_never::Never;
use console_test_stages::checking::{
    Body, Check, Done, cannot, empty, failed, happened, less_than, same, seen,
};
use console_test_stages::desktop::Desktop;
use console_test_stages::device::{Device, PATIENCE, Seen, Waited};
use console_test_stages::here::{Here, TURNS};
use console_test_stages::palette::palette;

const THE_HOME_SCREEN: &str = r#"{"eDP-1":{"levels":{
    "0":[{"namespace":"awww-daemon","h":1600},{"namespace":"console-home","h":1562}],
    "2":[{"namespace":"waybar","h":38},{"namespace":"updating","h":2}]}}}"#;

pub const WHOSE_BUTTONS: Check = Check {
    name: "260-the-home-screens-buttons",
    about: "A is the pointer's until the d-pad wakes the home screen, and the \
            home screen's after.",
    feature: "home",
    since: "2026-09-03",
    bodies: &[Body::Here(whose_here), Body::Device(whose_there)],
};

pub const ARRANGING: Check = Check {
    name: "290-arranging-the-home-screen",
    about: "Y on a square offers to move it, the d-pad carries it, and A puts \
            it down where the highlight is.",
    feature: "home",
    since: "2026-09-04",
    bodies: &[Body::Device(arranging_there)],
};

pub const PRESSABLE: Check = Check {
    name: "270-the-bar-answers-a-finger",
    about: "A tap on the bar opens what the bar says it opens, with the home \
            screen drawn under it.",
    feature: "home",
    since: "2026-09-03",
    bodies: &[Body::Device(pressable_there)],
};

pub const POINTED: Check = Check {
    name: "310-the-pointer-stands-on-a-square",
    about: "The square under the pointer is lit the way the d-pad lights one, \
            and the home screen stays asleep while it is.",
    feature: "home",
    since: "2026-09-05",
    bodies: &[Body::Desktop(pointed_here), Body::Device(pointed_there)],
};

fn whose_here(stage: &mut Here) -> Done {
    stage.showing(THE_HOME_SCREEN)?;

    stage.press("a")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(clicked) = stage.sent(evdev::EventType::KEY, evdev::KeyCode::BTN_LEFT.0, 1);

    seen(clicked, || "A on a sleeping home screen was not the pointer's button".to_string())?;

    let Ok(asleep) = stage.told();

    empty(asleep, || format!("the home screen was told {asleep:?} while asleep"))?;

    let Ok(()) = stage.fresh();

    stage.press("dpad-right")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(walked) = stage.told();

    same(&walked, &[console_onscreen::Said::Right].as_slice(), || {
        format!("the d-pad said {walked:?} to the home screen")
    })?;

    let Ok(()) = stage.fresh();

    stage.press("a")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(awake) = stage.told();

    same(&awake, &[console_onscreen::Said::Pressed].as_slice(), || {
        format!("A on an awake home screen said {awake:?}")
    })?;

    empty(&stage.written, || "A was still the pointer's button".to_string())?;

    let Ok(()) = stage.fresh();

    stage.press("b")?;

    let Ok(()) = stage.settle(TURNS);

    stage.press("a")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(back) = stage.sent(evdev::EventType::KEY, evdev::KeyCode::BTN_LEFT.0, 1);

    seen(back, || "B did not give A back to the pointer".to_string())
}

fn whose_there(stage: &mut Device) -> Done {
    cleared(stage)?;
    let Ok(holding) = stage.home_awake();

    same(&holding, &Seen::NotYet, || {
        "the home screen was holding a highlight with nobody having asked for one".to_string()
    })?;

    let Ok(()) = stage.press("dpad-right");
    let Ok(_) = stage.until(Device::home_awake, PATIENCE);
    let Ok(woken) = stage.home_awake();

    same(&woken, &Seen::Yes, || "the d-pad did not wake the home screen".to_string())?;

    let Ok(()) = stage.press("y");
    let Ok(_) = showing(stage, "home-square");
    let Ok(up) = stage.menus();
    let Ok(on_screen) = on_screen(&up, "home-square");

    seen(on_screen, || format!("Y on an awake home screen opened {up:?}"))?;

    let Ok(()) = stage.press("b");
    let Ok(_) = stage.gone(PATIENCE);
    let Ok(()) = stage.press("b");
    let Ok(_) = stage.until(asleep, PATIENCE);
    let Ok(away) = stage.home_awake();

    same(&away, &Seen::NotYet, || "B did not put the highlight away".to_string())
}

fn pressable_there(stage: &mut Device) -> Done {
    cleared(stage)?;
    let Ok(bar) = stage.layer("waybar");

    let (left, top, _taken_2, tall) = match bar {
        Some((left, top, _taken_2, tall)) => (left, top, _taken_2, tall),
        None => return cannot("the bar is not on the screen to be pressed"),
    };

    let at = (
        left.saturating_add(tall.saturating_div(2)),
        top.saturating_add(tall.saturating_div(2)),
    );

    let Ok(before) = stage.menus();

    empty(&before, || format!("something was already up: {before:?}"))?;

    stage.touch(at)?;

    let Ok(_) = showing(stage, "launcher");
    let Ok(up) = stage.menus();
    let Ok(first) = on_screen(&up, "launcher");

    seen(first, || format!("a finger on the launcher icon at {at:?} opened {up:?}"))?;

    cleared(stage)?;

    stage.touch(at)?;

    let Ok(_) = showing(stage, "launcher");
    let Ok(again) = stage.menus();
    let Ok(second) = on_screen(&again, "launcher");

    seen(second, || format!("the second finger on the launcher icon opened {again:?}"))?;

    cleared(stage)?;

    Ok(())
}

fn cleared(stage: &mut Device) -> Done {
    let Ok(()) = stage.fresh();
    let Ok(ready) = stage.until::<Never>(
        |seen| {
            let Ok(menus) = seen.menus();
            let Ok(worn) = seen.profile();

            Ok(match menus.is_empty() && worn == crate::chooser::WORN {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        },
        PATIENCE,
    );

    happened(ready, || {
        let Ok(left) = stage.menus();
        let Ok(worn) = stage.profile();

        format!(
            "clearing the screen left {left:?} on it and the pad wearing {worn}, so a press \
             would not reach the home screen"
        )
    })
}

fn arranging_there(stage: &mut Device) -> Done {
    cleared(stage)?;
    let Ok(before) = placed(stage);
    let Ok(holding) = before.holding();

    match holding {
        Holding::Something => {},
        Holding::Nothing => return cannot("the home screen has nothing on it to move"),
    }

    let Ok(shape) = shaped(stage);
    let Ok(panes) = before.panes();

    let first = Spot::FIRST;
    let Ok(far) = Spot::new(
        panes,
        shape.rows.saturating_sub(1),
        shape.columns.saturating_sub(1),
    );

    let one = match before.at(first) {
        Ok(Some(one)) => one.to_string(),
        Ok(None) | Err(_) => {
            return cannot("the first square of the first pane has nothing on it to carry");
        }
    };

    let Ok(()) = stage.press("dpad-right");
    let Ok(woke) = stage.until(Device::home_awake, PATIENCE);

    happened(woke, || {
        let Ok(up) = stage.menus();
        let Ok(here) = stage.windows_here();

        format!(
            "the d-pad did not wake the home screen, with {up:?} drawn over it and {here} \
             windows on the workspace"
        )
    })?;

    let Ok(there) = far_corner(shape, panes);
    let Ok(back) = first_corner(shape, panes);

    walking(stage, &back)?;

    carried(stage, &there)?;

    let Ok(after) = placed(stage);
    let Ok(went) = after.where_(&one);

    same(&went, &Some(far), || {
        format!(
            "{one:?} was carried from {first:?} to the far corner and the home screen keeps it \
             at {went:?}"
        )
    })?;

    carried(stage, &back)?;

    let Ok(again) = placed(stage);

    same(&again, &before, || {
        "carried back, the home screen is not the one this started with".to_string()
    })?;

    let Ok(()) = stage.press("b");
    let Ok(_) = stage.until(asleep, PATIENCE);

    Ok(())
}

fn shaped(stage: &mut Device) -> Result<console_home_screen::Shape, Never> {
    let Ok(home) = stage.home();
    let Ok(at) = console_home_screen::shape::at(std::path::Path::new(&home));
    let Ok(said) = stage.user(&format!("cat {} 2>/dev/null", at.display()));

    console_home_screen::Shape::read(&said)
}

fn first_corner(
    shape: console_home_screen::Shape,
    panes: usize,
) -> Result<Vec<(&'static str, usize)>, Never> {
    let Ok(across) = crossing(shape, panes);

    Ok(vec![("dpad-left", across), ("dpad-up", shape.rows)])
}

fn far_corner(
    shape: console_home_screen::Shape,
    panes: usize,
) -> Result<Vec<(&'static str, usize)>, Never> {
    let Ok(across) = crossing(shape, panes);

    Ok(vec![("dpad-right", across), ("dpad-down", shape.rows)])
}

fn crossing(shape: console_home_screen::Shape, panes: usize) -> Result<usize, Never> {
    Ok(shape.columns.saturating_mul(panes.saturating_add(1)))
}

fn walking(stage: &mut Device, ways: &[(&str, usize)]) -> Done {
    for (way, steps) in ways {
        let Ok(()) = stage.presses(way, *steps);
    }

    Ok(())
}

fn carried(stage: &mut Device, ways: &[(&str, usize)]) -> Done {
    let Ok(was) = placed(stage);
    let Ok(awake) = stage.home_awake();

    same(&awake, &Seen::Yes, || {
        "the home screen is asleep, so Y is the desktop's menu rather than what else can be \
         done with the square"
            .to_string()
    })?;

    let Ok(()) = stage.press("y");
    let Ok(_) = showing(stage, "home-square");
    let Ok(up) = stage.menus();
    let Ok(on_screen) = on_screen(&up, "home-square");

    seen(on_screen, || format!("Y on the square under the highlight opened {up:?}"))?;

    let Ok(()) = stage.press("a");
    let Ok(went) = stage.gone(PATIENCE);

    happened(went, || {
        let Ok(up) = stage.menus();

        format!("A on the card left {up:?} on the screen, so the card said nothing on its way out")
    })?;

    let Ok(lifted) = stage.until(Device::home_carrying, PATIENCE);

    happened(lifted, || {
        "the card was answered and the home screen says its hand is empty, so there is nothing \
         to carry and the d-pad would only walk the highlight"
            .to_string()
    })?;

    walking(stage, ways)?;

    let Ok(()) = stage.press("a");
    let Ok(landed) = stage.changed(placed, &was, PATIENCE);

    happened(landed, || {
        let Ok(hand) = stage.home_carrying();

        format!(
            "the square was carried {ways:?} and put down, what the home screen keeps is what it \
             was before, and its hand says {hand:?}"
        )
    })
}

fn placed(stage: &mut Device) -> Result<Home, Never> {
    let home = stage.home()?;
    let at = console_home_screen::file(std::path::Path::new(&home))?;
    let said = stage.user(&format!("cat {} 2>/dev/null", at.display()))?;

    Home::read(&said)
}

fn asleep(seen: &mut Device) -> Result<Seen, Never> {
    let Ok(awake) = seen.home_awake();

    awake.flipped()
}

fn showing(stage: &mut Device, namespace: &str) -> Result<Waited, Never> {
    stage.until(
        |seen| {
            let Ok(up) = seen.menus();

            on_screen(&up, namespace)
        },
        PATIENCE,
    )
}

fn on_screen(up: &[String], namespace: &str) -> Result<Seen, Never> {
    Ok(match up.iter().any(|name| name == namespace) {
        true => Seen::Yes,
        false => Seen::NotYet,
    })
}

const STANDING: &str = "mint";

const ACROSS: usize = 1;
const DOWN: usize = 8;

const EDGE: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lit {
    Somewhere,
    Nowhere,
}

fn settled(
    stage: &mut Device,
    over: (u32, u32, u32, u32),
    spent: &str,
) -> Result<BTreeSet<u32>, String> {
    let mut twice = None;
    let mut now = BTreeSet::new();

    let waited = stage.until::<String>(
        |seen| {
            let Ok(()) = seen.again();

            let found = lit(over, spent, |across, down| seen.colour(across, down))?;
            let same = twice.as_ref() == Some(&found);

            twice = Some(found.clone());
            now = found;

            Ok(match same {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        },
        PATIENCE,
    )?;

    match waited {
        Waited::Happened => Ok(now),
        Waited::RanOut => {
            Err("the home screen went on repainting with the pointer standing still".to_string())
        },
    }
}

fn newly(
    stage: &mut Device,
    over: (u32, u32, u32, u32),
    spent: &str,
    before: &BTreeSet<u32>,
    want: Lit,
) -> Result<BTreeSet<u32>, String> {
    let mut found = BTreeSet::new();

    let _waited = stage.until::<String>(
        |seen| {
            let Ok(()) = seen.again();

            let now = lit(over, spent, |across, down| seen.colour(across, down))?;

            found = now.difference(before).copied().collect();

            let lighting = match found.is_empty() {
                true => Lit::Nowhere,
                false => Lit::Somewhere,
            };

            Ok(match lighting == want {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        },
        PATIENCE,
    )?;

    Ok(found)
}

fn lit(
    over: (u32, u32, u32, u32),
    spent: &str,
    colour: impl FnMut(f64, f64) -> Result<String, String>,
) -> Result<BTreeSet<u32>, String> {
    let found = lit_at(over, spent, colour)?;

    Ok(found.into_iter().map(|(across, _down)| across).collect())
}

fn lit_at(
    over: (u32, u32, u32, u32),
    spent: &str,
    mut colour: impl FnMut(f64, f64) -> Result<String, String>,
) -> Result<BTreeSet<(u32, u32)>, String> {
    let Ok(every) = palette();

    let plate = match every.get(spent) {
        Some(plate) => plate,
        None => return Err(format!("the palette spends no {spent} for a square to be read by")),
    };

    let (left, top, wide, tall) = over;
    let band = top..top.saturating_add(tall);
    let side = left..left.saturating_add(wide);
    let mut found = BTreeSet::new();

    for down in band.step_by(DOWN) {
        for across in side.clone().step_by(ACROSS) {
            let said = colour(f64::from(across), f64::from(down))?;

            match &said == plate {
                true => {
                    let _ = found.insert((across, down));
                },
                false => {},
            }
        }
    }

    Ok(found)
}

fn reaches(found: &BTreeSet<u32>) -> Result<Option<(u32, u32)>, Never> {
    let first = match found.iter().next() {
        Some(first) => first,
        None => return Ok(None),
    };

    let last = match found.iter().next_back() {
        Some(last) => last,
        None => return Ok(None),
    };

    Ok(Some((*first, *last)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Under {
    ThePointer,
    SomewhereElse,
}

fn under(reaches: (u32, u32), at: u32) -> Result<Under, Never> {
    Ok(match reaches.0 <= at && at <= reaches.1 {
        true => Under::ThePointer,
        false => Under::SomewhereElse,
    })
}

fn stood_on(reaches: (u32, u32), at: (u32, u32), wide: u32) -> Done {
    let Ok(under) = under(reaches, at.0);

    same(&under, &Under::ThePointer, || {
        format!("the pointer stood at {at:?} and what lit up was {reaches:?}")
    })?;

    less_than(reaches.1.saturating_sub(reaches.0), wide.saturating_div(2), || {
        format!("{reaches:?} is wider than a square, so more than one is lit")
    })
}

fn pointed_here(stage: &mut Desktop) -> Done {
    let screen = console_test_stages::screen()?;
    let Ok(room) = screen.logical();

    let over = (0, 0, room.0, room.1);
    let at = (room.0.saturating_div(2), room.1.saturating_div(2));

    stage.open("console-home")?;
    stage.point(at)?;

    let found = lit(over, STANDING, |across, down| stage.colour(across, down))?;

    let reached = match reaches(&found) {
        Ok(Some(reached)) => reached,
        Ok(None) | Err(_) => {
            return failed(format!(
                "the pointer stood at {at:?} on the home screen and no square lit up"
            ));
        }
    };

    stood_on(reached, at, room.0)
}

fn pointed_there(stage: &mut Device) -> Done {
    cleared(stage)?;
    let Ok(drawn) = stage.layer("console-home");

    let over = match drawn {
        Some(over) => over,
        None => return cannot("the home screen is not drawn, so there is nothing to point at"),
    };

    let middle = over.1.saturating_add(over.3.saturating_div(2));
    let away = (over.0.saturating_add(EDGE), middle);
    let at = (over.0.saturating_add(over.2.saturating_div(2)), middle);

    stage.point(away)?;

    let before = settled(stage, over, STANDING)?;

    stage.point(at)?;

    let found = newly(stage, over, STANDING, &before, Lit::Somewhere)?;

    let reached = match reaches(&found) {
        Ok(Some(reached)) => reached,
        Ok(None) | Err(_) => {
            return failed(format!(
                "the pointer moved from {away:?} to {at:?} and no square lit up"
            ));
        }
    };

    stood_on(reached, at, over.2)?;

    let Ok(awake) = stage.home_awake();

    same(&awake, &Seen::NotYet, || {
        "the pointer woke the home screen, so A is the home screen's and not the pointer's"
            .to_string()
    })?;

    stage.point(away)?;

    let last = newly(stage, over, STANDING, &before, Lit::Nowhere)?;

    let Ok(still) = reaches(&last);

    match still {
        Some(reached) => {
            failed(format!("the pointer left the squares and {reached:?} is lit without it"))
        },
        None => Ok(()),
    }
}
