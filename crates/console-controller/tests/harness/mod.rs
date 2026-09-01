//! The daemon, its world, and what it did.
//!
//! A world of devices that is not this machine's, and a clock that is not this
//! machine's either. Nothing inside the daemon is stood in for: what it
//! decides, it decides.

use std::collections::BTreeMap;

use evdev::{AbsoluteAxisCode, EventType, InputEvent};
use console_controller::doing::{Doing, Out};
use console_controller::finding::Says;
use console_controller::reading::Ranges;
use console_controller::turning::{Gone, Plugged, Turning};
use console_pad::capture::{Descriptor, captured};
use console_pad::devices::Devices;
use console_pad::go::{Held, LegionGo};
use console_pad::router::every_profile;
use console_pad::world::World;

/// The front of a Legion Go, and the four devices behind it.
pub type Go = LegionGo<World, Held>;

/// The repository, which is where the profiles are.
pub fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository")
}

/// A machine holding a profile, and nothing of this one taking part.
pub fn go(profile: &str) -> Go {
    let devices = Devices::new(captured(), World::of(captured()));
    LegionGo::new(every_profile(&root()).expect("the profiles"), devices, Held::default(), profile)
        .expect("a pad")
}

/// That world, offered to the daemon as somewhere devices are plugged in.
struct Plug<'a> {
    devices: &'a mut Devices<World>,
}

impl Plug<'_> {
    fn descriptor(&self, path: &str) -> Option<&Descriptor> {
        self.devices.descriptors.get(self.devices.sink.role_at(path)?)
    }
}

impl Plugged for Plug<'_> {
    fn every(&self) -> Vec<Says> {
        self.devices
            .sink
            .plugged()
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

    fn open(&mut self, path: &str) -> bool {
        self.devices.sink.role_at(path).is_some()
    }

    fn ranges(&self, path: &str) -> Ranges {
        let Some(told) = self.descriptor(path) else { return Ranges::default() };
        Ranges {
            stick: told.axis(AbsoluteAxisCode::ABS_RX.0).map_or(1, |axis| axis.span()),
            trigger: told.axis(AbsoluteAxisCode::ABS_Z.0).map_or((0, 1), |axis| (axis.min, axis.max)),
        }
    }

    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Gone> {
        let Some(role) = self.devices.sink.role_at(path).map(str::to_string) else {
            return Err(Gone);
        };
        Ok(self.devices.sink.devices.get_mut(&role).map(console_pad::world::Device::drain).unwrap_or_default())
    }
}

/// Every command the daemon started, and everything it wrote.
#[derive(Debug, Default)]
pub struct Did {
    pub commands: Vec<Vec<String>>,
    pub written: Vec<Out>,
}

impl Did {
    /// Just the program of each, which is usually the whole question.
    pub fn names(&self) -> Vec<String> {
        self.commands
            .iter()
            .filter_map(|argv| argv.first())
            .map(|program| program.rsplit('/').next().unwrap_or(program).to_string())
            .collect()
    }

    /// What was asked of the compositor, as the argument it was given.
    pub fn dispatched(&self) -> Vec<String> {
        self.commands
            .iter()
            .filter(|argv| argv.first().is_some_and(|word| word.ends_with("hyprctl")))
            .filter(|argv| argv.get(1).is_some_and(|word| word == "dispatch"))
            .filter_map(|argv| argv.last().cloned())
            .collect()
    }

    /// Everything written of one kind and one code, in order.
    pub fn of_kind(&self, kind: EventType, code: u16) -> Vec<i32> {
        self.written.iter().filter(|out| out.kind == kind && out.code == code).map(|out| out.value).collect()
    }

    /// One axis or one button added up, which is how far a wheel turned.
    pub fn total(&self, kind: EventType, code: u16) -> i32 {
        self.of_kind(kind, code).iter().sum()
    }
}

/// The daemon, loaded but not yet running.
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

/// What happens partway through a run, by the turn it happens on.
///
/// Anything that has to happen while the daemon is running rather than before
/// it starts belongs here, because a daemon started twice is two daemons.
pub type Script<'a> = BTreeMap<usize, Box<dyn FnMut(&mut Go) + 'a>>;

impl Daemon {
    /// The daemon was stopped for a while, and the world went on without it.
    ///
    /// Which is what the on-screen keyboard does to it: the process is
    /// stopped outright, its devices stay open, and everything pressed
    /// meanwhile is waiting on them when it starts again.
    pub fn stopped_for(&mut self, seconds: f64) -> &mut Self {
        self.now += seconds;
        self
    }

    /// Turn the daemon's loop over, and stop it after so many turns.
    pub fn run(&mut self, go: &mut Go, turns: usize) -> &mut Self {
        self.between(go, turns, &mut Script::new())
    }

    /// The same, with something happening partway through.
    pub fn between(&mut self, go: &mut Go, turns: usize, script: &mut Script) -> &mut Self {
        for turn in 1..=turns {
            let mut plug = Plug { devices: &mut go.devices };
            for what in self.turning.turn(&mut plug, self.now) {
                match what {
                    Doing::Run(argv) => self.did.commands.push(argv),
                    Doing::Frame(frame) => self.did.written.extend(frame),
                }
            }
            self.now += self.turning.poll();
            if let Some(happens) = script.get_mut(&turn) {
                happens(go);
            }
        }
        self
    }
}
