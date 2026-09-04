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
use console_controller::turning::Turning;
use console_pad::capture::captured;
use console_pad::devices::Devices;
use console_pad::go::{Held, LegionGo};
use console_pad::router::every_profile;
use console_pad::world::World;

use crate::device::Seen;
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
    /// Every word it said to the home screen, in the order it said them.
    pub told: Vec<console_door::Said>,
    /// What the compositor last said was on the screen, kept so that the mode
    /// can be worked out again when the home screen wakes or sleeps.
    layers: Option<serde_json::Value>,
    /// Whether the home screen is holding a highlight.
    ///
    /// Modelled here rather than asked of a file, because the home screen is
    /// not running: what wakes it is a word the daemon said, and this is what
    /// hearing that word would have come to. Without it a check could press
    /// the d-pad and then A and get the answer for a home screen that had
    /// never been woken -- which is the fault this was written for, tested
    /// from the wrong side.
    awake: Awake,
}

impl Here {
    pub fn new() -> Result<Self, String> {
        let devices = Devices::new(captured()?, World::of(captured()?));
        let go =
            LegionGo::new(every_profile(&crate::root())?, devices, Held::default(), console_pad::router::NAME)?;
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

    /// Hand the daemon a table somebody has said something about.
    ///
    /// On the device the daemon reads `~/.config/console/buttons.toml` itself
    /// and rebuilds the table when the file changes underneath it. There is no
    /// home here, so the table is handed over already built -- and it is the
    /// same table, built by the same `Table::of`, so what is being checked is
    /// what somebody's answers come to and not a second reading of them.
    pub fn bound_by(&mut self, table: Table) {
        self.turning.bound_by(table);
    }

    /// Say what is in front of the daemon, in the compositor's own words.
    ///
    /// On the device the daemon asks `hyprctl layers -j` and reads the answer.
    /// There is no compositor here, so the answer is handed over instead --
    /// and it is the same answer, read by the same `Mode::seen`, so what is
    /// being checked is the reading and not a second opinion about it.
    ///
    /// Without this, everything the mode decides could only be asked on the
    /// device: whether the daemon acts at all, and which profile the pad wants.
    /// That is most of what was written the night the keyboard was untangled,
    /// and none of it could be pressed anywhere but on hardware.
    pub fn showing(&mut self, layers: &str) -> Result<(), String> {
        let said = serde_json::from_str(layers).map_err(|fault| format!("layers: {fault}"))?;
        self.layers = Some(said);
        self.reckons();
        Ok(())
    }

    /// Work out where you are again, from what is on the screen and whether
    /// the home screen has a highlight up.
    fn reckons(&mut self) {
        let Some(layers) = self.layers.clone() else { return };

        self.in_front(Mode::seen(&layers, self.awake));
    }

    /// Whether the home screen is awake, as far as this stage is concerned.
    pub fn awake(&self) -> Awake {
        self.awake
    }

    /// Say what is in front, having already worked it out.
    pub fn in_front(&mut self, mode: Mode) {
        self.turning.held.now_in(mode);
    }

    /// What the daemon takes to be in front of it.
    pub fn mode(&self) -> Mode {
        self.turning.held.mode
    }

    /// Which profile the pad wants, given what is in front.
    ///
    /// What the daemon would ask for. `profile` is the other half -- the one
    /// the pad is actually wearing -- and the two being different is the whole
    /// of what a load is for.
    pub fn wanted(&self) -> &'static str {
        self.mode().profile()
    }

    /// Let the daemon read what was sent.
    pub fn settle(&mut self, turns: usize) {
        for _ in 0..turns {
            let mut plug = Plug { devices: &mut self.go.devices };

            for what in self.turning.turn(&mut plug, self.now) {
                match what {
                    Doing::Run(argv) => self.commands.push(argv),
                    Doing::Frame(frame) => self.written.extend(frame),
                    // What the home screen would have done about it. The
                    // d-pad raises the highlight and B puts it away, and both
                    // change what A means -- which is the whole reason the
                    // stage has to model it rather than collect it.
                    Doing::Tell(said) => {
                        self.told.push(said);
                        self.awake = match said {
                            console_door::Said::Back => Awake::No,
                            _ => Awake::Yes,
                        };
                        self.reckons();
                    }
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
    pub fn sent(&self, kind: EventType, code: u16, value: i32) -> Seen {
        let found = self
            .written
            .iter()
            .any(|out| out.kind == kind && out.code == code && out.value == value);

        match found {
            true => Seen::Yes,
            false => Seen::NotYet,
        }
    }

    /// Forget what the last check did; another one is next.
    pub fn fresh(&mut self) {
        self.commands.clear();
        self.written.clear();
        self.told.clear();
    }

    /// Every word said to the home screen since the last clearing.
    pub fn told(&self) -> &[console_door::Said] {
        &self.told
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
