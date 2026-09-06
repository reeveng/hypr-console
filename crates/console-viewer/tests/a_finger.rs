//! The viewer, opened alone and asked whether a hand could use it.
//!
//!     cargo test -p console-viewer --test a_finger
//!
//! One panel, one nested compositor, a few seconds. It is the tier a change to
//! this card is tried in while it is being written, which is why it is here
//! rather than in the whole-desktop checks: those need the desktop up and this
//! needs one program.
//!
//! Every one of these is a fault this panel actually had. Y reached the speed,
//! the subtitles and the way to open a film over the whole screen, and a hand
//! reached none of them, because no row in any panel here had ever drawn a mark
//! for Y. The way out of a picture opened over the whole screen was drawn where
//! the strip used to be, which opening out takes off the screen. And the mark
//! that replaced it was right-aligned inside a surface the list's own margin
//! had made wider than the glass, so it hung a margin's worth off the edge --
//! visible in every screenshot taken of it, and invisible to every check.
//!
//! What made all three invisible is that nothing ever asked a panel what it had
//! put on the screen. It says now, and this reads it back.

use console_test_stages::panels::{
    Panel, a_way_out_is_drawn, every_mark_reachable, every_offer_answered, one_mark_for_one_subject,
};

fn a_picture() -> std::path::PathBuf {
    let here = std::env::temp_dir().join(format!("console-viewer-a-finger-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&here);
    let at = here.join("beach.png");

    if !at.exists() {
        let bytes = [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xdd, 0x8d,
            0xb0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];

        if let Err(fault) = std::fs::write(&at, bytes) {
            eprintln!("a_finger: {}: {fault}", at.display());
        }
    }

    at
}

fn drawn(args: &[&str]) -> Vec<console_panel::telling::Told> {
    let Ok(mut panel) = Panel::opening("viewer-panel", args);

    match panel.drawn() {
        Ok(drawn) => drawn,
        Err(why) => panic!("the viewer could not be asked what it drew: {why}"),
    }
}

fn holds(
    every: &[console_panel::telling::Told],
    rule: fn(&console_panel::telling::Told) -> Result<(), String>,
) {
    for card in every {
        if let Err(why) = rule(card) {
            panic!("{why}");
        }
    }
}

fn holds_somewhere(
    every: &[console_panel::telling::Told],
    rule: fn(&console_panel::telling::Told) -> Result<(), String>,
) {
    let mut why = None;

    for card in every {
        match rule(card) {
            Ok(()) => return,
            Err(said) => why = Some(said),
        }
    }

    match why {
        Some(said) => panic!("in none of the {} draws: {said}", every.len()),
        None => panic!("the card drew nothing at all"),
    }
}

#[test]
fn the_card_a_hand_is_given() {
    let at = a_picture();
    let every = drawn(&[&at.to_string_lossy()]);

    holds(&every, every_offer_answered);
    holds(&every, every_mark_reachable);
    holds_somewhere(&every, one_mark_for_one_subject);
    holds_somewhere(&every, a_way_out_is_drawn);
}

#[test]
fn a_picture_opened_over_the_whole_screen_can_still_be_left() {
    let at = a_picture();
    let Ok(mut panel) = Panel::opening("viewer-panel", &[&at.to_string_lossy()]);

    if let Err(why) = panel.key("space") {
        panic!("{why}");
    }

    let every = match panel.drawn() {
        Ok(every) => every,
        Err(why) => panic!("the viewer could not be asked what it drew: {why}"),
    };

    let out: Vec<&console_panel::telling::Told> = every
        .iter()
        .filter(|card| card.out == console_panel::telling::Out::Yes)
        .collect();

    assert!(!out.is_empty(), "A on the picture never opened it out; the card drew {} times", every.len());

    for card in &out {
        if let Err(why) = every_mark_reachable(card) {
            panic!("{why}");
        }
    }

    let left = out.iter().any(|card| a_way_out_is_drawn(card).is_ok());

    assert!(
        left,
        "the picture opened over the whole screen and drew no way out in any of its {} draws",
        out.len()
    );
}
