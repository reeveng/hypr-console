//! What the compositor says when nobody asked it anything.
//!
//! The other half of this crate is questions. This half is the compositor
//! talking on its own: a window opened, a layer closed, the workspace in front
//! changed. `console-events` carries the line and is deliberately ignorant of
//! what any source's lines mean -- a relay that learns every vocabulary has to
//! be changed whenever a source says something new -- so the meaning is spelled
//! by the crate that already owns this compositor's words, which is this one.
//!
//! It was spelled four times before it was spelled here. The wallpaper held a
//! list of eight event names it wanted waking for, the home screen held a list
//! of eight it wanted to ask after, `console-onscreen` spelled two inline, and
//! the session held six and the rule for reading an address out of them. Two of
//! those lists were the same list in a different order and had already drifted:
//! one carries `workspacev2>>` and not `focusedmon>>`, the other the reverse,
//! and neither crate had decided that -- they had written the same thing twice
//! and edited one. What a caller cares about is a decision worth reading, so it
//! stays at the call site as a `match` naming every event; which words Hyprland
//! uses for those events is not a decision at all, and it is here.
//!
//! The addresses are the trap under all of it. `hyprctl clients` answers
//! `0x5634f2a0` and the event socket says `5634f2a0` for the same window, so a
//! window matched between the two matches nothing -- and nothing goes red,
//! because a program that adjusts no windows looks exactly like a program with
//! nothing to adjust. The `0x` goes back on here, once, so that no caller has to
//! know the two mouths spell a window differently.

use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stirred {
    WindowOpened(String),
    WindowClosed(String),
    WindowRenamed(String),
    WindowMoved,
    WindowFloated,
    WindowPinned,
    WindowFilled,
    LayerOpened,
    LayerClosed,
    WorkspaceChanged,
    ScreenFocused,
    ConfigReloaded,
    Nothing,
}

const OPENED: &str = "openwindow>>";
const CLOSED: &str = "closewindow>>";
const RENAMED: &str = "windowtitle>>";
const RENAMED_WITH_TITLE: &str = "windowtitlev2>>";

const CARRYING_NO_ADDRESS: [(&str, Stirred); 11] = [
    ("movewindowv2>>", Stirred::WindowMoved),
    ("movewindow>>", Stirred::WindowMoved),
    ("changefloatingmode>>", Stirred::WindowFloated),
    ("pin>>", Stirred::WindowPinned),
    ("fullscreen>>", Stirred::WindowFilled),
    ("openlayer>>", Stirred::LayerOpened),
    ("closelayer>>", Stirred::LayerClosed),
    ("workspacev2>>", Stirred::WorkspaceChanged),
    ("workspace>>", Stirred::WorkspaceChanged),
    ("focusedmon>>", Stirred::ScreenFocused),
    ("configreloaded>>", Stirred::ConfigReloaded),
];

pub fn addressed(said: &str) -> Result<String, Never> {
    let first = said.split(',').next().unwrap_or(said).trim();

    Ok(match first.starts_with("0x") {
        true => first.to_string(),
        false => format!("0x{first}"),
    })
}

fn carrying_no_address(line: &str) -> Result<Option<Stirred>, Never> {
    Ok(CARRYING_NO_ADDRESS.iter().find_map(|(word, then)| match line.starts_with(word) {
        true => Some(then.clone()),
        false => None,
    }))
}

pub fn read(line: &str) -> Result<Stirred, Never> {
    let line = line.trim();

    match line.strip_prefix(OPENED) {
        Some(said) => return addressed(said).map(Stirred::WindowOpened),
        None => {},
    }

    match line.strip_prefix(CLOSED) {
        Some(said) => return addressed(said).map(Stirred::WindowClosed),
        None => {},
    }

    let renamed = match line.strip_prefix(RENAMED_WITH_TITLE) {
        Some(said) => Some(said),
        None => line.strip_prefix(RENAMED),
    };

    match renamed {
        Some(said) => return addressed(said).map(Stirred::WindowRenamed),
        None => {},
    }

    let Ok(named) = carrying_no_address(line);

    Ok(match named {
        Some(stirred) => stirred,
        None => Stirred::Nothing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_opening_is_the_address_the_rest_of_the_line_belongs_to() {
        assert_eq!(
            read("openwindow>>5634f2a0,2,foot,a shell"),
            Ok(Stirred::WindowOpened("0x5634f2a0".to_string()))
        );
    }

    #[test]
    fn the_event_socket_leaves_off_the_nought_x_that_hyprctl_answers_with() {
        let Ok(bare) = addressed("5634f2a0");
        let Ok(already) = addressed("0x5634f2a0");

        assert_eq!(bare, "0x5634f2a0", "a saved window is matched by an address hyprctl spelled");
        assert_eq!(already, "0x5634f2a0");
    }

    #[test]
    fn a_window_renaming_itself_is_read_whichever_way_the_compositor_says_it() {
        assert_eq!(
            read("windowtitle>>5634f2a0"),
            Ok(Stirred::WindowRenamed("0x5634f2a0".to_string()))
        );
        assert_eq!(
            read("windowtitlev2>>5634f2a0,a shell"),
            Ok(Stirred::WindowRenamed("0x5634f2a0".to_string()))
        );
    }

    #[test]
    fn a_window_closing_carries_the_address_that_is_going() {
        assert_eq!(
            read("closewindow>>5634f2a0"),
            Ok(Stirred::WindowClosed("0x5634f2a0".to_string()))
        );
    }

    #[test]
    fn what_was_done_to_a_window_is_named_rather_than_lumped_together() {
        assert_eq!(read("movewindow>>5634f2a0,2"), Ok(Stirred::WindowMoved));
        assert_eq!(read("movewindowv2>>5634f2a0,2,name"), Ok(Stirred::WindowMoved));
        assert_eq!(read("changefloatingmode>>5634f2a0,1"), Ok(Stirred::WindowFloated));
        assert_eq!(read("pin>>5634f2a0,1"), Ok(Stirred::WindowPinned));
        assert_eq!(read("fullscreen>>1"), Ok(Stirred::WindowFilled));
    }

    #[test]
    fn what_happened_to_the_screen_rather_than_to_a_window_is_told_apart() {
        assert_eq!(read("openlayer>>waybar"), Ok(Stirred::LayerOpened));
        assert_eq!(read("closelayer>>waybar"), Ok(Stirred::LayerClosed));
        assert_eq!(read("workspace>>2"), Ok(Stirred::WorkspaceChanged));
        assert_eq!(read("workspacev2>>2,2"), Ok(Stirred::WorkspaceChanged));
        assert_eq!(read("focusedmon>>eDP-1,2"), Ok(Stirred::ScreenFocused));
    }

    #[test]
    fn the_longer_word_is_read_before_the_one_that_prefixes_it() {
        assert_eq!(read("movewindowv2>>5634f2a0,2,name"), Ok(Stirred::WindowMoved));
        assert_eq!(read("workspacev2>>2,2"), Ok(Stirred::WorkspaceChanged));
    }

    #[test]
    fn a_word_about_something_else_stirs_nothing() {
        assert_eq!(read("activelayout>>keyboard,English"), Ok(Stirred::Nothing));
        assert_eq!(read(""), Ok(Stirred::Nothing));
    }
}
