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

use std::fmt;
use std::path::PathBuf;

use console_compositor::stirred::Stirred;
use console_core_atomic_writes::Held;
use console_core_external_programs::Program;
use console_core_never::Never;

pub mod homeward;

pub use homeward::{Awake, Hand, Said, carrying, homeward, telling, waking};

#[derive(Debug)]
pub enum Amiss {
    Sessionless,
    Compositorless,
    Asking(console_compositor::Unanswered),
    Making(PathBuf, std::io::Error),
    Writing(console_core_atomic_writes::Unwritten),
    Removing(PathBuf, std::io::Error),
    Unreadable(PathBuf, String),
    Unbound(std::io::Error),
    Telling(PathBuf, std::io::Error),
}

impl fmt::Display for Amiss {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Amiss::Sessionless => write!(
                to,
                "XDG_RUNTIME_DIR: nothing says where this session keeps what it is holding"
            ),
            Amiss::Compositorless => write!(
                to,
                "HYPRLAND_INSTANCE_SIGNATURE: there is no compositor to listen to"
            ),
            Amiss::Asking(fault) => write!(to, "{fault}"),
            Amiss::Making(at, fault) => write!(to, "{}: making it: {fault}", at.display()),
            Amiss::Writing(fault) => write!(to, "{fault}"),
            Amiss::Removing(at, fault) => write!(to, "{}: removing it: {fault}", at.display()),
            Amiss::Unreadable(at, fault) => write!(to, "{}: reading it: {fault}", at.display()),
            Amiss::Unbound(fault) => write!(to, "no socket to say it on: {fault}"),
            Amiss::Telling(at, fault) => write!(to, "{}: {fault}", at.display()),
        }
    }
}

impl std::error::Error for Amiss {}

impl From<console_compositor::Unanswered> for Amiss {
    fn from(fault: console_compositor::Unanswered) -> Self {
        Amiss::Asking(fault)
    }
}

fn runtime() -> Result<PathBuf, Amiss> {
    let Ok(runtime) = console_core_places::runtime();

    match runtime {
        Some(runtime) => Ok(runtime),
        None => Err(Amiss::Sessionless),
    }
}

pub fn screens() -> Result<serde_json::Value, Amiss> {
    console_compositor::asked(console_compositor::Asked::Layers).map_err(Amiss::Asking)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Up {
    OnScreen,
    NotThere,
}

pub fn is_open(namespace: &str) -> Result<Up, Amiss> {
    let screens = screens()?;

    let Ok(up) = up(&screens, namespace);

    Ok(up)
}

fn drawn<'a>(
    screens: &'a serde_json::Value,
    namespace: &'a str,
) -> Result<impl Iterator<Item = &'a serde_json::Value>, Never> {
    let Ok(surfaces) = console_compositor::surfaces(screens);

    Ok(surfaces
        .filter(move |surface| {
            let Ok(named) = console_compositor::namespace(surface);

            named.is_some_and(|named| named.starts_with(namespace))
        })
        .filter(|surface| {
            let Ok(seen) = seen(surface);

            seen == Seen::Yes
        }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seen {
    Yes,
    No,
}

fn seen(surface: &serde_json::Value) -> Result<Seen, Never> {
    let Ok(tall) = console_compositor::tall(surface);

    Ok(match tall {
        Some(tall) => match tall > 0 {
            true => Seen::Yes,
            false => Seen::No,
        },
        None => Seen::Yes,
    })
}

pub fn up(screens: &serde_json::Value, namespace: &str) -> Result<Up, Never> {
    let Ok(mut drawn) = drawn(screens, namespace);

    Ok(match drawn.next() {
        Some(_surface) => Up::OnScreen,
        None => Up::NotThere,
    })
}

pub use console_compositor::Corner as Standing;

pub fn standing(screens: &serde_json::Value, namespace: &str) -> Result<Option<Standing>, Never> {
    let Ok(mut drawn) = drawn(screens, namespace);

    let surface = match drawn.next() {
        Some(surface) => surface,
        None => return Ok(None),
    };

    console_compositor::corner(surface)
}

pub const FURNITURE: [&str; 6] = [
    "awww-daemon",
    "console-keyboard",
    "notifications",
    BAR,
    NOTICE,
    HOME,
];

pub const BAR: &str = "console-bar";

pub const HOME: &str = "console-home";

pub const NOTICE: &str = "console-notify";

pub const KEYBOARD: &str = "console-keyboard";

pub const ASKING: &str = "console-asking";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Over {
    Something,
    Nothing,
}

pub fn over_the_desktop(screens: &serde_json::Value) -> Result<Over, Never> {
    let Ok(surfaces) = console_compositor::surfaces(screens);

    let over = surfaces
        .filter(|surface| {
            let Ok(seen) = seen(surface);

            seen == Seen::Yes
        })
        .filter_map(|surface| {
            let Ok(named) = console_compositor::namespace(surface);

            named
        })
        .any(|named| !FURNITURE.iter().any(|known| named.starts_with(known)));

    Ok(match over {
        true => Over::Something,
        false => Over::Nothing,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    Asking,
    Ignoring,
}

pub fn worth_asking_after(line: &str) -> Result<Worth, Never> {
    let stirred = console_compositor::stirred::read(line)?;

    Ok(match stirred {
        Stirred::LayerOpened | Stirred::LayerClosed => Worth::Asking,
        Stirred::WindowOpened(_)
        | Stirred::WindowClosed(_)
        | Stirred::WindowRenamed(_)
        | Stirred::WindowMoved
        | Stirred::WindowFloated
        | Stirred::WindowPinned
        | Stirred::WindowFilled
        | Stirred::WorkspaceChanged
        | Stirred::ScreenFocused
        | Stirred::ConfigReloaded
        | Stirred::Nothing => Worth::Ignoring,
    })
}

pub fn events() -> Result<PathBuf, Amiss> {
    let run = runtime()?;

    let Ok(instance) = console_compositor::instance();

    let instance = match instance {
        Some(instance) => instance,
        None => return Err(Amiss::Compositorless),
    };

    Ok(run.join("hypr").join(instance).join(".socket2.sock"))
}

fn note() -> Result<PathBuf, Amiss> {
    let runtime = runtime()?;

    Ok(runtime.join(console_core_places::OURS).join("tab"))
}

pub fn saying(tab: &str) -> Result<(), Amiss> {
    let note = note()?;

    match note.parent() {
        Some(above) => std::fs::create_dir_all(above)
            .map_err(|fault| Amiss::Making(above.to_path_buf(), fault))?,
        None => {}
    }

    console_core_atomic_writes::whole(&note, tab.as_bytes()).map_err(Amiss::Writing)?;

    let Ok(()) = wake();

    Ok(())
}

pub const WAKES_AT: i32 = 4;

pub const WAKING: &str = "-RTMIN+4";

pub fn wake() -> Result<(), Never> {
    let Ok(mut waking) = Program::Pkill.command();

    let _ = waking
        .arg(WAKING)
        .args(["-x", BAR])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    Ok(())
}

pub fn forget() -> Result<(), Amiss> {
    let note = note()?;

    let gone = match std::fs::remove_file(&note) {
        Ok(()) => Ok(()),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(fault) => Err(Amiss::Removing(note, fault)),
    };

    let Ok(()) = wake();

    gone
}

pub fn tab() -> Result<Option<String>, Amiss> {
    let note = note()?;

    let Ok(held) = console_core_atomic_writes::read(&note);

    match held {
        Held::Said(said) => Ok(Some(said.trim().to_string())),
        Held::Nothing => Ok(None),
        Held::Unreadable(fault) => Err(Amiss::Unreadable(note, fault)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tab<'a>(pub &'a str);

pub fn open_on(namespace: &str, tab_: Tab<'_>) -> Result<Up, Amiss> {
    let tab_ = tab_.0;
    let surface = is_open(namespace)?;

    match surface {
        Up::NotThere => Ok(Up::NotThere),
        Up::OnScreen => {
            let front = tab()?;

            match front.is_some_and(|said| said == tab_) {
                true => Ok(Up::OnScreen),
                false => Ok(Up::NotThere),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layers(said: &str) -> serde_json::Value {
        serde_json::from_str(said).expect("layers")
    }

    fn up(screens: &serde_json::Value, namespace: &str) -> Up {
        let Ok(up) = super::up(screens, namespace);

        up
    }

    fn standing(screens: &serde_json::Value, namespace: &str) -> Option<Standing> {
        let Ok(standing) = super::standing(screens, namespace);

        standing
    }

    fn worth_asking_after(line: &str) -> Worth {
        let Ok(worth) = super::worth_asking_after(line);

        worth
    }

    const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","h":1600}],
        "2":[{"namespace":"console-bar","h":40}]}}}"#;

    #[test]
    fn a_door_nothing_opened_is_shut() {
        assert_eq!(up(&layers(NOTHING_UP), "launcher"), Up::NotThere);
        assert_eq!(up(&layers(NOTHING_UP), "console-keyboard"), Up::NotThere);
    }

    #[test]
    fn the_menu_being_on_the_screen_opens_its_door() {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600}],
            "3":[{"namespace":"launcher","h":1562}]}}}"#;
        assert_eq!(up(&layers(said), "launcher"), Up::OnScreen);
        assert_eq!(up(&layers(said), "console-keyboard"), Up::NotThere);
    }

    #[test]
    fn the_name_wofi_used_opens_nothing() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"launcher","h":1562}]}}}"#;
        assert_eq!(up(&layers(said), "wofi"), Up::NotThere);
    }

    #[test]
    fn another_panel_does_not_open_the_menus_door() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"settings-panel","h":1562}]}}}"#;
        assert_eq!(up(&layers(said), "launcher"), Up::NotThere);
    }

    #[test]
    fn a_keyboard_with_no_height_is_a_keyboard_nobody_can_see() {
        let hidden = r#"{"eDP-1":{"levels":{"3":[{"namespace":"console-keyboard","h":0}]}}}"#;
        let up_ = r#"{"eDP-1":{"levels":{"3":[{"namespace":"console-keyboard","h":520}]}}}"#;
        assert_eq!(up(&layers(hidden), "console-keyboard"), Up::NotThere);
        assert_eq!(up(&layers(up_), "console-keyboard"), Up::OnScreen);
    }

    const A_PANEL: &str = r#"{"eDP-1":{"levels":{
        "2":[{"namespace":"console-bar","x":0,"y":0,"w":1920,"h":40}],
        "3":[{"namespace":"settings-panel","x":260,"y":140,"w":1400,"h":900}]}}}"#;

    #[test]
    fn a_surface_stands_where_the_compositor_says_it_does() {
        assert_eq!(
            standing(&layers(A_PANEL), "settings-panel"),
            Some(Standing { across: 260, down: 140, wide: 1400, tall: 900 })
        );
    }

    #[test]
    fn nothing_is_standing_where_nothing_is_drawn() {
        assert_eq!(standing(&layers(A_PANEL), "launcher"), None);
    }

    #[test]
    fn a_panel_nobody_can_see_is_standing_nowhere_either() {
        let gone = r#"{"eDP-1":{"levels":{
            "3":[{"namespace":"settings-panel","x":260,"y":140,"w":1400,"h":0}]}}}"#;
        assert_eq!(up(&layers(gone), "settings-panel"), Up::NotThere);
        assert_eq!(standing(&layers(gone), "settings-panel"), None);
    }

    #[test]
    fn a_corner_half_said_is_no_corner_rather_than_a_corner_with_nought_in_it() {
        let half = r#"{"eDP-1":{"levels":{
            "3":[{"namespace":"settings-panel","y":140,"w":1400,"h":900}]}}}"#;
        assert_eq!(up(&layers(half), "settings-panel"), Up::OnScreen, "it is on the screen");
        assert_eq!(standing(&layers(half), "settings-panel"), None, "but not measurable");
    }

    #[test]
    fn only_a_layer_opening_or_closing_is_asked_after() {
        assert_eq!(worth_asking_after("openlayer>>console-keyboard"), Worth::Asking);
        assert_eq!(worth_asking_after("closelayer>>launcher"), Worth::Asking);
        assert_eq!(worth_asking_after("mousemove>>640,400"), Worth::Ignoring);
        assert_eq!(worth_asking_after("openwindow>>a4f,3,alacritty,Alacritty"), Worth::Ignoring);
        assert_eq!(worth_asking_after(""), Worth::Ignoring);
    }
}
