//! The daemon, its world, and what it did.
//!
//! A world of devices that is not this machine's, and a clock that is not this
//! machine's either. Nothing inside the daemon is stood in for: what it
//! decides, it decides.

use std::collections::BTreeMap;

use evdev::{AbsoluteAxisCode, EventType, InputEvent};
use console_input_controller::doing::{Doing, Out};
use console_input_controller::finding::Says;
use console_input_controller::reading::Ranges;
use console_input_controller::turning::{Gone, Plugged, Took, Turning};
use console_input_gamepad::capture::{Descriptor, captured};
use console_input_gamepad::devices::Devices;
use console_input_gamepad::go::{Held, LegionGo};
use console_input_gamepad::router::every_profile;
use console_input_gamepad::world::World;

pub type Go = LegionGo<World, Held>;

pub fn root() -> std::path::PathBuf {
    let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}

fn ok<T>(answer: Result<T, console_core_never::Never>) -> T {
    let Ok(value) = answer;

    value
}

pub fn go(profile: &str) -> Go {
    let world = ok(World::of(captured().expect("the capture carried in this program parses")));
    let devices =
        ok(Devices::new(captured().expect("the capture carried in this program parses"), world));
    LegionGo::new(every_profile(&root()).expect("the profiles"), devices, Held::default(), profile)
        .expect("a pad")
}

struct Plug<'a> {
    devices: &'a mut Devices<World>,
}

impl Plug<'_> {
    fn descriptor(&self, path: &str) -> Option<&Descriptor> {
        self.devices.descriptors.get(ok(self.devices.sink.role_at(path))?)
    }
}

impl Plugged for Plug<'_> {
    fn every(&self) -> Vec<Says> {
        ok(self.devices.sink.plugged())
            .into_iter()
            .filter_map(|path| {
                let told = self.descriptor(&path)?;
                Some(Says {
                    path,
                    name: told.name.clone(),
                    phys: told.phys.clone(),
                    keys: told.capabilities.key.clone(),
                    axes: told.capabilities.abs.iter().map(|axis| axis.code).collect(),
                })
            })
            .collect()
    }

    fn open(&mut self, path: &str) -> Took {
        match ok(self.devices.sink.role_at(path)).is_some() {
            true => Took::Held,
            false => Took::Refused,
        }
    }

    fn ranges(&self, path: &str) -> Ranges {
        let told = match self.descriptor(path) {
            Some(told) => told,
            None => return Ranges::default(),
        };
        Ranges {
            stick: ok(told.axis(AbsoluteAxisCode::ABS_RX.0)).map_or(1, |axis| ok(axis.span())),
            trigger: ok(told.axis(AbsoluteAxisCode::ABS_Z.0))
                .map_or((0, 1), |axis| (axis.min, axis.max)),
        }
    }

    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Gone> {
        let role = match ok(self.devices.sink.role_at(path)).map(str::to_string) {
            Some(role) => role,
            None => return Err(Gone),
        };
        let arrived =
            self.devices.sink.devices.get_mut(&role).map(|device| ok(device.drain()));

        Ok(arrived.unwrap_or_default())
    }
}

#[derive(Debug, Default)]
pub struct Did {
    pub commands: Vec<Vec<String>>,
    pub written: Vec<Out>,
    pub told: Vec<console_onscreen::Said>,
    pub using: Vec<console_input_bindings::bound::Input>,
}

impl Did {
    pub fn names(&self) -> Vec<String> {
        self.commands
            .iter()
            .filter_map(|argv| argv.first())
            .map(|program| program.rsplit('/').next().unwrap_or(program).to_string())
            .collect()
    }

    pub fn dispatched(&self) -> Vec<String> {
        self.commands
            .iter()
            .filter(|argv| argv.first().is_some_and(|word| word.ends_with("hyprctl")))
            .filter(|argv| argv.get(1).is_some_and(|word| word == "dispatch"))
            .filter_map(|argv| argv.last().cloned())
            .collect()
    }

    pub fn of_kind(&self, kind: EventType, code: u16) -> Vec<i32> {
        self.written.iter().filter(|out| out.kind == kind && out.code == code).map(|out| out.value).collect()
    }

    pub fn total(&self, kind: EventType, code: u16) -> i32 {
        self.of_kind(kind, code).iter().sum()
    }
}

pub struct Daemon {
    turning: Turning,
    now: f64,
    pub did: Did,
}

impl Default for Daemon {
    fn default() -> Self {
        Daemon { turning: Turning::default(), now: 1000.0, did: Did::default() }
    }
}

pub type Script<'a> = BTreeMap<usize, Box<dyn FnMut(&mut Go) + 'a>>;

impl Daemon {
    pub fn stopped_for(&mut self, seconds: f64) -> &mut Self {
        self.now += seconds;
        self
    }

    pub fn run(&mut self, go: &mut Go, turns: usize) -> &mut Self {
        self.between(go, turns, &mut Script::new())
    }

    pub fn between(&mut self, go: &mut Go, turns: usize, script: &mut Script) -> &mut Self {
        for turn in 1..=turns {
            let mut plug = Plug { devices: &mut go.devices };
            let Ok(turned) = self.turning.turn(&mut plug, self.now);

            for what in turned {
                match what {
                    Doing::Run(argv) => self.did.commands.push(argv),
                    Doing::Frame(frame) => self.did.written.extend(frame),
                    Doing::Tell(said) => self.did.told.push(said),
                    Doing::Using(on) => self.did.using.push(on),
                }
            }
            let Ok(poll) = self.turning.poll();

            self.now += poll;
            if let Some(happens) = script.get_mut(&turn) {
                happens(go);
            }
        }
        self
    }
}
