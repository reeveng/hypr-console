//! The four readings, each as the bar draws it.

use console_default_applications::battery::{Charge, Filling};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::whole_u32;
use console_panel::door::Up;
use console_panel::running::said;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Says {
    pub text: String,
    pub class: String,
}

impl Says {
    pub fn new(text: impl Into<String>, class: impl Into<String>) -> Result<Self, Never> {
        Ok(Says { text: text.into(), class: class.into() })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum What {
    Battery,
    Bluetooth,
    Network,
    Sound,
}

impl What {
    pub fn named(word: &str) -> Result<Option<Self>, Never> {
        Ok(match word {
            "battery" => Some(What::Battery),
            "bluetooth" => Some(What::Bluetooth),
            "network" => Some(What::Network),
            "sound" => Some(What::Sound),
            _ => None,
        })
    }

    pub fn tab(self) -> Result<&'static str, Never> {
        Ok(match self {
            What::Battery => "Battery",
            What::Bluetooth => "Bluetooth",
            What::Network => "Wi-Fi",
            What::Sound => "Sound",
        })
    }

    pub fn says(self) -> Result<Says, Never> {
        match self {
            What::Battery => {
                let said = console_default_applications::battery::charge()?;

                battery(&said)
            },
            What::Bluetooth => {
                let Ok(connected) = connections();
                let Ok(shown) = said(Program::Bluetoothctl, &["show"]);

                bluetooth(&shown, connected)
            },
            What::Network => {
                let asked = &["-t", "-f", "TYPE,STATE,CONNECTION", "device", "status"];
                let Ok(wifi) = wifi();
                let Ok(devices) = said(Program::Nmcli, asked);

                network(&devices, &wifi)
            },
            What::Sound => {
                let Ok(level) = said(Program::Wpctl, &["get-volume", "@DEFAULT_AUDIO_SINK@"]);

                sound(&level)
            },
        }
    }
}

pub fn line(says: &Says, open: Up) -> Result<String, Never> {
    let lit = match open {
        Up::OnScreen => Some("open"),
        Up::NotThere => None,
    };
    let worn: Vec<&str> = says.class.split_whitespace().chain(lit).collect();
    let class = match worn.is_empty() {
        true => String::new(),
        false => format!(r#","class":{}"#, serde_json::Value::from(worn)),
    };
    Ok(format!(r#"{{"text":{}{class}}}"#, serde_json::Value::String(says.text.clone())))
}

fn whole(percent: i32) -> Result<Option<u32>, Never> {
    let Ok(whole) = u32::try_from(percent) else { return Ok(None) };

    Ok(Some(whole))
}

fn number<T: std::str::FromStr>(said: &str) -> Result<Option<T>, Never> {
    let Ok(number) = said.trim().parse::<T>() else { return Ok(None) };

    Ok(Some(number))
}

pub fn battery(said: &str) -> Result<Says, Never> {
    let Ok(reading) = Charge::of(said);

    let told = reading.percent.and_then(|percent| {
        let Ok(told) = whole(percent);

        told
    });

    let Some(charge) = told else {
        let Ok(blank) = wide("");
        let Ok(nothing) = small(&blank);

        return Says::new(format!("\u{f008e} {nothing}"), "");
    };

    let filling = reading.filling;
    let Ok(level) = stepped(&LEVELS, charge);

    let icon = match filling {
        Filling::Yes => "\u{f0084}",
        Filling::No => level,
    };
    let class = match (filling, charge) {
        (Filling::Yes, _) => "charging",
        (_, 0..=10) => "critical",
        (_, 11..=25) => "warning",
        _ => "",
    };

    let Ok(percent) = wide(&format!("{charge}%"));
    let Ok(shown) = small(&percent);

    Says::new(format!("{icon} {shown}"), class)
}

fn wide(reading: &str) -> Result<String, Never> {
    let short = WIDEST.saturating_sub(reading.chars().count());
    Ok(format!("{reading}{}", FIGURE.repeat(short)))
}

const WIDEST: usize = 4;

const FIGURE: &str = "\u{2007}";

fn small(what: &str) -> Result<String, Never> {
    Ok(format!(r#"<span size="x-small">{what}</span>"#))
}

const LEVELS: [&str; 5] = ["\u{f007a}", "\u{f007c}", "\u{f007e}", "\u{f0080}", "\u{f0079}"];

fn stepped(icons: &[&'static str], percent: u32) -> Result<&'static str, Never> {
    let last = icons.len().saturating_sub(1);
    let at = match usize::try_from(percent.min(100)) {
        Ok(percent) => percent.saturating_mul(last).checked_div(100).unwrap_or(0),
        Err(_) => 0,
    };

    Ok(icons.get(at).copied().unwrap_or(""))
}

fn connections() -> Result<usize, Never> {
    let Ok(said) = said(Program::Bluetoothctl, &["devices", "Connected"]);

    Ok(said.lines().filter(|line| !line.is_empty()).count())
}

pub fn bluetooth(shown: &str, connected: usize) -> Result<Says, Never> {
    let powered = shown.lines().any(|line| line.trim() == "Powered: yes");

    match (powered, connected) {
        (false, _) => Says::new("\u{f00b2}", "off"),
        (true, 0) => Says::new("\u{f00af}", ""),
        (true, _) => Says::new("\u{f00b1}", "connected"),
    }
}

fn wifi() -> Result<String, Never> {
    said(Program::Nmcli, &["-t", "-f", "IN-USE,SIGNAL", "device", "wifi"])
}

pub fn network(devices: &str, wifi: &str) -> Result<Says, Never> {
    let connected = |kind: &str| {
        devices
            .lines()
            .filter_map(|line| {
                let mut fields = line.split(':');
                let type_ = fields.next()?;
                let state = fields.next()?;

                Some((type_, state))
            })
            .any(|(type_, state)| type_ == kind && state == "connected")
    };

    match connected("wifi") {
        true => {
            let strength = wifi
                .lines()
                .find(|line| line.starts_with('*'))
                .and_then(|line| line.split(':').nth(1))
                .and_then(|said| {
                    let Ok(told) = number::<u32>(said);

                    told
                })
                .unwrap_or(0);

            let Ok(bars) = stepped(&BARS, strength);

            return Says::new(bars, "wifi");
        }
        false => {}
    }

    match connected("ethernet") {
        true => Says::new("\u{f0200}", "wired"),
        false => Says::new("\u{f05aa}", "off"),
    }
}

const BARS: [&str; 4] =
    ["\u{f091f}", "\u{f0922}", "\u{f0925}", "\u{f0928}"];

const SILENT: &str = "\u{f075f}";

pub fn sound(said: &str) -> Result<Says, Never> {
    let told = said.split_whitespace().nth(1).and_then(|said| {
        let Ok(told) = number::<f64>(said);

        told
    });

    let Some(volume) = told else {
        return Says::new(SILENT, "");
    };

    match said.contains("[MUTED]") {
        true => return Says::new(SILENT, "muted"),
        false => {}
    }

    let Ok(percent) = whole_u32(volume * 100.0);

    match percent == 0 {
        true => return Says::new(SILENT, "muted"),
        false => {}
    }

    let icon = match percent {
        1..=33 => "\u{f057f}",
        34..=66 => "\u{f0580}",
        _ => "\u{f057e}",
    };

    Says::new(icon, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saying(text: &str, class: &str) -> Says {
        Says { text: text.to_string(), class: class.to_string() }
    }

    fn line(says: &Says, open: Up) -> String {
        let Ok(said) = super::line(says, open);

        said
    }

    fn battery(said: &str) -> Says {
        let Ok(says) = super::battery(said);

        says
    }

    fn bluetooth(shown: &str, connected: usize) -> Says {
        let Ok(says) = super::bluetooth(shown, connected);

        says
    }

    fn network(devices: &str, wifi: &str) -> Says {
        let Ok(says) = super::network(devices, wifi);

        says
    }

    fn sound(said: &str) -> Says {
        let Ok(says) = super::sound(said);

        says
    }

    fn held(said: &str) -> serde_json::Value {
        serde_json::from_str(said).expect("json")
    }

    fn worn(said: &str) -> Vec<String> {
        held(said)["class"]
            .as_array()
            .expect("a list of classes")
            .iter()
            .map(|name| name.as_str().expect("a name").to_string())
            .collect()
    }

    #[test]
    fn a_reading_with_nothing_to_say_about_itself_carries_no_class() {
        let said = line(&saying("64%", ""), Up::NotThere);
        assert!(held(&said).get("class").is_none());
    }

    #[test]
    fn the_tab_in_front_is_the_only_thing_that_lights_it() {
        assert_eq!(worn(&line(&saying("64%", ""), Up::OnScreen)), ["open"]);
    }

    #[test]
    fn a_reading_that_says_something_says_it_beside_being_open() {
        assert_eq!(worn(&line(&saying("muted", "muted"), Up::OnScreen)), ["muted", "open"]);
        assert_eq!(worn(&line(&saying("muted", "muted"), Up::NotThere)), ["muted"]);
    }

    #[test]
    fn every_class_is_one_name_and_never_a_line_of_words() {
        for says in [saying("x", ""), saying("x", "muted"), saying("x", "wifi")] {
            for open in [Up::OnScreen, Up::NotThere] {
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

    #[test]
    fn the_text_is_written_as_json_rather_than_pasted_in() {
        let said = line(&saying(r#"a "quoted" \ name"#, ""), Up::NotThere);
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

    fn drawn(says: &Says) -> String {
        let mut out = String::new();
        let mut inside = false;
        for letter in says.text.chars() {
            match (letter, inside) {
                ('<', _) => inside = true,
                ('>', _) => inside = false,
                (_, false) => out.push(letter),
                (_, true) => {}
            }
        }
        out
    }

    #[test]
    fn no_reading_is_a_different_width_for_saying_a_different_thing() {
        let one_width = |what: &str, said: Vec<Says>| {
            let widths: std::collections::BTreeSet<usize> =
                said.iter().map(|says| drawn(says).chars().count()).collect();
            assert_eq!(widths.len(), 1, "{what} is drawn {widths:?} wide: {:?}",
                said.iter().map(drawn).collect::<Vec<_>>());
        };

        let mut charges = vec![battery("")];
        for charge in 0..=100 {
            charges.push(battery(&format!("{charge} Discharging")));
            charges.push(battery(&format!("{charge} Charging")));
        }
        one_width("the battery", charges);

        let mut volumes = vec![sound(""), sound("Volume: 0.35 [MUTED]")];
        for step in 0..=100 {
            volumes.push(sound(&format!("Volume: {}.{:02}", step / 100, step % 100)));
        }
        one_width("the sound", volumes);

        let mut networks =
            vec![network("wifi:disconnected:", ""), network("ethernet:connected:wired", "")];
        for strength in 0..=100 {
            networks.push(network(DEVICES, &format!("*:{strength}")));
        }
        one_width("the network", networks);

        one_width("bluetooth", vec![
            bluetooth("Powered: no", 0),
            bluetooth("\tPowered: yes", 0),
            bluetooth("\tPowered: yes", 1),
            bluetooth("\tPowered: yes", 9),
        ]);
    }

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
        assert_eq!(bluetooth("\tPowered: yes", 2).text, bluetooth("\tPowered: yes", 9).text);
    }

    const DEVICES: &str = "wifi:connected:home\nethernet:unavailable:\nloopback:connected:lo";

    #[test]
    fn the_wireless_this_machine_is_on_is_the_one_with_the_star() {
        let says = network(DEVICES, "*:72\n :41\n :12");
        assert_eq!(says.class, "wifi");
        assert_eq!(says.text, BARS[2]);
    }

    #[test]
    fn every_strength_lands_on_a_bar_and_the_ends_are_not_the_same_bar() {
        for strength in 0..=100 {
            let says = network(DEVICES, &format!("*:{strength}"));
            assert!(BARS.contains(&says.text.as_str()), "{strength}: {:?}", says.text);
        }
        assert_ne!(network(DEVICES, "*:5").text, network(DEVICES, "*:95").text);
    }

    #[test]
    fn a_strength_that_cannot_be_read_is_the_faintest_bar() {
        assert_eq!(network(DEVICES, "*:").text, BARS[0]);
        assert_eq!(network(DEVICES, "").text, BARS[0]);
    }

    #[test]
    fn a_cable_is_not_a_wireless_and_neither_is_nothing() {
        assert_eq!(network("ethernet:connected:wired\nwifi:disconnected:", "").class, "wired");
        assert_eq!(network("wifi:disconnected:", "").class, "off");
    }

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
    fn a_volume_of_nothing_is_drawn_as_the_silence_it_is() {
        let nothing = sound("Volume: 0.00");
        assert_eq!(nothing.text, SILENT, "it says {:?}", nothing.text);
        assert_eq!(nothing.class, sound("Volume: 0.35 [MUTED]").class);
    }

    #[test]
    fn the_quietest_reading_that_is_not_silence_is_not_drawn_as_silence() {
        assert!(!sound("Volume: 0.01").text.starts_with(SILENT));
    }

    #[test]
    fn the_volume_is_read_as_a_share_and_drawn_as_one_of_three_marks() {
        let quiet = sound("Volume: 0.15").text;
        let middling = sound("Volume: 0.50").text;
        let loud = sound("Volume: 1.00").text;
        for said in [&quiet, &middling, &loud] {
            assert!(!said.contains('%'), "the number is back on the bar: {said:?}");
        }
        assert_ne!(quiet, middling);
        assert_ne!(middling, loud);
    }
}
