//! What each tab holds, as a function of what the machine said.
//!
//! Reading the machine is one thing and knowing what to draw from it is
//! another. Everything here is the second, so the shape of every tab can be
//! asked without a machine to ask.

use std::sync::Arc;

use console_defaults::{battery, engines};
use console_notices::reading::QUIET;
use console_panel::page::{Does, Level, NOW, Row, Showing, YET};
use console_voice::languages;

use crate::level::{CELLS, bar, volume};
use crate::warm::Warmth;
use crate::{bluetooth, sound, wifi};

/// A row that turns something on or off and stays where it is.
pub fn switch(says: &str, argv: &[&str]) -> Row {
    let argv: Vec<String> = argv.iter().map(|word| (*word).to_string()).collect();
    Row::new(says, "", Does::and_stay(move |showing| showing.later(argv.clone())))
}

/// The speakers, and then whatever is playing through them.
///
/// A row for the speakers and a row for each thing playing through them, so a
/// video can be turned down without turning down the game.
pub fn sound_rows(
    sinks: &[sound::Thing],
    playing: &[sound::Thing],
    default: &str,
    hush: impl Fn(i64, &'static str) -> Does,
    turn: impl Fn(i64, &'static str) -> Level,
) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(speakers) = sound::speakers(sinks, default) {
        rows.push(
            Row::new("Speakers", &volume(speakers.level(), speakers.mute), hush(speakers.index, "sink"))
                .levelled(turn(speakers.index, "sink")),
        );
    }
    for stream in playing {
        rows.push(
            Row::new(
                &stream.said(),
                &volume(stream.level(), stream.mute),
                hush(stream.index, "sink-input"),
            )
            .levelled(turn(stream.index, "sink-input")),
        );
    }
    if playing.is_empty() {
        rows.push(Row::nothing("Nothing else is playing"));
    }
    rows
}

/// How the machine runs, which is the screen as much as the profile.
///
/// Both readings are handed in as answers that may not be back yet. The three
/// profiles are the same three whatever the machine says, and each of the two
/// things it does say is a subprocess away, so the tab is drawn without them
/// and they arrive into a list already on the screen.
/// The switch that makes the screen warm, and says which way it is standing.
///
/// The words are what pressing it will do and the mark is what it is now,
/// which is the shape every switch on this panel has. A row that said "Warm
/// colours" with a tick beside it would be two readings of the same row and
/// somebody would have to guess which.
pub fn warmth(warm: Warmth) -> Row {
    let says = match warm {
        Warmth::Warm => "Ordinary colours",
        Warmth::Ordinary => "Warm colours",
    };
    let mut row = switch(says, &["/usr/local/bin/console-warm"]);
    row.aside = match warm {
        Warmth::Warm => "warm".to_string(),
        Warmth::Ordinary => String::new(),
    };
    row
}

/// What a threshold reads as beside its row.
///
/// The number and nothing else. Every other level on this panel is drawn as a
/// bar, and a bar here would be read as how full the battery is -- on the one
/// tab that also carries how full the battery is. These are three places on
/// the way down rather than three readings.
///
/// Nought is *never*, said in a word. A person who walked a level to its end
/// should not have to work out that a warning at nought per cent is a warning
/// that cannot arrive.
pub fn threshold(level: i32) -> String {
    match level {
        battery::NEVER => "never".to_string(),
        level => format!("{level}%"),
    }
}

/// Where the machine starts saying something about the battery, and where it
/// stops itself.
///
/// Three rows under a heading of their own, at the bottom of the tab, because
/// they are the only thing on it that is set once and then never touched
/// again. What each row says is what will happen, not what the number is: the
/// number is already beside it.
pub fn dwindling(levels: battery::Levels, guard: impl Fn(battery::Step) -> Level) -> Vec<Row> {
    let mut rows = vec![Row::naming("When it runs out", "")];
    rows.extend(battery::EVERY.into_iter().map(|step| {
        Row::said(step.says(), &threshold(levels.at(step))).levelled(guard(step))
    }));
    rows
}

pub fn battery_rows(
    brightness: Option<i32>,
    running: Option<&str>,
    dim: Level,
    warm: Warmth,
    levels: battery::Levels,
    guard: impl Fn(battery::Step) -> Level,
) -> Vec<Row> {
    let profile = |says: &str, name: &'static str| {
        let mark = match running {
            Some(running) if running == name => NOW,
            _ => "",
        };
        Row::new(says, mark, Does::run(&["powerprofilesctl", "set", name]))
    };
    let screen = match brightness {
        Some(level) => volume(level, false),
        None => YET.to_string(),
    };
    let mut rows = vec![
        // The screen is the largest thing on this tab in every sense: it is what
        // the battery is mostly spent on, and until now it was the one setting
        // with no way to it but a button held down with another button.
        Row::said("Screen", &screen).levelled(dim),
        // Under it, because it is the same screen and the same evening. What
        // it is standing at is said beside the row as well as by the words in
        // it, for the reason the notifications switch is: a machine wearing a
        // colour nobody asked for should say so where the asking happens.
        warmth(warm),
        profile("Balanced", "balanced"),
        profile("Battery life", "power-saver"),
        profile("Full speed", "performance"),
    ];
    rows.extend(dwindling(levels, guard));
    rows
}

/// How well a network is heard, drawn the way a volume is, so both read alike.
pub fn strength(signal: i32) -> String {
    bar(signal, false, CELLS / 2)
}

/// What the machine talks to, and the way in to each.
pub fn wifi_rows(
    on: bool,
    networks: Vec<wifi::Network>,
    known: &[String],
    join: impl Fn(wifi::Network, bool) -> Does,
) -> Vec<Row> {
    if !on {
        return vec![switch("Turn Wi-Fi on", &["nmcli", "radio", "wifi", "on"])];
    }
    let mut rows = vec![switch("Turn Wi-Fi off", &["nmcli", "radio", "wifi", "off"])];
    for network in networks {
        if network.here {
            rows.push(Row::said(&network.name, NOW));
            continue;
        }
        let (says, aside) = (network.name.clone(), strength(network.signal));
        let already = known.contains(&network.name);
        rows.push(Row::new(&says, &aside, join(network, already)));
    }
    rows
}

/// The same over the short road.
pub fn bluetooth_rows(
    on: bool,
    devices: Vec<(bluetooth::Device, bool)>,
) -> Vec<Row> {
    if !on {
        return vec![switch("Turn Bluetooth on", &["bluetoothctl", "power", "on"])];
    }
    let mut rows = vec![switch("Turn Bluetooth off", &["bluetoothctl", "power", "off"])];
    for (device, joined) in devices {
        let doing = match joined {
            true => "disconnect",
            false => "connect",
        };
        let aside = match joined {
            true => NOW,
            false => "",
        };
        let mut row = switch(&device.name, &["bluetoothctl", doing, &device.address]);
        row.aside = aside.to_string();
        rows.push(row);
    }
    rows.push(switch("Look for devices", &["bluetoothctl", "--timeout", "8", "scan", "on"]));
    rows
}

/// Which engine a question is asked of.
///
/// It was written into the menu once. A setting nobody can reach is a setting
/// somebody has to be asked to change, and there is nobody to ask on a machine
/// with one person on it.
///
/// A list under the tab rather than part of it, because which engine answers a
/// question and which program opens a link are two questions, and a tab that
/// asks both at once is one nobody can read down.
///
/// Choosing one is two things. The file is written here and at once, because
/// it is what the menu reads and the menu is the next thing she will open. The
/// browsers are told through console-engine, which is slow enough to be worth
/// doing out of the way: it is sudo, and three files under /etc.
///
/// Chosen, it goes back up the way it came. What was chosen is on the row it
/// came from, so the answer to "which one is it now" is where the question was
/// asked rather than a list down that has to be left by hand.
pub fn search_rows(engine: &str, back: Chosen) -> Vec<Row> {
    let leaving = Arc::clone(&back);
    let mut rows =
        vec![Row::back(CONFIGURATION, move |showing| leaving(showing)), Row::naming("Search with", "")];
    for offered in &engines::EVERY {
        let mark = match offered.key == engine {
            true => NOW,
            false => "",
        };
        let key = offered.key;
        let back = Arc::clone(&back);
        rows.push(Row::new(
            offered.says,
            mark,
            Does::and_stay(move |showing| {
                engines::choose(key);
                showing.later(telling(key));
                back(showing);
            }),
        ));
    }
    rows
}

/// What the name of the engine in use reads as on the row above the list.
pub fn engine_says(engine: &str) -> String {
    engines::one(engine).map(|found| found.says.to_string()).unwrap_or_default()
}

/// Which language the paddle on the back is listening for.
///
/// A list under the tab, the way the engine is, and for the same reason: it is
/// a question with one answer that is asked once and then left alone.
///
/// It was not asked at all until now. The hearing was told to work the
/// language out for itself, which it does well on a sentence and badly on the
/// one or two words this button is mostly pressed for -- there is not enough
/// of a word to guess from, and English is what it guesses. So a Dutch word
/// came back as an English one that sounds like it, which is a wrong answer
/// wearing the shape of a right one.
///
/// Nothing is told and nothing is run. The file is what `dictate` reads on the
/// press after this one, and there is nothing between the panel closing and
/// that press.
pub fn dictation_rows(language: &str, back: Chosen) -> Vec<Row> {
    let leaving = Arc::clone(&back);
    let mut rows =
        vec![Row::back(CONFIGURATION, move |showing| leaving(showing)), Row::naming("Listen for", "")];
    for offered in &languages::EVERY {
        let mark = match offered.key == language {
            true => NOW,
            false => "",
        };
        let key = offered.key;
        let back = Arc::clone(&back);
        rows.push(Row::new(
            offered.says,
            mark,
            Does::and_stay(move |showing| {
                languages::choose(key);
                back(showing);
            }),
        ));
    }
    rows
}

/// What the language being listened for reads as on the row above the list.
pub fn dictation_says(language: &str) -> String {
    languages::one(language).map(|found| found.says.to_string()).unwrap_or_default()
}

/// What tells the browsers, which is the one thing the panel does as root.
///
/// Told rather than asked: -n so a rule that has gone missing is a command
/// that fails at once, rather than a panel sitting on a password prompt that
/// nothing on this machine can answer.
pub fn telling(engine: &str) -> Vec<String> {
    ["sudo", "-n", "console-engine", engine].iter().map(|word| (*word).to_string()).collect()
}

/// How it stops.
///
/// Nothing that turns the machine off shares a page with anything you would
/// touch every day.
pub fn system_rows() -> Vec<Row> {
    vec![
        // Leaving for Steam is a button on the front of the machine, and a
        // button on the front of the machine is a thing a hand holding nothing
        // cannot press. This is that hand's way out.
        Row::new("Game Mode", "", Does::run(&["/usr/local/bin/game-mode"])),
        Row::naming("Power", ""),
        Row::new("Sleep", "", Does::run(&["systemctl", "suspend"])),
        Row::new("Restart", "", Does::run(&["systemctl", "reboot"])),
        Row::new("Shut down", "", Does::run(&["systemctl", "poweroff"])),
    ]
}

/// Whether notifications are drawn on the screen as they arrive.
///
/// The switch is toggled rather than set, so nothing here has to know which
/// way round it is: `-t` adds the mode if it is missing and takes it away if
/// it is not. What the mode means is mako's own configuration's to say, under
/// a criteria of the same name.
///
/// Which way round it stands is said beside the row as well as by the words in
/// it. A desktop that has been quietened and does not say so is a desktop that
/// appears to have stopped working, which is the fault `docs/notifications.md`
/// is about, arrived at from the other end.
pub fn notifications_rows(held_back: bool) -> Vec<Row> {
    let says = match held_back {
        true => "Show them on the screen as they arrive",
        false => "Keep them off the screen",
    };
    let mark = match held_back {
        true => "held back",
        false => "",
    };
    let mut row = switch(says, &["makoctl", "mode", "-t", QUIET]);
    row.aside = mark.to_string();
    vec![
        row,
        // Nothing is lost while they are held back, and the bell is where
        // they are, so the switch says where to go and does not pretend this
        // is the only way to see them.
        Row::said("The bell on the bar", "Everything that arrived, held back or not"),
    ]
}

/// The words on the tabs, in the order they are drawn.
///
/// The first four are the bar's own four, in the order the bar draws them.
/// Sound, Bluetooth, Wi-Fi and Battery are each an icon along the top of the
/// screen that opens this panel at the tab it stands for, and the bar is still
/// up there while the panel is being read. They stood in one order along the
/// top and another along the tabs, which is the kind of thing nobody notices
/// and everybody follows: a thumb that had just tapped the speaker went
/// looking for the battery on the far side of the tabs from where it is on the
/// bar.
///
/// Notifications is the fifth because the bell is the fifth icon, and it is
/// here at all because a preference is not a notification. It stood at the
/// bottom of the bell's own Waiting tab, where on a desktop with nothing
/// waiting -- which is the desktop nearly always -- it was the only thing that
/// could be pressed: the bell opened onto one grey line saying nothing was
/// waiting, and one switch about what would happen later.
///
/// The last three are nobody's icon. Wallpaper and Configuration are how it
/// looks and what it answers with, which are the two anybody changes once and
/// then leaves alone. System is how it stops, and nothing that turns the
/// machine off shares a page with anything you would touch every day.
pub const TABS: [&str; 8] = [
    "Sound",
    "Bluetooth",
    "Wi-Fi",
    "Battery",
    "Notifications",
    "Wallpaper",
    CONFIGURATION,
    "System",
];

/// The one tab named on its own, because the lists under it say where the way
/// back goes and a word written twice is a word that goes out of step.
pub const CONFIGURATION: &str = "Configuration";

/// What choosing something on a list under a tab comes to, once the choice has
/// been made: the way back up to the tab it was opened from.
pub type Chosen = Arc<dyn Fn(&dyn Showing) + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    fn nothing() -> Level {
        std::sync::Arc::new(|_| ())
    }

    fn silence(_: i64, _: &'static str) -> Does {
        Does::and_stay(|_| ())
    }

    /// A way back that goes nowhere, for a list read without a panel under it.
    fn nowhere() -> Chosen {
        Arc::new(|_: &dyn Showing| ())
    }

    fn turning(_: i64, _: &'static str) -> Level {
        nothing()
    }

    fn says(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|row| row.says.as_str()).collect()
    }

    /// Sound was on a panel and the screen was not: brightness lived on the
    /// d-pad held under L2 and nowhere else, which is two buttons at once for
    /// the setting a person changes when the room gets dark.
    #[test]
    fn the_two_things_that_are_held_at_a_level_are_on_a_panel() {
        let screen = battery_rows(Some(50), Some("balanced"), nothing(), Warmth::Ordinary, battery::Levels::default(), |_| nothing());
        assert!(screen[0].level.is_some(), "the screen is not a level");
        let sinks = sound::read(r#"[{"index": 1, "name": "a", "volume": {}}]"#);
        let speakers = sound_rows(&sinks, &[], "a", silence, turning);
        assert!(speakers[0].level.is_some(), "the speakers are not a level");
    }

    #[test]
    fn the_profile_in_use_is_the_one_marked() {
        let rows = battery_rows(Some(50), Some("performance"), nothing(), Warmth::Ordinary, battery::Levels::default(), |_| nothing());
        let marked: Vec<&str> =
            rows.iter().filter(|row| row.now()).map(|row| row.says.as_str()).collect();
        assert_eq!(marked, ["Full speed"]);
    }

    /// The three places the battery is watched at, each a level a thumb walks.
    /// Without that they are three numbers in a file nobody can reach, which
    /// is the state this desktop is here to be the opposite of.
    #[test]
    fn where_the_battery_is_watched_is_three_rows_that_can_be_moved() {
        let rows = dwindling(battery::Levels::default(), |_| nothing());
        assert_eq!(
            says(&rows),
            [
                "When it runs out",
                "Say it is getting low",
                "Say it is getting really low",
                "Stop before the battery does",
            ]
        );
        assert!(rows[1..].iter().all(|row| row.level.is_some()), "a threshold that cannot be moved");
        assert_eq!(rows[1].aside, "25%");
    }

    /// Nought is a word, because a level walked to its end has to say what it
    /// means there. "0%" would be a warning that arrives when the machine is
    /// already off.
    #[test]
    fn a_threshold_turned_off_says_so_in_a_word() {
        assert_eq!(threshold(battery::NEVER), "never");
        assert_eq!(threshold(5), "5%");
    }

    /// A tab that says nothing at all reads as a panel that is still loading.
    #[test]
    fn a_radio_that_is_off_still_has_a_row_to_turn_it_on() {
        assert_eq!(says(&wifi_rows(false, Vec::new(), &[], |_, _| silence(0, ""))), ["Turn Wi-Fi on"]);
        assert_eq!(says(&bluetooth_rows(false, Vec::new())), ["Turn Bluetooth on"]);
    }

    #[test]
    fn the_one_we_are_on_is_marked_rather_than_offered() {
        let networks = wifi::networks("yes:Home:71:WPA2\nno:Cafe:50:");
        let rows = wifi_rows(true, networks, &[], |_, _| silence(0, ""));
        let home = rows.iter().find(|row| row.says == "Home").expect("home");
        assert!(home.now());
        assert!(home.does.is_none(), "there is nothing to do about being where you are");
        let cafe = rows.iter().find(|row| row.says == "Cafe").expect("cafe");
        assert!(cafe.does.is_some());
    }

    #[test]
    fn a_joined_device_is_marked_and_offers_the_way_out_of_it() {
        let devices = bluetooth::devices("Device AA Pads\nDevice BB Speaker");
        let rows = bluetooth_rows(true, vec![(devices[0].clone(), true), (devices[1].clone(), false)]);
        assert!(rows.iter().find(|row| row.says == "Pads").expect("pads").now());
        assert!(!rows.iter().find(|row| row.says == "Speaker").expect("speaker").now());
    }

    #[test]
    fn nothing_playing_is_said_rather_than_left_blank() {
        assert_eq!(says(&sound_rows(&[], &[], "", silence, turning)), ["Nothing else is playing"]);
    }

    #[test]
    fn the_engine_in_use_is_the_one_marked() {
        let rows = search_rows("startpage", nowhere());
        let marked: Vec<&str> =
            rows.iter().filter(|row| row.now()).map(|row| row.says.as_str()).collect();
        assert_eq!(marked, ["Startpage"]);
    }

    #[test]
    fn the_browsers_are_told_without_stopping_to_ask_for_a_password() {
        assert_eq!(telling("startpage"), ["sudo", "-n", "console-engine", "startpage"]);
    }

    /// A heading that could be chosen would set something by being landed on.
    #[test]
    fn the_list_is_the_way_back_and_then_a_row_that_only_reads() {
        let rows = search_rows("duckduckgo", nowhere());
        assert!(rows[0].says.ends_with(CONFIGURATION), "{:?} is not the way back", rows[0].says);
        assert_eq!(rows[1].says, "Search with");
        assert!(rows[1].heading());
    }

    /// The tab says which engine is in use, so the list under it is somewhere
    /// to go rather than the only place the answer is written.
    #[test]
    fn the_engine_is_named_the_way_it_is_named_on_its_own_row() {
        assert_eq!(engine_says("startpage"), "Startpage");
        assert_eq!(engine_says("telepathy"), "");
    }

    #[test]
    fn the_language_being_listened_for_is_the_one_marked() {
        let rows = dictation_rows("nl", nowhere());
        let marked: Vec<&str> =
            rows.iter().filter(|row| row.now()).map(|row| row.says.as_str()).collect();
        assert_eq!(marked, ["Dutch"]);
    }

    /// The one that was taken out, which is the whole of taking it out: a
    /// language nobody can choose is a language this desktop is not for.
    #[test]
    fn chinese_is_not_on_the_list() {
        let rows = dictation_rows("auto", nowhere());
        assert!(!says(&rows).contains(&"Chinese"));
        assert_eq!(says(&rows)[2..], ["Whichever is spoken", "English", "Dutch", "Thai"]);
    }

    /// The list reads the way the search list does: the way back, then what it
    /// is about, then the answers.
    #[test]
    fn the_languages_are_a_list_under_the_tab_like_the_engines() {
        let rows = dictation_rows("auto", nowhere());
        assert!(rows[0].says.ends_with(CONFIGURATION), "{:?} is not the way back", rows[0].says);
        assert_eq!(rows[1].says, "Listen for");
        assert!(rows[1].heading());
    }

    #[test]
    fn the_language_is_named_the_way_it_is_named_on_its_own_row() {
        assert_eq!(dictation_says("th"), "Thai");
        assert_eq!(dictation_says("auto"), "Whichever is spoken");
        assert_eq!(dictation_says("zh"), "");
    }

    /// A tab nobody can reach from the bar is a tab, and a tab the bar asks for
    /// that does not exist opens the first one instead, which is a wrong place
    /// rather than an error.
    #[test]
    fn every_tab_is_named_once() {
        let mut named = TABS.to_vec();
        named.sort_unstable();
        named.dedup();
        assert_eq!(named.len(), TABS.len());
    }
}
