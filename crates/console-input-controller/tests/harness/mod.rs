//! The daemon, its world, and what it did.
//!
//! A world of devices that is not this machine's, and a clock that is not this
//! machine's either. Nothing inside the daemon is stood in for: what it
//! decides, it decides.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_input_event_devices::{AbsoluteAxisCode, EventType, InputEvent};
use console_input_controller::effect::{Effect, Output};
use console_input_controller::finding::DeviceInfo;
use console_input_controller::clock::Instant;
use console_input_controller::reading::{POLL, Ranges, Wake};
use console_input_controller::turning::{Closed, Plugged, Took, Turning};
use console_input_gamepad::axis::Range;
use console_input_gamepad::capture::{Descriptor, load_capture};
use console_input_gamepad::devices::Devices;
use console_input_gamepad::go::LegionGo;
use console_waiting::clock::TestClock;
use console_input_gamepad::router::every_profile;
use console_input_gamepad::world::World;

pub type Go = LegionGo<World, TestClock>;

pub type Failure = Box<dyn Error>;

pub fn root() -> Result<PathBuf, Never> {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    match from.canonicalize() {
        Ok(found) => Ok(found),
        Err(_not_there) => Ok(from),
    }
}

pub fn go(profile: &str) -> Result<Go, Failure> {
    let seen = load_capture()?;
    let world = load_capture()?;
    let Ok(world) = World::of(world);
    let Ok(devices) = Devices::new(seen, world);
    let Ok(root) = root();
    let profiles = every_profile(&root)?;
    let go = LegionGo::new(profiles, devices, TestClock::default(), profile)?;

    Ok(go)
}

struct Plug<'a> {
    devices: &'a mut Devices<World>,
}

impl Plug<'_> {
    fn descriptor(&self, path: &str) -> Result<Option<&Descriptor>, Never> {
        let Ok(role) = self.devices.sink.role_at(path);

        let role = match role {
            Some(role) => role,
            None => return Ok(None),
        };

        Ok(self.devices.descriptors.get(role))
    }
}

impl Plugged for Plug<'_> {
    fn every(&self) -> Vec<DeviceInfo> {
        let Ok(plugged) = self.devices.sink.plugged();

        plugged
            .into_iter()
            .filter_map(|path| {
                let Ok(found) = self.descriptor(&path);
                let descriptor = found?;

                Some(DeviceInfo {
                    path,
                    name: descriptor.name.clone(),
                    phys: descriptor.phys.clone(),
                    vendor: descriptor.vendor,
                    product: descriptor.product,
                    keys: descriptor.capabilities.key.clone(),
                    axes: descriptor.capabilities.absolute.iter().map(|axis| axis.code).collect(),
                })
            })
            .collect()
    }

    fn open(&mut self, path: &str) -> Took {
        let Ok(role) = self.devices.sink.role_at(path);

        match role {
            Some(_) => Took::Acquired,
            None => Took::Denied,
        }
    }

    fn ranges(&self, path: &str) -> Ranges {
        let Ok(found) = self.descriptor(path);

        let descriptor = match found {
            Some(descriptor) => descriptor,
            None => return Ranges::default(),
        };

        let Ok(stick) = descriptor.axis(AbsoluteAxisCode::ABS_RX.0);
        let Ok(trigger) = descriptor.axis(AbsoluteAxisCode::ABS_Z.0);

        Ranges {
            thumbstick: stick.map_or(Range { low: -1, high: 1 }, |axis| Range { low: axis.minimum, high: axis.maximum }),
            trigger: trigger.map_or((0, 1), |axis| (axis.minimum, axis.maximum)),
        }
    }

    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Closed> {
        let Ok(at) = self.devices.sink.role_at(path);

        let role = match at.map(str::to_string) {
            Some(role) => role,
            None => return Err(Closed),
        };

        let arrived = self.devices.sink.devices.get_mut(&role).map(|device| {
            let Ok(drained) = device.drain();

            drained
        });

        Ok(match arrived {
            Some(arrived) => arrived,
            None => Vec::new(),
        })
    }
}

#[derive(Debug, Default)]
pub struct Did {
    pub commands: Vec<Vec<String>>,
    pub written: Vec<Output>,
    pub told: Vec<console_onscreen::PadInput>,
    pub using: Vec<console_input_bindings::bound::Input>,
    pub reconnected: Vec<console_input_controller::effect::Reconnected>,
}

impl Did {
    pub fn names(&self) -> Result<Vec<String>, Never> {
        Ok(self
            .commands
            .iter()
            .filter_map(|arguments| arguments.first())
            .map(|program| program.rsplit_once('/').map_or(program.as_str(), |(_, name)| name).to_string())
            .collect())
    }

    pub fn dispatched(&self) -> Result<Vec<String>, Never> {
        console_compositor::batch_arguments(&self.commands)
    }

    pub fn of_kind(&self, (kind, code): (EventType, u16)) -> Result<Vec<i32>, Never> {
        Ok(self.written.iter().filter(|out| out.kind == kind && out.code == code).map(|out| out.value).collect())
    }

    pub fn total(&self, of: (EventType, u16)) -> Result<i32, Never> {
        let Ok(every) = self.of_kind(of);

        Ok(every.iter().fold(0, |sum, value| sum.saturating_add(*value)))
    }
}

pub struct Daemon {
    turning: Turning,
    now: Instant,
    pub did: Did,
}

impl Default for Daemon {
    fn default() -> Self {
        Daemon {
            turning: Turning::default(),
            now: Instant { since_boot: 1000.0, suspended: 0.0 },
            did: Did::default(),
        }
    }
}

pub type Script<'a> = BTreeMap<u32, Box<dyn FnMut(&mut Go) -> Result<(), Failure> + 'a>>;

impl Daemon {
    pub fn asleep_for(&mut self, seconds: f64) -> Result<(), Never> {
        self.now.since_boot += seconds;
        self.now.suspended += seconds;

        Ok(())
    }

    pub fn idle_for(&mut self, seconds: f64) -> Result<(), Never> {
        self.now.since_boot += seconds;

        Ok(())
    }

    pub fn run(&mut self, go: &mut Go, turns: u32) -> Result<(), Never> {
        for _ in 0..turns {
            let Ok(()) = self.turn(go);
        }

        Ok(())
    }

    pub fn between(&mut self, go: &mut Go, turns: u32, script: &mut Script) -> Result<(), Failure> {
        for turn in 1..=turns {
            let Ok(()) = self.turn(go);

            match script.get_mut(&turn) {
                Some(happens) => {
                    happens(go)?;
                },
                None => {},
            }
        }

        Ok(())
    }

    fn turn(&mut self, go: &mut Go) -> Result<(), Never> {
        let mut plug = Plug { devices: &mut go.devices };
        let Ok(turned) = self.turning.turn(&mut plug, self.now);

        for what in turned {
            match what {
                Effect::Run(arguments) => self.did.commands.push(arguments),
                Effect::Frame(frame) => self.did.written.extend(frame),
                Effect::Tell(said) => self.did.told.push(said),
                Effect::Using(on) => self.did.using.push(on),
                Effect::Reconnected(back) => self.did.reconnected.push(back),
            }
        }

        let Ok(wake) = self.turning.wake();

        self.now.since_boot += match wake {
            Wake::Within(seconds) => seconds.min(POLL),
            Wake::OnInput => POLL,
        };

        Ok(())
    }
}
