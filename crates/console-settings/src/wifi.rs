//! What the machine talks to, as nmcli reports it.
//!
//! **The machine moves between the Wi-Fi networks it knows by how they do.**
//! NetworkManager picks between saved networks by priority and then by which
//! was used last, never by signal, so a handheld carried to the far end of the
//! house stayed on the five gigahertz network it joined in the kitchen and
//! received at the slowest rate there is, with the two-and-a-half gigahertz
//! one it also knows beside it. iwd ranks by signal and band and would have
//! answered part of this, and replacing the daemon everything else on this
//! machine talks to was not worth it.
//!
//! A signal strength is how loud the router is and not how fast it is heard:
//! five gigahertz at -70 dBm was receiving at MCS 0 while two and a half at
//! -71 received at MCS 7. So a network is left for being [`Receiving::Slowest`],
//! which the driver says, as well as for being weak, and what it is left for is
//! a saved one clearly stronger, a two-and-a-half gigahertz one as strong, since
//! that band holds a rate further from the router, or -- for a link at its
//! slowest -- any saved one that is not weak itself, since almost anything is
//! faster than the bottom rate. The other way is
//! nearness: on two and a half, a saved five gigahertz network that is strong
//! is where the machine is close enough for the faster band to win.
//!
//! Those two would chase each other where five gigahertz is loud and slow, so
//! a network left for being slow is only gone back to once it is heard
//! [`CLOSER`] than it was when it was left. Walking towards the router gets
//! there; standing still does not swap at every scan. The margin between two
//! networks at the same strength is the same argument, one step earlier.

use std::collections::{BTreeMap, BTreeSet};
use std::collections::btree_map::Entry;

use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    dylint_lib = "explicit048_no_unreal_state",
    allow(
        explicit048_no_unreal_state,
        reason = "`here` is whether this is the network the machine is on and `locked` is whether it wants a password; every combination of the two is a network someone can see in the list"
    )
)]
pub struct Network {
    pub name: String,
    pub here: bool,
    pub signal: i32,
    pub band: Band,
    pub locked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    TwoPointFourGigahertz,
    FiveGigahertz,
    Unknown,
}

fn band_of(said: &str) -> Result<Band, Never> {
    let megahertz = said.split_whitespace().next().map(str::parse::<u32>);

    Ok(match megahertz {
        Some(Ok(megahertz)) => match megahertz < 4000 {
            true => Band::TwoPointFourGigahertz,
            false => Band::FiveGigahertz,
        },
        None => Band::Unknown,
        Some(Err(_not_a_number)) => Band::Unknown,
    })
}

fn signal_of(said: &str) -> Result<i32, Never> {
    let signal = match said.parse::<i32>() {
        Ok(signal) => signal,
        Err(_not_a_number) => return Ok(0),
    };

    Ok(signal)
}

pub fn networks(said: &str) -> Result<Vec<Network>, Never> {
    let mut strongest: BTreeMap<String, Network> = BTreeMap::new();

    for line in said.lines() {
        let parts: Vec<&str> = line.split(':').collect();

        let (here, name, signal, frequency, locked) = match parts.as_slice() {
            [here, name, signal, frequency, locked @ ..] => (here, name, signal, frequency, locked),
            _ => continue,
        };

        match name.is_empty() {
            true => continue,
            false => {},
        }

        let signal = signal_of(signal)?;
        let band = band_of(frequency)?;

        let found = Network {
            name: (*name).to_string(),
            here: *here == "yes",
            signal,
            band,
            locked: !locked.join(":").is_empty(),
        };

        match strongest.entry(found.name.clone()) {
            Entry::Occupied(mut already) => match already.get().signal < found.signal {
                true => {
                    let here = already.get().here || found.here;
                    let _ = already.insert(Network { here, ..found });
                },
                false => {
                    let here = already.get().here || found.here;
                    already.get_mut().here = here;
                },
            },
            Entry::Vacant(nothing) => {
                let _ = nothing.insert(found);
            },
        }
    }

    let mut seen: Vec<Network> = strongest.into_values().collect();

    seen.sort_by_key(|network| network.signal.saturating_neg());

    Ok(seen)
}

pub const FIELDS: &str = "ACTIVE,SSID,SIGNAL,FREQ,SECURITY";

pub const IN_RANGE: [&str; 8] = ["-t", "-f", FIELDS, "device", "wifi", "list", "--rescan", "no"];

pub const KNOWN: [&str; 5] = ["-t", "-f", "NAME,TYPE", "connection", "show"];

pub const DEVICES: [&str; 4] = ["-t", "-f", "DEVICE,TYPE", "device"];

pub const WEAK: i32 = 60;

pub const NEAR: i32 = 75;

pub const CLEARLY: i32 = 20;

pub const CLOSER: i32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Receiving {
    Slowest,
    Faster,
    Unknown,
}

pub fn receiving(link: &str) -> Result<Receiving, Never> {
    let rate = link.lines().find_map(|line| line.trim().strip_prefix("rx bitrate:"));

    let words: Vec<&str> = match rate {
        Some(rate) => rate.split_whitespace().collect(),
        None => return Ok(Receiving::Unknown),
    };

    let index = words.windows(2).find_map(|pair| match pair {
        [named, index] => match named.ends_with("MCS") {
            true => Some(index.parse::<u32>()),
            false => None,
        },
        _ => None,
    });

    Ok(match index {
        Some(Ok(0 | 1)) => Receiving::Slowest,
        Some(Ok(_)) => Receiving::Faster,
        None => Receiving::Unknown,
        Some(Err(_not_a_number)) => Receiving::Unknown,
    })
}

pub fn wireless(devices: &str) -> Result<Option<String>, Never> {
    Ok(devices.lines().find_map(|line| line.strip_suffix(":wifi")).map(str::to_string))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    Weak,
    Slow,
    Near,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub from: Network,
    pub to: Network,
    pub why: Why,
}

pub type Left = BTreeMap<String, i32>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Distance {
    Closer,
    NoCloser,
}

fn distance(other: &Network, left: &Left) -> Result<Distance, Never> {
    Ok(match left.get(&other.name).map(|then| other.signal >= then.saturating_add(CLOSER)) {
        Some(true) | None => Distance::Closer,
        Some(false) => Distance::NoCloser,
    })
}

pub fn better(networks: &[Network], saved: &[String], link: Receiving, left: &Left) -> Result<Option<Move>, Never> {
    let here = match networks.iter().find(|network| network.here) {
        Some(here) => here,
        None => return Ok(None),
    };

    let saved: BTreeSet<&String> = saved.iter().collect();

    let others = networks
        .iter()
        .filter(|network| !network.here && saved.contains(&network.name))
        .filter(|network| distance(network, left) == Ok(Distance::Closer));

    let trouble = match (link, here.signal < WEAK) {
        (Receiving::Slowest, _) => Some(Why::Slow),
        (Receiving::Faster | Receiving::Unknown, true) => Some(Why::Weak),
        (Receiving::Faster | Receiving::Unknown, false) => None,
    };

    let chosen = match (trouble, here.band) {
        (Some(why), _) => others
            .filter(|other| {
                let clearly = other.signal >= here.signal.saturating_add(CLEARLY);
                let further_reaching = here.band == Band::FiveGigahertz
                    && other.band == Band::TwoPointFourGigahertz
                    && other.signal >= here.signal;
                let anything_faster = why == Why::Slow && other.signal >= WEAK;

                clearly || further_reaching || anything_faster
            })
            .max_by_key(|other| other.signal)
            .map(|other| Move { from: here.clone(), to: other.clone(), why }),
        (None, Band::TwoPointFourGigahertz) => others
            .filter(|other| other.band == Band::FiveGigahertz && other.signal >= NEAR)
            .max_by_key(|other| other.signal)
            .map(|other| Move { from: here.clone(), to: other.clone(), why: Why::Near }),
        (None, Band::FiveGigahertz | Band::Unknown) => None,
    };

    Ok(chosen)
}

pub fn saved(said: &str) -> Result<Vec<String>, Never> {
    Ok(said
        .lines()
        .filter(|line| line.ends_with("802-11-wireless"))
        .filter_map(|line| line.split(':').next())
        .map(str::to_string)
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Radio {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Known {
    Yes,
    No,
}

pub fn on(said: &str) -> Result<Radio, Never> {
    match said.trim() == "enabled" {
        true => Ok(Radio::On),
        false => Ok(Radio::Off),
    }
}

pub fn password(said: &str) -> Result<Option<String>, Never> {
    let line = match said.strip_suffix('\n') {
        Some(line) => line,
        None => said,
    };

    Ok(match line.is_empty() {
        true => None,
        false => Some(line.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAID: &str = "\
yes:Home:71:2437 MHz:WPA2
no:Home:44:2437 MHz:WPA2
no:Cafe:88:5180 MHz:
no::60:2437 MHz:WPA2
no:Locked:30:2437 MHz:WPA1 WPA2";

    #[test]
    fn the_strongest_of_a_name_is_the_one_drawn() {
        let found = networks(SAID).expect("the networks");
        let home = found.iter().find(|network| network.name == "Home").expect("home");
        assert_eq!(home.signal, 71);
        assert!(home.here);
        assert_eq!(found.iter().filter(|network| network.name == "Home").count(), 1);
    }

    #[test]
    fn the_network_in_use_is_still_in_use_when_a_stronger_access_point_of_it_is_in_range() {
        let found = networks("yes:Home:40:5180 MHz:WPA2\nno:Home:70:5180 MHz:WPA2").expect("the networks");

        assert_eq!(
            found,
            vec![Network {
                name: "Home".to_string(),
                here: true,
                signal: 70,
                band: Band::FiveGigahertz,
                locked: true,
            }]
        );
    }

    #[test]
    fn the_band_is_read_from_the_frequency() {
        let found = networks("no:Two:50:2437 MHz:WPA2\nno:Five:50:5180 MHz:WPA2\nno:What:50:?:WPA2")
            .expect("the networks");
        let bands: Vec<(&str, Band)> = found.iter().map(|network| (network.name.as_str(), network.band)).collect();

        assert_eq!(
            bands,
            vec![("Five", Band::FiveGigahertz), ("Two", Band::TwoPointFourGigahertz), ("What", Band::Unknown)]
        );
    }

    const FIVE: &str = "5180 MHz";
    const TWO: &str = "2437 MHz";

    fn range(here: (&str, i32), other: (&str, i32)) -> String {
        format!("yes:Here:{}:{}:WPA2\nno:Other:{}:{}:WPA2", here.1, here.0, other.1, other.0)
    }

    fn moving(said: &str, link: Receiving, left: &Left) -> Option<(String, Why)> {
        let found = networks(said).expect("the networks");
        let saved = ["Here".to_string(), "Other".to_string()];
        let Ok(better) = better(&found, &saved, link, left);

        better.map(|chosen| (chosen.to.name, chosen.why))
    }

    fn fresh(said: &str, link: Receiving) -> Option<(String, Why)> {
        moving(said, link, &Left::new())
    }

    #[test]
    fn a_weak_network_is_left_for_a_saved_one_clearly_stronger() {
        assert_eq!(fresh(&range((TWO, 30), (FIVE, 64)), Receiving::Faster), Some(("Other".to_string(), Why::Weak)));
    }

    #[test]
    fn two_networks_about_as_strong_on_one_band_are_not_swapped() {
        assert_eq!(fresh(&range((TWO, 49), (TWO, 60)), Receiving::Faster), None);
    }

    #[test]
    fn a_weak_five_gigahertz_network_is_left_for_a_two_and_a_half_one_as_strong() {
        assert_eq!(fresh(&range((FIVE, 50), (TWO, 50)), Receiving::Unknown), Some(("Other".to_string(), Why::Weak)));
    }

    #[test]
    fn a_link_at_its_slowest_is_left_however_strong_it_reads() {
        assert_eq!(fresh(&range((FIVE, 80), (TWO, 64)), Receiving::Slowest), Some(("Other".to_string(), Why::Slow)));
    }

    #[test]
    fn a_link_at_its_slowest_is_not_left_for_a_weak_one() {
        assert_eq!(fresh(&range((FIVE, 80), (TWO, 40)), Receiving::Slowest), None);
    }

    #[test]
    fn a_strong_fast_network_is_kept_however_strong_the_other_is() {
        assert_eq!(fresh(&range((FIVE, 70), (TWO, 100)), Receiving::Faster), None);
    }

    #[test]
    fn near_the_router_a_strong_five_gigahertz_network_is_joined() {
        assert_eq!(fresh(&range((TWO, 90), (FIVE, 80)), Receiving::Faster), Some(("Other".to_string(), Why::Near)));
        assert_eq!(fresh(&range((TWO, 90), (FIVE, 70)), Receiving::Faster), None);
    }

    #[test]
    fn a_network_left_for_being_slow_is_gone_back_to_only_once_it_is_heard_closer() {
        let left = Left::from([("Other".to_string(), 80)]);

        assert_eq!(moving(&range((TWO, 90), (FIVE, 85)), Receiving::Faster, &left), None);
        assert_eq!(
            moving(&range((TWO, 90), (FIVE, 90)), Receiving::Faster, &left),
            Some(("Other".to_string(), Why::Near))
        );
    }

    #[test]
    fn a_stronger_network_nobody_saved_is_not_joined() {
        assert_eq!(fresh("yes:Here:30:2437 MHz:WPA2\nno:Cafe:90:2437 MHz:", Receiving::Faster), None);
    }

    #[test]
    fn a_machine_on_no_network_is_left_to_networkmanager() {
        assert_eq!(fresh("no:Here:30:2437 MHz:WPA2\nno:Other:90:2437 MHz:WPA2", Receiving::Slowest), None);
    }

    #[test]
    fn the_driver_says_how_fast_a_frame_comes_in() {
        let link = |rate: &str| receiving(&format!("Connected to 68:02:b8:d0:19:fe (on wlan0)\n\tsignal: -71 dBm\n\trx bitrate: {rate}\n"));

        assert_eq!(link("72.2 MBit/s MCS 7 short GI"), Ok(Receiving::Faster));
        assert_eq!(link("6.5 MBit/s MCS 0"), Ok(Receiving::Slowest));
        assert_eq!(link("58.5 MBit/s VHT-MCS 1 80MHz VHT-NSS 1"), Ok(Receiving::Slowest));
        assert_eq!(link("286.7 MBit/s HE-MCS 11 HE-NSS 2"), Ok(Receiving::Faster));
        assert_eq!(link("6.0 MBit/s"), Ok(Receiving::Unknown));
        assert_eq!(receiving("Not connected.\n"), Ok(Receiving::Unknown));
    }

    #[test]
    fn the_wifi_device_is_the_one_nmcli_calls_wifi() {
        assert_eq!(wireless("wlan0:wifi\ntailscale0:tun\np2p-dev-wlan0:wifi-p2p\n"), Ok(Some("wlan0".to_string())));
        assert_eq!(wireless("lo:loopback\n"), Ok(None));
    }

    #[test]
    fn the_strongest_is_first() {
        let found = networks(SAID).expect("the networks");

        assert_eq!(found.first().expect("the strongest").name, "Cafe");
    }

    #[test]
    fn a_network_with_no_name_is_not_a_row() {
        let found = networks(SAID).expect("the networks");

        assert!(!found.iter().any(|network| network.name.is_empty()));
    }

    #[test]
    fn a_network_is_locked_if_it_says_anything_at_all_about_security() {
        let found = networks(SAID).expect("the networks");
        assert!(!found.iter().find(|network| network.name == "Cafe").expect("cafe").locked);
        assert!(found.iter().find(|network| network.name == "Locked").expect("locked").locked);
    }

    #[test]
    fn the_ones_we_already_know_the_way_into() {
        let said = "Home:802-11-wireless\nWired:802-3-ethernet\nCafe:802-11-wireless";
        assert_eq!(saved(said), Ok(vec!["Home".to_string(), "Cafe".to_string()]));
    }

    #[test]
    fn the_radio_is_off_unless_it_says_it_is_on() {
        assert_eq!(on("enabled\n"), Ok(Radio::On));
        assert_eq!(on("disabled"), Ok(Radio::Off));
        assert_eq!(on(""), Ok(Radio::Off));
    }

    #[test]
    fn a_password_is_kept_whole_and_only_the_line_ending_goes() {
        assert_eq!(password("  two words; and: a colon \n"), Ok(Some("  two words; and: a colon ".to_string())));
        assert_eq!(password("\n"), Ok(None));
        assert_eq!(password(""), Ok(None));
    }
}
