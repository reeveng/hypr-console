//! What the machine talks to, as nmcli reports it.

/// One network, as it is worth drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    pub name: String,
    /// Whether this is the one we are on.
    pub here: bool,
    pub signal: i32,
    pub locked: bool,
}

/// Everything in range, strongest first, one row per name.
///
/// One name can be several radios in one house, and the panel is a list of
/// places to join rather than a list of aerials.
pub fn networks(said: &str) -> Vec<Network> {
    let mut seen: Vec<Network> = Vec::new();
    for line in said.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 4 {
            continue;
        }
        let name = parts[1];
        if name.is_empty() {
            continue;
        }
        let found = Network {
            name: name.to_string(),
            here: parts[0] == "yes",
            signal: parts[2].parse().unwrap_or(0),
            locked: !parts[3..].join(":").is_empty(),
        };
        match seen.iter_mut().find(|network| network.name == found.name) {
            Some(already) if already.signal < found.signal => *already = found,
            Some(_) => (),
            None => seen.push(found),
        }
    }
    seen.sort_by_key(|network| -network.signal);
    seen
}

/// The networks this machine already knows the way into.
pub fn saved(said: &str) -> Vec<String> {
    said.lines()
        .filter(|line| line.ends_with("802-11-wireless"))
        .filter_map(|line| line.split(':').next())
        .map(str::to_string)
        .collect()
}

/// Whether the radio is on at all.
pub fn on(said: &str) -> bool {
    said.trim() == "enabled"
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
        let found = networks(SAID);
        let home = found.iter().find(|network| network.name == "Home").expect("home");
        assert_eq!(home.signal, 71);
        assert!(home.here);
        assert_eq!(found.iter().filter(|network| network.name == "Home").count(), 1);
    }

    #[test]
    fn the_strongest_is_first() {
        assert_eq!(networks(SAID)[0].name, "Cafe");
    }

    /// A network with no name is an aerial that will not say who it is, and
    /// there is nothing to draw and nothing to join.
    #[test]
    fn a_network_with_no_name_is_not_a_row() {
        assert!(!networks(SAID).iter().any(|network| network.name.is_empty()));
    }

    /// The security column can hold more than one word, and a colon of its own.
    #[test]
    fn a_network_is_locked_if_it_says_anything_at_all_about_security() {
        let found = networks(SAID);
        assert!(!found.iter().find(|n| n.name == "Cafe").expect("cafe").locked);
        assert!(found.iter().find(|n| n.name == "Locked").expect("locked").locked);
    }

    #[test]
    fn the_ones_we_already_know_the_way_into() {
        let said = "Home:802-11-wireless\nWired:802-3-ethernet\nCafe:802-11-wireless";
        assert_eq!(saved(said), ["Home", "Cafe"]);
    }

    #[test]
    fn the_radio_is_off_unless_it_says_it_is_on() {
        assert!(on("enabled\n"));
        assert!(!on("disabled"));
        assert!(!on(""));
    }
}
