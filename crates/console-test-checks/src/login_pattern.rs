//! The screen a login pattern is chosen on sits still while nobody presses it,
//! and a hand drawn across its dots keeps the pattern it drew.
//!
//! It once drew a frame on every turn of its loop, and every frame it handed
//! the compositor came back as a buffer released, which woke the loop to draw
//! again: thousands of frames a second into the one buffer on the screen, which
//! a person saw as a screen that glitched and would not take a press. What a
//! person sees cannot be read off one picture, because the picture is the
//! same either way, so this reads what the screen spent while it was left
//! alone. Two seconds of it are the thing measured, not a guess at how long
//! something takes, which is why the script waits a number of seconds.
//!
//! The screen once drew the dots and never listened for a finger, so a hand
//! could press every dot and nothing joined. What a person sees when it works
//! is the pattern kept, so the check draws one stroke twice the way the screen
//! asks and reads back what it kept, in a home of the check's own so nobody's
//! pattern is touched. The pointer holds its button through the stroke, which
//! is a finger as far as the screen can tell: both arrive as a press, a line of
//! moves and a lift.

use std::path::{Path, PathBuf};

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_login_greeter::picture::from_the_room;
use console_login_pattern::dot;
use console_login_window::stored_pattern::{self, StoredPattern};
use console_test_stages::checking::{Body, Check, CheckResult, failed, less_than};
use console_test_stages::desktop::Desktop;

use crate::Unchecked;

pub const STILL: Check = Check {
    name: "460-the-login-pattern-sits-still",
    about: "The login pattern screen, left alone, spends next to nothing redrawing itself.",
    feature: "login-pattern",
    since: "2026-09-23",
    bodies: &[Body::Desktop(still)],
};

pub const BY_HAND: Check = Check {
    name: "461-a-login-pattern-is-drawn-by-hand",
    about: "A stroke drawn across four dots, twice, keeps the pattern it drew.",
    feature: "login-pattern",
    since: "2026-09-24",
    bodies: &[Body::Desktop(by_hand)],
};

const SCREEN: &str = "settings-login-pattern";

const STROKE: [u32; 4] = [0, 4, 2, 7];

const SPENT: &str = "spent";

const MOST: u64 = 20;

fn still(stage: &mut Desktop) -> CheckResult {
    let at = ours()?;
    let spent = at.join(SPENT);
    let Ok(script) = script(&spent);

    stage.open(&script)?;
    stage.not_before(&spent)?;
    stage.windows()?;

    let said = std::fs::read_to_string(&spent);
    let _ = std::fs::remove_dir_all(&at);
    let said = match said {
        Ok(said) => said,
        Err(fault) => return failed(format!("the screen never said what it spent: {fault}")),
    };
    let Ok(ticks) = ticks(&said);

    match ticks {
        Some(ticks) => less_than(ticks, MOST, || {
            format!(
                "left alone for two seconds the screen spent {ticks} ticks of a core, so it is \
                 drawing when nothing changed"
            )
        }),
        None => failed(format!("what the screen spent did not read as two readings: {said:?}")),
    }
}

fn by_hand(stage: &mut Desktop) -> CheckResult {
    let home = ours()?;
    let canvas = stage.logical()?;
    let Ok(()) = stage.fresh();
    let Ok(places) = stroke(canvas);
    let (first, rest) = match places.split_first() {
        Some(split) => split,
        None => return failed("the stroke has no dots in it".to_string()),
    };

    stage.open(&format!("HOME={} {SCREEN}", home.display()))?;
    stage.drag_in(SCREEN, *first, rest)?;
    stage.drag_in(SCREEN, *first, rest)?;
    stage.windows()?;

    let kept = stored_pattern::stored(&home);
    let _ = std::fs::remove_dir_all(&home);

    match kept {
        Ok(StoredPattern::Hash(_)) => Ok(()),
        Ok(StoredPattern::Absent) => failed(format!(
            "a stroke across dots {STROKE:?} was drawn twice at {places:?} and no pattern was kept, \
             so the screen is not hearing the hand"
        )),
        Err(fault) => failed(format!("what the screen kept could not be read: {fault}")),
    }
}

fn stroke(canvas: Size<u32>) -> Result<Vec<(u32, u32)>, Never> {
    Ok(STROKE
        .iter()
        .filter_map(|at| {
            let Ok(dot) = dot(*at);

            dot
        })
        .map(|dot| {
            let Ok(on_screen) = from_the_room(canvas, dot.centre);
            let Ok(across) = fitted::<i32, u32>(on_screen.x);
            let Ok(down) = fitted::<i32, u32>(on_screen.y);

            (across, down)
        })
        .collect())
}

fn ours() -> Result<PathBuf, Unchecked> {
    console_core_temporary_directories::fresh("login-pattern").map_err(Unchecked::Temporary)
}

fn script(spent: &Path) -> Result<String, Never> {
    let at = spent.display();
    let read = "cut -d' ' -f14,15 /proc/$p/stat";

    Ok(format!("{SCREEN} & p=$!; sleep 2; a=$({read}); sleep 2; b=$({read}); echo \"$a $b\" > {at}.part && mv {at}.part {at}"))
}

fn ticks(said: &str) -> Result<Option<u64>, Never> {
    let read: Result<Vec<u64>, _> = said.split_whitespace().map(str::parse).collect();
    let read = match read {
        Ok(read) => read,
        Err(_not_a_number) => return Ok(None),
    };

    Ok(match read.as_slice() {
        [user, system, user_after, system_after] => {
            let before = user.saturating_add(*system);
            let after = user_after.saturating_add(*system_after);

            Some(after.saturating_sub(before))
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_readings_are_what_was_spent_between_them() {
        assert_eq!(ticks("10 5 13 6\n"), Ok(Some(4)));
    }

    #[test]
    fn anything_else_is_not_a_reading() {
        assert_eq!(ticks("10 5\n"), Ok(None));
        assert_eq!(ticks(""), Ok(None));
        assert_eq!(ticks("10 5 x 6\n"), Ok(None));
    }
}
