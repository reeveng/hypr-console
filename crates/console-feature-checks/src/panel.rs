//! The settings panel: it opens, it draws, and it lets go.

use std::collections::BTreeSet;

use console_test_stages::checking::{Body, Check, Done, empty, happened, more_than, not_empty, same, seen};
use console_test_stages::desktop::Desktop;
use console_test_stages::here::{Acts, Here, TURNS};
use console_test_stages::device::{Device, PATIENCE, Seen};
use console_test_stages::palette::palette;

use crate::chooser::opens;

pub const PANEL: Check = Check {
    name: "080-the-panel",
    about: "Legion right opens the settings panel.",
    feature: "panel",
    since: "2026-08-28",
    bodies: &[Body::Here(here), Body::Device(there)],
};

pub const DRAWS: Check = Check {
    name: "160-the-panel-draws",
    about: "The settings panel opens, and draws itself.",
    feature: "panel",
    since: "2026-08-28",
    bodies: &[Body::Desktop(draws)],
};

pub const WITH_THE_KEYBOARD: Check = Check {
    name: "180-out-of-a-panel-with-the-keyboard-up",
    about: "B closes a panel with the keyboard over it, and leaves the pad usable.",
    feature: "panel",
    since: "2026-08-28",
    bodies: &[Body::Here(without_a_screen), Body::Device(with_the_keyboard)],
};

const OVER_A_PANEL: &str = r#"{"eDP-1":{"levels":{
    "0":[{"namespace":"awww-daemon","h":1600}],
    "2":[{"namespace":"waybar","h":38},{"namespace":"updating","h":2}],
    "3":[{"namespace":"settings-panel","h":1562},{"namespace":"virtual-keyboard","h":520}]}}}"#;

const THE_PANEL_ALONE: &str = r#"{"eDP-1":{"levels":{
    "0":[{"namespace":"awww-daemon","h":1600}],
    "2":[{"namespace":"waybar","h":38},{"namespace":"updating","h":2}],
    "3":[{"namespace":"settings-panel","h":1562}]}}}"#;

const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
    "0":[{"namespace":"awww-daemon","h":1600}],
    "2":[{"namespace":"waybar","h":38},{"namespace":"updating","h":2}]}}}"#;

fn without_a_screen(stage: &mut Here) -> Done {
    stage.showing(THE_PANEL_ALONE)?;

    let Ok(alone) = stage.acts();

    same(&alone, &Acts::OnPresses, || {
        "a panel up and the daemon has stood down, so no button on it does anything".to_string()
    })?;

    stage.showing(OVER_A_PANEL)?;
    stage.press("legion-right")?;
    stage.press("b")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(under) = stage.commands();

    empty(under, || format!("the daemon acted under the keyboard: {under:?}"))?;

    stage.showing(NOTHING_UP)?;

    let Ok(after) = stage.acts();

    same(&after, &Acts::OnPresses, || {
        "the keyboard went and the daemon is still standing down".to_string()
    })?;

    let Ok(commands) = stage.commands();
    let before = commands.len();

    stage.press("legion-right")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(again) = stage.commands();

    more_than(again.len(), before, || {
        "the keyboard went and the daemon never started acting again".to_string()
    })
}

pub const ACROSS: f64 = 200.0;
pub const DOWN: std::ops::Range<i32> = 150..520;
pub const EVERY: usize = 6;

pub fn drew(stage: &mut Desktop) -> Done {
    let Ok(wanted) = palette();
    let down: BTreeSet<String> = DOWN
        .step_by(EVERY)
        .map(|y| stage.colour(ACROSS, f64::from(y)))
        .collect::<Result<_, _>>()?;

    let any_of = |names: &[&str]| {
        let any = names
            .iter()
            .any(|name| wanted.get(*name).is_some_and(|colour| down.contains(colour)));

        match any {
            true => Seen::Yes,
            false => Seen::NotYet,
        }
    };

    seen(any_of(&["panel", "ground"]), || {
        format!("nothing of the panel is on the screen where it should be: {down:?}")
    })?;

    seen(any_of(&["pink"]), || {
        format!("the panel drew but nothing on it is highlighted: {down:?}")
    })
}

fn here(stage: &mut Here) -> Done {
    stage.press("legion-right")?;
    let Ok(()) = stage.settle(TURNS);
    let Ok(ran) = stage.names();

    same(&ran, &["settings-panel"], || format!("it ran {ran:?}"))
}

fn there(stage: &mut Device) -> Done {
    opens(stage, "legion-right", "panel")
}

fn draws(stage: &mut Desktop) -> Done {
    stage.open("settings-panel Sound")?;
    drew(stage)
}

fn with_the_keyboard(stage: &mut Device) -> Done {
    let Ok(()) = stage.press("legion-right");
    let Ok(drawn) = stage.drawn(PATIENCE);

    happened(drawn, || "the panel did not draw".to_string())?;

    let Ok(()) = stage.press("x");
    let Ok(over) = stage.until(Device::keyboard, PATIENCE);

    happened(over, || "the keyboard did not come up over the panel".to_string())?;

    let Ok(up) = stage.menus();

    not_empty(&up, || "the keyboard came up and the panel went".to_string())?;

    let Ok(()) = stage.press("b");
    let Ok(closed) = stage.gone(PATIENCE);

    happened(closed, || "B did not close the panel".to_string())?;

    let Ok(()) = stage.press("x");
    let Ok(went) = stage.until(
        |seen| {
            let Ok(up) = seen.keyboard();

            up.flipped()
        },
        PATIENCE,
    );

    happened(went, || "the keyboard would not go away".to_string())?;

    let Ok(held) = stage.profile();

    same(&held, crate::chooser::WORN, || {
        format!("the keyboard went and the pad was left wearing {held}")
    })?;

    let Ok(()) = stage.press("legion-right");
    let Ok(again) = stage.drawn(PATIENCE);

    happened(again, || "the settings button stopped drawing anything".to_string())?;

    let Ok(()) = stage.press("b");
    let Ok(gone) = stage.gone(PATIENCE);

    happened(gone, || "the panel would not close again".to_string())
}
