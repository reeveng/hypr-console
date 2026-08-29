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
use console_controller::turning::Turning;
use console_pad::capture::captured;
use console_pad::devices::Devices;
use console_pad::go::{Held, LegionGo};
use console_pad::profile::load_all;
use console_pad::world::World;

use crate::plug::Plug;

/// How many turns of the loop a settle is, when a check does not say.
pub const TURNS: usize = 3;

/// Where the clock starts, which is nowhere in particular.
const STARTED: f64 = 1000.0;

pub struct Here {
    pub go: LegionGo<World, Held>,
    turning: Turning,
    now: f64,
    /// Every command the daemon started, instead of starting any of them.
    pub commands: Vec<Vec<String>>,
    /// Everything it wrote to the device it publishes.
    pub written: Vec<Out>,
}

impl Here {
    pub fn new() -> Result<Self, String> {
        let devices = Devices::new(captured(), World::of(captured()));
        let go = LegionGo::new(load_all(&crate::root())?, devices, Held::default(), "desktop")?;
        Ok(Here {
            go,
            turning: Turning::default(),
            now: STARTED,
            commands: Vec::new(),
            written: Vec::new(),
        })
    }

    // doing

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

    pub fn tap(&mut self, across: i32, down: i32) {
        self.go.tap(across, down);
    }

    pub fn drag(&mut self, from: (i32, i32), to: (i32, i32)) {
        self.go.drag(from, to, 8, 0.0);
    }

    pub fn load_profile(&mut self, name: &str) -> Result<(), String> {
        self.go.load_profile(name)
    }

    /// Let the daemon read what was sent.
    pub fn settle(&mut self, turns: usize) {
        for _ in 0..turns {
            let mut plug = Plug { devices: &mut self.go.devices };
            for what in self.turning.turn(&mut plug, self.now) {
                match what {
                    Doing::Run(argv) => self.commands.push(argv),
                    Doing::Frame(frame) => self.written.extend(frame),
                }
            }
            self.now += self.turning.poll();
        }
    }

    // seeing

    pub fn commands(&self) -> &[Vec<String>] {
        &self.commands
    }

    /// What was asked of the compositor, as the argument it was given.
    pub fn dispatches(&self) -> Vec<String> {
        self.commands
            .iter()
            .filter(|argv| argv.first().is_some_and(|word| word.ends_with("hyprctl")))
            .filter(|argv| argv.get(1).is_some_and(|word| word == "dispatch"))
            .filter_map(|argv| argv.last().cloned())
            .collect()
    }

    /// Just the program of each, which is usually the whole question.
    pub fn names(&self) -> Vec<String> {
        self.commands
            .iter()
            .filter_map(|argv| argv.first())
            .map(|program| program.rsplit('/').next().unwrap_or(program).to_string())
            .collect()
    }

    pub fn profile(&self) -> &str {
        self.go.profile_name()
    }

    /// How much of something the daemon sent to the pointer.
    pub fn wrote(&self, kind: EventType, code: u16) -> i32 {
        self.written
            .iter()
            .filter(|out| out.kind == kind && out.code == code)
            .map(|out| out.value)
            .sum()
    }

    /// Whether it ever sent exactly that.
    pub fn sent(&self, kind: EventType, code: u16, value: i32) -> bool {
        self.written
            .iter()
            .any(|out| out.kind == kind && out.code == code && out.value == value)
    }

    /// Forget what the last check did; another one is next.
    pub fn fresh(&mut self) {
        self.commands.clear();
        self.written.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::RelativeAxisCode;

    #[test]
    fn a_press_reaches_the_daemon_and_comes_out_as_what_it_runs() {
        let mut here = Here::new().expect("a stage");
        here.press("left-paddle-top").expect("a paddle");
        here.settle(TURNS);
        assert_eq!(here.names(), ["launcher"]);
    }

    #[test]
    fn a_stick_held_over_turns_of_the_loop_turns_the_wheel() {
        let mut here = Here::new().expect("a stage");
        here.stick("right-stick", 0.0, -1.0).expect("a stick");
        here.settle(12);
        assert!(here.wrote(EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0) > 0);
    }

    /// One check's idea of what has been run is its own.
    #[test]
    fn a_fresh_stage_remembers_nothing() {
        let mut here = Here::new().expect("a stage");
        here.press("left-paddle-top").expect("a paddle");
        here.settle(TURNS);
        here.fresh();
        assert!(here.commands().is_empty());
        assert_eq!(here.wrote(EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0), 0);
    }
}
