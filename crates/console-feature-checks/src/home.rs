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

use std::collections::BTreeSet;

use console_home_screen::{Holding, Home, Spot};
use console_never::Never;
use console_test_stages::checking::{
    Body, Check, Done, cannot, empty, failed, less_than, same, seen,
};
use console_test_stages::desktop::Desktop;
use console_test_stages::device::{Device, Seen};
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
    let Ok(_) = stage.exec_cmd("put-away");
    let Ok(()) = stage.settle(1.2);
    let Ok(asleep) = stage.home_awake();

    same(&asleep, &Seen::NotYet, || {
        "the home screen was holding a highlight with nobody having asked for one".to_string()
    })?;

    let Ok(()) = stage.press("dpad-right");
    let Ok(()) = stage.settle(1.0);
    let Ok(woken) = stage.home_awake();

    same(&woken, &Seen::Yes, || "the d-pad did not wake the home screen".to_string())?;

    let Ok(()) = stage.press("y");
    let Ok(()) = stage.settle(1.4);
    let Ok(up) = stage.menus();
    let Ok(on_screen) = on_screen(&up, "home-square");

    seen(on_screen, || format!("Y on an awake home screen opened {up:?}"))?;

    let Ok(()) = stage.press("b");
    let Ok(()) = stage.settle(1.2);
    let Ok(()) = stage.press("b");
    let Ok(()) = stage.settle(1.0);
    let Ok(away) = stage.home_awake();

    same(&away, &Seen::NotYet, || "B did not put the highlight away".to_string())
}

fn pressable_there(stage: &mut Device) -> Done {
    let Ok(_) = stage.exec_cmd("put-away");
    let Ok(()) = stage.settle(1.0);
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

    let Ok(()) = stage.settle(1.6);
    let Ok(up) = stage.menus();
    let Ok(first) = on_screen(&up, "launcher");

    seen(first, || format!("a finger on the launcher icon at {at:?} opened {up:?}"))?;

    let Ok(_) = stage.exec_cmd("put-away");
    let Ok(()) = stage.settle(1.0);

    stage.touch(at)?;

    let Ok(()) = stage.settle(1.6);
    let Ok(again) = stage.menus();
    let Ok(second) = on_screen(&again, "launcher");

    seen(second, || format!("the second finger on the launcher icon opened {again:?}"))?;

    let Ok(_) = stage.exec_cmd("put-away");
    let Ok(()) = stage.settle(0.8);

    Ok(())
}

fn arranging_there(stage: &mut Device) -> Done {
    let Ok(_) = stage.exec_cmd("put-away");
    let Ok(()) = stage.settle(1.2);
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
    let Ok(()) = stage.settle(1.0);
    let Ok(()) = walked(stage, "dpad-left", 2.5);
    let Ok(()) = walked(stage, "dpad-up", 1.5);
    let Ok(()) = stage.press("y");
    let Ok(()) = stage.settle(1.4);
    let Ok(up) = stage.menus();
    let Ok(on_screen) = on_screen(&up, "home-square");

    seen(on_screen, || format!("Y on the first square opened {up:?}"))?;

    let Ok(()) = stage.press("a");
    let Ok(()) = stage.settle(1.4);
    let Ok(holding) = stage.home_awake();

    same(&holding, &Seen::Yes, || {
        "the card closed and left no highlight holding anything".to_string()
    })?;

    let Ok(()) = swapped(stage, "dpad-right");

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
    let Ok(()) = stage.settle(1.4);
    let Ok(()) = stage.press("a");
    let Ok(()) = stage.settle(1.4);
    let Ok(()) = swapped(stage, "dpad-left");

    let Ok(back) = placed(stage);

    same(&back, &before, || {
        "carried back, the home screen is not the one this started with".to_string()
    })?;

    let Ok(()) = stage.press("b");
    let Ok(()) = stage.settle(0.8);

    Ok(())
}

fn swapped(stage: &mut Device, way: &str) -> Result<(), Never> {
    let Ok(()) = stage.press(way);
    let Ok(()) = stage.settle(0.8);
    let Ok(()) = stage.press("a");
    let Ok(()) = stage.settle(1.2);

    Ok(())
}

fn walked(stage: &mut Device, way: &str, seconds: f64) -> Result<(), Never> {
    let Ok(()) = stage.hold(way);
    let Ok(()) = stage.settle(seconds);
    let Ok(()) = stage.release(Some(way));
    let Ok(()) = stage.settle(0.4);

    Ok(())
}

fn placed(stage: &mut Device) -> Result<Home, Never> {
    let home = stage.home()?;
    let at = console_home_screen::file(std::path::Path::new(&home))?;
    let said = stage.user(&format!("cat {} 2>/dev/null", at.display()))?;

    Home::read(&said)
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

const SEEN: f64 = 0.9;

fn lit(
    over: (u32, u32, u32, u32),
    mut colour: impl FnMut(f64, f64) -> Result<String, String>,
) -> Result<BTreeSet<u32>, String> {
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
                    let _ = found.insert(across);
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
    let Ok(_) = stage.exec_cmd("put-away");
    let Ok(()) = stage.settle(1.2);
    let Ok(drawn) = stage.layer("console-home");

    let Some(over) = drawn else {
        return cannot("the home screen is not drawn, so there is nothing to point at");
    };

    let middle = over.1.saturating_add(over.3.saturating_div(2));
    let away = (over.0.saturating_add(EDGE), middle);
    let at = (over.0.saturating_add(over.2.saturating_div(2)), middle);

    stage.point(away)?;

    let Ok(()) = stage.settle(SEEN);

    let before = lit(over, |across, down| stage.colour(across, down))?;

    stage.point(at)?;

    let Ok(()) = stage.settle(SEEN);

    let after = lit(over, |across, down| stage.colour(across, down))?;
    let found: BTreeSet<u32> = after.difference(&before).copied().collect();

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

    let Ok(()) = stage.settle(SEEN);

    let last = lit(over, |across, down| stage.colour(across, down))?;
    let still: BTreeSet<u32> = last.difference(&before).copied().collect();

    let Ok(still) = reaches(&still);

    match still {
        Some(reached) => {
            failed(format!("the pointer left the squares and {reached:?} is lit without it"))
        },
        None => Ok(()),
    }
}
