//! What the machine talks to over the short road.
//!
//! Three things are true of a device at once and they are not the same
//! question. Bonded is whether the two machines have exchanged keys, trusted is
//! whether this one will let it back in without being asked again, and
//! connected is whether it is here now. The tab read only the last of those for
//! a long time and offered `connect` on every row, which is a word bluez
//! refuses for anything it has not been introduced to -- so the one device that
//! most needs the tab, a keyboard nobody has paired yet, was the one device the
//! tab could not do anything with.
//!
//! A stranger is drawn by what it has said about itself, which for most of what
//! is in the air is nothing at all. bluez names a device that has not told it a
//! name after its own address, so a room full of watches and earbuds arrives as
//! a column of hex, and the thing somebody is actually holding is one line in
//! it. Two answers to that, and both come off the same reading: what said a
//! name goes above what did not, and what is loud goes above what is faint. A
//! mouse in the hand is the loudest thing in the room.

use console_core_never::Never;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Known {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    Says,
    Itself,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Looking {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Met {
    pub device: Device,
    pub known: Known,
    pub joined: Joined,
    pub heard: Option<i32>,
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

pub fn known(said: &str) -> Result<Known, Never> {
    match said.contains("Paired: yes") {
        true => Ok(Known::Yes),
        false => Ok(Known::No),
    }
}

pub fn looking(said: &str) -> Result<Looking, Never> {
    match said.contains("Discovering: yes") {
        true => Ok(Looking::Yes),
        false => Ok(Looking::No),
    }
}

pub fn named(device: &Device) -> Result<Named, Never> {
    match device.name == device.address.replace(':', "-") {
        true => Ok(Named::Itself),
        false => Ok(Named::Says),
    }
}

pub const FAINTEST: i32 = -100;

pub const LOUDEST: i32 = -50;

pub fn heard(said: &str) -> Result<Option<i32>, Never> {
    let found = said
        .lines()
        .filter_map(|line| line.trim().strip_prefix("RSSI:"))
        .filter_map(|said| said.split('(').nth(1))
        .filter_map(|said| said.split(')').next())
        .next();

    let word = match found {
        Some(word) => word.trim(),
        None => return Ok(None),
    };

    Ok(match word.parse() {
        Ok(heard) => Some(heard),
        Err(fault) => {
            eprintln!("console-settings: the radio said {word:?} for a strength: {fault}");

            None
        }
    })
}

pub fn share(heard: i32) -> Result<i32, Never> {
    let over = heard.saturating_sub(FAINTEST);
    let whole = LOUDEST.saturating_sub(FAINTEST);
    let share = over.saturating_mul(100).checked_div(whole).unwrap_or(0);

    Ok(share.clamp(0, 100))
}

fn place(met: &Met) -> Result<usize, Never> {
    let Ok(named) = named(&met.device);

    Ok(match met.joined {
        Joined::Yes => 0,
        Joined::No => match met.known {
            Known::Yes => 1,
            Known::No => match named {
                Named::Says => 2,
                Named::Itself => 3,
            },
        },
    })
}

pub fn in_order(met: Vec<Met>) -> Result<Vec<Met>, Never> {
    let mut met = met;

    met.sort_by_key(|one| {
        let Ok(place) = place(one);
        let loud = one.heard.unwrap_or(FAINTEST).saturating_neg();

        (place, loud, one.device.name.clone())
    });

    Ok(met)
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

    #[test]
    fn being_bonded_and_being_here_are_two_answers() {
        let keyboard = "\tPaired: yes\n\tBonded: yes\n\tTrusted: yes\n\tConnected: no\n";

        assert_eq!(known(keyboard), Ok(Known::Yes));
        assert_eq!(joined(keyboard), Ok(Joined::No));

        let stranger = "\tPaired: no\n\tTrusted: no\n\tConnected: no\n";

        assert_eq!(known(stranger), Ok(Known::No));
        assert_eq!(joined(stranger), Ok(Joined::No));
    }

    #[test]
    fn the_radio_says_when_it_is_looking() {
        assert_eq!(looking("\tDiscovering: yes\n"), Ok(Looking::Yes));
        assert_eq!(looking("\tDiscovering: no\n"), Ok(Looking::No));
        assert_eq!(looking(""), Ok(Looking::No));
    }

    #[test]
    fn a_device_named_after_its_own_address_has_said_nothing_about_itself() {
        let quiet = Device {
            address: "AA:BB:CC:DD:EE:FF".to_string(),
            name: "AA-BB-CC-DD-EE-FF".to_string(),
        };
        let says = Device {
            address: "AA:BB:CC:DD:EE:FF".to_string(),
            name: "Blue Keys".to_string(),
        };

        assert_eq!(named(&quiet), Ok(Named::Itself));
        assert_eq!(named(&says), Ok(Named::Says));
    }

    #[test]
    fn how_loud_a_device_is_is_read_off_the_word_bluez_writes_it_in() {
        let said = "\tName: Mouse\n\tRSSI: 0xffffffa9 (-87)\n\tTxPower: 0x0000 (0)\n";

        assert_eq!(heard(said), Ok(Some(-87)));
        assert_eq!(heard("\tName: Mouse\n"), Ok(None), "a device the radio has not heard");
        assert_eq!(heard("\tRSSI: nothing\n"), Ok(None));
    }

    #[test]
    fn the_ends_of_the_bar_are_the_ends_of_what_a_radio_hears() {
        assert_eq!(share(LOUDEST), Ok(100));
        assert_eq!(share(FAINTEST), Ok(0));
        assert_eq!(share(-30), Ok(100), "nearer than the loudest is still the whole bar");
        assert_eq!(share(-120), Ok(0));
        assert_eq!(share(-75), Ok(50));
    }

    #[test]
    fn what_is_here_goes_above_what_is_known_above_what_said_a_name_above_the_rest() {
        let one = |address: &str, name: &str, known, joined, heard| Met {
            device: Device { address: address.to_string(), name: name.to_string() },
            known,
            joined,
            heard,
        };
        let met = vec![
            one("11:11:11:11:11:11", "11-11-11-11-11-11", Known::No, Joined::No, Some(-40)),
            one("22:22:22:22:22:22", "Faint Mouse", Known::No, Joined::No, Some(-90)),
            one("33:33:33:33:33:33", "Near Mouse", Known::No, Joined::No, Some(-45)),
            one("44:44:44:44:44:44", "Known Pads", Known::Yes, Joined::No, None),
            one("55:55:55:55:55:55", "Speaker", Known::Yes, Joined::Yes, None),
        ];

        let Ok(in_order) = in_order(met);
        let names: Vec<&str> = in_order.iter().map(|met| met.device.name.as_str()).collect();

        assert_eq!(names, [
            "Speaker",
            "Known Pads",
            "Near Mouse",
            "Faint Mouse",
            "11-11-11-11-11-11"
        ]);
    }

    #[test]
    fn a_device_nothing_was_said_about_is_a_stranger_rather_than_a_friend() {
        assert_eq!(known(""), Ok(Known::No));
        assert_eq!(joined(""), Ok(Joined::No));
    }
}
