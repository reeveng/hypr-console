//! The notices panel opens, and a card comes up over whatever is there.
//!
//! The panel had a check and the card had none, which is the wrong way round:
//! the panel is a surface somebody goes to, and the card is the one this
//! desktop puts in front of them without being asked. A package drew it until
//! now, so there was nothing here to break and nothing here to look at; it is
//! `console-notify` that draws it, out of this repository's own palette, and a
//! card that came up in the wrong colour or did not come up at all looks from
//! every other angle exactly like a desktop with nothing to say.
//!
//! Both of the card's checks run against a bus of the stage's own, which is
//! [`Desktop::notifying`]. A notification daemon takes
//! `org.freedesktop.Notifications`, and the session bus a check inherits
//! belongs to the desktop of whoever is running it: a daemon started on that
//! one would take the name from whatever is holding it and leave the machine
//! unable to say anything for the rest of the day. A check may take away what
//! it made and may not take away what it found.
//!
//! On the handheld it is the other way round, and that is the point of the
//! card having a body there as well. The daemon is already up under its own
//! unit, so what is raised goes on the desktop's real bus and comes back
//! through everything the nested desktop does not have: the confinement, the
//! private `/dev`, the seccomp filter. That is not a second check of the same
//! thing -- a card drew here every time it was asked while every notification
//! on the device died of a SIGSYS the moment it tried to draw, and nothing
//! that ran here could have told anybody. A daemon that dies on its first
//! frame is also a daemon with a restart count of nought until something asks
//! it to draw, which is why `210-nothing-has-had-to-be-started-again` had
//! nothing to say either.
//!
//! Where to look is asked of the compositor rather than worked out: the layer
//! surface's own corner, a padding in. The stage's arithmetic for the same
//! point got it wrong once already, and on a handheld somebody is holding
//! there is no second reading to catch it with. What is left behind is a row
//! in Earlier saying where it came from -- the card itself is touched away, on
//! the device, the way a thumb would.
//!
//! Where the card is is not where its own numbers say. `showing::DOWN` is a
//! layer-shell margin, and a layer-shell margin is counted from the bottom of
//! whatever above it is exclusive -- both of waybar's rows are -- so the row
//! looked at here is the bar's own heights plus that margin. Written from the
//! screen edge instead it lands in the gap between the bar and the card, which
//! is the wallpaper, and reads exactly like a card that never came up. It did
//! that first, and the checks were green and red in the wrong order.
//!
//! What is raised is critical, which is the one urgency that stays. Five
//! seconds is longer than a picture takes and shorter than a hand takes, so a
//! card that goes by itself would make the second check green whether touching
//! it did anything or not -- and a check that says the same thing when the
//! feature is broken is not a check.
//!
//! What the second one presses is the card itself. A notification is the only
//! thing on this desktop somebody can reach without opening anything, and
//! touching it is how it goes -- it was mako's `on-touch=dismiss` and it is a
//! gesture on the card now, which is a line of code nothing but a screen would
//! ever catch the loss of.

use console_core_external_programs::Program;
use console_core_geometry::Point;
use console_core_never::Never;
use console_notifications::saying::{Notice, Said};
use console_notifications::serving;
use console_notifications::showing;
use console_test_stages::checking::{Body, Check, Done, cannot, failed, seen};
use console_test_stages::desktop::Desktop;
use console_test_stages::device::{Device, PATIENCE, Seen, Waited};
use console_test_stages::palette::palette;
use console_test_stages::watching;

use crate::panel::drew;
use crate::Unchecked;

pub const DRAWS: Check = Check {
    name: "220-the-notices-draw",
    about: "The notices panel opens, and draws what the desktop has said.",
    feature: "notices",
    since: "2026-08-30",
    bodies: &[Body::Desktop(draws)],
};

pub const CARD: Check = Check {
    name: "221-a-notice-draws-a-card",
    about: "A notification raised on the bus comes up as a card, top right.",
    feature: "notices",
    since: "2026-09-10",
    bodies: &[Body::Desktop(card), Body::Device(on_the_device)],
};

pub const TOUCHED: Check = Check {
    name: "222-a-card-goes-when-it-is-touched",
    about: "Touching a notification card takes it off the screen.",
    feature: "notices",
    since: "2026-09-10",
    bodies: &[Body::Desktop(touched)],
};

const SUMMARY: &str = "A card on the screen";

const SAID: &str = "raised by the checks, on a bus of their own";

const RAISED_HERE: &str = "raised by the checks, on this desktop's own bus";

const JOURNAL: u32 = 20;

const OUT: f64 = 40.0;

fn draws(stage: &mut Desktop) -> Done {
    stage.open("notifications-panel")?;
    drew(stage)
}

fn raising() -> Result<String, Never> {
    let Ok(notify) = Program::NotifySend.name();
    let where_ = serving::WHERE;

    Ok(format!(
        "console-notify >/dev/null 2>&1 & while [ ! -f \"${where_}\" ]; do sleep 0.05; done; \
         {notify} --urgency=critical '{SUMMARY}' '{SAID}'"
    ))
}

fn inside(stage: &mut Desktop) -> Result<Point<f64>, Unchecked> {
    let room = stage.logical()?;
    let under = crate::updating::reserved()?;

    Ok(Point {
        across: f64::from(room.wide) - f64::from(showing::IN) - f64::from(showing::PAD),
        down: under + f64::from(showing::DOWN) + f64::from(showing::PAD),
    })
}

fn spent(name: &str) -> Result<String, Unchecked> {
    let Ok(wanted) = palette();

    wanted.get(name).cloned().ok_or_else(|| Unchecked::NoColour(name.to_string()))
}

fn card(stage: &mut Desktop) -> Done {
    stage.notifying()?;

    let Ok(raising) = raising();

    stage.open(&raising)?;

    let panel = spent("panel")?;
    let at = inside(stage)?;
    let beside = Point { across: OUT, down: at.down };
    let on = stage.colour(at)?;
    let off = stage.colour(beside)?;

    let drawn = match (on == panel, off == panel) {
        (true, false) => Seen::Yes,
        (true, true) | (false, true) | (false, false) => Seen::NotYet,
    };

    seen(drawn, || {
        format!(
            "a notification was raised, so {} across and {} down is inside the card and should \
             be its own #{panel}, and {OUT} across on the same row is beside it and should not \
             be. What is there instead: #{on} on the card and #{off} beside it",
            at.across, at.down
        )
    })
}

const PRESS: i32 = 12;

fn touched(stage: &mut Desktop) -> Done {
    stage.notifying()?;

    let Ok(raising) = raising();

    stage.open(&raising)?;

    let Ok(across) = console_core_number_conversion::fitted::<i32, u32>(
        showing::WIDE.saturating_div(2),
    );
    let Ok(down) = console_core_number_conversion::fitted::<i32, u32>(PRESS);

    stage.click_in(showing::WHO, (across, down))?;

    let panel = spent("panel")?;
    let at = inside(stage)?;
    let after = stage.colour(at)?;

    let gone = match after == panel {
        true => Seen::NotYet,
        false => Seen::Yes,
    };

    seen(gone, || {
        format!(
            "the card was touched {across} across and {down} down inside its own surface, so \
             nothing should be standing at {} across and {} down afterwards. What is there \
             instead is the card's own #{panel}",
            at.across, at.down
        )
    })
}

fn came_up(stage: &mut Device) -> Result<Seen, Never> {
    let Ok(where_) = stage.layer(showing::WHO);

    Ok(match where_ {
        Some(_) => Seen::Yes,
        None => Seen::NotYet,
    })
}

fn went(stage: &mut Device) -> Result<Seen, Never> {
    let Ok(up) = came_up(stage);

    up.flipped()
}

fn in_the_corner(box_: (u32, u32, u32, u32)) -> Result<Point<f64>, Never> {
    let (across, down, wide, _tall) = box_;

    Ok(Point {
        across: f64::from(across) + f64::from(wide) - f64::from(showing::PAD),
        down: f64::from(down) + f64::from(showing::PAD),
    })
}

fn on_the_device(stage: &mut Device) -> Done {
    let Ok(already) = came_up(stage);

    match already {
        Seen::NotYet => {},
        Seen::Yes => {
            return cannot(
                "there is already a card on the screen, and a check may not take away what it \
                 found",
            );
        },
    }

    let Ok(notice) = Notice::new(Said { summary: SUMMARY, body: RAISED_HERE });
    let Ok(notice) = notice.urgent();
    let Ok(notice) = notice.staying();
    let Ok(_id) = watching::said(stage, &notice);
    let Ok(up) = stage.until(came_up, PATIENCE);

    match up {
        Waited::Happened => {},
        Waited::RanOut => {
            let Ok(said) = stage.journal("console-notify", JOURNAL);

            return failed(format!(
                "a notification was raised on the device and no card came up, so the daemon \
                 under its own unit never drew what the nested desktop draws. What \
                 console-notify said: {said}"
            ));
        },
    }

    let Ok(where_) = stage.layer(showing::WHO);

    let box_ = match where_ {
        Some(box_) => box_,
        None => {
            return failed(
                "the compositor had a card a moment ago and has none now, so there is nowhere \
                 to look for it"
                    .to_string(),
            );
        },
    };

    let Ok(at) = in_the_corner(box_);
    let panel = spent("panel")?;
    let Ok(()) = stage.again();
    let on = stage.colour(at)?;

    let Ok(across) =
        console_core_number_conversion::fitted::<i32, u32>(showing::WIDE.saturating_div(2));
    let Ok(down) = console_core_number_conversion::fitted::<i32, u32>(PRESS);

    stage.click_in(showing::WHO, (across, down))?;

    let Ok(away) = stage.until(went, PATIENCE);
    let Ok(()) = stage.again();
    let after = stage.colour(at)?;

    let drawn = match (on == panel, away, after == panel) {
        (true, Waited::Happened, false) => Seen::Yes,
        (false, _, _) | (_, Waited::RanOut, _) | (_, _, true) => Seen::NotYet,
    };

    seen(drawn, || {
        format!(
            "a notification was raised on the device, so {} across and {} down is inside the \
             card the compositor says is there and should be its own #{panel}, and touching it \
             should leave something else in that place. What is there instead: #{on} while the \
             card was up and #{after} after it was touched",
            at.across, at.down
        )
    })
}
