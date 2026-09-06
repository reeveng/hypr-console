//! The emulator, and the daemon running against it in this process.
//!
//! No machine takes part. What can be seen is what the daemon decided to run,
//! and what it wrote to the pointer it publishes.
//!
//! Time here is turns of the daemon's own loop rather than seconds. A daemon
//! that turns how long a stick was held into how far a page scrolled is
//! arithmetic, and arithmetic has one right answer; reading the machine's clock
//! would make it a race instead.

use evdev::EventType;
use console_controller::doing::{Doing, Out};
use console_controller::means::Table;
use console_controller::mode::{Awake, Mode};

pub use console_controller::mode::Acts;
use console_controller::turning::Turning;
use console_gamepad::capture::captured;
use console_gamepad::devices::Devices;
use console_gamepad::go::{Held, LegionGo};
use console_gamepad::router::every_profile;
use console_gamepad::world::World;
use console_never::Never;

use crate::device::Seen;
use crate::plug::Plug;

pub const TURNS: usize = 3;

const STARTED: f64 = 1000.0;

pub struct Here {
    pub go: LegionGo<World, Held>,
    turning: Turning,
    now: f64,
    pub commands: Vec<Vec<String>>,
    pub written: Vec<Out>,
    pub told: Vec<console_onscreen::Said>,
    layers: Option<serde_json::Value>,
    awake: Awake,
}

impl Here {
    pub fn new() -> Result<Self, String> {
        let seen = captured()?;
        let world = captured()?;
        let Ok(world) = World::of(world);
        let Ok(devices) = Devices::new(seen, world);
        let Ok(root) = crate::root();
        let profiles = every_profile(&root)?;
        let go =
            LegionGo::new(profiles, devices, Held::default(), console_gamepad::router::NAME)?;
        Ok(Here {
            go,
            turning: Turning::default(),
            now: STARTED,
            commands: Vec::new(),
            written: Vec::new(),
            told: Vec::new(),
            layers: None,
            awake: Awake::No,
        })
    }

    pub fn press(&mut self, button: &str) -> Result<(), String> {
        self.go.press(button)
    }

    pub fn hold(&mut self, button: &str) -> Result<(), String> {
        self.go.hold(button)
    }

    pub fn release(&mut self, button: Option<&str>) -> Result<(), String> {
        match button {
            Some(button) => self.go.release(button),
            None => self.go.release_all(),
        }
    }

    pub fn stick(&mut self, which: &str, across: f64, down: f64) -> Result<(), String> {
        self.go.stick(which, across, down)
    }

    pub fn trigger(&mut self, which: &str, amount: f64) -> Result<(), String> {
        self.go.trigger(which, amount)
    }

    pub fn tap(&mut self, across: i32, down: i32) -> Result<(), Never> {
        self.go.tap(across, down)
    }

    pub fn drag(&mut self, from: (i32, i32), to: (i32, i32)) -> Result<(), Never> {
        self.go.drag(from, to, 8, 0.0)
    }

    pub fn load_profile(&mut self, name: &str) -> Result<(), String> {
        self.go.load_profile(name)
    }

    pub fn bound_by(&mut self, table: Table) -> Result<(), Never> {
        let Ok(()) = self.turning.bound_by(table);

        Ok(())
    }

    pub fn showing(&mut self, layers: &str) -> Result<(), String> {
        let said = serde_json::from_str(layers).map_err(|fault| format!("layers: {fault}"))?;
        self.layers = Some(said);

        let Ok(()) = self.reckons();

        Ok(())
    }

    fn reckons(&mut self) -> Result<(), Never> {
        let Some(layers) = self.layers.clone() else { return Ok(()) };

        let Ok(seen) = Mode::seen(&layers, self.awake);

        self.in_front(seen)
    }

    pub fn awake(&self) -> Result<Awake, Never> {
        Ok(self.awake)
    }

    pub fn in_front(&mut self, mode: Mode) -> Result<(), Never> {
        let Ok(now_in) = self.turning.held.now_in(mode);

        for what in now_in {
            match what {
                Doing::Run(argv) => self.commands.push(argv),
                Doing::Frame(frame) => self.written.extend(frame),
                Doing::Tell(said) => self.told.push(said),
            }
        }

        Ok(())
    }

    pub fn mode(&self) -> Result<Mode, Never> {
        Ok(self.turning.held.mode)
    }

    pub fn acts(&self) -> Result<Acts, Never> {
        let Ok(mode) = self.mode();

        mode.acts()
    }

    pub fn settle(&mut self, turns: usize) -> Result<(), Never> {
        for _ in 0..turns {
            let mut plug = Plug { devices: &mut self.go.devices };

            let Ok(turned) = self.turning.turn(&mut plug, self.now);

            for what in turned {
                match what {
                    Doing::Run(argv) => self.commands.push(argv),
                    Doing::Frame(frame) => self.written.extend(frame),
                    Doing::Tell(said) => {
                        self.told.push(said);
                        self.awake = match said {
                            console_onscreen::Said::Back => Awake::No,
                            console_onscreen::Said::Up
                            | console_onscreen::Said::Down
                            | console_onscreen::Said::Left
                            | console_onscreen::Said::Right
                            | console_onscreen::Said::Pressed
                            | console_onscreen::Said::More
                            | console_onscreen::Said::Again
                            | console_onscreen::Said::Carry
                            | console_onscreen::Said::Off => Awake::Yes,
                        };
                        let Ok(()) = self.reckons();
                    }
                }
            }

            let Ok(poll) = self.turning.poll();

            self.now += poll;
        }

        Ok(())
    }

    pub fn commands(&self) -> Result<&[Vec<String>], Never> {
        Ok(&self.commands)
    }

    pub fn dispatches(&self) -> Result<Vec<String>, Never> {
        Ok(self
            .commands
            .iter()
            .filter(|argv| argv.first().is_some_and(|word| word.ends_with("hyprctl")))
            .filter(|argv| argv.get(1).is_some_and(|word| word == "dispatch"))
            .filter_map(|argv| argv.last().cloned())
            .collect())
    }

    pub fn names(&self) -> Result<Vec<String>, Never> {
        Ok(self
            .commands
            .iter()
            .filter_map(|argv| argv.first())
            .map(|program| program.rsplit('/').next().unwrap_or(program).to_string())
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

    pub fn sent(&self, kind: EventType, code: u16, value: i32) -> Result<Seen, Never> {
        let found = self
            .written
            .iter()
            .any(|out| out.kind == kind && out.code == code && out.value == value);

        Ok(match found {
            true => Seen::Yes,
            false => Seen::NotYet,
        })
    }

    pub fn fresh(&mut self) -> Result<(), Never> {
        self.commands.clear();
        self.written.clear();
        self.told.clear();

        Ok(())
    }

    pub fn told(&self) -> Result<&[console_onscreen::Said], Never> {
        Ok(&self.told)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::RelativeAxisCode;

    fn names(here: &Here) -> Vec<String> {
        let Ok(names) = here.names();

        names
    }

    fn wrote(here: &Here, kind: EventType, code: u16) -> i32 {
        let Ok(wrote) = here.wrote(kind, code);

        wrote
    }

    fn commands(here: &Here) -> Vec<Vec<String>> {
        let Ok(commands) = here.commands();

        commands.to_vec()
    }

    #[test]
    fn a_press_reaches_the_daemon_and_comes_out_as_what_it_runs() {
        let mut here = Here::new().expect("a stage");
        here.press("left-paddle-top").expect("a paddle");
        here.settle(TURNS);
        assert_eq!(names(&here), ["launcher"]);
    }

    #[test]
    fn a_stick_held_over_turns_of_the_loop_turns_the_wheel() {
        let mut here = Here::new().expect("a stage");
        here.stick("right-stick", 0.0, -1.0).expect("a stick");
        here.settle(12);
        assert!(wrote(&here, EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0) > 0);
    }

    #[test]
    fn a_fresh_stage_remembers_nothing() {
        let mut here = Here::new().expect("a stage");
        here.press("left-paddle-top").expect("a paddle");
        here.settle(TURNS);
        here.fresh();
        assert!(commands(&here).is_empty());
        assert_eq!(wrote(&here, EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0), 0);
    }
}
