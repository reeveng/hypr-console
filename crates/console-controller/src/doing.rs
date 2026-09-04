//! What the daemon decided to do, said without doing it.
//!
//! Everything in this crate that thinks is a function from what arrived to one
//! of these. Nothing that thinks touches a device, which is why the arithmetic
//! can be held still and asked the same question twice.

use evdev::EventType;

/// One thing written to the device this daemon publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Out {
    pub kind: EventType,
    pub code: u16,
    pub value: i32,
}

impl Out {
    pub fn rel(code: u16, value: i32) -> Self {
        Out { kind: EventType::RELATIVE, code, value }
    }

    pub fn key(code: u16, value: i32) -> Self {
        Out { kind: EventType::KEY, code, value }
    }
}

/// One decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doing {
    /// Start something. Whatever this starts stays in the daemon's own control
    /// group, so a signal sent to the unit reaches all of it.
    Run(Vec<String>),
    /// One frame, which is written and then reported as one.
    Frame(Vec<Out>),
    /// A word to the home screen.
    ///
    /// Not a key and not a program: the home screen holds no keyboard, for the
    /// reason `console_door::homeward` sets out, so what the pad did is said to
    /// it directly. Nothing is started and nothing is waited for.
    Tell(console_door::Said),
}

/// Whether a workspace move takes the window with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carry {
    /// Hold L2 and the window comes along, which is the only way to move one
    /// somewhere else without a keyboard.
    Window,
    /// Just the view moves, and everything stays where it was.
    Nothing,
}

impl Doing {
    /// Something to run, from words.
    pub fn run(argv: &[&str]) -> Self {
        Doing::Run(argv.iter().map(|word| (*word).to_string()).collect())
    }

    /// A workspace, moved to or carried to.
    ///
    /// The shoulders move between workspaces. Hold L2 and they carry the
    /// window with you instead, which is the only way to move a window
    /// somewhere else without a keyboard.
    pub fn workspace(where_: &str, carrying: Carry) -> Self {
        let verb = match carrying {
            Carry::Window => "hl.dsp.window.move",
            Carry::Nothing => "hl.dsp.focus",
        };
        Doing::Run(vec![
            "hyprctl".to_string(),
            "dispatch".to_string(),
            format!("{verb}({{workspace = \"{where_}\"}})"),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shoulder_moves_you_and_holding_l2_carries_the_window() {
        assert_eq!(
            Doing::workspace("+1", Carry::Nothing),
            Doing::run(&["hyprctl", "dispatch", "hl.dsp.focus({workspace = \"+1\"})"])
        );
        assert_eq!(
            Doing::workspace("-1", Carry::Window),
            Doing::run(&["hyprctl", "dispatch", "hl.dsp.window.move({workspace = \"-1\"})"])
        );
    }
}
