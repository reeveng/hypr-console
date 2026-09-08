//! What the daemon decided to do, said without doing it.
//!
//! Everything in this crate that thinks is a function from what arrived to one
//! of these. Nothing that thinks touches a device, which is why the arithmetic
//! can be held still and asked the same question twice.
//!
//! [`Doing::Using`] is the odd one and is here rather than anywhere else for
//! the same reason as the rest: which input somebody is using is decided by
//! what arrived, so it is decided where everything else is, and carrying it
//! out -- saying so to `console-events` and writing the word down for the next
//! session -- is the binary's, like starting a program is.

use console_core_never::Never;
use evdev::EventType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Out {
    pub kind: EventType,
    pub code: u16,
    pub value: i32,
}

impl Out {
    pub fn rel(code: u16, value: i32) -> Result<Self, Never> {
        Ok(Out { kind: EventType::RELATIVE, code, value })
    }

    pub fn key(code: u16, value: i32) -> Result<Self, Never> {
        Ok(Out { kind: EventType::KEY, code, value })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doing {
    Run(Vec<String>),
    Frame(Vec<Out>),
    Tell(console_onscreen::Said),
    Using(console_input_bindings::bound::Input),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carry {
    Window,
    Nothing,
}

impl Doing {
    pub fn run(argv: &[&str]) -> Result<Self, Never> {
        Ok(Doing::Run(argv.iter().map(|word| (*word).to_string()).collect()))
    }

    pub fn dispatch(lua: &str) -> Result<Self, Never> {
        let Ok(hyprctl) = console_core_external_programs::Program::Hyprctl.name();
        let Ok(dispatch) = console_compositor::Told::Dispatch.word();

        Ok(Doing::Run(vec![hyprctl.to_string(), dispatch.to_string(), lua.to_string()]))
    }

    pub fn workspace(where_: &str, carrying: Carry) -> Result<Self, Never> {
        let verb = match carrying {
            Carry::Window => "hl.dsp.window.move",
            Carry::Nothing => "hl.dsp.focus",
        };

        Doing::dispatch(&format!("{verb}({{workspace = \"{where_}\"}})"))
    }
}

#[cfg(test)]
mod tests {
    use console_core_external_programs::Program;

    use super::*;

    #[test]
    fn a_shoulder_moves_you_and_holding_l2_carries_the_window() {
        let Ok(hyprctl) = Program::Hyprctl.name();

        assert_eq!(
            Doing::workspace("+1", Carry::Nothing),
            Doing::run(&[hyprctl, "dispatch", "hl.dsp.focus({workspace = \"+1\"})"])
        );
        assert_eq!(
            Doing::workspace("-1", Carry::Window),
            Doing::run(&[hyprctl, "dispatch", "hl.dsp.window.move({workspace = \"-1\"})"])
        );
    }
}
