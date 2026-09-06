//! The on-screen keyboard: X brings it up, X puts it away, and it has depth.

use std::collections::BTreeSet;

use console_never::Never;
use console_test_stages::checking::{Body, Check, Done, empty, happened, seen};
use console_test_stages::desktop::Desktop;
use console_test_stages::device::{Device, Seen};
use console_test_stages::palette::palette;

pub const KEYBOARD: Check = Check {
    name: "110-the-keyboard",
    about: "X shows the keyboard, and X puts it away.",
    feature: "keyboard",
    since: "2026-08-25",
    bodies: &[Body::Device(there)],
};

pub const DRAWS: Check = Check {
    name: "170-the-keyboard-draws",
    about: "The on-screen keyboard comes up, and every key has a key under it.",
    feature: "keyboard",
    since: "2026-08-28",
    bodies: &[Body::Desktop(draws)],
};

const ACROSS: (i32, i32, usize) = (20, 1010, 14);
const DOWN: (i32, i32, usize) = (390, 636, 12);

const SHADES: [&str; 3] = ["ground", "night", "panel"];

fn there(stage: &mut Device) -> Done {
    let Ok(up) = stage.keyboard();

    match up {
        Seen::Yes => {
            let Ok(()) = stage.press("x");
            let Ok(()) = stage.settle(1.5);
        }
        Seen::NotYet => {}
    }

    let Ok(()) = stage.press("x");
    let Ok(()) = stage.settle(1.5);
    let Ok(came) = stage.keyboard();

    seen(came, || "the keyboard did not come up".to_string())?;

    let Ok(()) = stage.press("x");
    let Ok(()) = stage.settle(1.5);
    let Ok(still) = stage.keyboard();
    let Ok(away) = still.flipped();

    seen(away, || "the keyboard would not go away".to_string())
}

fn draws(stage: &mut Desktop) -> Done {
    let Ok(wanted) = palette();

    stage.open("keyboard-toggle")?;

    let mut there = BTreeSet::new();

    for across in (ACROSS.0..ACROSS.1).step_by(ACROSS.2) {
        for down in (DOWN.0..DOWN.1).step_by(DOWN.2) {
            let colour = stage.colour(f64::from(across), f64::from(down))?;
            there.insert(colour);
        }
    }

    let missing: Vec<&str> = SHADES
        .into_iter()
        .filter(|name| match wanted.get(*name) {
            Some(colour) => !there.contains(colour),
            None => true,
        })
        .collect();

    empty(&missing, || {
        format!(
            "the keyboard is not three shades; nothing is {}. The slab, a letter key and a key \
             that is not a letter have to differ or some of the keys have nothing under them.",
            missing.join(" or ")
        )
    })
}

pub const EVERY_TIME: Check = Check {
    name: "240-the-keyboard-comes-back-with-the-desktop",
    about: "X raises the keyboard on every one of twenty restarts, not most of them.",
    feature: "keyboard",
    since: "2026-08-31",
    bodies: &[Body::Device(every_time)],
};

const RESTARTS: usize = 20;

const UP: f64 = 90.0;

const ANSWERS: f64 = 4.0;

fn routing(stage: &mut Device) -> Result<Seen, Never> {
    let Ok(worn) = stage.profile();

    Ok(match worn.as_str() {
        "Router" => Seen::Yes,
        _ => Seen::NotYet,
    })
}

fn every_time(stage: &mut Device) -> Done {
    for round in 1..=RESTARTS {
        let Ok(_) = stage.user("systemctl --user restart console.target");
        let Ok(up) = stage.until(routing, UP);

        happened(up, || {
            format!(
                "round {round} of {RESTARTS}: the controller never loaded the router, so nothing \
                 here has been asked yet. journalctl --user -u console-controller says why."
            )
        })?;

        let Ok(already) = stage.keyboard();

        match already {
            Seen::Yes => {
                let Ok(()) = stage.press("x");
                let Ok(()) = stage.settle(1.5);
            }
            Seen::NotYet => {}
        }

        let Ok(()) = stage.press("x");
        let Ok(came) = stage.until(Device::keyboard, ANSWERS);

        happened(came, || {
            format!(
                "round {round} of {RESTARTS}: X did not raise the keyboard. Either the daemon \
                 never saw the key -- X arrives on the keyboard InputPlumber publishes, not on \
                 the pad -- or the keyboard could not take the devices when its surface went up. \
                 journalctl --user -u console-keyboard says which."
            )
        })?;

        let Ok(()) = stage.press("x");
        let Ok(went) = stage.until(
            |seen| {
                let Ok(up) = seen.keyboard();

                up.flipped()
            },
            ANSWERS,
        );

        happened(went, || {
            format!(
                "round {round} of {RESTARTS}: X raised the keyboard and would not put it away, \
                 which is one pad read twice rather than none read at all."
            )
        })?;
    }

    Ok(())
}

pub const IN_A_PAGE: Check = Check {
    name: "250-the-keyboard-types-into-a-page",
    about: "X raises the keyboard over the browser, and what it types reaches the page.",
    feature: "keyboard",
    since: "2026-09-01",
    bodies: &[Body::Device(in_a_page)],
};

const PAGE: &str = r#"<!doctype html><title>keyboard-check</title>
<input id=i autofocus style="font-size:32px;width:90%">
<script>
var i = document.getElementById("i");
i.focus();
i.addEventListener("input", function () { document.title = "GOT[" + i.value + "]"; });
</script>"#;

const TYPED: &str = "hello";

const OPENS: f64 = 45.0;

const ARRIVES: f64 = 6.0;

fn titled(stage: &mut Device, part: &str) -> Result<Seen, Never> {
    let Ok(titles) = stage.titles();

    Ok(match titles.iter().any(|title| title.contains(part)) {
        true => Seen::Yes,
        false => Seen::NotYet,
    })
}

fn in_a_page(stage: &mut Device) -> Done {
    let Ok(home) = stage.home();
    let at = format!("{home}/.cache/console-keyboard-check.html");
    let Ok(_) = stage.user(&format!("mkdir -p {home}/.cache && printf %s '{PAGE}' > {at}"));
    let Ok(up) = stage.open(&format!("librewolf --new-window file://{at}"), OPENS);

    happened(up, || {
        "the browser never came up, so nothing here has been asked yet".to_string()
    })?;

    let Ok(page) = stage.until(|stage| titled(stage, "keyboard-check"), OPENS);

    happened(page, || {
        "the browser came up on something other than the page this check wrote".to_string()
    })?;

    let Ok(already) = stage.keyboard();

    match already {
        Seen::Yes => {
            let Ok(()) = stage.press("x");
            let Ok(()) = stage.settle(1.5);
        }
        Seen::NotYet => {}
    }

    let Ok(()) = stage.press("x");
    let Ok(over) = stage.until(Device::keyboard, ARRIVES);

    happened(over, || {
        "X did not raise the keyboard over the browser, though it does over everything else.          The browser is a layer surface away from the pad, not a profile away from it."
            .to_string()
    })?;

    let Ok(_) = stage.types(TYPED);
    let Ok(arrived) = stage.until(|stage| titled(stage, &format!("GOT[{TYPED}]")), ARRIVES);
    let Ok(()) = stage.press("x");
    let Ok(()) = stage.settle(1.0);
    let Ok(_) = stage.user("pkill -x librewolf");
    let Ok(_) = stage.user(&format!("rm -f {at}"));

    happened(arrived, || {
        format!(
            "the keyboard came up over the browser and {TYPED:?} did not reach the field. The              keys go to whoever holds the focus, so either the page never had it or the browser              is not taking a virtual keyboard -- MOZ_ENABLE_WAYLAND is what decides the second."
        )
    })
}
