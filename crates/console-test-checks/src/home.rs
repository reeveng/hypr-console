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
//! one.
//!
//! That colour is what the waiting is made of too. A repaint is not a thing the
//! home screen announces, so there is nothing to ask but the screen itself, and
//! the checks here used to hand that to a number of seconds instead. They read
//! it now: `settled` waits for the same reading twice running, which is a
//! screen that has finished arriving, and `newly` waits for a square that was
//! not lit to light, or for one that lit to go out. Each is the assertion after
//! it asked as the wait, which is why neither carries a message of its own.

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

    let Some((left, top, _, tall)) = bar else {
        return cannot("the bar is not on the screen to be pressed");
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

    let first = Spot::FIRST;
    let Ok(along) = Spot::new(0, 0, 1);

    let (Ok(Some(one)), Ok(Some(two))) = (before.at(first), before.at(along)) else {
        return cannot("the first row of the first pane has not two applications on it");
    };

    let (one, two) = (one.to_string(), two.to_string());

    let Ok(()) = stage.press("dpad-right");
    let Ok(_) = stage.until(Device::home_awake, PATIENCE);

    into_the_corner(stage)?;

    let Ok(()) = stage.press("y");
    let Ok(_) = showing(stage, "home-square");
    let Ok(up) = stage.menus();
    let Ok(on_screen) = on_screen(&up, "home-square");

    seen(on_screen, || format!("Y on the first square opened {up:?}"))?;

    let Ok(()) = stage.press("a");
    let Ok(_) = stage.gone(PATIENCE);
    let Ok(holding) = stage.home_awake();

    same(&holding, &Seen::Yes, || {
        "the card closed and left no highlight holding anything".to_string()
    })?;

    swapped(stage, "dpad-right")?;

    let Ok(after) = placed(stage);

    let Ok(moved) = after.at(along);

    let Ok(stayed) = after.at(first);

    same(&moved, &Some(one.as_str()), || {
        format!("{one:?} was carried one square along and {moved:?} is there")
    })?;
    same(&stayed, &Some(two.as_str()), || {
        format!("{two:?} did not move over to make room; {stayed:?} is there")
    })?;

    let Ok(()) = stage.press("y");
    let Ok(_) = showing(stage, "home-square");
    let Ok(()) = stage.press("a");
    let Ok(_) = stage.gone(PATIENCE);
    swapped(stage, "dpad-left")?;

    let Ok(back) = placed(stage);

    same(&back, &before, || {
        "carried back, the home screen is not the one this started with".to_string()
    })?;

    let Ok(()) = stage.press("b");
    let Ok(_) = stage.until(asleep, PATIENCE);

    Ok(())
}

fn swapped(stage: &mut Device, way: &str) -> Done {
    let Ok(was) = placed(stage);
    let Ok(drawn) = stage.layer("console-home");

    let Some(over) = drawn else {
        return cannot("the home screen is not drawn, so nothing is being carried on it");
    };

    let before = lit(over, |across, down| stage.colour(across, down))?;
    let Ok(()) = stage.press(way);

    let _carried = newly(stage, over, &before, Lit::Somewhere)?;

    let Ok(()) = stage.press("a");
    let Ok(_) = stage.changed(placed, &was, PATIENCE);

    Ok(())
}

const ROUNDS: usize = 4;

fn into_the_corner(stage: &mut Device) -> Done {
    let Ok(drawn) = stage.layer("console-home");

    let Some(over) = drawn else {
        return cannot("the home screen is not drawn, so there is no corner to walk to");
    };

    walked_into(stage, "dpad-left", 2.5, over)?;
    walked_into(stage, "dpad-up", 1.5, over)?;

    Ok(())
}

fn walked_into(
    stage: &mut Device,
    way: &str,
    seconds: f64,
    over: (u32, u32, u32, u32),
) -> Done {
    for _round in 0..ROUNDS {
        let Ok(()) = walked(stage, way, seconds);

        let before = settled_at(stage, over)?;
        let Ok(()) = stage.press(way);
        let after = settled_at(stage, over)?;

        match after == before {
            true => return Ok(()),
            false => {},
        }
    }

    failed(format!(
        "the highlight was still walking {way} after {ROUNDS} goes at it, so nothing here \
         knows where it is standing"
    ))
}

fn walked(stage: &mut Device, way: &str, seconds: f64) -> Result<(), Never> {
    let Ok(()) = stage.hold(way);

    #[cfg_attr(
        dylint_lib = "explicit022_no_settling",
        allow(
            explicit022_no_settling,
            reason = "the duration is the press: this is how long the d-pad is held down, and the walk it makes is as long as the hold"
        )
    )]
    let Ok(()) = stage.settle(seconds);

    let Ok(()) = stage.release(Some(way));

    #[cfg_attr(
        dylint_lib = "explicit022_no_settling",
        allow(
            explicit022_no_settling,
            reason = "the highlight is still walking when the key comes up, and a walk stopping is not a thing the home screen says: this is the window the repeats have to stop in"
        )
    )]
    let Ok(()) = stage.settle(0.4);

    Ok(())
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

const LIT: &str = "panel";

const ACROSS: usize = 2;
const DOWN: usize = 8;

const EDGE: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lit {
    Somewhere,
    Nowhere,
}

fn settled(stage: &mut Device, over: (u32, u32, u32, u32)) -> Result<BTreeSet<u32>, String> {
    let mut twice = None;
    let mut now = BTreeSet::new();

    let waited = stage.until::<String>(
        |seen| {
            let Ok(()) = seen.again();

            let found = lit(over, |across, down| seen.colour(across, down))?;
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

fn settled_at(
    stage: &mut Device,
    over: (u32, u32, u32, u32),
) -> Result<BTreeSet<(u32, u32)>, String> {
    let mut twice = None;
    let mut now = BTreeSet::new();

    let waited = stage.until::<String>(
        |seen| {
            let Ok(()) = seen.again();

            let found = lit_at(over, |across, down| seen.colour(across, down))?;
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
            Err("the home screen went on repainting with the highlight standing still".to_string())
        },
    }
}

fn newly(
    stage: &mut Device,
    over: (u32, u32, u32, u32),
    before: &BTreeSet<u32>,
    want: Lit,
) -> Result<BTreeSet<u32>, String> {
    let mut found = BTreeSet::new();

    let _waited = stage.until::<String>(
        |seen| {
            let Ok(()) = seen.again();

            let now = lit(over, |across, down| seen.colour(across, down))?;

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
    colour: impl FnMut(f64, f64) -> Result<String, String>,
) -> Result<BTreeSet<u32>, String> {
    let found = lit_at(over, colour)?;

    Ok(found.into_iter().map(|(across, _down)| across).collect())
}

fn lit_at(
    over: (u32, u32, u32, u32),
    mut colour: impl FnMut(f64, f64) -> Result<String, String>,
) -> Result<BTreeSet<(u32, u32)>, String> {
    let Ok(spent) = palette();

    let Some(plate) = spent.get(LIT) else {
        return Err(format!("the palette spends no {LIT} for a lit square to be"));
    };

    let (left, top, wide, tall) = over;
    let band = top.saturating_add(tall.saturating_div(4))
        ..top.saturating_add(tall.saturating_mul(3).saturating_div(4));
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
    let Some(first) = found.iter().next() else { return Ok(None) };

    let Some(last) = found.iter().next_back() else { return Ok(None) };

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

    let found = lit(over, |across, down| stage.colour(across, down))?;

    let Ok(Some(reached)) = reaches(&found) else {
        return failed(format!(
            "the pointer stood at {at:?} on the home screen and no square lit up"
        ));
    };

    stood_on(reached, at, room.0)
}

fn pointed_there(stage: &mut Device) -> Done {
    cleared(stage)?;
    let Ok(drawn) = stage.layer("console-home");

    let Some(over) = drawn else {
        return cannot("the home screen is not drawn, so there is nothing to point at");
    };

    let middle = over.1.saturating_add(over.3.saturating_div(2));
    let away = (over.0.saturating_add(EDGE), middle);
    let at = (over.0.saturating_add(over.2.saturating_div(2)), middle);

    stage.point(away)?;

    let before = settled(stage, over)?;

    stage.point(at)?;

    let found = newly(stage, over, &before, Lit::Somewhere)?;

    let Ok(Some(reached)) = reaches(&found) else {
        return failed(format!(
            "the pointer moved from {away:?} to {at:?} and no square lit up"
        ));
    };

    stood_on(reached, at, over.2)?;

    let Ok(awake) = stage.home_awake();

    same(&awake, &Seen::NotYet, || {
        "the pointer woke the home screen, so A is the home screen's and not the pointer's"
            .to_string()
    })?;

    stage.point(away)?;

    let last = newly(stage, over, &before, Lit::Nowhere)?;

    let Ok(still) = reaches(&last);

    match still {
        Some(reached) => {
            failed(format!("the pointer left the squares and {reached:?} is lit without it"))
        },
        None => Ok(()),
    }
}
