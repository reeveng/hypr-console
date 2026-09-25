//! What the daemon decided to do, said without doing it.
//!
//! Everything in this crate that thinks is a function from what arrived to one
//! of these. Nothing that thinks touches a device, which is why the arithmetic
//! can be held still and asked the same question twice.
//!
//! [`Effect::Using`] is the odd one and is here rather than anywhere else for
//! the same reason as the rest: which input someone is using is decided by
//! what arrived, so it is decided where everything else is, and carrying it
//! out -- saying so to `console-events` and writing the word down for the next
//! session -- is the binary's, like starting a program is.

use console_core_never::Never;
use console_input_event_devices::EventType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    pub kind: EventType,
    pub code: u16,
    pub value: i32,
}

impl Output {
    pub fn rel(code: u16, value: i32) -> Result<Self, Never> {
        Ok(Output { kind: EventType::RELATIVE, code, value })
    }

    pub fn key(code: u16, value: i32) -> Result<Self, Never> {
        Ok(Output { kind: EventType::KEY, code, value })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Run(Vec<String>),
    Frame(Vec<Output>),
    Tell(console_onscreen::PadInput),
    Using(console_input_bindings::bound::Input),
}

pub use console_compositor::Carrying as Payload;

impl Effect {
    pub fn run(arguments: &[&str]) -> Result<Self, Never> {
        Ok(Effect::Run(arguments.iter().map(|word| (*word).to_string()).collect()))
    }

    pub fn dispatch(lua: &str) -> Result<Self, Never> {
        let Ok(hyprctl) = console_core_external_programs::Program::Hyprctl.name();
        let Ok(dispatch) = console_compositor::Request::Dispatch.word();

        Ok(Effect::Run(vec![hyprctl.to_string(), dispatch.to_string(), lua.to_string()]))
    }

    pub fn workspace(where_: &str, carrying: Payload) -> Result<Self, Never> {
        let Ok(lua) = console_compositor::onto(where_, carrying);

        Effect::dispatch(&lua)
    }

    pub fn dispatched(&self) -> Result<Option<&str>, Never> {
        let Ok(hyprctl) = console_core_external_programs::Program::Hyprctl.name();
        let Ok(dispatch) = console_compositor::Request::Dispatch.word();

        Ok(match self {
            Effect::Run(arguments) => match arguments.as_slice() {
                [program, verb, lua] => match program == hyprctl && verb == dispatch {
                    true => Some(lua.as_str()),
                    false => None,
                },
                _not_a_dispatch => None,
            },
            Effect::Frame(_) | Effect::Tell(_) | Effect::Using(_) => None,
        })
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
            Effect::workspace("+1", Payload::None),
            Effect::run(&[hyprctl, "dispatch", "hl.dsp.focus({workspace = \"+1\"})"])
        );
        assert_eq!(
            Effect::workspace("-1", Payload::Window),
            Effect::run(&[hyprctl, "dispatch", "hl.dsp.window.move({workspace = \"-1\"})"])
        );
    }

    #[test]
    fn a_dispatch_is_known_for_one_so_it_can_be_told_without_starting_hyprctl() {
        let Ok(moved) = Effect::workspace("+1", Payload::None);
        let Ok(started) = Effect::run(&["launcher"]);

        assert_eq!(moved.dispatched(), Ok(Some("hl.dsp.focus({workspace = \"+1\"})")));
        assert_eq!(started.dispatched(), Ok(None));
    }
}
