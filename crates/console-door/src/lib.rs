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

use console_again::{Round, keep};

pub mod homeward;

pub use homeward::{Awake, Said, homeward, telling, waking};

/// What the session was started with, or why this program cannot know.
///
/// An unset name and a name that is not text are both answered here rather
/// than at each call, because neither is something a caller can do anything
/// about beyond saying so, and both mean the same thing: this program was
/// started by something that is not the session it is asking about.
fn asked(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|fault| format!("{name}: {fault}"))
}

/// What the compositor says is on the screen, or why it could not be asked.
///
/// One call, so a caller wanting to know about several surfaces at once asks
/// once. The controller daemon wants exactly that: which of them is up decides
/// what its buttons are for, and it is not going to run `hyprctl` per surface.
///
/// A compositor that will not answer is not a screen with nothing on it. That
/// difference used to be thrown away here, and what it looked like on the
/// machine was every door on the bar reporting shut -- the one answer that is
/// indistinguishable from the desktop working.
pub fn screens() -> Result<serde_json::Value, String> {
    let said = Command::new("hyprctl")
        .args(["layers", "-j"])
        .output()
        .map_err(|fault| format!("asking hyprctl what is on the screen: {fault}"))?;

    serde_json::from_slice(&said.stdout)
        .map_err(|fault| format!("reading hyprctl's answer about what is on the screen: {fault}"))
}

/// Whether a surface is on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Up {
    /// The compositor is showing it, with a height somebody can see.
    OnScreen,
    /// It is not there, or it is listed with no height, which comes to the same
    /// thing for anybody looking at the machine.
    NotThere,
}

/// Whether the compositor is showing a surface with this name on it.
///
/// A surface with no height is one that is there and not on the screen, which
/// is a thing a keyboard can be: the keyboard is started `--hidden` and stays
/// for the session. So being listed is not enough, and being listed with a
/// height is.
pub fn is_open(namespace: &str) -> Result<Up, String> {
    Ok(up(&screens()?, namespace))
}

/// The same reading, off the text, so it can be tested without a compositor.
pub fn up(screens: &serde_json::Value, namespace: &str) -> Up {
    let on = screens
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
        .any(|surface| surface.get("h").and_then(serde_json::Value::as_i64).unwrap_or(1) > 0);

    match on {
        true => Up::OnScreen,
        false => Up::NotThere,
    }
}

/// The events that can change the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    Asking,
    Ignoring,
}

/// A layer surface opened or closed, so the answer may have moved.
pub fn worth_asking_after(line: &str) -> Worth {
    match line.starts_with("openlayer>>") || line.starts_with("closelayer>>") {
        true => Worth::Asking,
        false => Worth::Ignoring,
    }
}

/// Where the compositor says them.
pub fn events() -> Result<PathBuf, String> {
    let run = asked("XDG_RUNTIME_DIR")?;
    let instance = asked("HYPRLAND_INSTANCE_SIGNATURE")?;

    Ok(std::path::Path::new(&run).join("hypr").join(instance).join(".socket2.sock"))
}

/// Where a panel says which of its tabs is in front.
///
/// The runtime directory, so it goes when the session does. The compositor
/// stays the one that says whether the panel is up at all; this only answers
/// which tab, and only where the answer to the first is yes.
fn note() -> Result<PathBuf, String> {
    Ok(std::path::Path::new(&asked("XDG_RUNTIME_DIR")?).join("console").join("tab"))
}

/// Say which tab is in front.
pub fn saying(tab: &str) -> Result<(), String> {
    let note = note()?;

    if let Some(above) = note.parent() {
        std::fs::create_dir_all(above)
            .map_err(|fault| format!("{}: making it: {fault}", above.display()))?;
    }

    std::fs::write(&note, tab).map_err(|fault| format!("{}: writing it: {fault}", note.display()))
}

/// Say that no tab is, which is what a panel going away means.
pub fn forget() -> Result<(), String> {
    let note = note()?;

    match std::fs::remove_file(&note) {
        Ok(()) => Ok(()),
        // Already gone is the state this asks for, so arriving there is not a
        // failure to report. Every other way of failing is.
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(fault) => Err(format!("{}: removing it: {fault}", note.display())),
    }
}

/// Which tab is in front, or nothing if no panel has said.
///
/// Three answers rather than two. Nothing said is ordinary -- it is what a
/// desktop with no panel up looks like -- and a note that is there and will
/// not be read is a fault. Folded together, as they were, an unreadable note
/// read as "no tab is in front", which is the answer that makes the bar draw a
/// door shut over a panel that is open.
pub fn tab() -> Result<Option<String>, String> {
    let note = note()?;

    match std::fs::read_to_string(&note) {
        Ok(said) => Ok(Some(said.trim().to_string())),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(fault) => Err(format!("{}: reading it: {fault}", note.display())),
    }
}

/// Whether that surface is on the screen with that tab in front of it.
pub fn open_on(namespace: &str, tab_: &str) -> Result<Up, String> {
    let showing = is_open(namespace)? == Up::OnScreen && tab()?.is_some_and(|said| said == tab_);

    match showing {
        true => Ok(Up::OnScreen),
        false => Ok(Up::NotThere),
    }
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
pub fn watching_layers(say: Sender<()>) -> Result<(), String> {
    // The one thing here that cannot come back. The socket's name is taken from
    // the environment this program was started with, so a compositor this
    // program has never heard of is not something waiting will fix.
    let socket = events()?;

    keep(move || {
        let Ok(stream) = UnixStream::connect(&socket) else {
            // Not there. It may be in a moment: this is what a bar that
            // started before the compositor did sees, and what a resume looks
            // like from here.
            return Round::Another;
        };

        // A word on connecting, before a single event has been read. What
        // happened while there was no connection was said to nobody, and a
        // reader that only speaks up on the next event carries whatever it
        // believed before the gap for as long as the screen stays still. That
        // is how an icon comes to say the keyboard is up with no keyboard on
        // the screen: the closing went past while nothing was listening, and
        // nothing has opened or closed since to correct it.
        let Ok(()) = say.send(()) else { return Round::Done };

        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if worth_asking_after(&line) == Worth::Asking {
                let Ok(()) = say.send(()) else { return Round::Done };
            }
        }

        Round::Another
    });

    Ok(())
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
        assert_eq!(up(&layers(NOTHING_UP), "launcher"), Up::NotThere);
        assert_eq!(up(&layers(NOTHING_UP), "virtual-keyboard"), Up::NotThere);
    }

    /// The menu lists itself under its own program name, as every panel does.
    #[test]
    fn the_menu_being_on_the_screen_opens_its_door() {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600}],
            "3":[{"namespace":"launcher","h":1562}]}}}"#;
        assert_eq!(up(&layers(said), "launcher"), Up::OnScreen);
        assert_eq!(up(&layers(said), "virtual-keyboard"), Up::NotThere);
    }

    /// What the menu used to be listed as, which is nothing now. Left pointed
    /// here the icon would stay dark with the menu up, which is what it did.
    #[test]
    fn the_name_wofi_used_opens_nothing() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"launcher","h":1562}]}}}"#;
        assert_eq!(up(&layers(said), "wofi"), Up::NotThere);
    }

    /// Every panel is its own program, and the settings are not the menu. Left
    /// to gtk4-layer-shell they would all be "gtk4-layer-shell" and this would
    /// light an icon for any of them.
    #[test]
    fn another_panel_does_not_open_the_menus_door() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"settings-panel","h":1562}]}}}"#;
        assert_eq!(up(&layers(said), "launcher"), Up::NotThere);
    }

    /// Kept for the surface that is listed without being on the screen. The
    /// keyboard is not one: measured on the device, `--hidden` takes its
    /// surface away entirely and brings it back on the way up, so the height
    /// is a guard against a case the keyboard does not present rather than the
    /// one it does.
    #[test]
    fn a_keyboard_with_no_height_is_a_keyboard_nobody_can_see() {
        let hidden = r#"{"eDP-1":{"levels":{"3":[{"namespace":"virtual-keyboard","h":0}]}}}"#;
        let up_ = r#"{"eDP-1":{"levels":{"3":[{"namespace":"virtual-keyboard","h":520}]}}}"#;
        assert_eq!(up(&layers(hidden), "virtual-keyboard"), Up::NotThere);
        assert_eq!(up(&layers(up_), "virtual-keyboard"), Up::OnScreen);
    }

    /// The commonest events there are, sent whenever a thumb moves.
    #[test]
    fn only_a_layer_opening_or_closing_is_asked_after() {
        assert_eq!(worth_asking_after("openlayer>>virtual-keyboard"), Worth::Asking);
        assert_eq!(worth_asking_after("closelayer>>launcher"), Worth::Asking);
        assert_eq!(worth_asking_after("mousemove>>640,400"), Worth::Ignoring);
        assert_eq!(worth_asking_after("openwindow>>a4f,3,alacritty,Alacritty"), Worth::Ignoring);
        assert_eq!(worth_asking_after(""), Worth::Ignoring);
    }
}
