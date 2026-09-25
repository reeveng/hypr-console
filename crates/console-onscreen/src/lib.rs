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

use console_compositor::events::CompositorEvent;
use console_core_atomic_writes::Stored;
use console_core_never::Never;

pub mod homeward;

pub use homeward::{Woken, Hand, PadInput, carrying, homeward, telling, waking};

#[derive(Debug)]
pub enum Error {
    Sessionless,
    Compositorless,
    Query(console_compositor::HyprctlError),
    UnexpectedReply(console_compositor::Query),
    Making(PathBuf, std::io::Error),
    Writing(console_core_atomic_writes::Unwritten),
    Removing(PathBuf, std::io::Error),
    Read(PathBuf, String),
    Unbound(std::io::Error),
    Sending(PathBuf, std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sessionless => write!(
                to,
                "XDG_RUNTIME_DIR: nothing says where this session keeps what it is holding"
            ),
            Error::Compositorless => write!(
                to,
                "HYPRLAND_INSTANCE_SIGNATURE: there is no compositor to listen to"
            ),
            Error::Query(fault) => write!(to, "{fault}"),
            Error::UnexpectedReply(question) => {
                let Ok(about) = question.about();

                write!(to, "hyprctl answered something other than {about}")
            }
            Error::Making(at, fault) => write!(to, "{}: making it: {fault}", at.display()),
            Error::Writing(fault) => write!(to, "{fault}"),
            Error::Removing(at, fault) => write!(to, "{}: removing it: {fault}", at.display()),
            Error::Read(at, fault) => write!(to, "{}: reading it: {fault}", at.display()),
            Error::Unbound(fault) => write!(to, "no socket to say it on: {fault}"),
            Error::Sending(at, fault) => write!(to, "{}: {fault}", at.display()),
        }
    }
}

impl std::error::Error for Error {}

impl From<console_compositor::HyprctlError> for Error {
    fn from(fault: console_compositor::HyprctlError) -> Self {
        Error::Query(fault)
    }
}

fn runtime() -> Result<PathBuf, Error> {
    let Ok(runtime) = console_core_places::runtime();

    match runtime {
        Some(runtime) => Ok(runtime),
        None => Err(Error::Sessionless),
    }
}

pub fn screens() -> Result<Vec<console_compositor::Layer>, Error> {
    let asked = console_compositor::Query::Layers;
    let answer = console_compositor::query(asked).map_err(Error::Query)?;

    match answer {
        console_compositor::Answer::Layers(layers) => Ok(layers),
        console_compositor::Answer::ActiveWorkspace(_)
        | console_compositor::Answer::Workspaces(_)
        | console_compositor::Answer::Monitors(_)
        | console_compositor::Answer::EveryMonitor(_)
        | console_compositor::Answer::Clients(_)
        | console_compositor::Answer::Devices(_)
        | console_compositor::Answer::Binds(_) => Err(Error::UnexpectedReply(asked)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Up {
    OnScreen,
    NotThere,
}

pub fn is_open(namespace: &str) -> Result<Up, Error> {
    let layers = screens()?;

    let Ok(up) = up(&layers, namespace);

    Ok(up)
}

fn drawn<'a>(
    layers: &'a [console_compositor::Layer],
    namespace: &'a str,
) -> Result<impl Iterator<Item = &'a console_compositor::Layer>, Never> {
    Ok(layers
        .iter()
        .filter(move |layer| layer.namespace.starts_with(namespace))
        .filter(|layer| {
            let Ok(drawn) = layer.drawn();

            drawn == console_compositor::Visible::Yes
        }))
}

pub fn up(layers: &[console_compositor::Layer], namespace: &str) -> Result<Up, Never> {
    let Ok(mut drawn) = drawn(layers, namespace);

    Ok(match drawn.next() {
        Some(_layer) => Up::OnScreen,
        None => Up::NotThere,
    })
}

pub use console_compositor::Frame as Standing;

pub fn standing(layers: &[console_compositor::Layer], namespace: &str) -> Result<Option<Standing>, Never> {
    let Ok(mut drawn) = drawn(layers, namespace);

    let layer = match drawn.next() {
        Some(layer) => layer,
        None => return Ok(None),
    };

    layer.corner()
}

pub const SYSTEM_SURFACES: [&str; 7] = [
    "awww-daemon",
    "console-keyboard",
    "notifications",
    BAR,
    NOTIFICATION,
    HOME,
    CONTROL_CENTER,
];

pub const BAR: &str = "console-bar";

pub const HOME: &str = "console-home";

pub const NOTIFICATION: &str = "console-notify";

pub const KEYBOARD: &str = "console-keyboard";

pub const ASKING: &str = "console-asking";

pub const CONTROL_CENTER: &str = "console-control-center";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Over {
    Some,
    None,
}

pub fn over_the_desktop(layers: &[console_compositor::Layer]) -> Result<Over, Never> {
    let over = layers
        .iter()
        .filter(|layer| {
            let Ok(drawn) = layer.drawn();

            drawn == console_compositor::Visible::Yes
        })
        .any(|layer| {
            !SYSTEM_SURFACES.iter().any(|known| layer.namespace.starts_with(known))
        });

    Ok(match over {
        true => Over::Some,
        false => Over::None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    Querying,
    Ignoring,
}

pub fn worth_asking_after(line: &str) -> Result<Worth, Never> {
    let stirred = console_compositor::events::read(line)?;

    Ok(match stirred {
        CompositorEvent::LayerOpened | CompositorEvent::LayerClosed => Worth::Querying,
        CompositorEvent::WindowOpened(_)
        | CompositorEvent::WindowClosed(_)
        | CompositorEvent::WindowRenamed(_)
        | CompositorEvent::WindowMoved
        | CompositorEvent::WindowFloated
        | CompositorEvent::WindowPinned
        | CompositorEvent::WindowFilled
        | CompositorEvent::WorkspaceChanged
        | CompositorEvent::ScreenFocused
        | CompositorEvent::ConfigurationReloaded
        | CompositorEvent::Ignored => Worth::Ignoring,
    })
}

pub fn events() -> Result<PathBuf, Error> {
    use console_compositor::socket::{Socket, Unplaced, socket};

    socket(Socket::Events).map_err(|why| match why {
        Unplaced::Sessionless => Error::Sessionless,
        Unplaced::Compositorless => Error::Compositorless,
    })
}

pub fn note() -> Result<PathBuf, Error> {
    let runtime = runtime()?;

    Ok(runtime.join(console_core_places::OURS).join("tab"))
}

pub fn saying(tab: &str) -> Result<(), Error> {
    let note = note()?;

    match note.parent() {
        Some(above) => std::fs::create_dir_all(above)
            .map_err(|fault| Error::Making(above.to_path_buf(), fault))?,
        None => {}
    }

    console_core_atomic_writes::whole(&note, tab.as_bytes()).map_err(Error::Writing)
}

pub fn forget_if_still(said: &str) -> Result<(), Error> {
    let front = tab()?;

    match front.as_deref() == Some(said) {
        true => forget(),
        false => Ok(()),
    }
}

pub fn forget() -> Result<(), Error> {
    let note = note()?;

    match std::fs::remove_file(&note) {
        Ok(()) => Ok(()),
        Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
            true => Ok(()),
            false => Err(Error::Removing(note, fault)),
        },
    }
}

pub fn tab() -> Result<Option<String>, Error> {
    let note = note()?;

    let Ok(held) = console_core_atomic_writes::read(&note);

    match held {
        Stored::Text(said) => Ok(Some(said.trim().to_string())),
        Stored::Absent => Ok(None),
        Stored::Failed(fault) => Err(Error::Read(note, fault)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tab<'a>(pub &'a str);

pub fn open_on(namespace: &str, tab_: Tab<'_>) -> Result<Up, Error> {
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

    fn layers(said: &str) -> Vec<console_compositor::Layer> {
        let value = match console_compositor::read_value(said) {
            Ok(value) => value,
            Err(_unreadable) => return Vec::new(),
        };

        match console_compositor::answer_of(console_compositor::Query::Layers, value) {
            Ok(console_compositor::Answer::Layers(layers)) => layers,
            Ok(_not_layers) => Vec::new(),
            Err(_unreadable) => Vec::new(),
        }
    }

    fn up(layers: &[console_compositor::Layer], namespace: &str) -> Up {
        let Ok(up) = super::up(layers, namespace);

        up
    }

    fn standing(layers: &[console_compositor::Layer], namespace: &str) -> Option<Standing> {
        let Ok(standing) = super::standing(layers, namespace);

        standing
    }

    fn worth_asking_after(line: &str) -> Worth {
        let Ok(worth) = super::worth_asking_after(line);

        worth
    }

    const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","x":0,"y":0,"w":1920,"h":1600}],
        "2":[{"namespace":"console-bar","x":0,"y":0,"w":1920,"h":40}]}}}"#;

    #[test]
    fn a_door_nothing_opened_is_shut() {
        assert_eq!(up(&layers(NOTHING_UP), "launcher"), Up::NotThere);
        assert_eq!(up(&layers(NOTHING_UP), "console-keyboard"), Up::NotThere);
    }

    #[test]
    fn the_menu_being_on_the_screen_opens_its_door() {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","x":0,"y":0,"w":1920,"h":1600}],
            "3":[{"namespace":"launcher","x":0,"y":0,"w":1920,"h":1562}]}}}"#;
        assert_eq!(up(&layers(said), "launcher"), Up::OnScreen);
        assert_eq!(up(&layers(said), "console-keyboard"), Up::NotThere);
    }

    #[test]
    fn the_name_wofi_used_opens_nothing() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"launcher","x":0,"y":0,"w":1920,"h":1562}]}}}"#;
        assert_eq!(up(&layers(said), "wofi"), Up::NotThere);
    }

    #[test]
    fn another_panel_does_not_open_the_menus_door() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"settings-panel","x":0,"y":0,"w":1920,"h":1562}]}}}"#;
        assert_eq!(up(&layers(said), "launcher"), Up::NotThere);
    }

    #[test]
    fn a_keyboard_with_no_height_is_a_keyboard_no_one_can_see() {
        let hidden = r#"{"eDP-1":{"levels":{"3":[{"namespace":"console-keyboard","x":0,"y":0,"w":1920,"h":0}]}}}"#;
        let up_ = r#"{"eDP-1":{"levels":{"3":[{"namespace":"console-keyboard","x":0,"y":0,"w":1920,"h":520}]}}}"#;
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
            Some(Standing { x: 260, y: 140, width: 1400, height: 900 })
        );
    }

    #[test]
    fn nothing_is_standing_where_nothing_is_drawn() {
        assert_eq!(standing(&layers(A_PANEL), "launcher"), None);
    }

    #[test]
    fn a_panel_no_one_can_see_is_standing_nowhere_either() {
        let empty = r#"{"eDP-1":{"levels":{
            "3":[{"namespace":"settings-panel","x":260,"y":140,"w":1400,"h":0}]}}}"#;
        assert_eq!(up(&layers(empty), "settings-panel"), Up::NotThere);
        assert_eq!(standing(&layers(empty), "settings-panel"), None);
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
        assert_eq!(worth_asking_after("openlayer>>console-keyboard"), Worth::Querying);
        assert_eq!(worth_asking_after("closelayer>>launcher"), Worth::Querying);
        assert_eq!(worth_asking_after("mousemove>>640,400"), Worth::Ignoring);
        assert_eq!(worth_asking_after("openwindow>>a4f,3,alacritty,Alacritty"), Worth::Ignoring);
        assert_eq!(worth_asking_after(""), Worth::Ignoring);
    }
}
