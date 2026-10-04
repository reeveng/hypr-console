//! The emulator, and the daemon running against it in this process.
//!
//! No machine takes part. What can be seen is what the daemon decided to run,
//! and what it wrote to the pointer it publishes.
//!
//! Time here is turns of the daemon's own loop rather than seconds. A daemon
//! that turns how long a stick was held into how far a page scrolled is
//! arithmetic, and arithmetic has one right answer; reading the machine's clock
//! would make it a race instead.

use console_core_geometry::Point;
use console_input_event_devices::EventType;
use console_input_controller::effect::{Effect, Output};
use console_input_controller::actions::Table;
use console_input_controller::mode::{Focused, WakeState, Mode};

pub use console_input_controller::mode::InputHandling;
use console_input_controller::clock::Instant;
use console_input_controller::reading::{POLL, Wake};
use console_input_controller::turning::Turning;
use console_input_gamepad::capture::load_capture;
use console_input_gamepad::devices::Devices;
use console_input_gamepad::go::LegionGo;
use console_waiting::clock::TestClock;
use console_input_gamepad::router::every_profile;
use console_input_gamepad::world::World;
use console_core_never::Never;

use crate::Error;
use crate::checking::{CheckResult, same};
use crate::device::Ready;
use crate::plug::Plug;

pub const TURNS: u32 = 3;

const STARTED: f64 = 1000.0;

pub struct Here {
    pub go: LegionGo<World, TestClock>,
    turning: Turning,
    now: Instant,
    pub commands: Vec<Vec<String>>,
    pub written: Vec<Output>,
    pub told: Vec<console_onscreen::PadInput>,
    pub using: Option<console_input_bindings::bound::Input>,
    layers: Option<Vec<console_compositor::Layer>>,
    awake: WakeState,
}

impl Here {
    pub fn new() -> Result<Self, Error> {
        let seen = load_capture()?;
        let world = load_capture()?;
        let Ok(world) = World::of(world);
        let Ok(devices) = Devices::new(seen, world);
        let Ok(root) = crate::root();
        let profiles = every_profile(&root)?;
        let go =
            LegionGo::new(profiles, devices, TestClock::default(), console_input_gamepad::router::NAME)?;
        Ok(Here {
            go,
            turning: Turning::default(),
            now: Instant { since_boot: STARTED, suspended: 0.0 },
            commands: Vec::new(),
            written: Vec::new(),
            told: Vec::new(),
            using: None,
            layers: None,
            awake: WakeState::No,
        })
    }

    pub fn press(&mut self, button: &str) -> Result<(), Error> {
        self.go.press(button).map_err(Error::Pressing)
    }

    pub fn hold(&mut self, button: &str) -> Result<(), Error> {
        self.go.hold(button).map_err(Error::Pressing)
    }

    pub fn release(&mut self, button: Option<&str>) -> Result<(), Error> {
        match button {
            Some(button) => self.go.release(button).map_err(Error::Pressing),
            None => self.go.release_all().map_err(Error::Pressing),
        }
    }

    pub fn thumbstick(&mut self, which: &str, to: Point<f64>) -> Result<(), Error> {
        self.go.thumbstick(which, to).map_err(Error::Pressing)
    }

    pub fn trigger(&mut self, which: &str, amount: f64) -> Result<(), Error> {
        self.go.trigger(which, amount).map_err(Error::Pressing)
    }

    pub fn tap(&mut self, at: Point<i32>) -> Result<(), Never> {
        self.go.tap(at)
    }

    pub fn drag(&mut self, from: Point<i32>, to: Point<i32>) -> Result<(), Never> {
        self.go.drag(from, to, 8, 0.0)
    }

    pub fn load_profile(&mut self, name: &str) -> Result<(), Error> {
        self.go.load_profile(name).map_err(Error::Pressing)
    }

    pub fn bound_by(&mut self, table: Table) -> Result<(), Never> {
        let Ok(()) = self.turning.bound_by(table);

        Ok(())
    }

    pub fn set_layers(&mut self, layers: &str) -> Result<(), Error> {
        let said = serde_json::from_str(layers).map_err(Error::Layers)?;

        let read = console_compositor::said_of(console_compositor::Layers, said);

        self.layers = match read {
            Ok(layers) => Some(layers),
            Err(_unreadable) => None,
        };

        let Ok(()) = self.reckons();

        Ok(())
    }

    fn reckons(&mut self) -> Result<(), Never> {
        let layers = match self.layers.clone() {
            Some(layers) => layers,
            None => return Ok(()),
        };

        let Ok(seen) = Mode::detect(&layers, self.awake, Focused::SomethingElse);

        self.in_front(seen)
    }

    pub fn awake(&self) -> Result<WakeState, Never> {
        Ok(self.awake)
    }

    pub fn in_front(&mut self, mode: Mode) -> Result<(), Never> {
        let Ok(now_in) = self.turning.held.now_in(mode);

        for what in now_in {
            match what {
                Effect::Run(arguments) => self.commands.push(arguments),
                Effect::Frame(frame) => self.written.extend(frame),
                Effect::Tell(said) => self.told.push(said),
                Effect::Using(on) => self.using = Some(on),
                Effect::Reconnected(_) => {}
            }
        }

        Ok(())
    }

    pub fn mode(&self) -> Result<Mode, Never> {
        Ok(self.turning.held.mode)
    }

    pub fn input_handling(&self) -> Result<InputHandling, Never> {
        let Ok(mode) = self.mode();

        mode.input_handling()
    }

    pub fn settle(&mut self, turns: u32) -> Result<(), Never> {
        for _ in 0..turns {
            let mut plug = Plug { devices: &mut self.go.devices };

            let Ok(turned) = self.turning.turn(&mut plug, self.now);

            for what in turned {
                match what {
                    Effect::Run(arguments) => self.commands.push(arguments),
                    Effect::Frame(frame) => self.written.extend(frame),
                    Effect::Tell(said) => {
                        self.told.push(said);
                        self.awake = match said {
                            console_onscreen::PadInput::Back => WakeState::No,
                            console_onscreen::PadInput::Up
                            | console_onscreen::PadInput::Down
                            | console_onscreen::PadInput::Left
                            | console_onscreen::PadInput::Right
                            | console_onscreen::PadInput::Pressed
                            | console_onscreen::PadInput::More
                            | console_onscreen::PadInput::Again
                            | console_onscreen::PadInput::Payload
                            | console_onscreen::PadInput::Off => WakeState::Yes,
                        };
                        let Ok(()) = self.reckons();
                    }
                    Effect::Using(on) => self.using = Some(on),
                    Effect::Reconnected(_) => {}
                }
            }

            let Ok(wake) = self.turning.wake();

            self.now.since_boot += match wake {
                Wake::Within(seconds) => seconds.min(POLL),
                Wake::OnInput => POLL,
            };
        }

        Ok(())
    }

    pub fn commands(&self) -> Result<&[Vec<String>], Never> {
        Ok(&self.commands)
    }

    pub fn ran(&mut self, wanted: &[&[&str]]) -> CheckResult {
        let Ok(()) = self.settle(TURNS);
        let ran = &self.commands;

        same(ran, wanted, || format!("it ran {ran:?}"))
    }

    pub fn dispatches(&self) -> Result<Vec<String>, Never> {
        console_compositor::batch_arguments(&self.commands)
    }

    pub fn names(&self) -> Result<Vec<String>, Never> {
        Ok(self
            .commands
            .iter()
            .filter_map(|arguments| arguments.first())
            .map(|program| match program.rsplit('/').next() {
                Some(named) => named.to_string(),
                None => program.clone(),
            })
            .collect())
    }

    pub fn profile(&self) -> Result<&str, Never> {
        self.go.profile_name()
    }

    pub fn wrote(&self, kind: EventType, code: u16) -> Result<i32, Never> {
        Ok(self
            .written
            .iter()
            .filter(|out| out.kind == kind && out.code == code)
            .map(|out| out.value)
            .sum())
    }

    pub fn check_sent(&self, kind: EventType, code: u16, value: i32) -> Result<Ready, Never> {
        let found = self
            .written
            .iter()
            .any(|out| out.kind == kind && out.code == code && out.value == value);

        Ok(match found {
            true => Ready::Yes,
            false => Ready::NotYet,
        })
    }

    pub fn fresh(&mut self) -> Result<(), Never> {
        self.commands.clear();
        self.written.clear();
        self.told.clear();

        Ok(())
    }

    pub fn pad_inputs(&self) -> Result<&[console_onscreen::PadInput], Never> {
        Ok(&self.told)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use console_input_event_devices::RelativeAxisCode;

    fn names(here: &Here) -> Result<Vec<String>, Never> {
        let Ok(names) = here.names();

        Ok(names)
    }

    fn wrote(here: &Here, kind: EventType, code: u16) -> Result<i32, Never> {
        let Ok(wrote) = here.wrote(kind, code);

        Ok(wrote)
    }

    fn commands(here: &Here) -> Result<Vec<Vec<String>>, Never> {
        let Ok(commands) = here.commands();

        Ok(commands.to_vec())
    }

    #[test]
    fn a_press_reaches_the_daemon_and_comes_out_as_what_it_runs() -> Result<(), Box<dyn Error>> {
        let mut here = Here::new()?;
        here.press("left-paddle-top")?;
        let Ok(()) = here.settle(TURNS);
        let Ok(names) = names(&here);

        assert_eq!(names, ["launcher"]);

        Ok(())
    }

    #[test]
    fn a_stick_held_over_turns_of_the_loop_turns_the_wheel() -> Result<(), Box<dyn Error>> {
        let mut here = Here::new()?;
        here.thumbstick("right-stick", Point { x: 0.0, y: -1.0 })?;
        let Ok(()) = here.settle(12);
        let Ok(turned) = wrote(&here, EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0);

        assert!(turned > 0);

        Ok(())
    }

    #[test]
    fn a_fresh_stage_remembers_nothing() -> Result<(), Box<dyn Error>> {
        let mut here = Here::new()?;
        here.press("left-paddle-top")?;
        let Ok(()) = here.settle(TURNS);
        let Ok(()) = here.fresh();
        let Ok(commands) = commands(&here);

        assert!(commands.is_empty());
        assert_eq!(wrote(&here, EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0), Ok(0));

        Ok(())
    }
}
