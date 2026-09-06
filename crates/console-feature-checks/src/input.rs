//! Who has the buttons open, asked of the machine rather than of the design.
//!
//! The on-screen keyboard is the only keyboard this device has, and it is an
//! input device like any other -- so anything that can open `/dev/input/event*`
//! sees every button pressed and every letter typed, and a keylogger here is a
//! file open rather than an exploit. That is a fact about the machine and not
//! about this repository, which is why it is asked rather than argued: the
//! kernel says who has a device open, and the answer is a list somebody can
//! read.
//!
//! What makes this a check and not a document is that the list is short and
//! nothing else should ever be on it. The compositor reads every device
//! because it is what draws. `systemd` and `logind` hold the power button and
//! the lid, which is how the machine suspends. InputPlumber is what turns a
//! Legion Go into a pad this desktop knows the shape of. And one program of
//! ours reads what it publishes. Anything else holding a button open is either
//! a claim that was taken and never handed back -- `console_input_claim` is
//! that contract -- or a program nobody meant to be listening.
//!
//! It is asked with nothing in front, because a claim is exactly what a
//! surface takes while it is up: the keyboard, the guide and the card that
//! asks which button you pressed all hold a device for as long as they are
//! drawn, and all three are right to. `fresh` is what makes the answer mean
//! "at rest".

use console_test_stages::checking::{Body, Check, Done, empty, failed};
use console_test_stages::device::Device;

pub const OWNED: Check = Check {
    name: "320-only-the-controller-is-reading-the-buttons",
    about: "Nothing but the pieces that must have an input device open has one.",
    feature: "input",
    since: "2026-09-05",
    bodies: &[Body::Device(owned)],
};

const ALLOWED: [&str; 5] =
    ["systemd", "systemd-logind", "Hyprland", "inputplumber", "stick-scroll"];

const OURS: &str = "stick-scroll";

const ASKING: &str = "for fd in /proc/[0-9]*/fd/*; do \
     seen=$(readlink \"$fd\" 2>/dev/null) || continue; \
     case \"$seen\" in /dev/input/*) \
       pid=${fd#/proc/}; pid=${pid%%/*}; \
       echo \"$(cat /proc/$pid/comm 2>/dev/null) $seen\";; \
     esac; \
   done | sort -u";

fn owned(stage: &mut Device) -> Done {
    let Ok(()) = stage.fresh();
    let Ok(said) = stage.ssh(ASKING);

    let holding: Vec<&str> =
        said.lines().map(str::trim).filter(|line| !line.is_empty()).collect();

    let ours = holding
        .iter()
        .filter_map(|line| line.split_whitespace().next())
        .any(|name| name == OURS);

    match ours {
        true => {},
        false => {
            return failed(format!(
                "nothing is reading the buttons: {OURS} has no input device open, \
                 and what does is {holding:?}"
            ));
        }
    }

    let strangers: Vec<&str> = holding
        .iter()
        .copied()
        .filter(|line| match line.split_whitespace().next() {
            Some(name) => !ALLOWED.contains(&name),
            None => false,
        })
        .collect();

    empty(&strangers, || {
        format!("something nobody declared is reading the buttons: {strangers:?}")
    })
}
