//! Whether something is on the screen, asked of the compositor.
//!
//! A panel names its own surface, so the question "is the menu up" is the
//! question "is there a layer called launcher with a height". The bar asks it
//! of the doors it draws, and anything else that says what is in front of you
//! asks it the same way.
//!
//! A crate of its own, and a small one, because of who has to ask. It began
//! inside `console-panel`, which is the panel drawn in GTK, and the controller
//! daemon needs the same answer: what a button means depends on what is in
//! front of you, and the compositor is the one thing that knows. A daemon that
//! reads a pad twenty times a second should not be carrying a toolkit to find
//! that out.
//!
//! It is also the honest place to ask it from. Whether the keyboard is up was
//! kept in a file, and which profile the pad had before it went up in another
//! file, and both were written by whichever program happened to be running.
//! The compositor is not a second opinion about what is on its own screen.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::Sender;

use console_again::keep;

/// What the compositor says is on the screen, or nothing if it cannot be asked.
///
/// One call, so a caller wanting to know about several surfaces at once asks
/// once. The controller daemon wants exactly that: which of them is up decides
/// what its buttons are for, and it is not going to run `hyprctl` per surface.
pub fn screens() -> Option<serde_json::Value> {
    let said = Command::new("hyprctl").args(["layers", "-j"]).output().ok()?;
    serde_json::from_slice(&said.stdout).ok()
}

/// Whether the compositor is showing a surface with this name on it.
///
/// A surface with no height is one that is there and not on the screen, which
/// is a thing a keyboard can be: wvkbd is started `--hidden` and stays for the
/// session. So being listed is not enough, and being listed with a height is.
pub fn is_open(namespace: &str) -> bool {
    screens().is_some_and(|screens| up(&screens, namespace))
}

/// The same reading, off the text, so it can be tested without a compositor.
pub fn up(screens: &serde_json::Value, namespace: &str) -> bool {
    screens
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, screen)| screen.get("levels")?.as_object())
        .flatten()
        .filter_map(|(_, level)| level.as_array())
        .flatten()
        .filter(|surface| {
            surface
                .get("namespace")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|named| named.starts_with(namespace))
        })
        .any(|surface| surface.get("h").and_then(serde_json::Value::as_i64).unwrap_or(1) > 0)
}

/// The events that can change the answer.
///
/// A layer surface opening or closing, and nothing else. The compositor says a
/// great deal more than this, most of it many times a second while somebody is
/// using the machine, and asking after all of it would be a `hyprctl` for every
/// thumb movement.
pub fn worth_asking_after(line: &str) -> bool {
    line.starts_with("openlayer>>") || line.starts_with("closelayer>>")
}

/// Where the compositor says them.
pub fn events() -> Option<PathBuf> {
    let run = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let instance = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    Some(std::path::Path::new(&run).join("hypr").join(instance).join(".socket2.sock"))
}

/// Where a panel says which of its tabs is in front.
///
/// The runtime directory, so it goes when the session does. The compositor
/// stays the one that says whether the panel is up at all; this only answers
/// which tab, and only where the answer to the first is yes.
fn note() -> Option<PathBuf> {
    let run = std::env::var("XDG_RUNTIME_DIR").ok()?;
    Some(std::path::Path::new(&run).join("console").join("tab"))
}

/// Say which tab is in front.
pub fn saying(tab: &str) {
    let Some(note) = note() else { return };
    if let Some(above) = note.parent() {
        let _ = std::fs::create_dir_all(above);
    }
    let _ = std::fs::write(note, tab);
}

/// Say that no tab is, which is what a panel going away means.
pub fn forget() {
    if let Some(note) = note() {
        let _ = std::fs::remove_file(note);
    }
}

/// Which tab is in front, as far as anything said.
pub fn tab() -> Option<String> {
    std::fs::read_to_string(note()?).ok().map(|said| said.trim().to_string())
}

/// Whether that surface is on the screen with that tab in front of it.
pub fn open_on(namespace: &str, tab_: &str) -> bool {
    is_open(namespace) && tab().is_some_and(|said| said == tab_)
}

/// A word on `say` whenever a layer surface opened or closed, for as long as
/// anything is listening.
///
/// The connection is made again whenever it ends, which is the whole of why
/// this exists. Every icon on the bar that lights while what it opens is in
/// front learns that from this socket, and each of them used to connect once
/// and give up for the session the first time the connection went: the
/// compositor was not up yet when the bar started, or the socket went away
/// under a resume from sleep. Nothing said so. The module kept running and
/// kept drawing, and the light simply never moved again, which is a harder
/// thing to notice than an icon that disappeared.
///
/// It ends only when nobody is listening any more. A subscription nothing
/// reads is a thread and a socket kept open for no one, and on the way out of
/// a program it is also a thread that would hold it open.
pub fn watching_layers(say: Sender<()>) {
    // The one thing here that cannot come back. The socket's name is taken from
    // the environment this program was started with, so a compositor this
    // program has never heard of is not something waiting will fix.
    let Some(socket) = events() else { return };
    keep(move || {
        let Ok(stream) = UnixStream::connect(&socket) else {
            // Not there. It may be in a moment: this is what a bar that
            // started before the compositor did sees, and what a resume looks
            // like from here.
            return true;
        };
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if worth_asking_after(&line) && say.send(()).is_err() {
                return false;
            }
        }
        true
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layers(said: &str) -> serde_json::Value {
        serde_json::from_str(said).expect("layers")
    }

    /// What the compositor answers with nothing up: the wallpaper and the bar.
    const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","h":1600}],
        "2":[{"namespace":"waybar","h":38}]}}}"#;

    #[test]
    fn a_door_nothing_opened_is_shut() {
        assert!(!up(&layers(NOTHING_UP), "launcher"));
        assert!(!up(&layers(NOTHING_UP), "wvkbd"));
    }

    /// The menu lists itself under its own program name, as every panel does.
    #[test]
    fn the_menu_being_on_the_screen_opens_its_door() {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600}],
            "3":[{"namespace":"launcher","h":1562}]}}}"#;
        assert!(up(&layers(said), "launcher"));
        assert!(!up(&layers(said), "wvkbd"));
    }

    /// What the menu used to be listed as, which is nothing now. Left pointed
    /// here the icon would stay dark with the menu up, which is what it did.
    #[test]
    fn the_name_wofi_used_opens_nothing() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"launcher","h":1562}]}}}"#;
        assert!(!up(&layers(said), "wofi"));
    }

    /// Every panel is its own program, and the settings are not the menu. Left
    /// to gtk4-layer-shell they would all be "gtk4-layer-shell" and this would
    /// light an icon for any of them.
    #[test]
    fn another_panel_does_not_open_the_menus_door() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"settings-panel","h":1562}]}}}"#;
        assert!(!up(&layers(said), "launcher"));
    }

    /// Kept for the surface that is listed without being on the screen. wvkbd
    /// is not one: measured on the device, `--hidden` takes its surface away
    /// entirely and brings it back on the way up, so the height is a guard
    /// against a case wvkbd does not present rather than the one it does.
    #[test]
    fn a_keyboard_with_no_height_is_a_keyboard_nobody_can_see() {
        let hidden = r#"{"eDP-1":{"levels":{"3":[{"namespace":"wvkbd-mobintl","h":0}]}}}"#;
        let up_ = r#"{"eDP-1":{"levels":{"3":[{"namespace":"wvkbd-mobintl","h":520}]}}}"#;
        assert!(!up(&layers(hidden), "wvkbd"));
        assert!(up(&layers(up_), "wvkbd"));
    }

    /// The commonest events there are, sent whenever a thumb moves.
    #[test]
    fn only_a_layer_opening_or_closing_is_asked_after() {
        assert!(worth_asking_after("openlayer>>wvkbd-mobintl"));
        assert!(worth_asking_after("closelayer>>launcher"));
        assert!(!worth_asking_after("mousemove>>640,400"));
        assert!(!worth_asking_after("openwindow>>a4f,3,alacritty,Alacritty"));
        assert!(!worth_asking_after(""));
    }
}
