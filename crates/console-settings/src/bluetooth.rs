//! What the machine talks to over the short road.

use console_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub address: String,
    pub name: String,
}

pub fn devices(said: &str) -> Result<Vec<Device>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let said = words.next()?;
            let address = words.next()?;
            let name = line.splitn(3, ' ').nth(2)?;
            (said == "Device").then(|| Device {
                address: address.to_string(),
                name: name.to_string(),
            })
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Radio {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Joined {
    Yes,
    No,
}

pub fn on(said: &str) -> Result<Radio, Never> {
    match said.contains("Powered: yes") {
        true => Ok(Radio::On),
        false => Ok(Radio::Off),
    }
}

pub fn joined(said: &str) -> Result<Joined, Never> {
    match said.contains("Connected: yes") {
        true => Ok(Joined::Yes),
        false => Ok(Joined::No),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_device_is_an_address_and_whatever_it_calls_itself() {
        let said = "Device AA:BB:CC:DD:EE:FF Some Headphones\nDevice 11:22:33:44:55:66 Pad";
        assert_eq!(
            devices(said),
            Ok(vec![
                Device {
                    address: "AA:BB:CC:DD:EE:FF".to_string(),
                    name: "Some Headphones".to_string()
                },
                Device { address: "11:22:33:44:55:66".to_string(), name: "Pad".to_string() },
            ])
        );
    }

    #[test]
    fn a_line_that_is_not_a_device_is_not_a_row() {
        assert_eq!(devices("No default controller available"), Ok(vec![]));
        assert_eq!(devices(""), Ok(vec![]));
    }

    #[test]
    fn the_radio_and_the_road_are_both_read_off_what_was_said() {
        assert_eq!(on("Controller AA\n\tPowered: yes\n"), Ok(Radio::On));
        assert_eq!(on("Controller AA\n\tPowered: no\n"), Ok(Radio::Off));
        assert_eq!(joined("\tConnected: yes\n"), Ok(Joined::Yes));
        assert_eq!(joined("\tConnected: no\n"), Ok(Joined::No));
    }
}
