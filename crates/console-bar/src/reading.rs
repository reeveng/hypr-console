//! The four readings, each as the bar draws it.

use console_defaults::battery::Charge;
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
            What::Battery => battery(&console_defaults::battery::charge()),
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

/// How full it is, and whether it is filling.
///
/// The two files behind this are read by `console_defaults::battery` and not
/// here, because this is no longer the only thing that wants them. One reading
/// draws this icon and decides whether the machine has to say something about
/// the battery or stop itself, and two readers on two clocks would be two
/// opinions about one battery.
pub fn battery(said: &str) -> Says {
    let reading = Charge::of(said);
    let Some(charge) = reading.percent.and_then(|percent| u32::try_from(percent).ok()) else {
        // A machine with no battery, or one whose battery would not answer.
        // Drawn the width of every other battery all the same: a reading that
        // shrank when it had nothing to say would move the whole bar along at
        // exactly the moment something had gone wrong with it.
        return Says::new(format!("\u{f008e} {}", small(&wide(""))), "");
    };
    let filling = reading.filling;
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
    Says::new(format!("{icon} {}", small(&wide(&format!("{charge}%")))), class)
}

/// A reading, padded to the width of the widest reading there is.
///
/// 9%, 95% and 100% are three widths, and the bar packs the right side from
/// the right, so every module left of the battery slid along a character's
/// width each time it crossed 10 or 100. The pad is a figure space, which is
/// the blank a font draws exactly as wide as one of its digits, so four cells
/// is four cells at every charge.
///
/// Padded on the right rather than the left: the slack then falls at the
/// module's outer edge, inside padding nobody can see, and the icon and the
/// number themselves never move. Left-padding would have held the module
/// still and walked the ink about inside it, which is the same shift in a
/// smaller box.
fn wide(reading: &str) -> String {
    let short = WIDEST.saturating_sub(reading.chars().count());
    format!("{reading}{}", FIGURE.repeat(short))
}

/// `100%`, and there is no charge wider than that.
const WIDEST: usize = 4;

/// U+2007 FIGURE SPACE, a blank the width of a digit.
const FIGURE: &str = "\u{2007}";

/// Drawn at the size of the words rather than the size of the icons.
///
/// The icons on this bar are set at 22px so the Mono cut's single-cell glyphs
/// read at arm's length, and a number at 22px is a second shouting element
/// beside them. Pango's relative sizes step down by 1.2 each, so `x-small`
/// off 22 is 15.3px, which is the size the clock and every other word on this
/// bar is set at. Relative rather than a figure, so it follows the icon size
/// if that is ever changed rather than quietly disagreeing with it.
///
/// waybar hands a custom module's text to GTK as markup unless `escape` is
/// set, which it is not, so this is drawn and not printed.
fn small(what: &str) -> String {
    format!(r#"<span size="x-small">{what}</span>"#)
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
        (true, _) => Says::new("\u{f00b1}", "connected"),
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
            .and_then(|said| said.trim().parse::<u32>().ok())
            .unwrap_or(0);
        return Says::new(BARS[(strength.min(100) as usize * (BARS.len() - 1)) / 100], "wifi");
    }
    match connected("ethernet") {
        true => Says::new("\u{f0200}", "wired"),
        false => Says::new("\u{f05aa}", "off"),
    }
}

/// Faint to full, which is where the strength went when the number left it.
///
/// The same trick the battery has always used: a reading whose number is a
/// picture takes one glyph of bar instead of five, and a bar of six evenly
/// sized marks reads as one row of readings rather than as three icons with
/// numbers stuck to them and three without.
const BARS: [&str; 4] =
    ["\u{f091f}", "\u{f0922}", "\u{f0925}", "\u{f0928}"];

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
    if percent == 0 {
        return Says::new(SILENT, "muted");
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

    /// What a reading is drawn as, with the markup taken back off, which is
    /// what the eye is given and so what a width has to be counted over.
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

    /// Nothing on this bar may change width when what it says changes.
    ///
    /// The right side is packed from the right, so a reading that grows a
    /// character pushes every module left of it along -- the clock moves
    /// because the battery crossed 10, the workspaces move because it crossed
    /// 100. Which is the one thing the eye catches on a bar it is not reading.
    ///
    /// Counted in characters, which stands in for width because every icon
    /// here is drawn from the Mono cut of the Nerd Font, where each glyph is
    /// one cell of the same advance, and the only letters are digits and a
    /// per-cent sign in a font whose digits are one width. The padding is a
    /// figure space, which is a digit's width of nothing.
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
        // The glyph says connected and the class colours it. How many were
        // connected was a number nobody acted on, and it made this one
        // reading wider than the five beside it.
        assert_eq!(bluetooth("\tPowered: yes", 2).text, bluetooth("\tPowered: yes", 9).text);
    }

    const DEVICES: &str = "wifi:connected:home\nethernet:unavailable:\nloopback:connected:lo";

    #[test]
    fn the_wireless_this_machine_is_on_is_the_one_with_the_star() {
        let says = network(DEVICES, "*:72\n :41\n :12");
        assert_eq!(says.class, "wifi");
        // 72 of 100 lands on the third of four bars, which is where the
        // strength lives now that it is not written out beside the glyph.
        assert_eq!(says.text, BARS[2]);
    }

    /// The strength is still readable, because the glyph moves with it. A
    /// picture of the same size at every strength is a reading that says
    /// nothing, and that is what dropping the number would have made this.
    #[test]
    fn every_strength_lands_on_a_bar_and_the_ends_are_not_the_same_bar() {
        for strength in 0..=100 {
            let says = network(DEVICES, &format!("*:{strength}"));
            assert!(BARS.contains(&says.text.as_str()), "{strength}: {:?}", says.text);
        }
        assert_ne!(network(DEVICES, "*:5").text, network(DEVICES, "*:95").text);
    }

    /// Nothing readable where the strength should be is the faintest bar and
    /// not a panic and not an empty module.
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
        assert_eq!(nothing.text, SILENT, "it says {:?}", nothing.text);
        assert_eq!(nothing.class, sound("Volume: 0.35 [MUTED]").class);
    }

    /// The quietest speaker glyph is for a volume there is some of.
    #[test]
    fn the_quietest_reading_that_is_not_silence_is_not_drawn_as_silence() {
        assert!(!sound("Volume: 0.01").text.starts_with(SILENT));
    }

    /// Quiet, middling and loud, which is the whole of what the icon was for.
    /// The figure is said by the rocker that changes it, at the moment it
    /// changes, where somebody pressing it is already looking.
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
