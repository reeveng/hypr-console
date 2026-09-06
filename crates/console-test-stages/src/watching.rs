//! What somebody holding the device sees while the checks are run at it.
//!
//! A device run is driven from a laptop, and everything it says it says there:
//! a line per check on somebody else's terminal, in another room. The person
//! actually holding the machine sees the menus open by themselves for several
//! minutes and has nothing at all telling them what it is, how much of it is
//! left, or that it ended well.
//!
//! So the run says it on the device. Two things, and no more.
//!
//! ## The strip, because it is already there
//!
//! An apply fills a row of pixels under the bar as it goes, and that is exactly
//! this question asked about a different long thing:
//! `console_notifications::updating` is the file both ends of it agree on, and
//! it is filled here over the same ssh the checks are pressed through.
//!
//! Not a panel. A panel over the desktop is a layer the checks then have to
//! press through, and several of them ask what is on the screen and what colour
//! it is -- a surface put up to report the run would be the run's own worst
//! interference, and it would be the checks that went red for it.
//!
//! ## A card, because the strip has no words
//!
//! The strip is two pixels of fill with its tooltip turned off, so it can say
//! how much is left and nothing else. What it cannot say is what is happening,
//! or how it went once it is gone. A notification is raised when the run starts
//! -- so a person whose device has begun pressing its own buttons is told why
//! -- and replaced by one at the end saying how it went. Replaced rather than
//! added: one run is one card, which is the same rule everything else here
//! raising a notice keeps.
//!
//! The card at the end stays on the screen when something failed. A run that
//! ends badly while somebody is making tea is the whole reason to say it twice.

use std::time::Duration;

use console_never::Never;
use console_notifications::saying::Notice;
use console_notifications::updating::{self, Far, WAKING};

use crate::device::{Device, quoted};
use crate::lasting::{Ahead, about};

pub fn showing(device: &mut Device, ahead: &Ahead, doing: &str) -> Result<(), Never> {
    let Ok(percent) = ahead.percent();
    let far = Far { percent, doing: doing.to_string() };
    let Ok(where_at) = updating::at();
    let Ok(written) = updating::written(&far);

    let at = where_at.display().to_string();
    let beside = format!("{at}.writing");
    let Ok(said) = quoted(&written);
    let Ok(holding) = holding_of(&at);
    let Ok(holding) = quoted(&holding);
    let Ok(writing) = quoted(&beside);
    let Ok(where_) = quoted(&at);
    let Ok(waking) = waking();
    let Ok(_) = device.ssh(&format!(
        "mkdir -p {holding} && printf %s {said} > {writing} && mv {writing} {where_}; {waking}"
    ));

    Ok(())
}

pub fn done_showing(device: &mut Device) -> Result<(), Never> {
    let Ok(where_at) = updating::at();
    let Ok(at) = quoted(&where_at.display().to_string());
    let Ok(waking) = waking();
    let Ok(_) = device.ssh(&format!("rm -f {at}; {waking}"));

    Ok(())
}

fn waking() -> Result<String, Never> {
    Ok(format!("pkill {WAKING} -x waybar || true"))
}

fn holding_of(at: &str) -> Result<String, Never> {
    Ok(match at.rsplit_once('/') {
        Some((above, _)) => above.to_string(),
        None => ".".to_string(),
    })
}

pub fn said(device: &mut Device, notice: &Notice) -> Result<Option<u32>, Never> {
    let Ok(said) = notice.argv();

    let argv: Vec<String> = said
        .iter()
        .map(|word| {
            let Ok(quoted) = quoted(word);

            quoted
        })
        .collect();
    let Ok(said) = device.in_session(&argv.join(" "));

    Ok(match said.trim().parse::<u32>() {
        Ok(id) => Some(id),
        Err(_no_id) => None,
    })
}

pub const STARTING: &str = "Checking the desktop";

pub fn starting(many: usize, ahead: &Ahead) -> Result<Notice, Never> {
    let Ok(whole) = ahead.whole();

    let long = match whole {
        Some(whole) => {
            let Ok(about) = about(whole);

            format!(", about {about}")
        }
        None => String::new(),
    };

    let Ok(notice) =
        Notice::new(STARTING, &format!("{many} checks{long}. Don't touch the controls."));

    notice.staying()
}

pub fn ended(
    ok: usize,
    failed: &[String],
    took: Duration,
    was: Option<u32>,
) -> Result<Notice, Never> {
    let notice = match failed.first() {
        None => {
            let Ok(about) = about(took);
            let Ok(notice) =
                Notice::new("Checks passed", &format!("{ok} of them, in {about}."));
            let Ok(notice) = notice.lasting(8000);

            notice
        }

        Some(_something) => {
            let Ok(notice) = Notice::new(
                "Checks failed",
                &format!("{ok} passed, {} failed: {}.", failed.len(), failed.join(", ")),
            );
            let Ok(notice) = notice.urgent();
            let Ok(notice) = notice.staying();

            notice
        }
    };

    notice.replacing(was)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    #[test]
    fn the_card_at_the_start_says_how_long_when_the_machine_has_been_timed() {
        let ahead = ok(Ahead::of(&crate::lasting::Lengths::default(), &[]));
        let said = ok(starting(10, &ahead));

        assert!(!said.body.contains("about"), "a machine nobody timed was promised a length");
    }

    #[test]
    fn the_card_at_the_end_replaces_the_one_at_the_start() {
        let said = ok(ended(10, &[], Duration::from_secs(120), Some(41)));

        let argv = ok(said.argv());

        assert_eq!(said.replacing, Some(41));
        assert!(argv.iter().any(|word| word == "--replace-id=41"));
    }

    #[test]
    fn a_run_that_failed_names_what_failed_and_stays_on_the_screen() {
        let said = ok(ended(9, &["120-a-page".to_string()], Duration::from_secs(120), None));

        assert!(said.body.contains("120-a-page"), "the card does not say what failed");
        assert_eq!(said.expiry, console_notifications::saying::Expiry::Stays);
    }

    #[test]
    fn the_directory_the_strip_is_written_in_is_the_one_holding_it() {
        assert_eq!(ok(holding_of("/run/console/updating")), "/run/console");
        assert_eq!(ok(holding_of("updating")), ".");
    }
}
