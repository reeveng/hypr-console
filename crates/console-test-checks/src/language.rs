//! Where this machine keeps the hour, chosen the way a person chooses it.
//!
//! The zone is the one thing on the Language and General tabs whose answer the
//! machine will say out loud afterwards: `timedatectl` knows where it is, and
//! it is the same fact the clock on the bar is drawn from. So this presses the
//! rows and then asks the machine, rather than asking whether the row was
//! drawn.
//!
//! The walk is worked out from the machine's own list rather than counted into
//! this file. `timedatectl list-timezones` is what the panel builds its two
//! pages from, so the number of presses is a function of the same six hundred
//! lines the rows are; a zone added to tzdata moves both together, and a check
//! that had the count written into it would go red for a reason that is not
//! the feature.
//!
//! A panel is a layer and not a window. This waited for a new entry in
//! `hyprctl clients` and never saw one, because nothing the panel host draws
//! is a toplevel; the panel came up on the device in a seventh of a second
//! every time and the check sat out its whole budget beside it, then left the
//! panel standing on someone's screen holding the one-picker lock, so the
//! settings button did nothing until it was killed. What is waited for here is
//! this panel's own layer and not `drawn`, which answers for any menu at all:
//! a launcher someone left up would answer it in the moment the settings
//! panel was still coming, and the red would name the wrong thing.
//!
//! The tab is asked for by the name the panel spells it, because `page::find`
//! answers an unknown name with the first tab rather than with a complaint --
//! a tab named wrongly here is a walk down the wrong page and a red that reads
//! like the hour never moved.
//!
//! It puts the zone back. A machine in the wrong hour is a machine whose
//! alarms are wrong, and that is someone's afternoon rather than something
//! this made. It puts the panel away on the way out of either ending, for the
//! same reason.
//!
//! **It walks to a row by what the row says, not by counting presses.** This
//! used to press down once for the zone row and a counted number of times for
//! the region, from wherever it assumed the highlight began. The panel opened
//! one day with the highlight two rows down, and the walk opened the machine's
//! name instead. So the panel is asked to say what it drew -- the host is
//! handed `CONSOLE_PANEL_TELLS` by the program asking for it -- and each step
//! reads which row is highlighted, presses towards the one it wants, and waits
//! for the next draw. A row not on the screen yet is below, because every page
//! opens at its top.

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_panel::description::{Description, Standing};
use console_settings::hours;
use console_settings::rows::{configuration, where_you_are};
use console_test_stages::checking::{Body, Check, CheckResult, cannot, failed, happened, happened_handed};
use console_test_stages::device::{Device, PATIENCE, Ready, Outcome};

pub const HOUR: Check = Check {
    name: "390-the-hour-is-chosen-on-the-panel",
    about: "The time zone is chosen on the General tab, and the machine moves.",
    feature: "language",
    since: "2026-09-07",
    bodies: &[Body::Device(there)],
};

pub const FIRST_ROW: Check = Check {
    name: "391-a-panel-opens-on-its-first-row",
    about: "Settings opens with its first row highlighted, wherever the pointer is resting.",
    feature: "panel",
    since: "2026-09-25",
    bodies: &[Body::Device(first_row)],
};

const MIDDLE: (u32, u32) = (512, 320);

const OFFERED: [&str; 2] = ["Europe/Amsterdam", "Europe/London"];

const ANSWERS: f64 = 10.0;

const TOLD: &str = "$XDG_RUNTIME_DIR/console-check-hour.jsonl";

const STEPS: u32 = 200;

fn zone(stage: &mut Device) -> Result<String, Never> {
    let Ok(said) = stage.user("timedatectl show --property=Timezone --value");

    Ok(said.trim().to_string())
}

fn latest(seen: &mut Device) -> Result<(u32, Option<Description>), Never> {
    let Ok(mut drawn) = seen.told(TOLD);
    let Ok(many) = fitted::<_, u32>(drawn.len());

    Ok((many, drawn.pop()))
}

fn highlighted(card: &Description) -> Result<Option<(u32, &str)>, Never> {
    Ok(card
        .lines
        .iter()
        .find(|line| line.standing == Standing::On)
        .map(|line| (line.at, line.says.as_str())))
}

fn drawn_again(stage: &mut Device, since: u32) -> Result<Outcome, Never> {
    stage.until(
        |seen| {
            let Ok((now, _)) = latest(seen);

            Ok(match now > since {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        },
        PATIENCE,
    )
}

fn left(stage: &mut Device, says: &str) -> Result<Outcome, Never> {
    stage.until(
        |seen| {
            let Ok((_, card)) = latest(seen);
            let Ok(on) = match card.as_ref() {
                Some(card) => highlighted(card),
                None => Ok(None),
            };

            Ok(match on.is_some_and(|(_, on)| on != says) {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        },
        PATIENCE,
    )
}

enum Next {
    Arrived,
    Button(&'static str),
}

fn toward(card: &Description, says: &str) -> Result<Next, Never> {
    let Ok(on) = highlighted(card);
    let wanted = card.lines.iter().find(|line| line.says == says).map(|line| line.at);

    Ok(match (on, wanted) {
        (Some((at, on)), wanted) => match (on == says, wanted.is_some_and(|wanted| wanted < at)) {
            (true, _) => Next::Arrived,
            (false, true) => Next::Button("dpad-up"),
            (false, false) => Next::Button("dpad-down"),
        },
        (None, _) => Next::Button("dpad-down"),
    })
}

fn walked_to(stage: &mut Device, says: &str) -> Result<Outcome, Never> {
    for _ in 0..STEPS {
        let Ok((drawn, card)) = latest(stage);

        let card = match card {
            Some(card) => card,
            None => return Ok(Outcome::RanOut),
        };

        let Ok(next) = toward(&card, says);

        let way = match next {
            Next::Arrived => {
                let Ok(()) = stage.press("a");

                return left(stage, says);
            }
            Next::Button(way) => way,
        };

        let Ok(()) = stage.press(way);
        let Ok(moved) = drawn_again(stage, drawn);

        match moved {
            Outcome::Happened => {},
            Outcome::RanOut => return Ok(Outcome::RanOut),
        }
    }

    Ok(Outcome::RanOut)
}

fn console_put_away(stage: &mut Device) -> Result<(), Never> {
    let Ok(()) = stage.press("b");
    let Ok(_) = stage.closed(PATIENCE);

    Ok(())
}

fn asked_for(stage: &mut Device, tab: &str) -> Result<Outcome, Never> {
    let Ok(_) = stage.in_session(&format!("rm -f {TOLD}"));
    let Ok(_) = stage.exec_cmd(&format!("CONSOLE_PANEL_TELLS={TOLD} {} {tab}", console_settings::WHO));

    stage.until(
        |seen| {
            let Ok(where_) = seen.layer(console_settings::WHO);

            Ok(match where_ {
                Some(_) => Ready::Yes,
                None => Ready::NotYet,
            })
        },
        PATIENCE,
    )
}

fn put_back(stage: &mut Device, was: &str) -> Result<(), Never> {
    let Ok(_) = stage.user(&format!("sudo -n console-machine hour {was}"));

    Ok(())
}

fn there(stage: &mut Device) -> CheckResult {
    let Ok(was) = zone(stage);
    let Ok(listed) = stage.user("timedatectl list-timezones");
    let Ok(zones) = hours::zones(&listed);

    match zones.is_empty() {
        true => return cannot("this machine will not say what zones it has"),
        false => {},
    }

    let going = OFFERED
        .into_iter()
        .find(|zone| *zone != was && zones.iter().any(|known| known == zone));

    let going = match going {
        Some(going) => going,
        None => return cannot("neither of the two zones this presses is one this machine has"),
    };

    let Ok(part) = hours::part_of(going);
    let Ok(places) = hours::places(&zones, &part);

    let place = match places.into_iter().find(|place| place.zone == going) {
        Some(place) => place.says,
        None => return failed(format!("{going} is in the list and not in the pages built from it")),
    };

    let Ok(tab) = configuration();
    let Ok(opened) = asked_for(stage, &tab);

    match opened {
        Outcome::RanOut => {
            let Ok(()) = console_put_away(stage);
        },
        Outcome::Happened => {},
    }

    happened(opened, || format!("the settings panel did not draw on {tab}"))?;

    let Ok(zone_row) = where_you_are();

    for row in [zone_row.as_str(), part.as_str(), place.as_str()] {
        let Ok(walked) = walked_to(stage, row);

        match walked {
            Outcome::Happened => {},
            Outcome::RanOut => {
                let Ok((_, card)) = latest(stage);
                let on = match card.as_ref().map(highlighted) {
                    Some(Ok(Some((_, on)))) => on.to_string(),
                    Some(Ok(None)) | None => "nothing".to_string(),
                };
                let Ok(()) = console_put_away(stage);
                let Ok(()) = put_back(stage, &was);

                return failed(format!("walking to {row} on {tab} for {going}, the highlight stopped on {on}"));
            }
        }
    }

    let there = |seen: &mut Device| -> Result<Ready, Never> {
        let Ok(now) = zone(seen);

        Ok(match now == going {
            true => Ready::Yes,
            false => Ready::NotYet,
        })
    };

    let Ok(moved) = stage.until(there, ANSWERS);

    let Ok(()) = console_put_away(stage);
    let Ok(()) = put_back(stage, &was);

    happened_handed(moved, stage, |stage| {
        let Ok(now) = zone(stage);

        format!(
            "{going} was chosen on the panel and the machine is still in {now}. It was asked for \
             on {tab} and walked to as {part} and then {place}."
        )
    })
}

fn first_row(stage: &mut Device) -> CheckResult {
    stage.point(MIDDLE)?;

    let Ok(tab) = configuration();
    let Ok(opened) = asked_for(stage, &tab);
    let Ok(drawn) = drawn_again(stage, 0);
    let Ok((_, card)) = latest(stage);
    let Ok(()) = console_put_away(stage);

    happened(opened, || format!("the settings panel did not draw on {tab}"))?;
    happened(drawn, || "the settings panel did not say what it drew".to_string())?;

    let card = match card {
        Some(card) => card,
        None => return failed("the settings panel did not say what it drew".to_string()),
    };

    let first = card.lines.iter().find(|line| line.heading == console_panel::description::Heading::No);
    let Ok(on) = highlighted(&card);

    match (first, on) {
        (Some(first), Some((at, _))) => match first.at == at {
            true => Ok(()),
            false => failed(format!(
                "{tab} opened with {} highlighted and not {}, the row under a pointer resting in the middle of the screen",
                on.map_or("nothing", |(_, says)| says),
                first.says
            )),
        },
        (Some(first), None) => failed(format!("{tab} opened with nothing highlighted rather than {}", first.says)),
        (None, _) => failed(format!("{tab} drew no row a highlight could stand on")),
    }
}
