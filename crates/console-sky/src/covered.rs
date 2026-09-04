//! Whether anything is in front of the wallpaper.
//!
//! A moving picture behind a window is a picture nobody is looking at, and it
//! costs the same as one somebody is. So the movement is put away while there
//! is anything over it: the daemon is handed the still instead, which is one
//! frame that lasts for ever, and that is a process asleep in `poll()` rather
//! than one drawing.
//!
//! Put away rather than paused, because the wallpaper daemon has no pause. It
//! plays what it was given, so what it is given is the thing that changes.
//!
//! And put away late rather than at once. A daemon handed a file starts it at
//! the first frame, so swapping in the still and back again is a picture that
//! begins over rather than one that carries on. `console-sky` waits out anything
//! that is about to go away again; this only answers whether the wallpaper is
//! covered now.
//!
//! Two things can be in front, and both count.
//!
//! A window. One window per workspace and nothing floats, so the wallpaper is
//! covered exactly when the workspace being looked at holds one, and that is a
//! number the compositor already keeps.
//!
//! A menu, a panel, the guide or the on-screen keyboard. None of those is a
//! window; they are layer surfaces, and the compositor lists them separately.
//! What is asked about them is not which they are but whether they are there at
//! all: everything on this desktop that comes up over the wallpaper is named
//! here by what is allowed to be behind rather than by what is allowed in
//! front, so a panel added next year is counted without anybody remembering to
//! add it.

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the compositor answered last time it was asked.
///
/// Said once rather than every pass. This is asked every five minutes for as
/// long as the machine is on, and a compositor that has stopped answering
/// would otherwise write the same line into the journal until the disk filled.
static ANSWERING: AtomicBool = AtomicBool::new(true);

/// The layers that are not in front of the wallpaper.
///
/// The wallpaper daemon's own surface is the wallpaper. The bar is always up,
/// at the top edge, over a picture that is sixteen times its height, and a
/// wallpaper that never moved because the bar exists would never move at all.
/// The strip under the bar is four more pixels of the same edge and is up just
/// as permanently, so it goes here for the same reason and on pain of the same
/// outcome: counted, it is a picture that never moved again, and the way that
/// fails is the way a covered wallpaper is meant to look.
///
/// Everything else counts.
pub const BEHIND: [&str; 3] = ["awww-daemon", "waybar", "updating"];

/// What the compositor says about the workspace being looked at.
///
/// Taken from the text rather than a path, so what is parsed can be tested
/// without a compositor to ask.
pub fn holds_a_window(activeworkspace: &str) -> Covered {
    let Ok(workspace) = serde_json::from_str::<serde_json::Value>(activeworkspace) else {
        // A compositor that will not answer is one this cannot know about, and
        // a still picture is the safe thing to be wrong with.
        return Covered::Yes;
    };

    let held = workspace.get("windows").and_then(serde_json::Value::as_u64).unwrap_or(1) > 0;

    match held {
        true => Covered::Yes,
        false => Covered::No,
    }
}

/// Whether anything is in front of the wallpaper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Covered {
    /// Something is over it, so nobody is looking at the picture.
    Yes,
    /// Nothing is, so what it draws is what is on the screen.
    No,
}

/// Whether anything but the wallpaper and the bar is up.
pub fn something_over_it(layers: &str) -> Covered {
    let Ok(screens) = serde_json::from_str::<serde_json::Value>(layers) else {
        return Covered::Yes;
    };

    let over = screens
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, screen)| screen.get("levels")?.as_object())
        .flatten()
        .filter_map(|(_, level)| level.as_array())
        .flatten()
        .filter_map(|surface| surface.get("namespace")?.as_str())
        .any(|namespace| !BEHIND.contains(&namespace));

    match over {
        true => Covered::Yes,
        false => Covered::No,
    }
}

/// Whether the wallpaper is covered, asked of the compositor.
pub fn now() -> Covered {
    let ask = |what: &str| match Command::new("hyprctl").args([what, "-j"]).output() {
        Ok(said) if said.status.success() => {
            Some(String::from_utf8_lossy(&said.stdout).into_owned())
        }
        // It ran and would not answer, which is the compositor's own business
        // and is what the caller's default is for.
        Ok(_) => None,
        // It would not run at all, which is not the compositor saying anything.
        Err(fault) => {
            eprintln!("console-sky: asking hyprctl for {what}: {fault}");

            None
        }
    };

    match (ask("activeworkspace"), ask("layers")) {
        (Some(workspace), Some(layers)) => {
            ANSWERING.store(true, Ordering::Relaxed);

            match holds_a_window(&workspace) == Covered::Yes
                || something_over_it(&layers) == Covered::Yes
            {
                true => Covered::Yes,
                false => Covered::No,
            }
        }
        // Covered is the safe thing to be wrong about, and it is also the
        // answer that looks like nothing being wrong: the picture goes still
        // and stays still, which is what it does when a window is open. So it
        // is said, once, because otherwise the only symptom of a compositor
        // that has stopped answering is a wallpaper that never moves again.
        _ => {
            if ANSWERING.swap(false, Ordering::Relaxed) {
                eprintln!(
                    "hyprctl will not say what is on the screen: \
                     the picture stays still until it does"
                );
            }

            Covered::Yes
        }
    }
}

/// The events worth waking up for.
///
/// Everything that can put something in front of the wallpaper or take it away.
/// The compositor reports a great deal more than this, most of it many times a
/// second while somebody is using the machine, and answering all of it would
/// mean asking the compositor a question every time they moved a thumb.
pub const WORTH_WAKING_FOR: [&str; 8] = [
    "closelayer>>",
    "closewindow>>",
    "fullscreen>>",
    "movewindow>>",
    "openlayer>>",
    "openwindow>>",
    "workspace>>",
    "workspacev2>>",
];

/// Whether a line the compositor sent is one of them.
pub fn worth_waking_for(line: &str) -> Worth {
    match WORTH_WAKING_FOR.iter().any(|event| line.starts_with(event)) {
        true => Worth::Waking,
        false => Worth::Ignoring,
    }
}

/// Whether a line from the compositor can change whether the wallpaper is seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    /// A window or a layer moved, so the answer may have moved with it.
    Waking,
    /// Anything else, which is most of what the compositor says.
    Ignoring,
}

/// Where the compositor sends them.
pub fn events() -> Option<std::path::PathBuf> {
    let run = crate::place::said("XDG_RUNTIME_DIR")?;
    let instance = crate::place::said("HYPRLAND_INSTANCE_SIGNATURE")?;

    Some(
        std::path::Path::new(&run)
            .join("hypr")
            .join(instance)
            .join(".socket2.sock"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the compositor answers on this machine with nothing up, taken off
    /// the device rather than made up: the wallpaper on the background level,
    /// and the bar and the strip under it on the top one.
    const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
        "0":[{"address":"0x1","namespace":"awww-daemon"}],
        "2":[{"address":"0x2","namespace":"waybar"},
             {"address":"0x3","namespace":"updating"}]}}}"#;

    #[test]
    fn a_workspace_with_a_window_on_it_covers_the_wallpaper() {
        assert_eq!(holds_a_window(r#"{"id":3,"name":"3","windows":1}"#), Covered::Yes);
        assert_eq!(holds_a_window(r#"{"id":3,"name":"3","windows":2}"#), Covered::Yes);
    }

    #[test]
    fn an_empty_workspace_does_not() {
        assert_eq!(holds_a_window(r#"{"id":1,"name":"1","windows":0}"#), Covered::No);
    }

    /// The bar is up for as long as the machine is on, and a wallpaper that
    /// counted it would never move at all. So is the strip under it, which is
    /// what taking this fixture off the device again caught.
    #[test]
    fn the_wallpaper_and_the_bar_are_not_in_front_of_the_wallpaper() {
        assert_eq!(something_over_it(NOTHING_UP), Covered::No);
    }

    /// A menu is not a window, and it is the thing most often in front of this
    /// wallpaper.
    #[test]
    fn a_menu_is_in_front_of_it() {
        let menu = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon"}],
            "2":[{"namespace":"waybar"}],
            "3":[{"namespace":"wofi"}]}}}"#;
        assert_eq!(something_over_it(menu), Covered::Yes);
    }

    /// Named by what may be behind rather than by what may be in front, so a
    /// panel nobody has written yet is counted the day it is.
    #[test]
    fn anything_this_has_never_heard_of_is_in_front_of_it() {
        let new_thing = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon"}],
            "3":[{"namespace":"something-written-next-year"}]}}}"#;
        assert_eq!(something_over_it(new_thing), Covered::Yes);
    }

    /// Being wrong towards the still picture costs a picture that did not move.
    /// Being wrong the other way costs the battery all day.
    #[test]
    fn a_compositor_that_will_not_answer_is_taken_as_covered() {
        assert_eq!(holds_a_window(""), Covered::Yes);
        assert_eq!(holds_a_window("no such option"), Covered::Yes);
        assert_eq!(holds_a_window(r#"{"id":1}"#), Covered::Yes);
        assert_eq!(something_over_it(""), Covered::Yes);
        assert_eq!(something_over_it("no such option"), Covered::Yes);
    }

    #[test]
    fn a_window_or_a_layer_opening_is_worth_waking_up_for() {
        assert_eq!(worth_waking_for("openwindow>>a4f,3,alacritty,Alacritty"), Worth::Waking);
        assert_eq!(worth_waking_for("closewindow>>a4f"), Worth::Waking);
        assert_eq!(worth_waking_for("openlayer>>wofi"), Worth::Waking);
        assert_eq!(worth_waking_for("closelayer>>wofi"), Worth::Waking);
        assert_eq!(worth_waking_for("workspacev2>>3,3"), Worth::Waking);
    }

    /// The commonest event there is, sent every time a thumb moves.
    #[test]
    fn a_mouse_moving_is_not() {
        assert_eq!(worth_waking_for("activelayout>>keyboard,English"), Worth::Ignoring);
        assert_eq!(worth_waking_for("mousemove>>640,400"), Worth::Ignoring);
        assert_eq!(worth_waking_for(""), Worth::Ignoring);
    }
}
