//! Where this machine keeps the hour, chosen the way a person chooses it.
//!
//! The zone is the one thing on the Language and Setup tabs whose answer the
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
//! panel standing on somebody's screen holding the one-chooser lock, so the
//! settings button did nothing until it was killed. What is waited for here is
//! this panel's own layer and not `drawn`, which answers for any menu at all:
//! a launcher somebody left up would answer it in the moment the settings
//! panel was still coming, and the red would name the wrong thing.
//!
//! The tab is asked for by the name the panel spells it, because `page::find`
//! answers an unknown name with the first tab rather than with a complaint --
//! a tab named wrongly here is a walk down the wrong page and a red that reads
//! like the hour never moved.
//!
//! It puts the zone back. A machine in the wrong hour is a machine whose
//! alarms are wrong, and that is somebody's afternoon rather than something
//! this made. It puts the panel away on the way out of either ending, for the
//! same reason.

use console_core_never::Never;
use console_settings::hours;
use console_settings::rows::configuration;
use console_test_stages::checking::{Body, Check, Done, cannot, failed, happened};
use console_test_stages::device::{Device, PATIENCE, Seen, Waited};

pub const HOUR: Check = Check {
    name: "390-the-hour-is-chosen-on-the-panel",
    about: "Where the hour is kept is chosen on the Setup tab, and the machine moves.",
    feature: "language",
    since: "2026-09-07",
    bodies: &[Body::Device(there)],
};

const OFFERED: [&str; 2] = ["Europe/Amsterdam", "Europe/London"];

const ANSWERS: f64 = 10.0;

const WHERE: usize = 1;

fn zone(stage: &mut Device) -> Result<String, Never> {
    let Ok(said) = stage.user("timedatectl show --property=Timezone --value");

    Ok(said.trim().to_string())
}

fn walk(stage: &mut Device, down: usize) -> Result<(), Never> {
    for _ in 0..down {
        let Ok(()) = stage.press("dpad-down");
    }

    stage.press("a")
}

fn put_away(stage: &mut Device) -> Result<(), Never> {
    let Ok(()) = stage.press("b");
    let Ok(_) = stage.gone(PATIENCE);

    Ok(())
}

fn asked_for(stage: &mut Device, tab: &str) -> Result<Waited, Never> {
    let Ok(_) = stage.exec_cmd(&format!("{} {tab}", console_settings::WHO));

    stage.until(
        |seen| {
            let Ok(where_) = seen.layer(console_settings::WHO);

            Ok(match where_ {
                Some(_) => Seen::Yes,
                None => Seen::NotYet,
            })
        },
        PATIENCE,
    )
}

fn put_back(stage: &mut Device, was: &str) -> Result<(), Never> {
    let Ok(_) = stage.user(&format!("sudo -n console-machine hour {was}"));

    Ok(())
}

fn there(stage: &mut Device) -> Done {
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
    let Ok(regions) = hours::regions(&zones);
    let Ok(places) = hours::places(&zones, &part);

    let (region_at, place_at) = match (
        regions.iter().position(|region| *region == part),
        places.iter().position(|place| place.zone == going),
    ) {
        (Some(region), Some(place)) => (region, place),
        (None, _) | (_, None) => {
            return failed(format!("{going} is in the list and not in the pages built from it"));
        }
    };

    let Ok(tab) = configuration();
    let Ok(opened) = asked_for(stage, &tab);

    match opened {
        Waited::RanOut => {
            let Ok(()) = put_away(stage);
        },
        Waited::Happened => {},
    }

    happened(opened, || format!("the settings panel did not draw on {tab}"))?;

    let Ok(()) = walk(stage, WHERE);
    let Ok(()) = walk(stage, region_at);
    let Ok(()) = walk(stage, place_at);

    let there = |seen: &mut Device| -> Result<Seen, Never> {
        let Ok(now) = zone(seen);

        Ok(match now == going {
            true => Seen::Yes,
            false => Seen::NotYet,
        })
    };

    let Ok(moved) = stage.until(there, ANSWERS);

    let Ok(()) = put_away(stage);
    let Ok(()) = put_back(stage, &was);

    happened(moved, || {
        let Ok(now) = zone(stage);

        format!(
            "{going} was chosen on the panel and the machine is still in {now}. It was asked for \
             on {tab} and walked to as region {region_at} and place {place_at} of what \
             timedatectl named."
        )
    })
}
