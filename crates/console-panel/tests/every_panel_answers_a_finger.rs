//! Every panel, opened alone, held to what the button contract promises a hand.
//!
//!     cargo test -p console-panel --test every_panel_answers_a_finger
//!     cargo test -p console-panel --test every_panel_answers_a_finger files
//!
//! One test per panel, so the second line runs one of them: a change to the
//! files panel is tried against the files panel in a few seconds rather than
//! against the whole desktop in a few minutes. That is the point of the tier.
//!
//! The rules are the ones in `docs/button-contract.md`, and they are asked of
//! the panel that ran rather than of the code that built it. Every one of them
//! was broken by every panel here until the day this was written, and not one
//! check could see it: Y reached six panels from the pad and none from the
//! glass, because no row had ever drawn a mark for Y.
//!
//! A panel that will not come up in the nested desktop fails rather than
//! skipping. A check that quietly passes because the thing it was checking
//! never started is the fault this whole tier exists to stop.

use console_test_stages::panels::{
    Panel, a_way_out_is_drawn, every_mark_reachable, every_offer_answered,
};

fn held_to_the_contract(program: &str, args: &[&str]) {
    let Ok(mut panel) = Panel::opening(program, args);

    let every = match panel.drawn() {
        Ok(every) => every,
        Err(why) => panic!("{program} could not be asked what it drew: {why}"),
    };

    for card in &every {
        if let Err(why) = every_offer_answered(card) {
            panic!("{why}");
        }

        if let Err(why) = every_mark_reachable(card) {
            panic!("{why}");
        }

        if let Err(why) = a_way_out_is_drawn(card) {
            panic!("{why}");
        }
    }
}

#[test]
fn the_files() {
    held_to_the_contract("files-panel", &[]);
}

#[test]
fn the_settings() {
    held_to_the_contract("settings-panel", &[]);
}

#[test]
fn the_menu() {
    held_to_the_contract("launcher", &[]);
}

#[test]
fn the_music() {
    held_to_the_contract("music-panel", &[]);
}

#[test]
fn the_notices() {
    held_to_the_contract("notices-panel", &[]);
}

#[test]
fn the_downloads() {
    held_to_the_contract("download-panel", &[]);
}

#[test]
fn the_guide() {
    held_to_the_contract("console-buttons", &["--menu"]);
}
