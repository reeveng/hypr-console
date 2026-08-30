//! The four readings, each as the bar draws it.

use console_panel::running::said;

/// One reading: what it says, and what it is called while it says it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Says {
    pub text: String,
    pub class: String,
}

impl Says {
    pub fn new(text: impl Into<String>, class: impl Into<String>) -> Self {
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

/// What the bar is told, as waybar reads it.
///
/// The classes go as a list and never as one string with a space in it. waybar
/// hands a string straight to GTK as a single class name, and a GTK class name
/// cannot hold a space, so `"wifi open"` arrives as one class called `wifi
/// open` and every selector in the stylesheet misses it -- including the two it
/// was meant to be.
///
/// What that looked like on the device is the wireless and the music never
/// lighting while their own panel was in front, and the speaker lighting only
/// while the sound was not muted. The three readings that always carry a word
/// of their own were the three that could never wear a second one, and the two
/// that usually carry nothing lit perfectly, which is why it read as three
/// broken icons rather than as one broken rule.
///
/// The class is left out rather than left empty, because waybar applies
/// whatever it is given and an empty name is a class nothing can be styled by.
///
/// `open` is whether the panel this icon opens is the one in front. It is
/// false for a reading that opens nothing, which is how the bell asks for a
/// line without having to know what a tab is.
pub fn line(says: &Says, open: bool) -> String {
    let worn: Vec<&str> = says.class.split_whitespace().chain(open.then_some("open")).collect();
    let class = match worn.is_empty() {
        true => String::new(),
        false => format!(r#","class":{}"#, serde_json::Value::from(worn)),
    };
    format!(r#"{{"text":{}{class}}}"#, serde_json::Value::String(says.text.clone()))
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

/// The mark for a machine you cannot hear.
///
/// Worn for silence however it was arrived at. Muting from the panel and
/// turning the volume down to nothing are two states to the machine -- one of
/// them comes back where it was, which is why the panel says "silent" against
/// a bar still showing the level it will come back to -- but they are one
/// state to somebody looking at the bar, because the question that icon
/// answers is whether anything is going to come out of the speakers. It wore
/// the quietest of the three speaker glyphs at nothing per cent and this glyph
/// when muted, so the same silence was two different pictures depending on
/// which way it had been reached.
const SILENT: &str = "\u{f075f}";

/// How loud it is, or that it is not.
pub fn sound(said: &str) -> Says {
    let Some(volume) = said.split_whitespace().nth(1).and_then(|word| word.parse::<f64>().ok())
    else {
        return Says::new(SILENT, "");
    };
    if said.contains("[MUTED]") {
        return Says::new(SILENT, "muted");
    }
    let percent = (volume * 100.0).round() as u32;
    // The number stays, because a volume of nothing is still a volume and
    // still steps: it is the one silence a scroll on the icon climbs out of.
    if percent == 0 {
        return Says::new(format!("{SILENT} 0%"), "muted");
    }
    let icon = match percent {
        1..=33 => "\u{f057f}",
        34..=66 => "\u{f0580}",
        _ => "\u{f057e}",
    };
    Says::new(format!("{icon} {percent}%"), "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saying(text: &str, class: &str) -> Says {
        Says { text: text.to_string(), class: class.to_string() }
    }

    fn held(said: &str) -> serde_json::Value {
        serde_json::from_str(said).expect("json")
    }

    /// The classes on a line, as waybar will read them.
    fn worn(said: &str) -> Vec<String> {
        held(said)["class"]
            .as_array()
            .expect("a list of classes")
            .iter()
            .map(|name| name.as_str().expect("a name").to_string())
            .collect()
    }

    /// The reading and the tab being in front are two things, and the icon
    /// says both at once.
    #[test]
    fn a_reading_with_nothing_to_say_about_itself_carries_no_class() {
        let said = line(&saying("64%", ""), false);
        assert!(held(&said).get("class").is_none());
    }

    #[test]
    fn the_tab_in_front_is_the_only_thing_that_lights_it() {
        assert_eq!(worn(&line(&saying("64%", ""), true)), ["open"]);
    }

    /// What the reading is doing and what the panel is doing are both classes,
    /// and the stylesheet expects them side by side.
    #[test]
    fn a_reading_that_says_something_says_it_beside_being_open() {
        assert_eq!(worn(&line(&saying("muted", "muted"), true)), ["muted", "open"]);
        assert_eq!(worn(&line(&saying("muted", "muted"), false)), ["muted"]);
    }

    /// The bug this file exists to have fixed once. waybar gives GTK whatever
    /// string it is handed as one class name, so a reading that wore two of
    /// them joined by a space wore neither: the speaker stopped lighting the
    /// moment the sound was muted, with the panel still in front of it.
    #[test]
    fn every_class_is_one_name_and_never_a_line_of_words() {
        for says in [saying("x", ""), saying("x", "muted"), saying("x", "wifi")] {
            for open in [true, false] {
                let said = line(&says, open);
                let Some(list) = held(&said).get("class").cloned() else { continue };
                assert!(list.is_array(), "{said} writes the classes as {list}");
                for name in worn(&said) {
                    assert!(!name.contains(char::is_whitespace), "{said} wears {name:?}");
                    assert!(!name.is_empty(), "{said} wears an empty class");
                }
            }
        }
    }

    /// Waybar reads the text as JSON, so a reading with a quote or a backslash
    /// in it has to survive being written down.
    #[test]
    fn the_text_is_written_as_json_rather_than_pasted_in() {
        let said = line(&saying(r#"a "quoted" \ name"#, ""), false);
        assert_eq!(held(&said)["text"], r#"a "quoted" \ name"#);
    }

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

    /// Turned down to nothing and muted are the same silence to look at, and
    /// the icon is what is being looked at.
    #[test]
    fn a_volume_of_nothing_is_drawn_as_the_silence_it_is() {
        let nothing = sound("Volume: 0.00");
        assert!(nothing.text.starts_with(SILENT), "it says {:?}", nothing.text);
        assert!(nothing.text.contains("0%"), "and it still says which silence");
        assert_eq!(nothing.class, sound("Volume: 0.35 [MUTED]").class);
    }

    /// The quietest speaker glyph is for a volume there is some of.
    #[test]
    fn the_quietest_reading_that_is_not_silence_is_not_drawn_as_silence() {
        assert!(!sound("Volume: 0.01").text.starts_with(SILENT));
    }

    #[test]
    fn the_volume_is_read_as_a_share_and_drawn_as_a_percentage() {
        assert!(sound("Volume: 0.15").text.contains("15%"));
        assert!(sound("Volume: 1.00").text.contains("100%"));
    }
}
