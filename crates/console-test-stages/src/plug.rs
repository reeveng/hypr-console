//! A world of devices, offered to a daemon as somewhere they are plugged in.
//!
//! The daemons find their devices by asking evdev what is plugged in. That is
//! the right way round on the machine and the wrong way round in a check: it
//! needs /dev/uinput, root, and a kernel that will then deliver whatever comes
//! out to whatever has focus. So the same daemon is run against this, with
//! devices built from the same capture the emulator uses.

use console_input_event_devices::{AbsoluteAxisCode, InputEvent};
use console_input_controller::finding::DeviceInfo;
use console_input_controller::reading::Ranges;
use console_input_controller::turning::{Closed, Plugged, Took};
use console_input_gamepad::capture::Descriptor;
use console_input_gamepad::devices::Devices;
use console_input_gamepad::world::World;
use console_core_never::Never;

pub struct Plug<'a> {
    pub devices: &'a mut Devices<World>,
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

        match role.is_some() {
            true => Took::Acquired,
            false => Took::Denied,
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
            stick: stick.map_or(1, |axis| {
                let Ok(span) = axis.span();

                span
            }),
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
