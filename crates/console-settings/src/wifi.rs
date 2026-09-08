//! What the machine talks to, as nmcli reports it.

use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    pub name: String,
    pub here: bool,
    pub signal: i32,
    pub locked: bool,
}

fn signal_of(said: &str) -> Result<i32, Never> {
    let signal = match said.parse::<i32>() {
        Ok(signal) => signal,
        Err(_) => return Ok(0),
    };

    Ok(signal)
}

pub fn networks(said: &str) -> Result<Vec<Network>, Never> {
    let mut seen: Vec<Network> = Vec::new();

    for line in said.lines() {
        let parts: Vec<&str> = line.split(':').collect();

        match parts.len() < 4 {
            true => continue,
            false => {},
        }

        let (here, name, signal, locked) = match parts.as_slice() {
            [here, name, signal, locked @ ..] => (here, name, signal, locked),
            _ => continue,
        };

        match name.is_empty() {
            true => continue,
            false => {},
        }

        let signal = signal_of(signal)?;

        let found = Network {
            name: (*name).to_string(),
            here: *here == "yes",
            signal,
            locked: !locked.join(":").is_empty(),
        };

        match seen.iter_mut().find(|network| network.name == found.name) {
            Some(already) if already.signal < found.signal => *already = found,
            Some(_) => (),
            None => seen.push(found),
        }
    }

    seen.sort_by_key(|network| network.signal.saturating_neg());

    Ok(seen)
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAID: &str = "\
yes:Home:71:WPA2
no:Home:44:WPA2
no:Cafe:88:
no::60:WPA2
no:Locked:30:WPA1 WPA2";

    #[test]
    fn the_strongest_of_a_name_is_the_one_drawn() {
        let found = networks(SAID).expect("the networks");
        let home = found.iter().find(|network| network.name == "Home").expect("home");
        assert_eq!(home.signal, 71);
        assert!(home.here);
        assert_eq!(found.iter().filter(|network| network.name == "Home").count(), 1);
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
        assert!(!found.iter().find(|n| n.name == "Cafe").expect("cafe").locked);
        assert!(found.iter().find(|n| n.name == "Locked").expect("locked").locked);
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
}
