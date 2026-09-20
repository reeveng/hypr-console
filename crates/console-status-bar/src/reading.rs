//! The four readings, each as the bar draws it.
//!
//! A reading says what it is and never what it should look like. It used to
//! carry a class -- `muted`, `off`, `critical`, `urgent` -- which is a name in a
//! stylesheet spelled again in Rust, and the stylesheet had nine of them for
//! five colours because every module had invented its own word for the same
//! thing. So what comes back now is a [`Tone`]: the resting colour, the quiet
//! one a reading with nothing to report wears, and the three a machine wears
//! when something is happening to it. What each of those is worth in ink is
//! decided once, where the bar is drawn.
//!
//! One of the six is not a reading at all. The two icons on the left of the bar
//! open a thing and close it again, and the stylesheet drew them a shade
//! brighter than everything beside them because a button that looks like a
//! reading is a button nobody presses. That is the same kind of statement as
//! `muted` -- what this is, said so that one place can decide what it is
//! worth -- so `Pressed` lives here with the rest of them and no reading ever
//! returns it.

use console_core_words::Words;
use console_default_applications::battery::{Charge, Filling};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::whole_u32;
use console_panel::door::Up;
use console_panel::running::said;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tone {
    Plain,
    Quiet,
    Pressed,
    Low,
    Wrong,
    Well,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Says {
    pub icon: String,
    pub beside: Option<String>,
    pub tone: Tone,
}

impl Says {
    pub fn new(icon: &str, tone: Tone) -> Result<Self, Never> {
        Ok(Says { icon: String::from(icon), beside: None, tone })
    }

    pub fn and(self, said: String) -> Result<Self, Never> {
        Ok(Says { beside: Some(said), ..self })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Words)]
pub enum What {
    #[words(tab = "Battery")]
    Battery,
    #[words(tab = "Bluetooth")]
    Bluetooth,
    #[words(tab = "Wi-Fi")]
    Network,
    #[words(tab = "Sound")]
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

                network(Asked { devices: &devices, wifi: &wifi })
            },
            What::Sound => {
                let Ok(level) = said(Program::Wpctl, &["get-volume", "@DEFAULT_AUDIO_SINK@"]);

                sound(&level)
            },
        }
    }
}

pub fn worn(tone: Tone) -> Result<Option<&'static str>, Never> {
    Ok(match tone {
        Tone::Plain | Tone::Pressed => None,
        Tone::Quiet => Some("quiet"),
        Tone::Low => Some("low"),
        Tone::Wrong => Some("wrong"),
        Tone::Well => Some("well"),
    })
}

pub fn line(says: &Says, open: Up) -> Result<String, Never> {
    let lit = match open {
        Up::OnScreen => Some("open"),
        Up::NotThere => None,
    };
    let Ok(state) = worn(says.tone);
    let worn: Vec<&str> = state.into_iter().chain(lit).collect();
    let class = match worn.is_empty() {
        true => String::new(),
        false => format!(r#","class":{}"#, serde_json::Value::from(worn)),
    };
    let text = match &says.beside {
        Some(beside) => {
            let Ok(small) = small(beside);

            format!("{} {small}", says.icon)
        }
        None => says.icon.clone(),
    };

    Ok(format!(r#"{{"text":{}{class}}}"#, serde_json::Value::String(text)))
}

fn whole(percent: i32) -> Result<Option<u32>, Never> {
    let whole = match u32::try_from(percent) {
        Ok(whole) => whole,
        Err(_fault) => return Ok(None),
    };

    Ok(Some(whole))
}

fn number<T: std::str::FromStr>(said: &str) -> Result<Option<T>, Never> {
    let number = match said.trim().parse::<T>() {
        Ok(number) => number,
        Err(_fault) => return Ok(None),
    };

    Ok(Some(number))
}

pub fn battery(said: &str) -> Result<Says, Never> {
    let Ok(reading) = Charge::of(said);

    let told = reading.percent.and_then(|percent| {
        let Ok(told) = whole(percent);

        told
    });

    let charge = match told {
        Some(charge) => charge,
        None => {
            let Ok(blank) = wide("");
            let Ok(says) = Says::new(NO_BATTERY, Tone::Plain);

            return says.and(blank);
        }
    };

    let filling = reading.filling;
    let Ok(level) = stepped(&LEVELS, charge);

    let icon = match filling {
        Filling::Yes => CHARGING,
        Filling::Held => PLUGGED,
        Filling::No => level,
    };
    let tone = match (filling, charge) {
        (Filling::Yes | Filling::Held, _) => Tone::Well,
        (_, 0..=10) => Tone::Wrong,
        (_, 11..=25) => Tone::Low,
        _ => Tone::Plain,
    };

    let Ok(percent) = wide(&format!("{charge}%"));
    let Ok(says) = Says::new(icon, tone);

    says.and(percent)
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

pub const CHARGING: &str = "\u{f0084}";

pub const PLUGGED: &str = "\u{f06a5}";

pub const NO_BATTERY: &str = "\u{f008e}";

const LEVELS: [&str; 5] = ["\u{f007a}", "\u{f007c}", "\u{f007e}", "\u{f0080}", "\u{f0079}"];

const A_WHOLE: std::num::NonZeroUsize = match std::num::NonZeroUsize::new(100) {
    Some(whole) => whole,
    None => std::num::NonZeroUsize::MIN,
};

const NO_ICON: &str = "";

const NOTHING_HEARD: u32 = 0;

fn stepped(icons: &[&'static str], percent: u32) -> Result<&'static str, Never> {
    let last = icons.len().saturating_sub(1);
    let at = match usize::try_from(percent.min(100)) {
        Ok(percent) => percent.saturating_mul(last) / A_WHOLE,
        Err(_a_percentage_does_not_reach_here) => 0,
    };

    Ok(match icons.get(at).copied() {
        Some(icon) => icon,
        None => NO_ICON,
    })
}

fn connections() -> Result<usize, Never> {
    let Ok(said) = said(Program::Bluetoothctl, &["devices", "Connected"]);

    Ok(said.lines().filter(|line| !line.is_empty()).count())
}

pub fn bluetooth(shown: &str, connected: usize) -> Result<Says, Never> {
    let powered = shown.lines().any(|line| line.trim() == "Powered: yes");

    match (powered, connected) {
        (false, _) => Says::new("\u{f00b2}", Tone::Quiet),
        (true, 0) => Says::new("\u{f00af}", Tone::Plain),
        (true, _) => Says::new("\u{f00b1}", Tone::Plain),
    }
}

fn wifi() -> Result<String, Never> {
    said(Program::Nmcli, &["-t", "-f", "IN-USE,SIGNAL", "device", "wifi"])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asked<'a> {
    pub devices: &'a str,
    pub wifi: &'a str,
}

pub fn network(asked: Asked<'_>) -> Result<Says, Never> {
    let Asked { devices, wifi } = asked;
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
                });

            let strength = match strength {
                Some(strength) => strength,
                None => NOTHING_HEARD,
            };

            let Ok(bars) = stepped(&BARS, strength);

            return Says::new(bars, Tone::Plain);
        }
        false => {}
    }

    match connected("ethernet") {
        true => Says::new("\u{f0200}", Tone::Plain),
        false => Says::new("\u{f05aa}", Tone::Quiet),
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

    let volume = match told {
        Some(volume) => volume,
        None => return Says::new(SILENT, Tone::Plain),
    };

    match said.contains("[MUTED]") {
        true => return Says::new(SILENT, Tone::Quiet),
        false => {}
    }

    let Ok(percent) = whole_u32(volume * 100.0);

    match percent == 0 {
        true => return Says::new(SILENT, Tone::Quiet),
        false => {}
    }

    let icon = match percent {
        1..=33 => "\u{f057f}",
        34..=66 => "\u{f0580}",
        _ => "\u{f057e}",
    };

    Says::new(icon, Tone::Plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saying(icon: &str, tone: Tone) -> Says {
        Says { icon: icon.to_string(), beside: None, tone }
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

    fn network(asked: Asked<'_>) -> Says {
        let Ok(says) = super::network(asked);

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
        let said = line(&saying("64%", Tone::Plain), Up::NotThere);
        assert!(held(&said).get("class").is_none());
    }

    #[test]
    fn the_tab_in_front_is_the_only_thing_that_lights_it() {
        assert_eq!(worn(&line(&saying("64%", Tone::Plain), Up::OnScreen)), ["open"]);
    }

    #[test]
    fn a_reading_that_says_something_says_it_beside_being_open() {
        assert_eq!(worn(&line(&saying("x", Tone::Quiet), Up::OnScreen)), ["quiet", "open"]);
        assert_eq!(worn(&line(&saying("x", Tone::Quiet), Up::NotThere)), ["quiet"]);
    }

    #[test]
    fn every_class_is_one_name_and_never_a_line_of_words() {
        for tone in [Tone::Plain, Tone::Quiet, Tone::Pressed, Tone::Low, Tone::Wrong, Tone::Well] {
            for open in [Up::OnScreen, Up::NotThere] {
                let said = line(&saying("x", tone), open);
                let list = match held(&said).get("class").cloned() {
                    Some(list) => list,
                    None => continue,
                };
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
        let said = line(&saying(r#"a "quoted" \ name"#, Tone::Plain), Up::NotThere);
        assert_eq!(held(&said)["text"], r#"a "quoted" \ name"#);
    }

    #[test]
    fn a_reading_with_something_beside_it_says_both_and_nothing_else_does() {
        let charge = battery("64 unplugged Discharging");
        let said = line(&charge, Up::NotThere);
        let text = held(&said)["text"].as_str().expect("a reading").to_string();

        assert!(text.starts_with(&charge.icon), "{text} does not start with its icon");
        assert!(text.contains("64%"), "{text} does not carry the charge");
        assert_eq!(sound("Volume: 0.35").beside, None);
    }

    #[test]
    fn a_battery_on_the_mains_says_so() {
        assert_eq!(battery("95 plugged Charging").tone, Tone::Well);
        assert_eq!(battery("95 unplugged Discharging").tone, Tone::Plain);
        assert_eq!(battery("8 unplugged Discharging").tone, Tone::Wrong);
        assert_eq!(battery("20 unplugged Discharging").tone, Tone::Low);
    }

    #[test]
    fn a_battery_held_at_a_limit_is_drawn_as_a_machine_on_the_cable() {
        let held = battery("78 plugged Not charging");

        assert_eq!(held.tone, Tone::Well, "a plugged-in device was drawn as one running flat");
        assert_eq!(held.icon, PLUGGED, "{} does not say the cable is in", held.icon);
        assert_ne!(
            held.icon, CHARGING,
            "a battery that is not filling was drawn as one that is"
        );
    }

    #[test]
    fn a_battery_nothing_answered_for_is_not_drawn_as_full() {
        let says = battery("");
        assert_eq!(says.icon, NO_BATTERY);
        assert!(!says.beside.unwrap_or_default().contains('%'));
    }

    fn drawn(says: &Says) -> usize {
        let beside = says.beside.clone().unwrap_or_default();

        says.icon.chars().count().saturating_add(beside.chars().count())
    }

    #[test]
    fn no_reading_is_a_different_width_for_saying_a_different_thing() {
        let one_width = |what: &str, said: Vec<Says>| {
            let widths: std::collections::BTreeSet<usize> = said.iter().map(drawn).collect();
            assert_eq!(widths.len(), 1, "{what} is drawn {widths:?} wide: {said:?}");
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
            vec![
                network(Asked { devices: "wifi:disconnected:", wifi: "" }),
                network(Asked { devices: "ethernet:connected:wired", wifi: "" }),
            ];
        for strength in 0..=100 {
            networks.push(network(Asked { devices: DEVICES, wifi: &format!("*:{strength}") }));
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
            assert!(LEVELS.contains(&says.icon.as_str()), "{charge}");
        }
    }

    #[test]
    fn bluetooth_that_is_off_is_not_bluetooth_with_nothing_on_it() {
        assert_eq!(bluetooth("Powered: no", 0).tone, Tone::Quiet);
        assert_eq!(bluetooth("\tPowered: yes", 0).tone, Tone::Plain);
        assert_ne!(bluetooth("\tPowered: yes", 0).icon, bluetooth("\tPowered: yes", 2).icon);
        assert_eq!(bluetooth("\tPowered: yes", 2).icon, bluetooth("\tPowered: yes", 9).icon);
    }

    const DEVICES: &str = "wifi:connected:home\nethernet:unavailable:\nloopback:connected:lo";

    #[test]
    fn the_wireless_this_machine_is_on_is_the_one_with_the_star() {
        let says = network(Asked { devices: DEVICES, wifi: "*:72\n :41\n :12" });
        assert_eq!(says.tone, Tone::Plain);
        assert_eq!(says.icon, BARS[2]);
    }

    #[test]
    fn every_strength_lands_on_a_bar_and_the_ends_are_not_the_same_bar() {
        for strength in 0..=100 {
            let says = network(Asked { devices: DEVICES, wifi: &format!("*:{strength}") });
            assert!(BARS.contains(&says.icon.as_str()), "{strength}: {:?}", says.icon);
        }
        assert_ne!(
            network(Asked { devices: DEVICES, wifi: "*:5" }).icon,
            network(Asked { devices: DEVICES, wifi: "*:95" }).icon
        );
    }

    #[test]
    fn a_strength_that_cannot_be_read_is_the_faintest_bar() {
        assert_eq!(network(Asked { devices: DEVICES, wifi: "*:" }).icon, BARS[0]);
        assert_eq!(network(Asked { devices: DEVICES, wifi: "" }).icon, BARS[0]);
    }

    #[test]
    fn a_cable_is_not_a_wireless_and_neither_is_nothing() {
        let both = "ethernet:connected:wired\nwifi:disconnected:";
        let wired = network(Asked { devices: both, wifi: "" });
        let nothing = network(Asked { devices: "wifi:disconnected:", wifi: "" });

        assert_eq!(wired.tone, Tone::Plain);
        assert_eq!(nothing.tone, Tone::Quiet);
        assert_ne!(wired.icon, nothing.icon);
    }

    #[test]
    fn the_loopback_is_not_a_network() {
        assert_eq!(network(Asked { devices: "loopback:connected:lo", wifi: "" }).tone, Tone::Quiet);
    }

    #[test]
    fn a_muted_sink_says_so_whatever_its_volume_is() {
        assert_eq!(sound("Volume: 0.35 [MUTED]").tone, Tone::Quiet);
        assert_eq!(sound("Volume: 0.35").tone, Tone::Plain);
    }

    #[test]
    fn a_volume_of_nothing_is_drawn_as_the_silence_it_is() {
        let nothing = sound("Volume: 0.00");
        assert_eq!(nothing.icon, SILENT, "it says {:?}", nothing.icon);
        assert_eq!(nothing.tone, sound("Volume: 0.35 [MUTED]").tone);
    }

    #[test]
    fn the_quietest_reading_that_is_not_silence_is_not_drawn_as_silence() {
        assert_ne!(sound("Volume: 0.01").icon, SILENT);
    }

    #[test]
    fn the_volume_is_read_as_a_share_and_drawn_as_one_of_three_marks() {
        let quiet = sound("Volume: 0.15").icon;
        let middling = sound("Volume: 0.50").icon;
        let loud = sound("Volume: 1.00").icon;
        for said in [&quiet, &middling, &loud] {
            assert!(!said.contains('%'), "the number is back on the bar: {said:?}");
        }
        assert_ne!(quiet, middling);
        assert_ne!(middling, loud);
    }
}
