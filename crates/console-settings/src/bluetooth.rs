//! What the machine talks to over the short road.

/// One device bluetoothctl knows about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub address: String,
    pub name: String,
}

/// Everything it has been introduced to.
///
/// Only the lines that say Device. bluetoothctl answers a machine with no radio
/// in a sentence, and a sentence of the right length reads as a device with a
/// word for an address.
pub fn devices(said: &str) -> Vec<Device> {
    said.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let (said, address) = (words.next()?, words.next()?);
            let name = line.splitn(3, ' ').nth(2)?;
            (said == "Device").then(|| Device {
                address: address.to_string(),
                name: name.to_string(),
            })
        })
        .collect()
}

/// Whether the radio is on at all.
pub fn on(said: &str) -> bool {
    said.contains("Powered: yes")
}

/// Whether one of them is joined just now.
pub fn joined(said: &str) -> bool {
    said.contains("Connected: yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_device_is_an_address_and_whatever_it_calls_itself() {
        let said = "Device AA:BB:CC:DD:EE:FF Some Headphones\nDevice 11:22:33:44:55:66 Pad";
        assert_eq!(
            devices(said),
            [
                Device {
                    address: "AA:BB:CC:DD:EE:FF".to_string(),
                    name: "Some Headphones".to_string()
                },
                Device { address: "11:22:33:44:55:66".to_string(), name: "Pad".to_string() },
            ]
        );
    }

    /// bluetoothctl says "No default controller available" when there is no
    /// radio, which is a line with no address in it.
    #[test]
    fn a_line_that_is_not_a_device_is_not_a_row() {
        assert!(devices("No default controller available").is_empty());
        assert!(devices("").is_empty());
    }

    #[test]
    fn the_radio_and_the_road_are_both_read_off_what_was_said() {
        assert!(on("Controller AA\n\tPowered: yes\n"));
        assert!(!on("Controller AA\n\tPowered: no\n"));
        assert!(joined("\tConnected: yes\n"));
        assert!(!joined("\tConnected: no\n"));
    }
}
