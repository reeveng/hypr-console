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
