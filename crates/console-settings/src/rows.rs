//! What each tab holds, as a function of what the machine said.
//!
//! Reading the machine is one thing and knowing what to draw from it is
//! another. Everything here is the second, so the shape of every tab can be
//! asked without a machine to ask.

use std::sync::Arc;

use console_defaults::{battery, engines};
use console_home::shape::Shape;
use console_words::say;

use crate::words::Word;
use console_notices::reading::{QUIET, Quiet};
use console_panel::page::{Does, Level, NOW, Row, Showing, YET};
use console_voice::languages;

use crate::level::{CELLS, Muted, bar, volume};
use crate::size::{EVERY, Size};
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
        let muted = match speakers.mute {
            true => Muted::Yes,
            false => Muted::No,
        };
        rows.push(
            Row::new("Speakers", &volume(speakers.level(), muted), hush(speakers.index, "sink"))
                .levelled(turn(speakers.index, "sink")),
        );
    }

    for stream in playing {
        let muted = match stream.mute {
            true => Muted::Yes,
            false => Muted::No,
        };
        rows.push(
            Row::new(
                &stream.said(),
                &volume(stream.level(), muted),
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

/// The switch that lets the screen warm in the evening, and says which way it
/// is standing.
///
/// The words are what pressing it will do and the mark is what it is now,
/// which is the shape every switch on this panel has. A row that said "Night
/// colours" with a tick beside it would be two readings of the same row and
/// somebody would have to guess which.
pub fn warmth(warm: Warmth) -> Row {
    let says = match warm {
        Warmth::Following => Word::NightColoursOff,
        Warmth::Ordinary => Word::NightColoursOn,
    };
    let mut row = switch(&say(&says), &["/usr/local/bin/console-warm"]);
    row.aside = match warm {
        Warmth::Following => say(&Word::On),
        Warmth::Ordinary => say(&Word::Off),
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
        battery::NEVER => say(&Word::Never),
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
    let mut rows = vec![Row::naming(&say(&Word::WhenTheBatteryGetsLow), "")];
    rows.extend(battery::EVERY.into_iter().map(|step| {
        Row::said(&say(&said_of(step)), &threshold(levels.at(step))).levelled(guard(step))
    }));
    rows
}

/// What each of the three is called on the screen.
///
/// Here rather than on `battery::Step`, which is about where the levels sit and
/// is read by things that draw nothing. The crate that draws a thing owns its
/// words.
fn said_of(step: battery::Step) -> Word {
    match step {
        battery::Step::Low => Word::WarnMe,
        battery::Step::Lower => Word::WarnMeAgain,
        battery::Step::Protect => Word::TurnOffBeforeItDies,
    }
}

/// The screen: how bright it is, what colour it goes, and how big it draws.
///
/// It was the top of the Battery tab, under the brightness and the evening,
/// and the three speeds sat straight underneath with nothing between them. So
/// five rows read as one list of five settings of a kind, and they are two
/// subjects: what the screen is doing, and how hard the machine is allowed to
/// work. A tab is the cheapest line that can be drawn between two subjects, and
/// this is the one the bar does not open, so it costs nothing up there either.
///
/// The brightness is handed in as an answer that may not be back yet, and the
/// size as the rung the compositor says it is standing on -- which may be none
/// of them, on a machine somebody has set a density of their own.
/// The three rows that shape the home screen, under the ladder that shapes
/// everything.
///
/// Here rather than on a tab of its own, and here rather than beside the
/// applications, because it is the same question the ladder above it asks. How
/// big everything is is one number for the whole desktop; how big the squares
/// on the home screen are is worked out from that -- a square is a share of
/// the room the screen has, so turning the desktop's size up already moves it
/// -- and these are the three ways to disagree with what it worked out.
///
/// What each of them does is handed in. Which square is free, what the file
/// says, and telling the home screen it has changed are the panel's, and this
/// is the list.
///
/// Read out loud beside each row rather than only in it. A row saying
/// "Applications across" with two arrows on it and no number is a row somebody
/// presses to find out what it was.
pub fn home_rows(shape: Shape, across: Level, down: Level, sized: Level) -> Vec<Row> {
    vec![
        Row::naming(&say(&Word::TheHomeScreen), ""),
        Row::said(&say(&Word::ApplicationsAcross), &shape.columns.to_string()).levelled(across),
        Row::said(&say(&Word::ApplicationsDown), &shape.rows.to_string()).levelled(down),
        // The same five words the ladder above uses, because it is the same
        // ladder asked of a smaller thing: somebody who found Bigger up there
        // should not have to learn a second vocabulary down here.
        Row::said(&say(&Word::HowBigTheyAre), &say(&said_of_home_size(shape.size)))
            .levelled(sized),
    ]
}

/// What each rung of the home screen's ladder is called.
///
/// The screen's own words, mapped onto the home screen's own rungs. Two enums
/// and one vocabulary: the crate that draws a thing owns its words, and neither
/// `size` nor `console_home::shape` draws anything.
fn said_of_home_size(size: console_home::shape::Size) -> Word {
    match size {
        console_home::shape::Size::Tiny => Word::SizeTiny,
        console_home::shape::Size::Smaller => Word::SizeSmaller,
        console_home::shape::Size::Normal => Word::SizeNormal,
        console_home::shape::Size::Bigger => Word::SizeBigger,
        console_home::shape::Size::Huge => Word::SizeHuge,
    }
}

pub fn screen_rows(
    brightness: Option<i32>,
    dim: Level,
    warm: Warmth,
    standing: Option<Size>,
    home: Vec<Row>,
) -> Vec<Row> {
    let level = match brightness {
        Some(level) => volume(level, Muted::No),
        None => YET.to_string(),
    };
    let mut rows = vec![
        // The largest thing on this tab in every sense: it is what the battery
        // is mostly spent on, and until this panel existed it was the one
        // setting with no way to it but a button held down with another button.
        Row::said(&say(&Word::ScreenBrightness), &level).levelled(dim),
        // Under it, because it is the same screen and the same evening. What it
        // is standing at is said beside the row as well as by the words in it,
        // for the reason the notifications switch is: a machine wearing a colour
        // nobody asked for should say so where the asking happens.
        warmth(warm),
        // Named, because the two rows above are what the screen is doing now
        // and these three are what shape it is. The two rows above keep the top
        // of the tab unnamed, the way Game Mode does above Power on the System
        // tab: what is under the tab's own word needs no second word.
        Row::naming(&say(&Word::HowBigEverythingIs), ""),
    ];
    rows.extend(EVERY.into_iter().map(|size| {
        let mut row = switch(&say(&said_of_size(size)), &["/usr/local/bin/console-scale", size.written()]);
        // Marked out of what the compositor says rather than out of what was
        // last chosen. They part company the moment anything else moves the
        // density, and a panel that marks the row it wrote down rather than the
        // one being drawn is a reading, and it is wrong.
        row.aside = match standing == Some(size) {
            true => NOW.to_string(),
            false => String::new(),
        };
        row
    }));
    rows.extend(home);
    rows
}

/// What each rung is called on the screen.
///
/// Here rather than on `size::Size`, for the reason the battery's thresholds
/// are: the crate that draws a thing owns its words, and `size` is read by
/// things that draw nothing.
fn said_of_size(size: Size) -> Word {
    match size {
        Size::Tiny => Word::SizeTiny,
        Size::Smaller => Word::SizeSmaller,
        Size::Normal => Word::SizeNormal,
        Size::Bigger => Word::SizeBigger,
        Size::Huge => Word::SizeHuge,
    }
}

/// How hard the machine is allowed to work, and what it says on the way down.
///
/// Two groups: the three speeds under a name of their own, then the three
/// places the battery is watched at under theirs. The screen used to be above
/// both and is its own tab now -- see [`screen_rows`].
///
/// The profile is handed in as an answer that may not be back yet. The three
/// speeds are the same three whatever the machine says, so the tab is drawn
/// without it and it arrives into a list already on the screen.
pub fn battery_rows(
    running: Option<&str>,
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
    let mut rows = vec![
        Row::naming(&say(&Word::HowFastTheMachineRuns), ""),
        // Least first. They are one scale rather than three choices -- how much
        // of the battery this machine is allowed to spend -- and a scale drawn
        // out of order is three buttons a thumb has to read every time instead
        // of a direction it can move in.
        profile(&say(&Word::SpeedSaving), "power-saver"),
        profile(&say(&Word::SpeedNormal), "balanced"),
        profile(&say(&Word::SpeedFast), "performance"),
    ];
    rows.extend(dwindling(levels, guard));
    rows
}

/// How well a network is heard, drawn the way a volume is, so both read alike.
pub fn strength(signal: i32) -> String {
    bar(signal, Muted::No, CELLS / 2)
}

/// What the machine talks to, and the way in to each.
pub fn wifi_rows(
    on: wifi::Radio,
    networks: Vec<wifi::Network>,
    known: &[String],
    join: impl Fn(wifi::Network, wifi::Known) -> Does,
) -> Vec<Row> {
    if on == wifi::Radio::Off {
        return vec![switch("Turn Wi-Fi on", &["nmcli", "radio", "wifi", "on"])];
    }

    let mut rows = vec![switch("Turn Wi-Fi off", &["nmcli", "radio", "wifi", "off"])];

    for network in networks {
        if network.here {
            rows.push(Row::said(&network.name, NOW));
            continue;
        }

        let (says, aside) = (network.name.clone(), strength(network.signal));
        let already = match known.contains(&network.name) {
            true => wifi::Known::Yes,
            false => wifi::Known::No,
        };
        rows.push(Row::new(&says, &aside, join(network, already)));
    }

    rows
}

/// The same over the short road.
pub fn bluetooth_rows(
    on: bluetooth::Radio,
    devices: Vec<(bluetooth::Device, bluetooth::Joined)>,
) -> Vec<Row> {
    if on == bluetooth::Radio::Off {
        return vec![switch("Turn Bluetooth on", &["bluetoothctl", "power", "on"])];
    }

    let mut rows = vec![switch("Turn Bluetooth off", &["bluetoothctl", "power", "off"])];

    for (device, joined) in devices {
        let doing = match joined {
            bluetooth::Joined::Yes => "disconnect",
            bluetooth::Joined::No => "connect",
        };
        let aside = match joined {
            bluetooth::Joined::Yes => NOW,
            bluetooth::Joined::No => "",
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
        vec![Row::back(&configuration(), move |showing| leaving(showing)), Row::naming("Search with", "")];

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
        vec![Row::back(&configuration(), move |showing| leaving(showing)), Row::naming("Listen for", "")];

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
pub fn notifications_rows(held_back: Quiet) -> Vec<Row> {
    let says = match held_back {
        Quiet::HeldBack => "Show them on the screen as they arrive",
        Quiet::Coming => "Keep them off the screen",
    };
    let mark = match held_back {
        Quiet::HeldBack => "held back",
        Quiet::Coming => "",
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
/// The last four are nobody's icon, and they are free to sit wherever they read
/// best. Screen and Wallpaper are how it looks, so they are next to each other;
/// Screen is the one that could have gone up among the first four, and did not,
/// because those four are the bar's and putting a fifth among them would break
/// the one thing that order is for. Configuration is what it answers with.
/// Screen, Wallpaper and Configuration are the three anybody changes once and
/// then leaves alone. System is how it stops, and nothing that turns the
/// machine off shares a page with anything you would touch every day.
pub fn tabs() -> [String; 9] {
    [
        say(&Word::Sound),
        say(&Word::Bluetooth),
        say(&Word::Wifi),
        say(&Word::Battery),
        say(&Word::Notifications),
        say(&Word::Screen),
        say(&Word::Wallpaper),
        configuration(),
        say(&Word::System),
    ]
}

/// The one tab named on its own, because the lists under it say where the way
/// back goes and a word written twice is a word that goes out of step.
pub fn configuration() -> String {
    say(&Word::Configuration)
}

/// What choosing something on a list under a tab comes to, once the choice has
/// been made: the way back up to the tab it was opened from.
pub type Chosen = Arc<dyn Fn(&dyn Showing) + Send + Sync>;

#[cfg(test)]
mod tests {
    use console_panel::page::{Heading, InEffect};
    use super::*;

    fn nothing() -> Level {
        std::sync::Arc::new(|_| ())
    }

    /// The home screen's own rows, with presses that do nothing. The grid is
    /// the one a machine nobody has asked has.
    fn grid() -> Vec<Row> {
        home_rows(Shape::USUAL, nothing(), nothing(), nothing())
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

    fn screen() -> Vec<Row> {
        screen_rows(Some(50), nothing(), Warmth::Ordinary, Some(Size::Normal), grid())
    }

    fn battery() -> Vec<Row> {
        battery_rows(Some("balanced"), battery::Levels::default(), |_| nothing())
    }

    /// Sound was on a panel and the screen was not: brightness lived on the
    /// d-pad held under L2 and nowhere else, which is two buttons at once for
    /// the setting a person changes when the room gets dark.
    #[test]
    fn the_two_things_that_are_held_at_a_level_are_on_a_panel() {
        assert!(screen()[0].level.is_some(), "the screen is not a level");
        let sinks = sound::read(r#"[{"index": 1, "name": "a", "volume": {}}]"#);
        let speakers = sound_rows(&sinks, &[], "a", silence, turning);
        assert!(speakers[0].level.is_some(), "the speakers are not a level");
    }

    #[test]
    fn the_profile_in_use_is_the_one_marked() {
        let rows = battery_rows(Some("performance"), battery::Levels::default(), |_| nothing());
        let marked: Vec<&str> =
            rows.iter().filter(|row| row.now() == InEffect::Yes).map(|row| row.says.as_str()).collect();
        assert_eq!(marked, [say(&Word::SpeedFast)]);
    }

    /// Least first, because they are one scale and not three choices. A scale
    /// out of order is three rows a thumb has to read every time instead of a
    /// direction it can move in.
    #[test]
    fn the_three_speeds_are_a_named_scale_with_the_least_of_them_first() {
        let rows = battery();
        assert!(rows[0].naming, "the speeds are not named");
        assert_eq!(rows[0].says, say(&Word::HowFastTheMachineRuns));
        assert_eq!(
            says(&rows[1..4]),
            [say(&Word::SpeedSaving), say(&Word::SpeedNormal), say(&Word::SpeedFast)]
        );
    }

    /// What the screen is doing and how hard the machine is allowed to work are
    /// two subjects, and they were one tab: the brightness, the evening and the
    /// three speeds ran together as five rows of a kind. Nothing about the
    /// screen is left on the Battery tab.
    #[test]
    fn the_screen_and_how_hard_the_machine_works_are_two_tabs() {
        let battery = says(&battery()).join("\n");
        for screen in [say(&Word::ScreenBrightness), say(&Word::HowBigEverythingIs)] {
            assert!(!battery.contains(&screen), "{screen:?} is still on the Battery tab");
        }
        assert!(!battery.contains("night colours"), "the evening is still on the Battery tab");
    }

    /// The same shape as the speeds: a name, then the rungs, smallest first.
    /// It is one scale -- how much fits on the screen -- and the rows read as a
    /// direction rather than as unrelated choices.
    #[test]
    fn the_sizes_are_a_named_scale_with_the_smallest_of_them_first() {
        let rows = screen();
        let at = rows
            .iter()
            .position(|row| row.says == say(&Word::HowBigEverythingIs))
            .expect("the sizes are named");
        assert!(rows[at].naming, "the name is a row the highlight can land on");
        assert_eq!(
            says(&rows[at + 1..at + 1 + EVERY.len()]),
            [
                say(&Word::SizeTiny),
                say(&Word::SizeSmaller),
                say(&Word::SizeNormal),
                say(&Word::SizeBigger),
                say(&Word::SizeHuge),
            ]
        );
        assert!(
            at > rows.iter().position(|row| row.level.is_some()).expect("the brightness"),
            "the brightness is above the name, not under it"
        );
    }

    /// The home screen's own three, under the ladder for the whole desktop and
    /// not on a tab of their own. It is the same question about a smaller thing:
    /// a square is a share of the room, so the ladder above already moves it,
    /// and these are the three ways to disagree with what it worked out.
    #[test]
    fn the_home_screens_own_shape_is_under_the_size_of_everything_else() {
        let rows = screen();
        let named = rows
            .iter()
            .position(|row| row.says == say(&Word::TheHomeScreen))
            .expect("the home screen is named");
        let ladder = rows
            .iter()
            .position(|row| row.says == say(&Word::HowBigEverythingIs))
            .expect("the sizes are named");

        assert!(named > ladder, "the home screen is under the ladder, not over it");
        assert!(rows[named].naming, "the name is a row the highlight can land on");
        assert_eq!(
            says(&rows[named + 1..]),
            [
                say(&Word::ApplicationsAcross),
                say(&Word::ApplicationsDown),
                say(&Word::HowBigTheyAre),
            ]
        );
    }

    /// Every one of them is a thumb's to move, and every one of them says what
    /// it is standing at. A row with two arrows on it and no number is a row
    /// somebody presses to find out what it was.
    #[test]
    fn the_home_screens_rows_say_what_they_are_at_and_can_all_be_moved() {
        let shape = Shape::USUAL.across(7).down(4).sized(console_home::shape::Size::Bigger);
        let rows = home_rows(shape, nothing(), nothing(), nothing());
        let moved: Vec<&Row> = rows.iter().filter(|row| row.level.is_some()).collect();

        assert_eq!(moved.len(), 3, "one of them cannot be moved");
        assert!(moved.iter().all(|row| !row.aside.is_empty()), "one of them says nothing");
        assert_eq!(moved[0].aside, "7");
        assert_eq!(moved[1].aside, "4");
        assert_eq!(moved[2].aside, say(&Word::SizeBigger));
    }

    /// The rung the compositor says it is standing on, and only that one.
    #[test]
    fn the_size_the_screen_is_at_is_the_one_marked() {
        let rows = screen_rows(Some(50), nothing(), Warmth::Ordinary, Some(Size::Bigger), grid());
        let marked: Vec<&str> =
            rows.iter().filter(|row| row.now() == InEffect::Yes).map(|row| row.says.as_str()).collect();
        assert_eq!(marked, [say(&Word::SizeBigger)]);
    }

    /// A machine standing at a density that is none of the rungs marks none of
    /// them. A mark on the nearest one would say "you are here" about somewhere
    /// the machine is not.
    #[test]
    fn a_screen_at_a_size_of_its_own_marks_none_of_the_rungs() {
        let rows = screen_rows(Some(50), nothing(), Warmth::Ordinary, None, grid());
        assert!(
            !rows.iter().any(|row| row.now() == InEffect::Yes),
            "something is marked"
        );
        // The rungs, and the evening switch above them.
        let pressable = rows.iter().filter(|row| row.does.is_some()).count();
        assert_eq!(pressable, EVERY.len() + 1, "a rung went missing");
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
                say(&Word::WhenTheBatteryGetsLow),
                say(&Word::WarnMe),
                say(&Word::WarnMeAgain),
                say(&Word::TurnOffBeforeItDies),
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
        assert_eq!(threshold(battery::NEVER), say(&Word::Never));
        assert_eq!(threshold(5), "5%");
    }

    /// A tab that says nothing at all reads as a panel that is still loading.
    #[test]
    fn a_radio_that_is_off_still_has_a_row_to_turn_it_on() {
        assert_eq!(
            says(&wifi_rows(wifi::Radio::Off, Vec::new(), &[], |_, _| silence(0, ""))),
            ["Turn Wi-Fi on"]
        );
        assert_eq!(says(&bluetooth_rows(bluetooth::Radio::Off, Vec::new())), ["Turn Bluetooth on"]);
    }

    #[test]
    fn the_one_we_are_on_is_marked_rather_than_offered() {
        let networks = wifi::networks("yes:Home:71:WPA2\nno:Cafe:50:");
        let rows = wifi_rows(wifi::Radio::On, networks, &[], |_, _| silence(0, ""));
        let home = rows.iter().find(|row| row.says == "Home").expect("home");
        assert_eq!(home.now(), InEffect::Yes);
        assert!(home.does.is_none(), "there is nothing to do about being where you are");
        let cafe = rows.iter().find(|row| row.says == "Cafe").expect("cafe");
        assert!(cafe.does.is_some());
    }

    #[test]
    fn a_joined_device_is_marked_and_offers_the_way_out_of_it() {
        let devices = bluetooth::devices("Device AA Pads\nDevice BB Speaker");
        let rows = bluetooth_rows(
            bluetooth::Radio::On,
            vec![
                (devices[0].clone(), bluetooth::Joined::Yes),
                (devices[1].clone(), bluetooth::Joined::No),
            ],
        );
        assert_eq!(rows.iter().find(|row| row.says == "Pads").expect("pads").now(), InEffect::Yes);
        assert_eq!(rows.iter().find(|row| row.says == "Speaker").expect("speaker").now(), InEffect::No);
    }

    #[test]
    fn nothing_playing_is_said_rather_than_left_blank() {
        assert_eq!(says(&sound_rows(&[], &[], "", silence, turning)), ["Nothing else is playing"]);
    }

    #[test]
    fn the_engine_in_use_is_the_one_marked() {
        let rows = search_rows("startpage", nowhere());
        let marked: Vec<&str> =
            rows.iter().filter(|row| row.now() == InEffect::Yes).map(|row| row.says.as_str()).collect();
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
        assert!(rows[0].says.ends_with(&configuration()), "{:?} is not the way back", rows[0].says);
        assert_eq!(rows[1].says, "Search with");
        assert_eq!(rows[1].heading(), Heading::Yes);
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
            rows.iter().filter(|row| row.now() == InEffect::Yes).map(|row| row.says.as_str()).collect();
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
        assert!(rows[0].says.ends_with(&configuration()), "{:?} is not the way back", rows[0].says);
        assert_eq!(rows[1].says, "Listen for");
        assert_eq!(rows[1].heading(), Heading::Yes);
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
        let mut named = tabs().to_vec();
        named.sort_unstable();
        named.dedup();
        assert_eq!(named.len(), tabs().len());
    }
}
