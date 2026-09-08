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
//! all, and the list of what is allowed to be behind is `console_onscreen`'s.
//!
//! It was this crate's own, and it was three names long: the wallpaper daemon,
//! the bar and the strip under it. The home screen is a layer surface too, it
//! is transparent, and it is up whenever the workspace holds no window -- which
//! is every moment somebody can see the wallpaper at all. So this said covered
//! with nothing over it, the daemon was handed the still, and the moving
//! picture never played. Not a picture that broke: one that was never asked
//! for, by a list that had not been told the home screen exists.
//!
//! Which is the argument for not keeping one here. Said in the crate that
//! already owns what is on the screen, a surface added next year is behind or
//! in front for every program at once, and this one cannot fall behind the
//! daemon's answer again.

use std::sync::atomic::{AtomicBool, Ordering};

use console_compositor::Asked;
use console_compositor::stirred::Stirred;
use console_core_never::Never;
use console_onscreen::{Over, over_the_desktop};

static ANSWERING: AtomicBool = AtomicBool::new(true);

pub use console_onscreen::FURNITURE as BEHIND;

pub fn holds_a_window(activeworkspace: &serde_json::Value) -> Result<Covered, Never> {
    let Ok(held) = console_compositor::windows(activeworkspace);

    Ok(match held {
        Some(0) => Covered::No,
        Some(_held) => Covered::Yes,
        None => Covered::Yes,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Covered {
    Yes,
    No,
}

pub fn something_over_it(layers: &serde_json::Value) -> Result<Covered, Never> {
    let over = over_the_desktop(layers)?;

    Ok(match over {
        Over::Something => Covered::Yes,
        Over::Nothing => Covered::No,
    })
}

fn asking(question: Asked) -> Result<Option<serde_json::Value>, Never> {
    Ok(match console_compositor::asked(question) {
        Ok(said) => Some(said),
        Err(why) => {
            match ANSWERING.swap(false, Ordering::Relaxed) {
                true => eprintln!("{why} -- the picture stays still until it does"),
                false => {},
            }

            None
        }
    })
}

pub fn now() -> Result<Covered, Never> {
    let Ok(front) = asking(Asked::ActiveWorkspace);
    let Ok(screens) = asking(Asked::Layers);

    match (front, screens) {
        (Some(workspace), Some(layers)) => {
            ANSWERING.store(true, Ordering::Relaxed);

            let window = holds_a_window(&workspace)?;

            let over = something_over_it(&layers)?;

            Ok(match window == Covered::Yes || over == Covered::Yes {
                true => Covered::Yes,
                false => Covered::No,
            })
        }
        _ => Ok(Covered::Yes),
    }
}

pub fn worth_waking_for(line: &str) -> Result<Worth, Never> {
    let stirred = console_compositor::stirred::read(line)?;

    Ok(match stirred {
        Stirred::WindowOpened(_)
        | Stirred::WindowClosed(_)
        | Stirred::WindowMoved
        | Stirred::WindowFilled
        | Stirred::LayerOpened
        | Stirred::LayerClosed
        | Stirred::WorkspaceChanged => Worth::Waking,
        Stirred::WindowRenamed(_)
        | Stirred::WindowFloated
        | Stirred::WindowPinned
        | Stirred::ScreenFocused
        | Stirred::ConfigReloaded
        | Stirred::Nothing => Worth::Ignoring,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    Waking,
    Ignoring,
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
        "0":[{"address":"0x1","namespace":"awww-daemon"}],
        "2":[{"address":"0x2","namespace":"waybar"},
             {"address":"0x3","namespace":"updating"}]}}}"#;

    fn said(text: &str) -> serde_json::Value {
        console_compositor::read(text).expect("what hyprctl said")
    }

    #[test]
    fn a_workspace_with_a_window_on_it_covers_the_wallpaper() {
        assert_eq!(holds_a_window(&said(r#"{"id":3,"name":"3","windows":1}"#)), Ok(Covered::Yes));
        assert_eq!(holds_a_window(&said(r#"{"id":3,"name":"3","windows":2}"#)), Ok(Covered::Yes));
    }

    #[test]
    fn an_empty_workspace_does_not() {
        assert_eq!(holds_a_window(&said(r#"{"id":1,"name":"1","windows":0}"#)), Ok(Covered::No));
    }

    #[test]
    fn the_wallpaper_and_the_bar_are_not_in_front_of_the_wallpaper() {
        assert_eq!(something_over_it(&said(NOTHING_UP)), Ok(Covered::No));
    }

    #[test]
    fn the_home_screen_is_the_desktop_and_not_something_in_front_of_it() {
        let home = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":800}],
            "1":[{"namespace":"console-home","h":760}],
            "2":[{"namespace":"waybar","h":38},{"namespace":"updating","h":2}]}}}"#;
        assert_eq!(something_over_it(&said(home)), Ok(Covered::No));
    }

    #[test]
    fn a_card_opened_from_the_home_screen_is() {
        let card = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":800}],
            "1":[{"namespace":"console-home","h":760}],
            "3":[{"namespace":"home-square","h":760}]}}}"#;
        assert_eq!(something_over_it(&said(card)), Ok(Covered::Yes));
    }

    #[test]
    fn a_card_that_has_gone_away_is_not_in_front_of_anything() {
        let gone = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":800}],
            "1":[{"namespace":"console-home","h":760}],
            "3":[{"namespace":"home-square","h":0}]}}}"#;
        assert_eq!(something_over_it(&said(gone)), Ok(Covered::No));
    }

    #[test]
    fn a_menu_is_in_front_of_it() {
        let menu = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon"}],
            "2":[{"namespace":"waybar"}],
            "3":[{"namespace":"wofi"}]}}}"#;
        assert_eq!(something_over_it(&said(menu)), Ok(Covered::Yes));
    }

    #[test]
    fn anything_this_has_never_heard_of_is_in_front_of_it() {
        let new_thing = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon"}],
            "3":[{"namespace":"something-written-next-year"}]}}}"#;
        assert_eq!(something_over_it(&said(new_thing)), Ok(Covered::Yes));
    }

    #[test]
    fn a_workspace_that_counts_nothing_is_taken_as_covered() {
        assert_eq!(holds_a_window(&said(r#"{"id":1}"#)), Ok(Covered::Yes));
    }

    #[test]
    fn a_window_or_a_layer_opening_is_worth_waking_up_for() {
        assert_eq!(worth_waking_for("openwindow>>a4f,3,alacritty,Alacritty"), Ok(Worth::Waking));
        assert_eq!(worth_waking_for("closewindow>>a4f"), Ok(Worth::Waking));
        assert_eq!(worth_waking_for("openlayer>>wofi"), Ok(Worth::Waking));
        assert_eq!(worth_waking_for("closelayer>>wofi"), Ok(Worth::Waking));
        assert_eq!(worth_waking_for("workspacev2>>3,3"), Ok(Worth::Waking));
    }

    #[test]
    fn a_mouse_moving_is_not() {
        assert_eq!(worth_waking_for("activelayout>>keyboard,English"), Ok(Worth::Ignoring));
        assert_eq!(worth_waking_for("mousemove>>640,400"), Ok(Worth::Ignoring));
        assert_eq!(worth_waking_for(""), Ok(Worth::Ignoring));
    }
}
