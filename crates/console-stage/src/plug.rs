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
use console_pad::capture::Descriptor;
use console_pad::devices::Devices;
use console_pad::world::{Device, World};

pub struct Plug<'a> {
    pub devices: &'a mut Devices<World>,
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

    fn open(&mut self, path: &str) -> Took {
        match self.devices.sink.role_at(path).is_some() {
            true => Took::Held,
            false => Took::Refused,
        }
    }

    fn ranges(&self, path: &str) -> Ranges {
        let Some(told) = self.descriptor(path) else { return Ranges::default() };

        Ranges {
            stick: told.axis(AbsoluteAxisCode::ABS_RX.0).map_or(1, |axis| axis.span()),
            trigger: told
                .axis(AbsoluteAxisCode::ABS_Z.0)
                .map_or((0, 1), |axis| (axis.min, axis.max)),
        }
    }

    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Gone> {
        let Some(role) = self.devices.sink.role_at(path).map(str::to_string) else {
            return Err(Gone);
        };

        Ok(self.devices.sink.devices.get_mut(&role).map(Device::drain).unwrap_or_default())
    }
}
