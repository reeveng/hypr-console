//! The daemon, its world, and what it did.
//!
//! A world of devices that is not this machine's, and a clock that is not this
//! machine's either. Nothing inside the daemon is stood in for: what it
//! decides, it decides.

use std::collections::BTreeMap;

use console_input_event_devices::{AbsoluteAxisCode, EventType, InputEvent};
use console_input_controller::effect::{Effect, Output};
use console_input_controller::finding::DeviceInfo;
use console_input_controller::clock::Instant;
use console_input_controller::reading::{POLL, Ranges, Wake};
use console_input_controller::turning::{Closed, Plugged, Took, Turning};
use console_input_gamepad::capture::{Descriptor, captured};
use console_input_gamepad::devices::Devices;
use console_input_gamepad::go::{RecordingClock, LegionGo};
use console_input_gamepad::router::every_profile;
use console_input_gamepad::world::World;

pub type Go = LegionGo<World, RecordingClock>;

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
    LegionGo::new(every_profile(&root()).expect("the profiles"), devices, RecordingClock::default(), profile)
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
    fn every(&self) -> Vec<DeviceInfo> {
        ok(self.devices.sink.plugged())
            .into_iter()
            .filter_map(|path| {
                let descriptor = self.descriptor(&path)?;
                Some(DeviceInfo {
                    path,
                    name: descriptor.name.clone(),
                    phys: descriptor.phys.clone(),
                    vendor: descriptor.vendor,
                    product: descriptor.product,
                    keys: descriptor.capabilities.key.clone(),
                    axes: descriptor.capabilities.abs.iter().map(|axis| axis.code).collect(),
                })
            })
            .collect()
    }

    fn open(&mut self, path: &str) -> Took {
        match ok(self.devices.sink.role_at(path)).is_some() {
            true => Took::Acquired,
            false => Took::Denied,
        }
    }

    fn ranges(&self, path: &str) -> Ranges {
        let descriptor = match self.descriptor(path) {
            Some(descriptor) => descriptor,
            None => return Ranges::default(),
        };
        Ranges {
            stick: ok(descriptor.axis(AbsoluteAxisCode::ABS_RX.0)).map_or(1, |axis| ok(axis.span())),
            trigger: ok(descriptor.axis(AbsoluteAxisCode::ABS_Z.0))
                .map_or((0, 1), |axis| (axis.min, axis.max)),
        }
    }

    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Closed> {
        let role = match ok(self.devices.sink.role_at(path)).map(str::to_string) {
            Some(role) => role,
            None => return Err(Closed),
        };
        let arrived =
            self.devices.sink.devices.get_mut(&role).map(|device| ok(device.drain()));

        Ok(arrived.unwrap_or_default())
    }
}

#[derive(Debug, Default)]
pub struct Did {
    pub commands: Vec<Vec<String>>,
    pub written: Vec<Output>,
    pub told: Vec<console_onscreen::PadInput>,
    pub using: Vec<console_input_bindings::bound::Input>,
}

impl Did {
    pub fn names(&self) -> Vec<String> {
        self.commands
            .iter()
            .filter_map(|arguments| arguments.first())
            .map(|program| program.rsplit('/').next().unwrap_or(program).to_string())
            .collect()
    }

    pub fn dispatched(&self) -> Vec<String> {
        let Ok(dispatched) = console_compositor::dispatched(&self.commands);

        dispatched
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

pub type Script<'a> = BTreeMap<u32, Box<dyn FnMut(&mut Go) + 'a>>;

impl Daemon {
    pub fn asleep_for(&mut self, seconds: f64) -> &mut Self {
        self.now.since_boot += seconds;
        self.now.suspended += seconds;
        self
    }

    pub fn idle_for(&mut self, seconds: f64) -> &mut Self {
        self.now.since_boot += seconds;
        self
    }

    pub fn run(&mut self, go: &mut Go, turns: u32) -> &mut Self {
        self.between(go, turns, &mut Script::new())
    }

    pub fn between(&mut self, go: &mut Go, turns: u32, script: &mut Script) -> &mut Self {
        for turn in 1..=turns {
            let mut plug = Plug { devices: &mut go.devices };
            let Ok(turned) = self.turning.turn(&mut plug, self.now);

            for what in turned {
                match what {
                    Effect::Run(arguments) => self.did.commands.push(arguments),
                    Effect::Frame(frame) => self.did.written.extend(frame),
                    Effect::Tell(said) => self.did.told.push(said),
                    Effect::Using(on) => self.did.using.push(on),
                }
            }
            let Ok(wake) = self.turning.wake();

            self.now.since_boot += match wake {
                Wake::Within(seconds) => seconds.min(POLL),
                Wake::OnInput => POLL,
            };
            if let Some(happens) = script.get_mut(&turn) {
                happens(go);
            }
        }
        self
    }
}
