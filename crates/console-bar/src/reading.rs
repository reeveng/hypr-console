//! The four readings, each as the bar draws it.

use console_panel::running::said;

/// One reading: what it says, and what it is called while it says it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Says {
    pub text: String,
    pub class: String,
}

impl Says {
    fn new(text: impl Into<String>, class: impl Into<String>) -> Self {
        Says { text: text.into(), class: class.into() }
    }
}

/// What each of them is, on the command line and as a tab of the settings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum What {
    Battery,
    Bluetooth,
    Network,
    Sound,
}

impl What {
    pub fn named(word: &str) -> Option<Self> {
        match word {
            "battery" => Some(What::Battery),
            "bluetooth" => Some(What::Bluetooth),
            "network" => Some(What::Network),
            "sound" => Some(What::Sound),
            _ => None,
        }
    }

    /// The tab of the settings panel this icon opens.
    pub fn tab(self) -> &'static str {
        match self {
            What::Battery => "Battery",
            What::Bluetooth => "Bluetooth",
            What::Network => "Wi-Fi",
            What::Sound => "Sound",
        }
    }

    /// What is asked, and how the answer is read.
    pub fn says(self) -> Says {
        match self {
            What::Battery => battery(&charge()),
            What::Bluetooth => bluetooth(&said(&["bluetoothctl", "show"]), connections()),
            What::Network => network(&said(&["nmcli", "-t", "-f", "TYPE,STATE,CONNECTION", "device", "status"]), &wifi()),
            What::Sound => sound(&said(&["wpctl", "get-volume", "@DEFAULT_AUDIO_SINK@"])),
        }
    }
}

// ------------------------------------------------------------------- battery

/// The charge and whether it is on the mains, out of the kernel.
fn charge() -> String {
    let Ok(supplies) = std::fs::read_dir("/sys/class/power_supply") else {
        return String::new();
    };
    supplies
        .flatten()
        .map(|supply| supply.path())
        .filter(|at| at.file_name().is_some_and(|name| name.to_string_lossy().starts_with("BAT")))
        .filter_map(|at| {
            let capacity = std::fs::read_to_string(at.join("capacity")).ok()?;
            let status = std::fs::read_to_string(at.join("status")).ok()?;
            Some(format!("{} {}", capacity.trim(), status.trim()))
        })
        .next()
        .unwrap_or_default()
}

/// How full it is, and whether it is filling.
pub fn battery(said: &str) -> Says {
    let mut words = said.split_whitespace();
    let Some(charge) = words.next().and_then(|word| word.parse::<u32>().ok()) else {
        return Says::new("\u{f008e}", "");
    };
    let filling = words.next().is_some_and(|word| word == "Charging" || word == "Full");
    let icon = match filling {
        true => "\u{f0084}",
        false => LEVELS[(charge as usize * (LEVELS.len() - 1)) / 100],
    };
    let class = match (filling, charge) {
        (true, _) => "charging",
        (_, 0..=10) => "critical",
        (_, 11..=25) => "warning",
        _ => "",
    };
    Says::new(format!("{icon} {charge}%"), class)
}

/// Empty to full, which is the ramp waybar drew before this.
const LEVELS: [&str; 5] = ["\u{f007a}", "\u{f007c}", "\u{f007e}", "\u{f0080}", "\u{f0079}"];

// ----------------------------------------------------------------- bluetooth

fn connections() -> usize {
    said(&["bluetoothctl", "devices", "Connected"]).lines().filter(|line| !line.is_empty()).count()
}

/// Off, on, or on with something connected to it.
pub fn bluetooth(shown: &str, connected: usize) -> Says {
    let powered = shown.lines().any(|line| line.trim() == "Powered: yes");
    match (powered, connected) {
        (false, _) => Says::new("\u{f00b2}", "off"),
        (true, 0) => Says::new("\u{f00af}", ""),
        (true, many) => Says::new(format!("\u{f00b1} {many}"), "connected"),
    }
}

// ------------------------------------------------------------------- network

/// The strength of the wireless this machine is on, where it is on one.
fn wifi() -> String {
    said(&["nmcli", "-t", "-f", "IN-USE,SIGNAL", "device", "wifi"])
}

/// Wireless with its strength, a cable, or nothing.
pub fn network(devices: &str, wifi: &str) -> Says {
    let connected = |kind: &str| {
        devices
            .lines()
            .filter_map(|line| {
                let mut fields = line.split(':');
                Some((fields.next()?, fields.next()?))
            })
            .any(|(type_, state)| type_ == kind && state == "connected")
    };
    if connected("wifi") {
        let strength = wifi
            .lines()
            .find(|line| line.starts_with('*'))
            .and_then(|line| line.split(':').nth(1))
            .unwrap_or("0");
        return Says::new(format!("\u{f05a9} {strength}%"), "wifi");
    }
    match connected("ethernet") {
        true => Says::new("\u{f0200}", "wired"),
        false => Says::new("\u{f05aa}", "off"),
    }
}

// --------------------------------------------------------------------- sound

/// How loud it is, or that it is not.
pub fn sound(said: &str) -> Says {
    let Some(volume) = said.split_whitespace().nth(1).and_then(|word| word.parse::<f64>().ok())
    else {
        return Says::new("\u{f075f}", "");
    };
    if said.contains("[MUTED]") {
        return Says::new("\u{f075f}", "muted");
    }
    let percent = (volume * 100.0).round() as u32;
    let icon = match percent {
        0..=33 => "\u{f057f}",
        34..=66 => "\u{f0580}",
        _ => "\u{f057e}",
    };
    Says::new(format!("{icon} {percent}%"), "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_battery_on_the_mains_says_so() {
        assert_eq!(battery("95 Charging").class, "charging");
        assert!(battery("95 Discharging").class.is_empty());
        assert!(battery("8 Discharging").class == "critical");
        assert!(battery("20 Discharging").class == "warning");
    }

    #[test]
    fn a_battery_nothing_answered_for_is_not_drawn_as_full() {
        let says = battery("");
        assert!(!says.text.contains('%'));
    }

    /// Every charge lands on the ramp, including the ends.
    #[test]
    fn the_ramp_holds_every_charge() {
        for charge in 0..=100 {
            let says = battery(&format!("{charge} Discharging"));
            assert!(LEVELS.iter().any(|level| says.text.starts_with(level)), "{charge}");
        }
    }

    #[test]
    fn bluetooth_that_is_off_is_not_bluetooth_with_nothing_on_it() {
        assert_eq!(bluetooth("Powered: no", 0).class, "off");
        assert!(bluetooth("\tPowered: yes", 0).class.is_empty());
        assert_eq!(bluetooth("\tPowered: yes", 2).class, "connected");
        assert!(bluetooth("\tPowered: yes", 2).text.ends_with('2'));
    }

    const DEVICES: &str = "wifi:connected:home\nethernet:unavailable:\nloopback:connected:lo";

    #[test]
    fn the_wireless_this_machine_is_on_is_the_one_with_the_star() {
        let says = network(DEVICES, "*:72\n :41\n :12");
        assert_eq!(says.class, "wifi");
        assert!(says.text.contains("72%"));
    }

    #[test]
    fn a_cable_is_not_a_wireless_and_neither_is_nothing() {
        assert_eq!(network("ethernet:connected:wired\nwifi:disconnected:", "").class, "wired");
        assert_eq!(network("wifi:disconnected:", "").class, "off");
    }

    /// A machine with the loopback up and nothing else is a machine with no
    /// network, which is what it used to be drawn as anyway.
    #[test]
    fn the_loopback_is_not_a_network() {
        assert_eq!(network("loopback:connected:lo", "").class, "off");
    }

    #[test]
    fn a_muted_sink_says_so_whatever_its_volume_is() {
        assert_eq!(sound("Volume: 0.35 [MUTED]").class, "muted");
        assert!(sound("Volume: 0.35").class.is_empty());
    }

    #[test]
    fn the_volume_is_read_as_a_share_and_drawn_as_a_percentage() {
        assert!(sound("Volume: 0.15").text.contains("15%"));
        assert!(sound("Volume: 1.00").text.contains("100%"));
    }
}
