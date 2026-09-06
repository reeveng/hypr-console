//! A world of devices, offered to a daemon as somewhere they are plugged in.
//!
//! The daemons find their devices by asking evdev what is plugged in. That is
//! the right way round on the machine and the wrong way round in a check: it
//! needs /dev/uinput, root, and a kernel that will then deliver whatever comes
//! out to whatever has focus. So the same daemon is run against this, with
//! devices built from the same capture the emulator uses.

use evdev::{AbsoluteAxisCode, InputEvent};
use console_controller::finding::Says;
use console_controller::reading::Ranges;
use console_controller::turning::{Gone, Plugged, Took};
use console_gamepad::capture::Descriptor;
use console_gamepad::devices::Devices;
use console_gamepad::world::World;
use console_never::Never;

pub struct Plug<'a> {
    pub devices: &'a mut Devices<World>,
}

impl Plug<'_> {
    fn descriptor(&self, path: &str) -> Result<Option<&Descriptor>, Never> {
        let Ok(role) = self.devices.sink.role_at(path);

        let Some(role) = role else { return Ok(None) };

        Ok(self.devices.descriptors.get(role))
    }
}

impl Plugged for Plug<'_> {
    fn every(&self) -> Vec<Says> {
        let Ok(plugged) = self.devices.sink.plugged();

        plugged
            .into_iter()
            .filter_map(|path| {
                let Ok(found) = self.descriptor(&path);
                let told = found?;

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
        let Ok(role) = self.devices.sink.role_at(path);

        match role.is_some() {
            true => Took::Held,
            false => Took::Refused,
        }
    }

    fn ranges(&self, path: &str) -> Ranges {
        let Ok(found) = self.descriptor(path);

        let Some(told) = found else { return Ranges::default() };

        let Ok(stick) = told.axis(AbsoluteAxisCode::ABS_RX.0);
        let Ok(trigger) = told.axis(AbsoluteAxisCode::ABS_Z.0);

        Ranges {
            stick: stick.map_or(1, |axis| {
                let Ok(span) = axis.span();

                span
            }),
            trigger: trigger.map_or((0, 1), |axis| (axis.min, axis.max)),
        }
    }

    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Gone> {
        let Ok(at) = self.devices.sink.role_at(path);

        let Some(role) = at.map(str::to_string) else {
            return Err(Gone);
        };

        let arrived = self.devices.sink.devices.get_mut(&role).map(|device| {
            let Ok(drained) = device.drain();

            drained
        });

        Ok(arrived.unwrap_or_default())
    }
}
