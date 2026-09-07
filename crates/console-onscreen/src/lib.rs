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

use std::path::PathBuf;

use console_core_external_programs::Program;
use console_core_never::Never;

pub mod homeward;

pub use homeward::{Awake, Said, homeward, telling, waking};

fn asked(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|fault| format!("{name}: {fault}"))
}

pub fn screens() -> Result<serde_json::Value, String> {
    let Ok(mut hyprctl) = Program::Hyprctl.command();

    let said = hyprctl
        .args(["layers", "-j"])
        .output()
        .map_err(|fault| format!("asking hyprctl what is on the screen: {fault}"))?;

    serde_json::from_slice(&said.stdout)
        .map_err(|fault| format!("reading hyprctl's answer about what is on the screen: {fault}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Up {
    OnScreen,
    NotThere,
}

pub fn is_open(namespace: &str) -> Result<Up, String> {
    let screens = screens()?;

    let Ok(up) = up(&screens, namespace);

    Ok(up)
}

fn drawn<'a>(
    screens: &'a serde_json::Value,
    namespace: &'a str,
) -> Result<impl Iterator<Item = &'a serde_json::Value>, Never> {
    Ok(screens
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, screen)| {
            let levels = screen.get("levels")?;

            levels.as_object()
        })
        .flatten()
        .filter_map(|(_, level)| level.as_array())
        .flatten()
        .filter(move |surface| {
            surface
                .get("namespace")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|named| named.starts_with(namespace))
        })
        .filter(|surface| surface.get("h").and_then(serde_json::Value::as_i64).unwrap_or(1) > 0))
}

pub fn up(screens: &serde_json::Value, namespace: &str) -> Result<Up, Never> {
    let Ok(mut drawn) = drawn(screens, namespace);

    Ok(match drawn.next() {
        Some(_surface) => Up::OnScreen,
        None => Up::NotThere,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    pub across: i64,
    pub down: i64,
    pub wide: i64,
    pub tall: i64,
}

pub fn standing(screens: &serde_json::Value, namespace: &str) -> Result<Option<Standing>, Never> {
    let Ok(mut drawn) = drawn(screens, namespace);

    let Some(surface) = drawn.next() else { return Ok(None) };

    let said = |name: &str| surface.get(name).and_then(serde_json::Value::as_i64);

    let Some(across) = said("x") else { return Ok(None) };

    let Some(down) = said("y") else { return Ok(None) };

    let Some(wide) = said("w") else { return Ok(None) };

    let Some(tall) = said("h") else { return Ok(None) };

    Ok(Some(Standing { across, down, wide, tall }))
}

pub const FURNITURE: [&str; 7] = [
    "awww-daemon",
    "waybar",
    "updating",
    "virtual-keyboard",
    "notifications",
    "mako",
    HOME,
];

pub const HOME: &str = "console-home";

pub const KEYBOARD: &str = "virtual-keyboard";

pub const ASKING: &str = "console-asking";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Over {
    Something,
    Nothing,
}

pub fn over_the_desktop(screens: &serde_json::Value) -> Result<Over, Never> {
    let over = screens
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, screen)| {
            let levels = screen.get("levels")?;

            levels.as_object()
        })
        .flatten()
        .filter_map(|(_, level)| level.as_array())
        .flatten()
        .filter(|surface| surface.get("h").and_then(serde_json::Value::as_i64).unwrap_or(1) > 0)
        .filter_map(|surface| {
            let namespace = surface.get("namespace")?;

            namespace.as_str()
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
    Ok(match line.starts_with("openlayer>>") || line.starts_with("closelayer>>") {
        true => Worth::Asking,
        false => Worth::Ignoring,
    })
}

pub fn events() -> Result<PathBuf, String> {
    let run = asked("XDG_RUNTIME_DIR")?;
    let instance = asked("HYPRLAND_INSTANCE_SIGNATURE")?;

    Ok(std::path::Path::new(&run).join("hypr").join(instance).join(".socket2.sock"))
}

fn note() -> Result<PathBuf, String> {
    let runtime = asked("XDG_RUNTIME_DIR")?;

    Ok(std::path::Path::new(&runtime).join("console").join("tab"))
}

pub fn saying(tab: &str) -> Result<(), String> {
    let note = note()?;

    match note.parent() {
        Some(above) => std::fs::create_dir_all(above)
            .map_err(|fault| format!("{}: making it: {fault}", above.display()))?,
        None => {}
    }

    std::fs::write(&note, tab).map_err(|fault| format!("{}: writing it: {fault}", note.display()))
}

pub fn forget() -> Result<(), String> {
    let note = note()?;

    match std::fs::remove_file(&note) {
        Ok(()) => Ok(()),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(fault) => Err(format!("{}: removing it: {fault}", note.display())),
    }
}

pub fn tab() -> Result<Option<String>, String> {
    let note = note()?;

    match std::fs::read_to_string(&note) {
        Ok(said) => Ok(Some(said.trim().to_string())),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(fault) => Err(format!("{}: reading it: {fault}", note.display())),
    }
}

pub fn open_on(namespace: &str, tab_: &str) -> Result<Up, String> {
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
        "2":[{"namespace":"waybar","h":38}]}}}"#;

    #[test]
    fn a_door_nothing_opened_is_shut() {
        assert_eq!(up(&layers(NOTHING_UP), "launcher"), Up::NotThere);
        assert_eq!(up(&layers(NOTHING_UP), "virtual-keyboard"), Up::NotThere);
    }

    #[test]
    fn the_menu_being_on_the_screen_opens_its_door() {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600}],
            "3":[{"namespace":"launcher","h":1562}]}}}"#;
        assert_eq!(up(&layers(said), "launcher"), Up::OnScreen);
        assert_eq!(up(&layers(said), "virtual-keyboard"), Up::NotThere);
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
        let hidden = r#"{"eDP-1":{"levels":{"3":[{"namespace":"virtual-keyboard","h":0}]}}}"#;
        let up_ = r#"{"eDP-1":{"levels":{"3":[{"namespace":"virtual-keyboard","h":520}]}}}"#;
        assert_eq!(up(&layers(hidden), "virtual-keyboard"), Up::NotThere);
        assert_eq!(up(&layers(up_), "virtual-keyboard"), Up::OnScreen);
    }

    const A_PANEL: &str = r#"{"eDP-1":{"levels":{
        "2":[{"namespace":"waybar","x":0,"y":0,"w":1920,"h":38}],
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
        assert_eq!(worth_asking_after("openlayer>>virtual-keyboard"), Worth::Asking);
        assert_eq!(worth_asking_after("closelayer>>launcher"), Worth::Asking);
        assert_eq!(worth_asking_after("mousemove>>640,400"), Worth::Ignoring);
        assert_eq!(worth_asking_after("openwindow>>a4f,3,alacritty,Alacritty"), Worth::Ignoring);
        assert_eq!(worth_asking_after(""), Worth::Ignoring);
    }
}
